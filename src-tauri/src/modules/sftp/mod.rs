use crate::modules::connection::DefaultConnectionManager;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, Mutex as StdMutex};
use tokio::sync::Mutex;
use uuid::Uuid;

mod common;
mod file_ops;
mod generation;
mod scp;
mod scp_flow;
mod session_cache;
mod ssh_bridge;
pub(crate) mod tauri_commands;
#[cfg(test)]
mod tests;

#[cfg(test)]
use self::generation::ensure_generation_current;
#[cfg(test)]
use self::generation::{finalize_io_result, is_generation_valid};
use self::generation::{SessionGenerationMap, SftpSessionLease};
use self::session_cache::CachedSftpSession;

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct FileEntry {
    pub name: String,
    pub is_dir: bool,
    pub size: u64,
    pub modified: u64,
    pub permissions: u32,
    pub owner: String,
    pub group: String,
}

pub struct SftpManager {
    sessions: Arc<Mutex<HashMap<Uuid, CachedSftpSession>>>,
    generations: SessionGenerationMap,
    connection_manager: Arc<std::sync::RwLock<DefaultConnectionManager>>,
}

impl SftpManager {
    pub fn new(connection_manager: Arc<std::sync::RwLock<DefaultConnectionManager>>) -> Self {
        Self {
            sessions: Arc::new(Mutex::new(HashMap::new())),
            generations: Arc::new(StdMutex::new(HashMap::new())),
            connection_manager,
        }
    }

    fn current_generation(&self, session_id: Uuid) -> Result<u64, String> {
        session_cache::current_generation(&self.generations, session_id)
    }

    pub async fn remove_session(&self, session_id: Uuid) {
        session_cache::remove_session(&self.sessions, &self.generations, session_id).await;
    }

    async fn get_session(&self, session_id: Uuid) -> Result<SftpSessionLease, String> {
        session_cache::get_session(
            &self.sessions,
            &self.generations,
            &self.connection_manager,
            session_id,
        )
        .await
    }

    pub async fn list_directory(
        &self,
        session_id: Uuid,
        path: String,
    ) -> Result<Vec<FileEntry>, String> {
        let session_lease = self.get_session(session_id).await?;
        file_ops::list_directory(session_lease, &self.connection_manager, session_id, path).await
    }

    pub async fn read_file(&self, session_id: Uuid, path: String) -> Result<Vec<u8>, String> {
        let session_lease = self.get_session(session_id).await?;
        file_ops::read_file(session_lease, path).await
    }

    pub async fn write_file(
        &self,
        session_id: Uuid,
        path: String,
        content: Vec<u8>,
        append: bool,
        offset: Option<u64>,
        truncate: bool,
    ) -> Result<(), String> {
        let session_lease = self.get_session(session_id).await?;
        file_ops::write_file(session_lease, path, content, append, offset, truncate).await
    }

    pub async fn create_directory(&self, session_id: Uuid, path: String) -> Result<(), String> {
        let session_lease = self.get_session(session_id).await?;
        file_ops::create_directory(session_lease, path).await
    }

    pub async fn remove_file(&self, session_id: Uuid, path: String) -> Result<(), String> {
        let session_lease = self.get_session(session_id).await?;
        file_ops::remove_file(session_lease, path).await
    }

    pub async fn remove_directory(&self, session_id: Uuid, path: String) -> Result<(), String> {
        let session_lease = self.get_session(session_id).await?;
        file_ops::remove_directory(session_lease, path).await
    }

    pub async fn rename(
        &self,
        session_id: Uuid,
        old_path: String,
        new_path: String,
    ) -> Result<(), String> {
        let session_lease = self.get_session(session_id).await?;
        file_ops::rename(session_lease, old_path, new_path).await
    }

    pub async fn scp_upload(
        &self,
        session_id: Uuid,
        remote_path: String,
        content: Vec<u8>,
    ) -> Result<(), String> {
        let generation = self.current_generation(session_id)?;
        scp_flow::scp_upload(
            &self.generations,
            &self.connection_manager,
            session_id,
            generation,
            remote_path,
            content,
        )
        .await
    }

    pub async fn scp_download(
        &self,
        session_id: Uuid,
        remote_path: String,
    ) -> Result<Vec<u8>, String> {
        let generation = self.current_generation(session_id)?;
        scp_flow::scp_download(
            &self.generations,
            &self.connection_manager,
            session_id,
            generation,
            remote_path,
        )
        .await
    }
}
