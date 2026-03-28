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
                notify.notified().await;
                continue;
            }
            AcquireAction::Retry => continue,
            AcquireAction::Create { notify, generation } => (notify, generation),
        };

        println!(
            "[SFTP] get_session: Creating new SFTP session for {}",
            session_id
        );

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
                    println!(
                        "[SFTP] get_session failed: SSH session not found for {}",
                        session_id
                    );
                    "SSH session not found".to_string()
                })?
            };

            let handle = ssh_conn.handle.lock().await;
            let channel = handle.channel_open_session().await.map_err(|e| {
                println!("[SFTP] get_session failed: channel open error: {}", e);
                e.to_string()
            })?;
            channel.request_subsystem(true, "sftp").await.map_err(|e| {
                println!("[SFTP] get_session failed: subsystem request error: {}", e);
                e.to_string()
            })?;

            let sftp = SftpSession::new(channel.into_stream()).await.map_err(|e| {
                println!("[SFTP] get_session failed: sftp init error: {}", e);
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
                            println!(
                                "[SFTP] get_session: Successfully created SFTP session for {}",
                                session_id
                            );
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
