use crate::modules::connection::tracking::ChannelTracker;
use crate::modules::connection::{
    ConnectionError, PreparedTerminalStart, PreparedTerminalStartOperation, SshConnection,
    StartedTerminal, TelnetConnection, TerminalCommand, TerminalSession,
};
use log::{debug, error, info, warn};
use std::sync::{Arc, Mutex, RwLock};
use tauri::{Emitter, Manager};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::mpsc;
use uuid::Uuid;

// 通过 shell 输入流注入 OSC 7 目录上报钩子，绕开 SSH `set_env`/`AcceptEnv` 限制。
// 同时兼容 bash（PROMPT_COMMAND）与 zsh（precmd hook）；结尾立即执行一次，
// 让文件浏览器在会话建立后即可拿到当前目录。printf 的 \033/\007 由远端 shell 自行解释。
// 首尾用唯一标记（`: __SS_OSC7_B__` / `: __SS_OSC7_E__`，`:` 是 shell no-op）包裹整行。
// 这两个标记会随输入被 PTY 原样回显到输出流，后端据此把这段回显整段剥掉，用户看不到；
// 而命令执行真正发出的 OSC 7 转义序列（ESC 开头，出现在 END 标记之后）不受影响。
const OSC7_ECHO_BEGIN: &[u8] = b"__SS_OSC7_B__";
const OSC7_ECHO_END: &[u8] = b"__SS_OSC7_E__";
const STARSHUTTLE_OSC7_SETUP_COMMAND: &str = r#": __SS_OSC7_B__ ; _starshuttle_osc7() { printf '\033]7;file://%s%s\007' "${HOSTNAME:-${HOST:-localhost}}" "$PWD"; }; if [ -n "$ZSH_VERSION" ]; then autoload -Uz add-zsh-hook 2>/dev/null && add-zsh-hook precmd _starshuttle_osc7; elif [ -n "$BASH_VERSION" ]; then case "$PROMPT_COMMAND" in *_starshuttle_osc7*) ;; *) PROMPT_COMMAND="_starshuttle_osc7${PROMPT_COMMAND:+; $PROMPT_COMMAND}";; esac; fi; _starshuttle_osc7 ; : __SS_OSC7_E__"#;

/// 在会话建立初期，从 SSH 输出流里剥掉注入命令自身的回显（BEGIN..END 整段，含结尾换行）。
/// 只在首次匹配前生效，匹配成功或超出预算后即彻底关闭，不影响后续正常输出。
enum SuppressState {
    /// 仍在扫描 BEGIN..END。
    Scanning,
    /// 已匹配 END，还需吞掉紧邻的结尾换行；`remaining` 是尚可吞掉的序列（`\r\n` / `\n` / 空）。
    /// 用于逐字节/跨 chunk 分片时，END 完成而换行尚未到达的情况。
    SwallowNewline { remaining: &'static [u8] },
    /// 完成，后续内容全量放行。
    Done,
}

struct Osc7EchoSuppressor {
    state: SuppressState,
    pending: Vec<u8>,
    budget: usize,
}

impl Osc7EchoSuppressor {
    fn new() -> Self {
        Self {
            state: SuppressState::Scanning,
            pending: Vec::new(),
            budget: 16384,
        }
    }

    fn filter(&mut self, input: &[u8]) -> Vec<u8> {
        match self.state {
            SuppressState::Done => input.to_vec(),
            SuppressState::SwallowNewline { .. } => self.swallow_newline(input),
            SuppressState::Scanning => self.scan(input),
        }
    }

    /// 消费 input 开头与 `remaining` 匹配的换行字节，其余原样放行并转入 Done。
    fn swallow_newline(&mut self, input: &[u8]) -> Vec<u8> {
        let SuppressState::SwallowNewline { remaining } = self.state else {
            unreachable!();
        };
        let mut consumed = 0;
        let mut rem = remaining;
        while consumed < input.len() {
            match rem.first() {
                Some(&expected) if input[consumed] == expected => {
                    consumed += 1;
                    rem = &rem[1..];
                }
                _ => break,
            }
        }
        if consumed == input.len() && !rem.is_empty() {
            // 整段都被吞掉，且仍可能有后续换行——继续等待下一片。
            self.state = SuppressState::SwallowNewline { remaining: rem };
            return Vec::new();
        }
        self.state = SuppressState::Done;
        input[consumed..].to_vec()
    }

