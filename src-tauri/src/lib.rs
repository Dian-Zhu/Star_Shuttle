use tauri::{State, AppHandle, Manager};
use uuid::Uuid;
use std::sync::{Arc, RwLock, Mutex};
use crate::modules::connection::{ConnectionConfig, DefaultConnectionManager};
use crate::modules::db::DatabaseManager;

// Create a separate module for commands to avoid macro name conflicts
mod commands {
    use super::*;
    use tauri::command;
    use crate::modules::connection::ConnectionManager;
    
    #[command]
    pub fn set_app_lock(
        db: State<Arc<Mutex<DatabaseManager>>>,
        password: String
    ) -> Result<(), String> {
        let hash = bcrypt::hash(password, bcrypt::DEFAULT_COST).map_err(|e| e.to_string())?;
        let db = db.lock().map_err(|e| e.to_string())?;
        db.save_setting("app_lock_hash", &hash).map_err(|e| e.to_string())
    }

    #[command]
    pub fn verify_app_lock(
        db: State<Arc<Mutex<DatabaseManager>>>,
        password: String
    ) -> Result<bool, String> {
        let db = db.lock().map_err(|e| e.to_string())?;
        if let Some(hash) = db.get_setting("app_lock_hash").map_err(|e| e.to_string())? {
            bcrypt::verify(password, &hash).map_err(|e| e.to_string())
        } else {
            Ok(false)
        }
    }

    #[command]
    pub fn is_app_lock_enabled(
        db: State<Arc<Mutex<DatabaseManager>>>
    ) -> Result<bool, String> {
        let db = db.lock().map_err(|e| e.to_string())?;
        let result = db.get_setting("app_lock_hash").map_err(|e| e.to_string())?;
        Ok(result.is_some())
    }

    #[command]
    pub fn remove_app_lock(
        db: State<Arc<Mutex<DatabaseManager>>>
    ) -> Result<(), String> {
        let db = db.lock().map_err(|e| e.to_string())?;
        db.delete_setting("app_lock_hash").map_err(|e| e.to_string())
    }

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

    #[command]
    pub fn exec_command(
        manager: State<Arc<RwLock<DefaultConnectionManager>>>,
        session_id: Uuid,
        command: String,
    ) -> Result<String, String> {
        let mut manager = manager.write().map_err(|e| format!("Failed to acquire write lock: {}", e))?;
        manager.exec_command(&session_id, &command).map_err(|e| e.to_string())
    }

    // Command snippet commands
    #[command]
    pub fn save_command_snippet(
        db: State<Arc<Mutex<DatabaseManager>>>,
        snippet: crate::modules::db::CommandSnippet,
    ) -> Result<(), String> {
        let db = db.lock().map_err(|e| e.to_string())?;
        db.save_command_snippet(&snippet).map_err(|e| e.to_string())
    }

    #[command]
    pub fn get_command_snippets(
        db: State<Arc<Mutex<DatabaseManager>>>,
    ) -> Result<Vec<crate::modules::db::CommandSnippet>, String> {
        let db = db.lock().map_err(|e| e.to_string())?;
        db.get_command_snippets().map_err(|e| e.to_string())
    }

    #[command]
    pub fn get_command_snippet_by_id(
        db: State<Arc<Mutex<DatabaseManager>>>,
        id: Uuid,
    ) -> Result<Option<crate::modules::db::CommandSnippet>, String> {
        let db = db.lock().map_err(|e| e.to_string())?;
        db.get_command_snippet_by_id(&id).map_err(|e| e.to_string())
    }

    #[command]
    pub fn delete_command_snippet(
        db: State<Arc<Mutex<DatabaseManager>>>,
        id: Uuid,
    ) -> Result<(), String> {
        let db = db.lock().map_err(|e| e.to_string())?;
        db.delete_command_snippet(&id).map_err(|e| e.to_string())
    }

    #[command]
    pub fn increment_command_snippet_usage(
        db: State<Arc<Mutex<DatabaseManager>>>,
        id: Uuid,
    ) -> Result<(), String> {
        let db = db.lock().map_err(|e| e.to_string())?;
        db.increment_usage_count(&id).map_err(|e| e.to_string())
    }

    #[command]
    pub fn log_audit_event(
        db: State<Arc<Mutex<DatabaseManager>>>,
        event: crate::modules::db::AuditEvent,
    ) -> Result<(), String> {
        let db = db.lock().map_err(|e| e.to_string())?;
        db.save_audit_event(&event).map_err(|e| e.to_string())
    }

    #[command]
    pub fn get_audit_events(
        db: State<Arc<Mutex<DatabaseManager>>>,
        limit: Option<u32>,
    ) -> Result<Vec<crate::modules::db::AuditEvent>, String> {
        let db = db.lock().map_err(|e| e.to_string())?;
        db.get_audit_events(limit).map_err(|e| e.to_string())
    }

    #[command]
    pub fn clear_audit_events(
        db: State<Arc<Mutex<DatabaseManager>>>,
    ) -> Result<(), String> {
        let db = db.lock().map_err(|e| e.to_string())?;
        db.clear_audit_events().map_err(|e| e.to_string())
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
        .setup(|app| {
            // Remove tauri_plugin_log to avoid logger initialization conflict
            // We use our custom structured logger instead
            
            let app_handle = app.handle();
            let app_dir = app_handle.path().app_data_dir().expect("failed to get app data dir");
            std::fs::create_dir_all(&app_dir).expect("failed to create app data dir");
            let db_path = app_dir.join("app.db");
            let db_manager = crate::modules::db::DatabaseManager::new(db_path.to_str().unwrap()).expect("failed to init db");
            app.manage(Arc::new(Mutex::new(db_manager)));
            
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
            commands::exec_command,
            // App Lock commands
            commands::set_app_lock,
            commands::verify_app_lock,
            commands::is_app_lock_enabled,
            commands::remove_app_lock,
            // SFTP commands
            crate::modules::sftp::sftp_ls,
            crate::modules::sftp::sftp_read,
            crate::modules::sftp::sftp_write,
            crate::modules::sftp::sftp_mkdir,
            crate::modules::sftp::sftp_rm,
            crate::modules::sftp::sftp_rmdir,
            crate::modules::sftp::sftp_rename,
            // Command snippet commands
            commands::save_command_snippet,
            commands::get_command_snippets,
            commands::get_command_snippet_by_id,
            commands::delete_command_snippet,
            commands::increment_command_snippet_usage,
            // Audit logging commands
            commands::log_audit_event,
            commands::get_audit_events,
            commands::clear_audit_events,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

// Modules
pub mod modules;
pub mod models;
pub mod utils;
