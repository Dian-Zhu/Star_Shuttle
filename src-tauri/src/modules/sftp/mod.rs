use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use uuid::Uuid;
use russh_sftp::client::SftpSession;
use serde::{Serialize, Deserialize};
use crate::modules::connection::DefaultConnectionManager;
use tauri::State;

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct FileEntry {
    pub name: String,
    pub is_dir: bool,
    pub size: u64,
    pub modified: u64,
    pub permissions: u32,
}

pub struct SftpManager {
    sessions: Arc<Mutex<HashMap<Uuid, Arc<Mutex<SftpSession>>>>>,
    connection_manager: Arc<std::sync::RwLock<DefaultConnectionManager>>,
}

impl SftpManager {
    pub fn new(connection_manager: Arc<std::sync::RwLock<DefaultConnectionManager>>) -> Self {
        Self {
            sessions: Arc::new(Mutex::new(HashMap::new())),
            connection_manager,
        }
    }
// ...

    async fn get_session(&self, session_id: Uuid) -> Result<Arc<Mutex<SftpSession>>, String> {
        let mut sessions = self.sessions.lock().await;
        if let Some(session) = sessions.get(&session_id) {
            return Ok(session.clone());
        }
        
        // Create new
        let ssh_conn = {
            let cm = self.connection_manager.read().map_err(|e| e.to_string())?;
            cm.get_ssh_connection(&session_id).ok_or("SSH session not found")?
        };
        
        let handle = ssh_conn.handle.lock().await;
        let channel = handle.channel_open_session().await.map_err(|e| e.to_string())?;
        channel.request_subsystem(true, "sftp").await.map_err(|e| e.to_string())?;
        
        let sftp = SftpSession::new(channel.into_stream()).await.map_err(|e| e.to_string())?;
        
        let sftp_arc = Arc::new(Mutex::new(sftp));
        sessions.insert(session_id, sftp_arc.clone());
        
        Ok(sftp_arc)
    }
    
    pub async fn list_directory(&self, session_id: Uuid, path: String) -> Result<Vec<FileEntry>, String> {
        let session_arc = self.get_session(session_id).await?;
        let session = session_arc.lock().await;
        
        let path = if path.is_empty() { "." } else { &path };
        
        let files = session.read_dir(path).await.map_err(|e| e.to_string())?;
        
        let entries = files.into_iter().map(|f| {
                let attrs = f.file_type();
                
                FileEntry {
                    name: f.file_name(),
                    is_dir: attrs.is_dir(),
                    size: f.metadata().size.unwrap_or(0),
                    modified: f.metadata().modified().unwrap_or(std::time::SystemTime::UNIX_EPOCH).duration_since(std::time::SystemTime::UNIX_EPOCH).unwrap_or_default().as_secs(),
                    permissions: f.metadata().permissions.unwrap_or(0),
                }
            }).collect();
        
        Ok(entries)
    }

    pub async fn read_file(&self, session_id: Uuid, path: String) -> Result<Vec<u8>, String> {
        let session_arc = self.get_session(session_id).await?;
        let session = session_arc.lock().await;
        
        let mut file = session.open(path).await.map_err(|e| e.to_string())?;
        let mut contents = Vec::new();
        file.read_to_end(&mut contents).await.map_err(|e| e.to_string())?;
        
        Ok(contents)
    }

    pub async fn write_file(&self, session_id: Uuid, path: String, content: Vec<u8>, append: bool) -> Result<(), String> {
        let session_arc = self.get_session(session_id).await?;
        let session = session_arc.lock().await;
        
        let mut file = if append {
            use russh_sftp::protocol::OpenFlags;
            session.open_with_flags(
                &path,
                OpenFlags::WRITE | OpenFlags::APPEND | OpenFlags::CREATE,
            ).await.map_err(|e| e.to_string())?
        } else {
            session.create(&path).await.map_err(|e| e.to_string())?
        };
        
        file.write_all(&content).await.map_err(|e| e.to_string())?;
        
        Ok(())
    }

    pub async fn create_directory(&self, session_id: Uuid, path: String) -> Result<(), String> {
        let session_arc = self.get_session(session_id).await?;
        let session = session_arc.lock().await;
        session.create_dir(path).await.map_err(|e| e.to_string())?;
        Ok(())
    }

    pub async fn remove_file(&self, session_id: Uuid, path: String) -> Result<(), String> {
        let session_arc = self.get_session(session_id).await?;
        let session = session_arc.lock().await;
        session.remove_file(path).await.map_err(|e| e.to_string())?;
        Ok(())
    }

    pub async fn remove_directory(&self, session_id: Uuid, path: String) -> Result<(), String> {
        let session_arc = self.get_session(session_id).await?;
        let session = session_arc.lock().await;
        session.remove_dir(path).await.map_err(|e| e.to_string())?;
        Ok(())
    }
    
    pub async fn rename(&self, session_id: Uuid, old_path: String, new_path: String) -> Result<(), String> {
        let session_arc = self.get_session(session_id).await?;
        let session = session_arc.lock().await;
        session.rename(old_path, new_path).await.map_err(|e| e.to_string())?;
        Ok(())
    }
}

#[tauri::command]
pub async fn sftp_ls(
    state: State<'_, SftpManager>,
    session_id: Uuid,
    path: String,
) -> Result<Vec<FileEntry>, String> {
    state.list_directory(session_id, path).await
}

#[tauri::command]
pub async fn sftp_read(
    state: State<'_, SftpManager>,
    session_id: Uuid,
    path: String,
) -> Result<Vec<u8>, String> {
    state.read_file(session_id, path).await
}

#[tauri::command]
pub async fn sftp_write(
    state: State<'_, SftpManager>,
    session_id: Uuid,
    path: String,
    content: Vec<u8>,
    append: Option<bool>,
) -> Result<(), String> {
    state.write_file(session_id, path, content, append.unwrap_or(false)).await
}

#[tauri::command]
pub async fn sftp_mkdir(
    state: State<'_, SftpManager>,
    session_id: Uuid,
    path: String,
) -> Result<(), String> {
    state.create_directory(session_id, path).await
}

#[tauri::command]
pub async fn sftp_rm(
    state: State<'_, SftpManager>,
    session_id: Uuid,
    path: String,
) -> Result<(), String> {
    state.remove_file(session_id, path).await
}

#[tauri::command]
pub async fn sftp_rmdir(
    state: State<'_, SftpManager>,
    session_id: Uuid,
    path: String,
) -> Result<(), String> {
    state.remove_directory(session_id, path).await
}

#[tauri::command]
pub async fn sftp_rename(
    state: State<'_, SftpManager>,
    session_id: Uuid,
    old_path: String,
    new_path: String,
) -> Result<(), String> {
    state.rename(session_id, old_path, new_path).await
}