    fn scan(&mut self, input: &[u8]) -> Vec<u8> {
        self.pending.extend_from_slice(input);

        match find_subslice(&self.pending, OSC7_ECHO_BEGIN) {
            None => {
                // 尚未看到 BEGIN：预算耗尽则放弃过滤，全量放行（容错）。
                if self.pending.len() > self.budget {
                    self.state = SuppressState::Done;
                    return std::mem::take(&mut self.pending);
                }
                // 立即放行除“可能是半截 BEGIN 标记”的尾部之外的全部内容（避免压住正常输出）。
                let keep_back = OSC7_ECHO_BEGIN.len().saturating_sub(1);
                if self.pending.len() <= keep_back {
                    return Vec::new();
                }
                let emit_upto = self.pending.len() - keep_back;
                let emitted = self.pending[..emit_upto].to_vec();
                self.pending.drain(..emit_upto);
                emitted
            }
            Some(begin_idx) => match find_subslice(&self.pending[begin_idx..], OSC7_ECHO_END) {
                None => {
                    // 看到 BEGIN 但 END 未到：先放行 BEGIN 之前的内容（提示符等），其余留存等待。
                    if self.pending.len() > self.budget {
                        self.state = SuppressState::Done;
                        return std::mem::take(&mut self.pending);
                    }
                    let emitted = self.pending[..begin_idx].to_vec();
                    self.pending.drain(..begin_idx);
                    emitted
                }
                Some(rel_end) => {
                    let end_idx = begin_idx + rel_end + OSC7_ECHO_END.len();
                    let mut emitted = self.pending[..begin_idx].to_vec();
                    // 吞掉 END 后紧邻的一个换行（\r\n / \n），避免留下空行；
                    // 已到达的部分就地吞掉，未到达的记入 SwallowNewline 状态跨 chunk 继续吞。
                    let after_end = &self.pending[end_idx..];
                    let (tail, next_state) = match after_end.first() {
                        // END 后尚无字节：整个换行可能还没到，转入等待。
                        None => (end_idx, SuppressState::SwallowNewline { remaining: b"\r\n" }),
                        // \r 已到：吞掉；若 \n 也已到则一并吞掉并完成，否则等待 \n。
                        Some(&b'\r') => {
                            if after_end.get(1) == Some(&b'\n') {
                                (end_idx + 2, SuppressState::Done)
                            } else {
                                (end_idx + 1, SuppressState::SwallowNewline { remaining: b"\n" })
                            }
                        }
                        // 裸 \n：吞掉并完成。
                        Some(&b'\n') => (end_idx + 1, SuppressState::Done),
                        // END 后紧跟非换行内容：无需吞噬，直接完成。
                        Some(_) => (end_idx, SuppressState::Done),
                    };
                    emitted.extend_from_slice(&self.pending[tail..]);
                    self.pending.clear();
                    self.state = next_state;
                    emitted
                }
            },
        }
    }
}

fn find_subslice(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() || haystack.len() < needle.len() {
        return None;
    }
    haystack.windows(needle.len()).position(|w| w == needle)
}

async fn apply_terminal_cwd_shell_integration<S>(channel: &russh::Channel<S>)
where
    S: From<(russh::ChannelId, russh::ChannelMsg)> + Send + Sync + 'static,
{
    let mut payload = Vec::with_capacity(STARSHUTTLE_OSC7_SETUP_COMMAND.len() + 1);
    payload.extend_from_slice(STARSHUTTLE_OSC7_SETUP_COMMAND.as_bytes());
    payload.push(b'\r');
    if let Err(err) = channel.data(&payload[..]).await {
        debug!("Failed to inject OSC 7 shell integration: {}", err);
    }
}

pub(crate) fn execute_prepared_terminal_start(
    app: &tauri::AppHandle,
    prepared: PreparedTerminalStart,
) -> Result<StartedTerminal, ConnectionError> {
    match prepared.operation {
        PreparedTerminalStartOperation::Telnet { telnet } => start_telnet_terminal(
            app,
            prepared.session_id,
            prepared.runtime,
            prepared.tracker,
            telnet,
        ),
        PreparedTerminalStartOperation::Ssh {
            ssh_connection,
            width,
            height,
        } => start_ssh_terminal(
            app,
            prepared.session_id,
            prepared.runtime,
            prepared.tracker,
            ssh_connection,
            width,
            height,
        ),
    }
}

fn sync_backend_state_on_terminal_exit(
    app: &tauri::AppHandle,
    session_id: Uuid,
    exit_reason: &str,
) {
    let manager_state = app.state::<Arc<RwLock<super::DefaultConnectionManager>>>();
    let write_result = manager_state.write();
    match write_result {
        Ok(mut manager) => manager.handle_terminal_exit(&session_id, exit_reason),
        Err(err) => error!(
            "Failed to sync backend terminal exit for session {} ({}): {}",
            session_id, exit_reason, err
        ),
    };
}

fn drain_output_buffer(output_buffer: &mut Vec<u8>) -> Option<(String, usize)> {
    if output_buffer.is_empty() {
        return None;
    }
    let byte_len = output_buffer.len();
    let data = String::from_utf8_lossy(output_buffer).to_string();
    output_buffer.clear();
    Some((data, byte_len))
}

fn flush_output_buffer(
    output_buffer: &mut Vec<u8>,
    tracker: &Arc<Mutex<ChannelTracker>>,
    app: &tauri::AppHandle,
    session_id: Uuid,
    output_stats_bytes: &mut usize,
    output_stats_messages: &mut u64,
    protocol: &str,
) {
    if output_buffer.is_empty() {
        return;
    }

    if let Ok(mut tracker) = tracker.lock() {
        tracker.log_data(session_id, output_buffer, "received");
    }

    let Some((data_str, byte_len)) = drain_output_buffer(output_buffer) else {
        return;
    };
    *output_stats_bytes += byte_len;
    *output_stats_messages += 1;

    let event_name = format!("terminal-output-{}", session_id);
    let emit_start = tokio::time::Instant::now();
    let _ = app.emit(&event_name, serde_json::json!({ "data": data_str }));
    let emit_ms = emit_start.elapsed().as_millis();
    if emit_ms > 10 {
        warn!(
            "Terminal output emit slow ({}) session {}: {}ms",
            protocol, session_id, emit_ms
        );
    }
}

fn start_telnet_terminal(
    app: &tauri::AppHandle,
    session_id: Uuid,
    runtime: Arc<tokio::runtime::Runtime>,
    tracker: Arc<Mutex<ChannelTracker>>,
    telnet: TelnetConnection,
) -> Result<StartedTerminal, ConnectionError> {
    info!("Starting Telnet terminal for session: {}", session_id);

    let terminal_id = Uuid::new_v4();
    let (tx, mut rx) = mpsc::channel::<TerminalCommand>(2048);

    let session_id_clone = session_id;
    let app_clone = app.clone();
    let tracker_clone = Arc::clone(&tracker);

    tracker_clone
        .lock()
        .unwrap()
        .register_session(session_id_clone);

    let mut read = telnet.read;
    let mut write = telnet.write;

    runtime.spawn(async move {
        let mut buf = vec![0u8; 8192];
        #[allow(unused_assignments)]
        let mut exit_reason = "unknown";
        let mut output_stats_last = tokio::time::Instant::now();
        let mut output_stats_bytes: usize = 0;
        let mut output_stats_messages: u64 = 0;
        let mut output_buffer = Vec::new();
        let mut flush_deadline: Option<tokio::time::Instant> = None;

        loop {
            tokio::select! {
                _ = async {
                    if let Some(deadline) = flush_deadline {
                        tokio::time::sleep_until(deadline).await
                    } else {
                        std::future::pending().await
                    }
                }, if flush_deadline.is_some() => {
                    flush_output_buffer(
                        &mut output_buffer,
                        &tracker_clone,
                        &app_clone,
                        session_id_clone,
                        &mut output_stats_bytes,
                        &mut output_stats_messages,
                        "telnet",
                    );
                    flush_deadline = None;
                }
                read_res = read.read(&mut buf) => {
                    match read_res {
                        Ok(0) => {
                            exit_reason = "connection_lost";
                            break;
                        }
                        Ok(n) => {
                            let mut display = Vec::<u8>::new();
                            let mut replies = Vec::<u8>::new();
                            super::telnet::telnet_process_incoming(&buf[..n], &mut display, &mut replies);

                            if !replies.is_empty() {
                                if let Ok(mut tracker) = tracker_clone.lock() {
                                    tracker.log_data(session_id_clone, &replies, "sent");
                                }
                                let _ = write.write_all(&replies).await;
                            }

                            if !display.is_empty() {
                                output_buffer.extend_from_slice(&display);

                                if output_buffer.len() >= 65536 {
                                    flush_output_buffer(
                                        &mut output_buffer,
                                        &tracker_clone,
                                        &app_clone,
                                        session_id_clone,
                                        &mut output_stats_bytes,
                                        &mut output_stats_messages,
                                        "telnet",
                                    );
                                    flush_deadline = None;
                                } else if flush_deadline.is_none() {
                                    flush_deadline = Some(tokio::time::Instant::now() + tokio::time::Duration::from_millis(15));
                                }

                                let elapsed = output_stats_last.elapsed();
                                if elapsed.as_millis() >= 1000 {
                                    info!(
                                        "Terminal output cadence (telnet) session {}: msgs={}, bytes={}, elapsed_ms={}",
                                        session_id_clone,
                                        output_stats_messages,
                                        output_stats_bytes,
                                        elapsed.as_millis()
                                    );
                                    output_stats_last = tokio::time::Instant::now();
                                    output_stats_bytes = 0;
                                    output_stats_messages = 0;
                                }
                            }
                        }
                        Err(e) => {
                            flush_output_buffer(
                                &mut output_buffer,
                                &tracker_clone,
                                &app_clone,
                                session_id_clone,
                                &mut output_stats_bytes,
                                &mut output_stats_messages,
                                "telnet",
                            );
                            let error_msg = format!("Telnet read error: {}", e);
                            let event_name = format!("terminal-error-{}", session_id_clone);
                            let _ = app_clone.emit(&event_name, serde_json::json!({ "error": error_msg }));
                            exit_reason = "read_error";
                            break;
                        }
                    }
                }
                cmd = rx.recv() => {
                    match cmd {
                        Some(TerminalCommand::Data(data)) => {
                            if let Ok(mut tracker) = tracker_clone.lock() {
                                tracker.log_data(session_id_clone, &data, "sent");
                            }
                            if let Err(e) = write.write_all(&data).await {
                                flush_output_buffer(
                                    &mut output_buffer,
                                    &tracker_clone,
                                    &app_clone,
                                    session_id_clone,
                                    &mut output_stats_bytes,
                                    &mut output_stats_messages,
                                    "telnet",
                                );
                                let error_msg = format!("Telnet write error: {}", e);
                                let event_name = format!("terminal-error-{}", session_id_clone);
                                let _ = app_clone.emit(&event_name, serde_json::json!({ "error": error_msg }));
                                exit_reason = "write_error";
                                break;
                            }
                        }
                        Some(TerminalCommand::Resize(_, _)) => {}
                        Some(TerminalCommand::Close) => {
                            flush_output_buffer(
                                &mut output_buffer,
                                &tracker_clone,
                                &app_clone,
                                session_id_clone,
                                &mut output_stats_bytes,
                                &mut output_stats_messages,
                                "telnet",
                            );
                            let _ = write.shutdown().await;
                            exit_reason = "user_closed";
                            break;
                        }
                        None => {
                            flush_output_buffer(
                                &mut output_buffer,
                                &tracker_clone,
                                &app_clone,
                                session_id_clone,
                                &mut output_stats_bytes,
                                &mut output_stats_messages,
                                "telnet",
                            );
                            exit_reason = "command_channel_closed";
                            break;
                        }
                    }
                }
            }
        }

        flush_output_buffer(
            &mut output_buffer,
            &tracker_clone,
            &app_clone,
            session_id_clone,
            &mut output_stats_bytes,
            &mut output_stats_messages,
            "telnet",
        );
        sync_backend_state_on_terminal_exit(&app_clone, session_id_clone, exit_reason);
        let event_name = format!("session-closed-{}", session_id_clone);
        let _ = app_clone.emit(&event_name, serde_json::json!({ "reason": exit_reason }));
    });

    Ok(StartedTerminal {
        session_id,
        terminal_id,
        terminal: TerminalSession {
            id: terminal_id,
            session_id,
            sender: tx,
        },
    })
}

fn start_ssh_terminal(
    app: &tauri::AppHandle,
    session_id: Uuid,
    runtime: Arc<tokio::runtime::Runtime>,
    tracker: Arc<Mutex<ChannelTracker>>,
    ssh_connection: SshConnection,
    width: u16,
    height: u16,
) -> Result<StartedTerminal, ConnectionError> {
    info!("Starting terminal for session: {}", session_id);

    let mut channel = runtime
        .block_on(async {
            let handle = ssh_connection.handle.lock().await;
            handle.channel_open_session().await
        })
        .map_err(|e| {
            ConnectionError::ConnectionFailed(format!("Failed to open terminal channel: {}", e))
        })?;

    runtime
        .block_on(async {
            channel
                .request_pty(
                    true,
                    "xterm-256color",
                    width as u32,
                    height as u32,
                    0,
                    0,
                    &[],
                )
                .await
        })
        .map_err(|e| ConnectionError::ConnectionFailed(format!("Failed to request PTY: {}", e)))?;

    runtime
        .block_on(async { channel.request_shell(true).await })
        .map_err(|e| ConnectionError::ConnectionFailed(format!("Failed to start shell: {}", e)))?;

    // shell 启动后再注入 OSC 7 钩子（写入 shell 输入流），此时远端 shell 已就绪可解释命令。
    runtime.block_on(async { apply_terminal_cwd_shell_integration(&channel).await });

    let newline_data = b"\r\n";
    if let Err(e) = runtime.block_on(async { channel.data(&newline_data[..]).await }) {
        error!("Failed to send initial newline: {:?}", e);
    }

    let terminal_id = Uuid::new_v4();
    let (tx, mut rx) = mpsc::channel::<TerminalCommand>(2048);

    let session_id_clone = session_id;
    let app_clone = app.clone();
    let tracker_clone = Arc::clone(&tracker);

    tracker_clone
        .lock()
        .unwrap()
        .register_session(session_id_clone);

    runtime.spawn(async move {
        let mut last_activity = tokio::time::Instant::now();
        #[allow(unused_assignments)]
        let mut exit_reason = "unknown";
        let mut output_stats_last = tokio::time::Instant::now();
        let mut output_stats_bytes: usize = 0;
        let mut output_stats_messages: u64 = 0;

        let mut output_buffer = Vec::new();
        let mut flush_deadline: Option<tokio::time::Instant> = None;
        // 剥离注入命令自身的回显（BEGIN..END），只在会话建立初期生效。
        let mut osc7_echo_suppressor = Osc7EchoSuppressor::new();

        loop {
            tokio::select! {
                _ = async {
                    if let Some(deadline) = flush_deadline {
                        tokio::time::sleep_until(deadline).await
                    } else {
                        std::future::pending().await
                    }
                }, if flush_deadline.is_some() => {
                    flush_output_buffer(
                        &mut output_buffer,
                        &tracker_clone,
                        &app_clone,
                        session_id_clone,
                        &mut output_stats_bytes,
                        &mut output_stats_messages,
                        "ssh",
                    );
                    flush_deadline = None;
                }

                msg = channel.wait() => {
                    match msg {
                        Some(russh::ChannelMsg::Data { ref data }) => {
                            last_activity = tokio::time::Instant::now();

                            let filtered = osc7_echo_suppressor.filter(data);
                            if filtered.is_empty() {
                                continue;
                            }
                            output_buffer.extend_from_slice(&filtered);

                            if output_buffer.len() >= 65536 {
                                flush_output_buffer(
                                    &mut output_buffer,
                                    &tracker_clone,
                                    &app_clone,
                                    session_id_clone,
                                    &mut output_stats_bytes,
                                    &mut output_stats_messages,
                                    "ssh",
                                );
                                flush_deadline = None;
                            } else if flush_deadline.is_none() {
                                flush_deadline = Some(tokio::time::Instant::now() + tokio::time::Duration::from_millis(15));
                            }

                            let elapsed = output_stats_last.elapsed();
                            if elapsed.as_millis() >= 1000 {
                                info!(
                                    "Terminal output cadence (ssh) session {}: msgs={}, bytes={}, elapsed_ms={}",
                                    session_id_clone,
                                    output_stats_messages,
                                    output_stats_bytes,
                                    elapsed.as_millis()
                                );
                                output_stats_last = tokio::time::Instant::now();
                                output_stats_bytes = 0;
                                output_stats_messages = 0;
                            }
                        },
                        Some(russh::ChannelMsg::ExitStatus { exit_status }) => {
                            flush_output_buffer(
                                &mut output_buffer,
                                &tracker_clone,
                                &app_clone,
                                session_id_clone,
                                &mut output_stats_bytes,
                                &mut output_stats_messages,
                                "ssh",
                            );
                            info!("Terminal exited with status: {}", exit_status);
                            exit_reason = "normal";
                            break;
                        },
                        Some(russh::ChannelMsg::Close) => {
                            flush_output_buffer(
                                &mut output_buffer,
                                &tracker_clone,
                                &app_clone,
                                session_id_clone,
                                &mut output_stats_bytes,
                                &mut output_stats_messages,
                                "ssh",
                            );
                            info!("Channel closed by server");
                            exit_reason = "server_closed";
                            break;
                        },
                        None => {
                            flush_output_buffer(
                                &mut output_buffer,
                                &tracker_clone,
                                &app_clone,
                                session_id_clone,
                                &mut output_stats_bytes,
                                &mut output_stats_messages,
                                "ssh",
                            );
                            debug!("Channel closed (connection lost)");
                            exit_reason = "connection_lost";
                            break;
                        },
                        _ => {}
                    }
                }
                cmd = rx.recv() => {
                    match cmd {
                        Some(TerminalCommand::Data(data)) => {
                            last_activity = tokio::time::Instant::now();
                            let _ = channel.data(&data[..]).await;
                        },
                        Some(TerminalCommand::Resize(w, h)) => {
                            let _ = channel.window_change(w, h, 0, 0).await;
                        },
                        Some(TerminalCommand::Close) => {
                            flush_output_buffer(
                                &mut output_buffer,
                                &tracker_clone,
                                &app_clone,
                                session_id_clone,
                                &mut output_stats_bytes,
                                &mut output_stats_messages,
                                "ssh",
                            );
                            let _ = channel.close().await;
                            exit_reason = "user_closed";
                            break;
                        },
                        None => {
                            flush_output_buffer(
                                &mut output_buffer,
                                &tracker_clone,
                                &app_clone,
                                session_id_clone,
                                &mut output_stats_bytes,
                                &mut output_stats_messages,
                                "ssh",
                            );
                            debug!("Command channel closed");
                            exit_reason = "command_channel_closed";
                            break;
                        }
                    }
                }
                _ = tokio::time::sleep(tokio::time::Duration::from_secs(30)) => {
                    if last_activity.elapsed() > tokio::time::Duration::from_secs(60) {
                        let null_byte = b"\0";
                        if let Err(e) = channel.data(&null_byte[..]).await {
                            flush_output_buffer(
                                &mut output_buffer,
                                &tracker_clone,
                                &app_clone,
                                session_id_clone,
                                &mut output_stats_bytes,
                                &mut output_stats_messages,
                                "ssh",
                            );
                            debug!("Keepalive failed, connection may be dead: {:?}", e);
                            exit_reason = "keepalive_failed";
                            break;
                        }
                        debug!("Sent keepalive to session: {}", session_id_clone);
                    }
                }
            }
        }

        flush_output_buffer(
            &mut output_buffer,
            &tracker_clone,
            &app_clone,
            session_id_clone,
            &mut output_stats_bytes,
            &mut output_stats_messages,
            "ssh",
        );

        sync_backend_state_on_terminal_exit(&app_clone, session_id_clone, exit_reason);
        let event_name = format!("session-closed-{}", session_id_clone);
        info!("Emitting session closed event: {} (reason: {})", event_name, exit_reason);
        let _ = app_clone.emit(&event_name, serde_json::json!({ "reason": exit_reason }));
    });

    Ok(StartedTerminal {
        session_id,
        terminal_id,
        terminal: TerminalSession {
            id: terminal_id,
            session_id,
            sender: tx,
        },
    })
}

#[cfg(test)]
mod tests {
    use super::{drain_output_buffer, Osc7EchoSuppressor, OSC7_ECHO_BEGIN, OSC7_ECHO_END};

