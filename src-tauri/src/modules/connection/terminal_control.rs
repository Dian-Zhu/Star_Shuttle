use log::{error, warn};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Instant;
use tokio::sync::mpsc;
use uuid::Uuid;

use super::ConnectionError;
use crate::modules::connection::tracking::ChannelTracker;

#[derive(Clone)]
pub struct TerminalSession {
    pub id: Uuid,
    pub session_id: Uuid,
    pub sender: mpsc::Sender<TerminalCommand>,
}

pub enum TerminalCommand {
    Data(Vec<u8>),
    Resize(u32, u32),
    Close,
}

pub fn try_send_terminal_command(
    sender: &mpsc::Sender<TerminalCommand>,
    command: TerminalCommand,
) -> Result<(), ConnectionError> {
    match sender.try_send(command) {
        Ok(()) => Ok(()),
        Err(tokio::sync::mpsc::error::TrySendError::Full(_)) => Err(
            ConnectionError::ConnectionFailed("Terminal command queue is full".to_string()),
        ),
        Err(tokio::sync::mpsc::error::TrySendError::Closed(_)) => Err(
            ConnectionError::ConnectionFailed("Terminal command channel is closed".to_string()),
        ),
    }
}

pub fn send_terminal_data(
    terminals: &HashMap<Uuid, TerminalSession>,
    tracker: &Arc<Mutex<ChannelTracker>>,
    session_id: &Uuid,
    data: &str,
) -> Result<(), ConnectionError> {
    let terminal = terminals
        .values()
        .find(|t| &t.session_id == session_id)
        .ok_or(ConnectionError::SessionNotFound(*session_id))?;

    let data_bytes = data.as_bytes().to_vec();

    if let Ok(mut tracker) = tracker.lock() {
        tracker.log_data(*session_id, &data_bytes, "sent");
    } else {
        error!("Failed to lock channel tracker for logging sent data");
    }

    let sender = terminal.sender.clone();
    let send_start = Instant::now();
    match try_send_terminal_command(&sender, TerminalCommand::Data(data_bytes)) {
        Ok(_) => {
            let send_ms = send_start.elapsed().as_millis();
            if send_ms > 10 {
                warn!(
                    "Terminal input send slow session {}: {}ms",
                    session_id, send_ms
                );
            }
            Ok(())
        }
        Err(e) => {
            error!("Failed to send terminal data: {}", e);
            Err(e)
        }
    }
}

pub fn resize_terminal(
    terminals: &HashMap<Uuid, TerminalSession>,
    session_id: &Uuid,
    width: u16,
    height: u16,
) -> Result<(), ConnectionError> {
    let terminal = terminals
        .values()
        .find(|t| &t.session_id == session_id)
        .ok_or(ConnectionError::SessionNotFound(*session_id))?;

    try_send_terminal_command(
        &terminal.sender,
        TerminalCommand::Resize(width as u32, height as u32),
    )
}

pub fn close_terminal(
    terminals: &mut HashMap<Uuid, TerminalSession>,
    session_id: &Uuid,
) -> Result<(), ConnectionError> {
    let terminal = terminals
        .values()
        .find(|t| &t.session_id == session_id)
        .cloned()
        .ok_or(ConnectionError::SessionNotFound(*session_id))?;

    match terminal.sender.try_send(TerminalCommand::Close) {
        Ok(()) => {}
        Err(tokio::sync::mpsc::error::TrySendError::Full(TerminalCommand::Close)) => {
            terminal
                .sender
                .blocking_send(TerminalCommand::Close)
                .map_err(|_| {
                    ConnectionError::ConnectionFailed(
                        "Terminal command channel is closed".to_string(),
                    )
                })?;
        }
        Err(tokio::sync::mpsc::error::TrySendError::Closed(TerminalCommand::Close)) => {
            warn!(
                "Terminal close channel already closed for session {}",
                session_id
            );
        }
        Err(tokio::sync::mpsc::error::TrySendError::Full(_)) => {
            return Err(ConnectionError::ConnectionFailed(
                "Terminal close command could not be queued".to_string(),
            ));
        }
        Err(tokio::sync::mpsc::error::TrySendError::Closed(_)) => {
            warn!(
                "Terminal close channel already closed for session {}",
                session_id
            );
        }
    }

    terminals.remove(&terminal.id);
    Ok(())
}

pub fn get_terminal_sender(
    terminals: &HashMap<Uuid, TerminalSession>,
    session_id: &Uuid,
) -> Option<mpsc::Sender<TerminalCommand>> {
    terminals
        .values()
        .find(|t| &t.session_id == session_id)
        .map(|t| t.sender.clone())
}

pub fn log_terminal_data(
    tracker: &Arc<Mutex<ChannelTracker>>,
    session_id: &Uuid,
    data: &[u8],
    direction: &str,
) {
    if let Ok(mut tracker) = tracker.lock() {
        tracker.log_data(*session_id, data, direction);
    } else {
        error!("Failed to lock channel tracker for logging data");
    }
}
