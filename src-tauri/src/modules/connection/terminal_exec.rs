use crate::modules::connection::tracking::ChannelTracker;
use crate::modules::connection::{
    ConnectionError, PreparedTerminalStart, PreparedTerminalStartOperation, SshConnection,
    StartedTerminal, TelnetConnection, TerminalCommand, TerminalSession,
};
use log::{debug, error, info, warn};
use std::sync::{Arc, Mutex};
use tauri::Emitter;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::mpsc;
use uuid::Uuid;

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

        loop {
            tokio::select! {
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
                                if let Ok(mut tracker) = tracker_clone.lock() {
                                    tracker.log_data(session_id_clone, &display, "received");
                                }
                                let data_str = String::from_utf8_lossy(&display).to_string();
                                let event_name = format!("terminal-output-{}", session_id_clone);
                                output_stats_bytes += display.len();
                                output_stats_messages += 1;
                                let emit_start = tokio::time::Instant::now();
                                let _ = app_clone.emit(&event_name, serde_json::json!({ "data": data_str }));
                                let emit_ms = emit_start.elapsed().as_millis();
                                if emit_ms > 10 {
                                    warn!(
                                        "Terminal output emit slow (telnet) session {}: {}ms",
                                        session_id_clone, emit_ms
                                    );
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
                                let error_msg = format!("Telnet write error: {}", e);
                                let event_name = format!("terminal-error-{}", session_id_clone);
                                let _ = app_clone.emit(&event_name, serde_json::json!({ "error": error_msg }));
                                exit_reason = "write_error";
                                break;
                            }
                        }
                        Some(TerminalCommand::Resize(_, _)) => {}
                        Some(TerminalCommand::Close) => {
                            let _ = write.shutdown().await;
                            exit_reason = "user_closed";
                            break;
                        }
                        None => {
                            exit_reason = "command_channel_closed";
                            break;
                        }
                    }
                }
            }
        }

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

        loop {
            tokio::select! {
                _ = async {
                    if let Some(deadline) = flush_deadline {
                        tokio::time::sleep_until(deadline).await
                    } else {
                        std::future::pending().await
                    }
                }, if flush_deadline.is_some() => {
                    if !output_buffer.is_empty() {
                        if let Ok(mut tracker) = tracker_clone.lock() {
                            tracker.log_data(session_id_clone, &output_buffer, "received");
                        }

                        let data_str = String::from_utf8_lossy(&output_buffer).to_string();
                        let event_name = format!("terminal-output-{}", session_id_clone);
                        output_stats_bytes += output_buffer.len();
                        output_stats_messages += 1;

                        let emit_start = tokio::time::Instant::now();
                        let _ = app_clone.emit(&event_name, serde_json::json!({ "data": data_str }));
                        let emit_ms = emit_start.elapsed().as_millis();
                        if emit_ms > 10 {
                            warn!("Terminal output emit slow (ssh) session {}: {}ms", session_id_clone, emit_ms);
                        }

                        output_buffer.clear();
                    }
                    flush_deadline = None;
                }

                msg = channel.wait() => {
                    match msg {
                        Some(russh::ChannelMsg::Data { ref data }) => {
                            last_activity = tokio::time::Instant::now();

                            output_buffer.extend_from_slice(data);

                            if output_buffer.len() >= 65536 {
                                if let Ok(mut tracker) = tracker_clone.lock() {
                                    tracker.log_data(session_id_clone, &output_buffer, "received");
                                }

                                let data_str = String::from_utf8_lossy(&output_buffer).to_string();
                                let event_name = format!("terminal-output-{}", session_id_clone);
                                output_stats_bytes += output_buffer.len();
                                output_stats_messages += 1;

                                let emit_start = tokio::time::Instant::now();
                                let _ = app_clone.emit(&event_name, serde_json::json!({ "data": data_str }));
                                let emit_ms = emit_start.elapsed().as_millis();
                                if emit_ms > 10 {
                                    warn!("Terminal output emit slow (ssh) session {}: {}ms", session_id_clone, emit_ms);
                                }

                                output_buffer.clear();
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
                            info!("Terminal exited with status: {}", exit_status);
                            exit_reason = "normal";
                            break;
                        },
                        Some(russh::ChannelMsg::Close) => {
                            info!("Channel closed by server");
                            exit_reason = "server_closed";
                            break;
                        },
                        None => {
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
                            let _ = channel.close().await;
                            exit_reason = "user_closed";
                            break;
                        },
                        None => {
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
                            debug!("Keepalive failed, connection may be dead: {:?}", e);
                            exit_reason = "keepalive_failed";
                            break;
                        }
                        debug!("Sent keepalive to session: {}", session_id_clone);
                    }
                }
            }
        }

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