    // 构造一段“被 PTY 回显的注入命令行”：前缀提示符 + BEGIN..END + 换行 + 后续正常输出。
    fn echoed_line(prefix: &[u8], trailer: &[u8]) -> Vec<u8> {
        let mut v = Vec::new();
        v.extend_from_slice(prefix);
        v.extend_from_slice(OSC7_ECHO_BEGIN);
        v.extend_from_slice(b" ; _starshuttle_osc7() { ... } ; ");
        v.extend_from_slice(OSC7_ECHO_END);
        v.extend_from_slice(b"\r\n");
        v.extend_from_slice(trailer);
        v
    }

    #[test]
    fn suppressor_strips_echo_in_single_chunk() {
        let mut s = Osc7EchoSuppressor::new();
        let input = echoed_line(b"user@host:~$ ", b"user@host:~$ ");
        let out = s.filter(&input);
        assert_eq!(out, b"user@host:~$ user@host:~$ ");
        // 匹配后过滤器关闭，后续内容原样放行。
        assert_eq!(s.filter(b"ls -la\r\n"), b"ls -la\r\n");
    }

    #[test]
    fn suppressor_strips_echo_across_chunk_boundaries() {
        let mut s = Osc7EchoSuppressor::new();
        let input = echoed_line(b"$ ", b"done\r\n");
        let mut collected = Vec::new();
        // 逐字节喂入，模拟最坏的分片情况。
        for b in &input {
            collected.extend_from_slice(&s.filter(&[*b]));
        }
        assert_eq!(collected, b"$ done\r\n");
    }

