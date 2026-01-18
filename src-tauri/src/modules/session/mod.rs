use crate::modules::error::{AppError, Result};
use std::collections::HashMap;
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq)]
pub enum SessionStatus {
    Connecting,
    Connected,
    Disconnecting,
    Disconnected,
    Error(String),
}

#[derive(Debug, Clone)]
pub struct Session {
    pub id: Uuid,
    pub connection_id: Uuid,
    pub status: SessionStatus,
    pub created_at: u64,
    pub last_activity: u64,
    pub terminal_size: (u16, u16),
}

#[derive(Default)]
pub struct SessionManager {
    sessions: HashMap<String, Session>,
}

impl SessionManager {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn create_session(&mut self, connection_id: &Uuid) -> Session {
        let id = Uuid::new_v4();
        let now = chrono::Utc::now().timestamp_millis() as u64;

        let session = Session {
            id,
            connection_id: *connection_id,
            status: SessionStatus::Disconnected,
            created_at: now,
            last_activity: now,
            terminal_size: (80, 24),
        };

        self.sessions.insert(id.to_string(), session.clone());
        session
    }

    pub fn get_session(&self, session_id: &str) -> Option<&Session> {
        self.sessions.get(session_id)
    }

    pub fn get_mut_session(&mut self, session_id: &str) -> Option<&mut Session> {
        self.sessions.get_mut(session_id)
    }

    pub fn update_session_status(&mut self, session_id: &str, status: SessionStatus) -> Result<()> {
        if let Some(session) = self.sessions.get_mut(session_id) {
            session.status = status;
            session.last_activity = chrono::Utc::now().timestamp_millis() as u64;
            Ok(())
        } else {
            Err(AppError::SessionError(format!(
                "Session not found: {}",
                session_id
            )))
        }
    }

    pub fn update_terminal_size(&mut self, session_id: &str, cols: u16, rows: u16) -> Result<()> {
        if let Some(session) = self.sessions.get_mut(session_id) {
            session.terminal_size = (cols, rows);
            session.last_activity = chrono::Utc::now().timestamp_millis() as u64;
            Ok(())
        } else {
            Err(AppError::SessionError(format!(
                "Session not found: {}",
                session_id
            )))
        }
    }

    pub fn remove_session(&mut self, session_id: &str) -> Result<()> {
        if self.sessions.remove(session_id).is_some() {
            Ok(())
        } else {
            Err(AppError::SessionError(format!(
                "Session not found: {}",
                session_id
            )))
        }
    }

    pub fn get_all_sessions(&self) -> Vec<Session> {
        self.sessions.values().cloned().collect()
    }
}
