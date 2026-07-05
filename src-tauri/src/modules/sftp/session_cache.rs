use log::{debug, warn};
use russh_sftp::client::SftpSession;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use tokio::sync::{Mutex, Notify};
use uuid::Uuid;

use crate::modules::connection::{ConnectionManager, ConnectionStatus, DefaultConnectionManager};

use super::generation::SftpSessionLease;
use super::SessionGenerationMap;

pub enum CachedSftpSession {
    Ready {
        session: Arc<Mutex<SftpSession>>,
        generation: u64,
    },
    Pending {
        notify: Arc<Notify>,
        generation: u64,
    },
}

fn summarize_error_for_log(message: &str, max_chars: usize) -> String {
    let mut out = String::with_capacity(message.len());
    enum EscapeState {
        None,
        Started,
        Csi,
    }
    let mut escape_state = EscapeState::None;
    for ch in message.chars() {
        match escape_state {
            EscapeState::None => {
                if ch == '\u{1b}' {
                    escape_state = EscapeState::Started;
                    continue;
                }
            }
            EscapeState::Started => {
                if ch == '[' {
                    escape_state = EscapeState::Csi;
                } else {
                    escape_state = EscapeState::None;
                }
                continue;
            }
            EscapeState::Csi => {
                if ('@'..='~').contains(&ch) {
                    escape_state = EscapeState::None;
                }
                continue;
            }
        }
        match ch {
            '\n' | '\r' | '\t' => out.push(' '),
            _ if ch.is_control() => out.push('?'),
            _ => out.push(ch),
        }
    }
    if out.chars().count() <= max_chars {
        return out;
    }
    let truncated: String = out.chars().take(max_chars).collect();
    format!("{}...(truncated)", truncated)
}

pub fn current_generation(
    generations: &SessionGenerationMap,
    session_id: Uuid,
) -> Result<u64, String> {
    let guard = generations.lock().map_err(|e| e.to_string())?;
    Ok(guard.get(&session_id).copied().unwrap_or(0))
}

pub fn bump_generation(
    generations: &SessionGenerationMap,
    session_id: Uuid,
) -> Result<u64, String> {
    let mut guard = generations.lock().map_err(|e| e.to_string())?;
    let next = guard
        .get(&session_id)
        .copied()
        .unwrap_or(0)
        .saturating_add(1);
    guard.insert(session_id, next);
    Ok(next)
}

pub async fn remove_session(
    sessions: &Arc<Mutex<HashMap<Uuid, CachedSftpSession>>>,
    generations: &SessionGenerationMap,
    session_id: Uuid,
) {
    let removed = {
        let mut sessions = sessions.lock().await;
        sessions.remove(&session_id)
    };

    let _ = bump_generation(generations, session_id);

    if let Some(CachedSftpSession::Pending { notify, .. }) = removed {
        notify.notify_waiters();
    }
}

pub fn is_session_still_connected(
    connection_manager: &Arc<RwLock<DefaultConnectionManager>>,
    session_id: Uuid,
) -> Result<bool, String> {
    let cm = connection_manager.read().map_err(|e| e.to_string())?;
    let Some(session) = cm.get_session(&session_id) else {
        return Ok(false);
    };
    if session.status != ConnectionStatus::Connected {
        return Ok(false);
    }
    Ok(cm.get_ssh_connection(&session_id).is_some())
}

