use russh_sftp::client::SftpSession;
use std::collections::HashMap;
use std::sync::{Arc, Mutex as StdMutex};
use tokio::sync::Mutex;
use uuid::Uuid;

pub type SessionGenerationMap = Arc<StdMutex<HashMap<Uuid, u64>>>;

pub fn is_generation_valid(
    generations: &StdMutex<HashMap<Uuid, u64>>,
    session_id: Uuid,
    generation: u64,
) -> Result<bool, String> {
    let guard = generations.lock().map_err(|e| e.to_string())?;
    let current = guard.get(&session_id).copied().unwrap_or(0);
    Ok(current == generation)
}

pub fn invalidated_session_error(session_id: Uuid) -> String {
    format!("SFTP session {} has been invalidated", session_id)
}

pub fn ensure_generation_current(
    generations: &StdMutex<HashMap<Uuid, u64>>,
    session_id: Uuid,
    generation: u64,
) -> Result<(), String> {
    if is_generation_valid(generations, session_id, generation)? {
        Ok(())
    } else {
        Err(invalidated_session_error(session_id))
    }
}

pub fn finalize_io_result<T>(
    io_result: Result<T, String>,
    generations: &StdMutex<HashMap<Uuid, u64>>,
    session_id: Uuid,
    generation: u64,
) -> Result<T, String> {
    let value = io_result?;
    if is_generation_valid(generations, session_id, generation)? {
        Ok(value)
    } else {
        Err(invalidated_session_error(session_id))
    }
}

#[derive(Clone)]
pub struct SftpSessionLease {
    pub session_id: Uuid,
    pub generation: u64,
    pub session: Arc<Mutex<SftpSession>>,
    pub generations: SessionGenerationMap,
}

impl SftpSessionLease {
    pub fn ensure_valid(&self) -> Result<(), String> {
        if is_generation_valid(&self.generations, self.session_id, self.generation)? {
            return Ok(());
        }
        Err(invalidated_session_error(self.session_id))
    }

    pub fn finish_io<T>(&self, io_result: Result<T, String>) -> Result<T, String> {
        finalize_io_result(
            io_result,
            self.generations.as_ref(),
            self.session_id,
            self.generation,
        )
    }

    pub async fn lock(&self) -> tokio::sync::MutexGuard<'_, SftpSession> {
        self.session.lock().await
    }
}