    #[test]
    fn suppressor_passes_through_normal_output_untouched() {
        let mut s = Osc7EchoSuppressor::new();
        // 从不出现 BEGIN，超出预算后应全量放行且不再改动。
        let big = vec![b'x'; 20000];
        let out = s.filter(&big);
        assert_eq!(out.len(), big.len());
        assert_eq!(s.filter(b"more"), b"more");
    }

    #[test]
    fn suppressor_does_not_swallow_real_osc7_after_marker() {
        let mut s = Osc7EchoSuppressor::new();
        // 注入行回显之后，紧跟命令真正发出的 OSC 7 序列（ESC 开头）——不能被吃掉。
        let mut input = echoed_line(b"", b"");
        input.extend_from_slice(b"\x1b]7;file:///home/user\x07");
        let out = s.filter(&input);
        assert_eq!(out, b"\x1b]7;file:///home/user\x07");
    }

    #[test]
    fn drain_output_buffer_returns_none_for_empty() {
        let mut buffer = Vec::new();
        assert!(drain_output_buffer(&mut buffer).is_none());
        assert!(buffer.is_empty());
    }

    #[test]
    fn drain_output_buffer_returns_data_and_clears_buffer() {
        let mut buffer = b"hello".to_vec();
        let drained = drain_output_buffer(&mut buffer).expect("buffer should drain");
        assert_eq!(drained.0, "hello");
        assert_eq!(drained.1, 5);
        assert!(buffer.is_empty());
    }

    #[test]
    fn drain_output_buffer_preserves_utf8_lossy_behavior() {
        let mut buffer = vec![0x66, 0x6f, 0x80, 0x6f];
        let drained = drain_output_buffer(&mut buffer).expect("buffer should drain");
        assert_eq!(drained.0, "fo�o");
        assert_eq!(drained.1, 4);
        assert!(buffer.is_empty());
    }
}