pub async fn get_session(
    sessions: &Arc<Mutex<HashMap<Uuid, CachedSftpSession>>>,
    generations: &SessionGenerationMap,
    connection_manager: &Arc<RwLock<DefaultConnectionManager>>,
    session_id: Uuid,
) -> Result<SftpSessionLease, String> {
    loop {
        enum AcquireAction {
            Wait(Arc<Notify>),
            Retry,
            Create {
                notify: Arc<Notify>,
                generation: u64,
            },
        }

        let action = {
            let mut sessions = sessions.lock().await;
            match sessions.get(&session_id) {
                Some(CachedSftpSession::Ready {
                    session,
                    generation,
                }) => {
                    if !is_session_still_connected(connection_manager, session_id)? {
                        sessions.remove(&session_id);
                        let _ = bump_generation(generations, session_id);
                        AcquireAction::Retry
                    } else {
                        return Ok(SftpSessionLease {
                            session_id,
                            generation: *generation,
                            session: session.clone(),
                            generations: generations.clone(),
                        });
                    }
                }
                Some(CachedSftpSession::Pending { notify, .. }) => {
                    AcquireAction::Wait(notify.clone())
                }
                None => {
                    let generation = current_generation(generations, session_id)?;
                    let notify = Arc::new(Notify::new());
                    sessions.insert(
                        session_id,
                        CachedSftpSession::Pending {
                            notify: notify.clone(),
                            generation,
                        },
                    );
                    AcquireAction::Create { notify, generation }
                }
            }
        };

        let (pending_notify, pending_generation) = match action {
            AcquireAction::Wait(notify) => {
                // 关键：先登记等待者，再在锁内复查状态，最后 await。
                //
                // `Notify::notify_waiters()` 不保存许可，只唤醒调用时已登记的等待者。
                // 若在「释放 sessions 锁」到「注册 notified」之间创建方完成初始化并
                // 通知，朴素的 `notify.notified().await` 会永久错过唤醒而挂起。
                //
                // 通过 `enable()` 先把当前 future 登记为等待者，然后重新持锁复查：
                // - 若已非 Pending（创建方已完成/失败），直接重试循环，不 await；
                // - 若仍是 Pending，创建方此刻被 sessions 锁挡住，其后续的
                //   `notify_waiters()` 必然命中已登记的本等待者，不会丢唤醒。
                let notified = notify.notified();
                tokio::pin!(notified);
                notified.as_mut().enable();

                let still_pending = {
                    let sessions = sessions.lock().await;
                    matches!(
                        sessions.get(&session_id),
                        Some(CachedSftpSession::Pending { .. })
                    )
                };
                if still_pending {
                    notified.await;
                }
                continue;
            }
            AcquireAction::Retry => continue,
            AcquireAction::Create { notify, generation } => (notify, generation),
        };

        debug!("SFTP cache miss, creating session {}", session_id);

        let create_result: Result<Arc<Mutex<SftpSession>>, String> = async {
            let ssh_conn = {
                let cm = connection_manager.read().map_err(|e| e.to_string())?;
                let session = cm
                    .get_session(&session_id)
                    .ok_or_else(|| "Session not found".to_string())?;
                if session.status != ConnectionStatus::Connected {
                    return Err(format!(
                        "Session {} is not connected (status: {:?})",
                        session_id, session.status
                    ));
                }
                cm.get_ssh_connection(&session_id).ok_or_else(|| {
                    warn!(
                        "SFTP session init failed: missing SSH session for {}",
                        session_id
                    );
                    "SSH session not found".to_string()
                })?
            };

            let handle = ssh_conn.handle.lock().await;
            let channel = handle.channel_open_session().await.map_err(|e| {
                warn!(
                    "SFTP session init channel open failed for {}: {}",
                    session_id,
                    summarize_error_for_log(&e.to_string(), 160)
                );
                e.to_string()
            })?;
            channel.request_subsystem(true, "sftp").await.map_err(|e| {
                warn!(
                    "SFTP session init subsystem request failed for {}: {}",
                    session_id,
                    summarize_error_for_log(&e.to_string(), 160)
                );
                e.to_string()
            })?;

            let sftp = SftpSession::new(channel.into_stream()).await.map_err(|e| {
                warn!(
                    "SFTP session init failed for {}: {}",
                    session_id,
                    summarize_error_for_log(&e.to_string(), 160)
                );
                e.to_string()
            })?;

            Ok(Arc::new(Mutex::new(sftp)))
        }
        .await;

        let result = match create_result {
            Ok(sftp_arc) => {
                if !is_session_still_connected(connection_manager, session_id)? {
                    Err(format!(
                        "Session {} is no longer connected during SFTP initialization",
                        session_id
                    ))
                } else {
                    let mut sessions = sessions.lock().await;
                    match sessions.get(&session_id) {
                        Some(CachedSftpSession::Pending {
                            notify: existing_notify,
                            generation: existing_generation,
                        }) if Arc::ptr_eq(existing_notify, &pending_notify)
                            && *existing_generation == pending_generation =>
                        {
                            sessions.insert(
                                session_id,
                                CachedSftpSession::Ready {
                                    session: sftp_arc.clone(),
                                    generation: pending_generation,
                                },
                            );
                            debug!("SFTP session initialized for {}", session_id);
                            Ok(SftpSessionLease {
                                session_id,
                                generation: pending_generation,
                                session: sftp_arc,
                                generations: generations.clone(),
                            })
                        }
                        Some(CachedSftpSession::Ready {
                            session: existing_session,
                            generation,
                        }) => Ok(SftpSessionLease {
                            session_id,
                            generation: *generation,
                            session: existing_session.clone(),
                            generations: generations.clone(),
                        }),
                        _ => Err(format!(
                            "Session {} SFTP initialization was canceled",
                            session_id
                        )),
                    }
                }
            }
            Err(err) => Err(err),
        };

        if result.is_err() {
            let mut sessions = sessions.lock().await;
            if matches!(
                sessions.get(&session_id),
                Some(CachedSftpSession::Pending {
                    notify: existing_notify,
                    generation: existing_generation
                })
                    if Arc::ptr_eq(existing_notify, &pending_notify)
                        && *existing_generation == pending_generation
            ) {
                sessions.remove(&session_id);
            }
        }

        pending_notify.notify_waiters();
        return result;
    }
}
