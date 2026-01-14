use tauri::{State}; use uuid::Uuid;
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
        let mut manager = manager.write().unwrap();
        manager.connect(&config).map_err(|e| e.to_string())
    }
    
    #[command]
    pub fn disconnect(
        manager: State<Arc<RwLock<DefaultConnectionManager>>>,
        session_id: Uuid,
    ) -> Result<(), String> {
        let mut manager = manager.write().unwrap();
        manager.disconnect(&session_id).map_err(|e| e.to_string())
    }
    
    #[command]
    pub fn get_session(
        manager: State<Arc<RwLock<DefaultConnectionManager>>>,
        session_id: Uuid,
    ) -> Result<Option<serde_json::Value>, String> {
        let manager = manager.read().unwrap();
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
        let manager = manager.read().unwrap();
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
        let mut manager = manager.write().unwrap();
        manager.save_connection_config(config).map_err(|e| e.to_string())
    }
    
    #[command]
    pub fn delete_connection_config(
        manager: State<Arc<RwLock<DefaultConnectionManager>>>,
        connection_id: Uuid,
    ) -> Result<(), String> {
        let mut manager = manager.write().unwrap();
        manager.delete_connection_config(&connection_id).map_err(|e| e.to_string())
    }
    
    #[command]
    pub fn get_all_connection_configs(
        manager: State<Arc<RwLock<DefaultConnectionManager>>>,
    ) -> Result<Vec<serde_json::Value>, String> {
        let manager = manager.read().unwrap();
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
        let manager = manager.read().unwrap();
        manager.test_connection(&config).map_err(|e| e.to_string())
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            if cfg!(debug_assertions) {
                app.handle().plugin(
                    tauri_plugin_log::Builder::default()
                        .level(log::LevelFilter::Info)
                        .build(),
                )?;
            }
            Ok(())
        })
        .manage(Arc::new(RwLock::new(DefaultConnectionManager::new())))
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
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

// Modules
pub mod modules;
pub mod models;
pub mod utils;
