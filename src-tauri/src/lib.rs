use tauri::{State, AppHandle, Emitter};
use uuid::Uuid;
use std::sync::{Arc, RwLock};
use crate::modules::connection::{ConnectionConfig, DefaultConnectionManager};

// Create a separate module for commands to avoid macro name conflicts
mod commands {
    use super::*;
    use tauri::command;
    use crate::modules::connection::ConnectionManager;
    
    #[command]
    pub fn connect(
        manager: State<Arc<RwLock<DefaultConnectionManager>>>,
        config: ConnectionConfig,
    ) -> Result<Uuid, String> {
        let mut manager = manager.write().map_err(|e| format!("Failed to acquire write lock: {}", e))?;
        manager.connect(&config).map_err(|e| e.to_string())
    }
    
    #[command]
    pub fn disconnect(
        manager: State<Arc<RwLock<DefaultConnectionManager>>>,
        session_id: Uuid,
    ) -> Result<(), String> {
        let mut manager = manager.write().map_err(|e| format!("Failed to acquire write lock: {}", e))?;
        manager.disconnect(&session_id).map_err(|e| e.to_string())
    }
    
    #[command]
    pub fn get_session(
        manager: State<Arc<RwLock<DefaultConnectionManager>>>,
        session_id: Uuid,
    ) -> Result<Option<serde_json::Value>, String> {
        let manager = manager.read().map_err(|e| format!("Failed to acquire read lock: {}", e))?;
        let session = manager.get_session(&session_id);
        match session {
            Some(s) => {
                let value = serde_json::to_value(s).map_err(|e| e.to_string())?;
                Ok(Some(value))
            },
            None => Ok(None),
        }
    }
    
    #[command]
    pub fn get_all_sessions(
        manager: State<Arc<RwLock<DefaultConnectionManager>>>,
    ) -> Result<Vec<serde_json::Value>, String> {
        let manager = manager.read().map_err(|e| format!("Failed to acquire read lock: {}", e))?;
        let sessions = manager.get_all_sessions();
        sessions.into_iter()
            .map(|s| serde_json::to_value(s).map_err(|e| e.to_string()))
            .collect()
    }
    
    #[command]
    pub fn save_connection_config(
        manager: State<Arc<RwLock<DefaultConnectionManager>>>,
        config: ConnectionConfig,
    ) -> Result<(), String> {
        let mut manager = manager.write().map_err(|e| format!("Failed to acquire write lock: {}", e))?;
        manager.save_connection_config(config).map_err(|e| e.to_string())
    }
    
    #[command]
    pub fn delete_connection_config(
        manager: State<Arc<RwLock<DefaultConnectionManager>>>,
        connection_id: Uuid,
    ) -> Result<(), String> {
        let mut manager = manager.write().map_err(|e| format!("Failed to acquire write lock: {}", e))?;
        manager.delete_connection_config(&connection_id).map_err(|e| e.to_string())
    }
    
    #[command]
    pub fn get_all_connection_configs(
        manager: State<Arc<RwLock<DefaultConnectionManager>>>,
    ) -> Result<Vec<serde_json::Value>, String> {
        let manager = manager.read().map_err(|e| format!("Failed to acquire read lock: {}", e))?;
        let configs = manager.get_all_connection_configs();
        configs.into_iter()
            .map(|c| serde_json::to_value(c).map_err(|e| e.to_string()))
            .collect()
    }
    
    #[command]
    pub fn test_connection(
        manager: State<Arc<RwLock<DefaultConnectionManager>>>,
        config: ConnectionConfig,
    ) -> Result<(), String> {
        let manager = manager.read().map_err(|e| format!("Failed to acquire read lock: {}", e))?;
        manager.test_connection(&config).map_err(|e| e.to_string())
    }

    #[command]
    pub fn start_terminal(
        app: AppHandle,
        manager: State<Arc<RwLock<DefaultConnectionManager>>>,
        session_id: Uuid,
        width: u16,
        height: u16,
    ) -> Result<bool, String> {
        let mut manager = manager.write().map_err(|e| format!("Failed to acquire write lock: {}", e))?;
        manager.start_terminal(&app, &session_id, width, height).map_err(|e| e.to_string())
    }

    #[command]
    pub fn send_terminal_data(
        manager: State<Arc<RwLock<DefaultConnectionManager>>>,
        session_id: Uuid,
        data: String,
    ) -> Result<(), String> {
        let mut manager = manager.write().map_err(|e| format!("Failed to acquire write lock: {}", e))?;
        manager.send_terminal_data(&session_id, &data).map_err(|e| e.to_string())
    }

    #[command]
    pub fn resize_terminal(
        manager: State<Arc<RwLock<DefaultConnectionManager>>>,
        session_id: Uuid,
        width: u16,
        height: u16,
    ) -> Result<(), String> {
        let mut manager = manager.write().map_err(|e| format!("Failed to acquire write lock: {}", e))?;
        manager.resize_terminal(&session_id, width, height).map_err(|e| e.to_string())
    }

    #[command]
    pub fn close_terminal(
        manager: State<Arc<RwLock<DefaultConnectionManager>>>,
        session_id: Uuid,
    ) -> Result<(), String> {
        let mut manager = manager.write().map_err(|e| format!("Failed to acquire write lock: {}", e))?;
        manager.close_terminal(&session_id).map_err(|e| e.to_string())
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Initialize our custom structured logger
    crate::modules::logging::LogManager::init(log::LevelFilter::Debug)
        .expect("Failed to initialize logger");
    
    let connection_manager = Arc::new(RwLock::new(DefaultConnectionManager::new()));
    let sftp_manager = crate::modules::sftp::SftpManager::new(connection_manager.clone());

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .setup(|_app| {
            // Remove tauri_plugin_log to avoid logger initialization conflict
            // We use our custom structured logger instead
            Ok(())
        })
        .manage(connection_manager)
        .manage(sftp_manager)
        .invoke_handler(tauri::generate_handler![
            // Connection management commands
            commands::connect,
            commands::disconnect,
            commands::get_session,
            commands::get_all_sessions,
            commands::save_connection_config,
            commands::delete_connection_config,
            commands::get_all_connection_configs,
            commands::test_connection,
            // Terminal commands
            commands::start_terminal,
            commands::send_terminal_data,
            commands::resize_terminal,
            commands::close_terminal,
            // SFTP commands
            crate::modules::sftp::sftp_ls,
            crate::modules::sftp::sftp_read,
            crate::modules::sftp::sftp_write,
            crate::modules::sftp::sftp_mkdir,
            crate::modules::sftp::sftp_rm,
            crate::modules::sftp::sftp_rmdir,
            crate::modules::sftp::sftp_rename,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

// Modules
pub mod modules;
pub mod models;
pub mod utils;
