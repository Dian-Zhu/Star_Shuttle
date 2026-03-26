use crate::modules::connection::{ConnectionConfig, ConnectionProtocol, DefaultConnectionManager};
use crate::modules::db::DatabaseManager;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, RwLock};
use tauri::{AppHandle, Manager, State};
use uuid::Uuid;

#[derive(Debug, Default)]
pub struct AppLockRuntimeState {
    pub unlocked: bool,
}

pub(crate) const APP_LOCKED_ERROR: &str = "App is locked. Please unlock first.";

pub(crate) fn ensure_app_unlocked_runtime(
    db: &Arc<Mutex<DatabaseManager>>,
    app_lock_state: &Arc<Mutex<AppLockRuntimeState>>,
) -> Result<(), String> {
    let lock_enabled = {
        let db = db.lock().map_err(|e| e.to_string())?;
        db.get_setting("app_lock_hash")
            .map_err(|e| e.to_string())?
            .is_some()
    };

    if !lock_enabled {
        let mut state = app_lock_state.lock().map_err(|e| e.to_string())?;
        state.unlocked = true;
        return Ok(());
    }

    let state = app_lock_state.lock().map_err(|e| e.to_string())?;
    if state.unlocked {
        Ok(())
    } else {
        Err(APP_LOCKED_ERROR.to_string())
    }
}

// Create a separate module for commands to avoid macro name conflicts
mod commands {
    use super::*;
    use crate::modules::connection::known_hosts::KnownHostsManager;
    use crate::modules::connection::ConnectionManager;
    use std::fs::OpenOptions;
    use std::io::Write;
    use std::process::Command;
    use std::thread;
    use std::time::Duration;
    use tauri::command;

    fn set_unlock_state(
        app_lock_state: &State<Arc<Mutex<AppLockRuntimeState>>>,
        unlocked: bool,
    ) -> Result<(), String> {
        let mut state = app_lock_state.lock().map_err(|e| e.to_string())?;
        state.unlocked = unlocked;
        Ok(())
    }

    fn ensure_app_unlocked(
        db: &State<Arc<Mutex<DatabaseManager>>>,
        app_lock_state: &State<Arc<Mutex<AppLockRuntimeState>>>,
    ) -> Result<(), String> {
        crate::ensure_app_unlocked_runtime(db.inner(), app_lock_state.inner())
    }

    fn schedule_rdp_file_cleanup(path: PathBuf) {
        thread::spawn(move || {
            thread::sleep(Duration::from_secs(120));
            let _ = std::fs::remove_file(path);
        });
    }

    #[command]
    pub fn set_app_lock(
        db: State<Arc<Mutex<DatabaseManager>>>,
        app_lock_state: State<Arc<Mutex<AppLockRuntimeState>>>,
        password: String,
    ) -> Result<(), String> {
        let db = db.lock().map_err(|e| e.to_string())?;
        if db
            .get_setting("app_lock_hash")
            .map_err(|e| e.to_string())?
            .is_some()
        {
            return Err("App lock is already enabled".to_string());
        }

        let hash = bcrypt::hash(password, bcrypt::DEFAULT_COST).map_err(|e| e.to_string())?;
        db.save_setting("app_lock_hash", &hash)
            .map_err(|e| e.to_string())?;
        drop(db);
        set_unlock_state(&app_lock_state, false)
    }

    #[command]
    pub fn change_app_lock(
        db: State<Arc<Mutex<DatabaseManager>>>,
        app_lock_state: State<Arc<Mutex<AppLockRuntimeState>>>,
        current_password: String,
        new_password: String,
    ) -> Result<(), String> {
        let db = db.lock().map_err(|e| e.to_string())?;
        let hash = db
            .get_setting("app_lock_hash")
            .map_err(|e| e.to_string())?
            .ok_or_else(|| "App lock is not enabled".to_string())?;

        let is_valid = bcrypt::verify(current_password, &hash).map_err(|e| e.to_string())?;
        if !is_valid {
            return Err("Current password is incorrect".to_string());
        }

        let new_hash =
            bcrypt::hash(new_password, bcrypt::DEFAULT_COST).map_err(|e| e.to_string())?;
        db.save_setting("app_lock_hash", &new_hash)
            .map_err(|e| e.to_string())?;
        drop(db);
        set_unlock_state(&app_lock_state, true)
    }

    #[command]
    pub fn verify_app_lock(
        db: State<Arc<Mutex<DatabaseManager>>>,
        app_lock_state: State<Arc<Mutex<AppLockRuntimeState>>>,
        password: String,
    ) -> Result<bool, String> {
        let db = db.lock().map_err(|e| e.to_string())?;
        if let Some(hash) = db.get_setting("app_lock_hash").map_err(|e| e.to_string())? {
            let ok = bcrypt::verify(password, &hash).map_err(|e| e.to_string())?;
            drop(db);
            if ok {
                set_unlock_state(&app_lock_state, true)?;
            } else {
                set_unlock_state(&app_lock_state, false)?;
            }
            Ok(ok)
        } else {
            drop(db);
            set_unlock_state(&app_lock_state, true)?;
            Ok(false)
        }
    }

    #[command]
    pub fn is_app_lock_enabled(db: State<Arc<Mutex<DatabaseManager>>>) -> Result<bool, String> {
        let db = db.lock().map_err(|e| e.to_string())?;
        let result = db.get_setting("app_lock_hash").map_err(|e| e.to_string())?;
        Ok(result.is_some())
    }

    #[command]
    pub fn remove_app_lock(
        db: State<Arc<Mutex<DatabaseManager>>>,
        app_lock_state: State<Arc<Mutex<AppLockRuntimeState>>>,
        current_password: String,
    ) -> Result<(), String> {
        let db = db.lock().map_err(|e| e.to_string())?;
        let hash = db
            .get_setting("app_lock_hash")
            .map_err(|e| e.to_string())?
            .ok_or_else(|| "App lock is not enabled".to_string())?;

        let is_valid = bcrypt::verify(current_password, &hash).map_err(|e| e.to_string())?;
        if !is_valid {
            return Err("Current password is incorrect".to_string());
        }

        db.delete_setting("app_lock_hash")
            .map_err(|e| e.to_string())?;
        drop(db);
        set_unlock_state(&app_lock_state, true)
    }

    #[command]
    pub fn connect(
        app: AppHandle,
        db: State<Arc<Mutex<DatabaseManager>>>,
        app_lock_state: State<Arc<Mutex<AppLockRuntimeState>>>,
        manager: State<Arc<RwLock<DefaultConnectionManager>>>,
        config: ConnectionConfig,
    ) -> Result<Uuid, String> {
        ensure_app_unlocked(&db, &app_lock_state)?;
        let prepared = {
            let mut manager = manager
                .write()
                .map_err(|e| format!("Failed to acquire write lock: {}", e))?;
            manager
                .prepare_connect(&app, &config)
                .map_err(|e| e.to_string())?
        };
        let session_id = prepared.session_id;
        let completion = match DefaultConnectionManager::execute_prepared_connect(prepared) {
            Ok(completion) => completion,
            Err(error) => {
                if let Ok(mut manager) = manager.write() {
                    manager.finish_connect_failure(session_id);
                }
                return Err(error.to_string());
            }
        };

        let mut manager = manager
            .write()
            .map_err(|e| format!("Failed to acquire write lock: {}", e))?;
        manager
            .finish_connect_success(completion)
            .map_err(|e| e.to_string())
    }

    fn write_temp_rdp_file(host: &str, port: u16, username: &str) -> Result<PathBuf, String> {
        let mut content = String::new();
        content.push_str(&format!("full address:s:{}:{}\n", host, port));
        if !username.trim().is_empty() {
            content.push_str(&format!("username:s:{}\n", username));
        }
        content.push_str("prompt for credentials:i:1\n");
        let temp_dir = std::env::temp_dir();

        for _ in 0..8 {
            let filename = format!("starshuttle-{}.rdp", Uuid::new_v4());
            let path = temp_dir.join(filename);
            let mut options = OpenOptions::new();
            options.write(true).create_new(true);
            #[cfg(unix)]
            {
                use std::os::unix::fs::OpenOptionsExt;
                options.mode(0o600);
            }
            let mut file = match options.open(&path) {
                Ok(file) => file,
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(e) => return Err(e.to_string()),
            };
            file.write_all(content.as_bytes())
                .map_err(|e| e.to_string())?;
            file.flush().map_err(|e| e.to_string())?;
            return Ok(path);
        }

        Err("Failed to create temporary RDP file".to_string())
    }

    #[command]
    pub fn launch_rdp(
        db: State<Arc<Mutex<DatabaseManager>>>,
        app_lock_state: State<Arc<Mutex<AppLockRuntimeState>>>,
        config: ConnectionConfig,
    ) -> Result<(), String> {
        ensure_app_unlocked(&db, &app_lock_state)?;
        if config.protocol != ConnectionProtocol::Rdp {
            return Err("Only RDP protocol is supported by this command".to_string());
        }

        let host = config.host.trim();
        if host.is_empty() {
            return Err("Host is required".to_string());
        }
        let port = if config.port == 0 { 3389 } else { config.port };
        let username = config.username.trim();

        let rdp_path = write_temp_rdp_file(host, port, username)?;
        let cleanup_path = rdp_path.clone();

        #[cfg(target_os = "windows")]
        {
            Command::new("mstsc").arg(&rdp_path).spawn().map_err(|e| {
                let _ = std::fs::remove_file(&rdp_path);
                e.to_string()
            })?;
            schedule_rdp_file_cleanup(cleanup_path);
            return Ok(());
        }

        #[cfg(target_os = "macos")]
        {
            Command::new("open").arg(&rdp_path).spawn().map_err(|e| {
                let _ = std::fs::remove_file(&rdp_path);
                e.to_string()
            })?;
            schedule_rdp_file_cleanup(cleanup_path);
            return Ok(());
        }

        #[cfg(target_os = "linux")]
        {
            Command::new("xdg-open")
                .arg(&rdp_path)
                .spawn()
                .map_err(|e| {
                    let _ = std::fs::remove_file(&rdp_path);
                    e.to_string()
                })?;
            schedule_rdp_file_cleanup(cleanup_path);
            return Ok(());
        }

        #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
        {
            let _ = std::fs::remove_file(&rdp_path);
            Err("Unsupported OS for launching RDP".to_string())
        }
    }

    #[command]
    pub fn known_hosts_save_host_key(
        db: State<Arc<Mutex<DatabaseManager>>>,
        app_lock_state: State<Arc<Mutex<AppLockRuntimeState>>>,
        host: String,
        port: u16,
        key_type: String,
        key_base64: String,
        replace: Option<bool>,
    ) -> Result<(), String> {
        ensure_app_unlocked(&db, &app_lock_state)?;
        let mut manager = KnownHostsManager::new().map_err(|e| e.to_string())?;
        manager
            .upsert_host_key_parts(&host, port, key_type, key_base64, replace.unwrap_or(false))
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    #[command]
    pub async fn disconnect(
        db: State<'_, Arc<Mutex<DatabaseManager>>>,
        app_lock_state: State<'_, Arc<Mutex<AppLockRuntimeState>>>,
        manager: State<'_, Arc<RwLock<DefaultConnectionManager>>>,
        sftp_manager: State<'_, crate::modules::sftp::SftpManager>,
        session_id: Uuid,
    ) -> Result<(), String> {
        ensure_app_unlocked(&db, &app_lock_state)?;
        {
            let mut manager = manager
                .write()
                .map_err(|e| format!("Failed to acquire write lock: {}", e))?;
            manager
                .disconnect(&session_id)
                .map_err(|e: crate::modules::connection::ConnectionError| e.to_string())?;
        }

        sftp_manager.remove_session(session_id).await;
        Ok(())
    }

    #[command]
    pub fn get_session(
        db: State<Arc<Mutex<DatabaseManager>>>,
        app_lock_state: State<Arc<Mutex<AppLockRuntimeState>>>,
        manager: State<Arc<RwLock<DefaultConnectionManager>>>,
        session_id: Uuid,
    ) -> Result<Option<serde_json::Value>, String> {
        ensure_app_unlocked(&db, &app_lock_state)?;
        let manager = manager
            .read()
            .map_err(|e| format!("Failed to acquire read lock: {}", e))?;
        let session = manager.get_session(&session_id);
        match session {
            Some(s) => {
                let value = serde_json::to_value(s).map_err(|e| e.to_string())?;
                Ok(Some(value))
            }
            None => Ok(None),
        }
    }

    #[command]
    pub fn get_all_sessions(
        db: State<Arc<Mutex<DatabaseManager>>>,
        app_lock_state: State<Arc<Mutex<AppLockRuntimeState>>>,
        manager: State<Arc<RwLock<DefaultConnectionManager>>>,
    ) -> Result<Vec<serde_json::Value>, String> {
        ensure_app_unlocked(&db, &app_lock_state)?;
        let manager = manager
            .read()
            .map_err(|e| format!("Failed to acquire read lock: {}", e))?;
        let sessions = manager.get_all_sessions();
        sessions
            .into_iter()
            .map(|s| serde_json::to_value(s).map_err(|e| e.to_string()))
            .collect()
    }

    #[command]
    pub fn save_connection_config(
        db: State<Arc<Mutex<DatabaseManager>>>,
        app_lock_state: State<Arc<Mutex<AppLockRuntimeState>>>,
        manager: State<Arc<RwLock<DefaultConnectionManager>>>,
        config: ConnectionConfig,
    ) -> Result<(), String> {
        ensure_app_unlocked(&db, &app_lock_state)?;
        let mut manager = manager
            .write()
            .map_err(|e| format!("Failed to acquire write lock: {}", e))?;
        manager
            .save_connection_config(config)
            .map_err(|e| e.to_string())
    }

    #[command]
    pub fn delete_connection_config(
        db: State<Arc<Mutex<DatabaseManager>>>,
        app_lock_state: State<Arc<Mutex<AppLockRuntimeState>>>,
        manager: State<Arc<RwLock<DefaultConnectionManager>>>,
        connection_id: Uuid,
    ) -> Result<(), String> {
        ensure_app_unlocked(&db, &app_lock_state)?;
        let mut manager = manager
            .write()
            .map_err(|e| format!("Failed to acquire write lock: {}", e))?;
        manager
            .delete_connection_config(&connection_id)
            .map_err(|e| e.to_string())
    }

    #[command]
    pub fn get_all_connection_configs(
        db: State<Arc<Mutex<DatabaseManager>>>,
        app_lock_state: State<Arc<Mutex<AppLockRuntimeState>>>,
        manager: State<Arc<RwLock<DefaultConnectionManager>>>,
    ) -> Result<Vec<serde_json::Value>, String> {
        ensure_app_unlocked(&db, &app_lock_state)?;
        let manager = manager
            .read()
            .map_err(|e| format!("Failed to acquire read lock: {}", e))?;
        let configs = manager.get_all_connection_configs();
        configs
            .into_iter()
            .map(|c| serde_json::to_value(c).map_err(|e| e.to_string()))
            .collect()
    }

    #[command]
    pub fn test_connection(
        app: AppHandle,
        db: State<Arc<Mutex<DatabaseManager>>>,
        app_lock_state: State<Arc<Mutex<AppLockRuntimeState>>>,
        manager: State<Arc<RwLock<DefaultConnectionManager>>>,
        config: ConnectionConfig,
    ) -> Result<(), String> {
        ensure_app_unlocked(&db, &app_lock_state)?;
        let manager = manager
            .read()
            .map_err(|e| format!("Failed to acquire read lock: {}", e))?;
        manager
            .test_connection(&app, &config)
            .map_err(|e| e.to_string())
    }

    #[command]
    pub fn keyboard_interactive_respond(
        db: State<Arc<Mutex<DatabaseManager>>>,
        app_lock_state: State<Arc<Mutex<AppLockRuntimeState>>>,
        coordinator: State<crate::modules::connection::KeyboardInteractiveCoordinator>,
        request_id: String,
        responses: Vec<String>,
    ) -> Result<(), String> {
        ensure_app_unlocked(&db, &app_lock_state)?;
        coordinator.respond(request_id, responses)
    }

    #[command]
    pub fn keyboard_interactive_cancel(
        db: State<Arc<Mutex<DatabaseManager>>>,
        app_lock_state: State<Arc<Mutex<AppLockRuntimeState>>>,
        coordinator: State<crate::modules::connection::KeyboardInteractiveCoordinator>,
        request_id: String,
    ) -> Result<(), String> {
        ensure_app_unlocked(&db, &app_lock_state)?;
        coordinator.cancel(request_id)
    }

    #[command]
    pub fn start_terminal(
        app: AppHandle,
        db: State<Arc<Mutex<DatabaseManager>>>,
        app_lock_state: State<Arc<Mutex<AppLockRuntimeState>>>,
        manager: State<Arc<RwLock<DefaultConnectionManager>>>,
        session_id: Uuid,
        width: u16,
        height: u16,
    ) -> Result<bool, String> {
        ensure_app_unlocked(&db, &app_lock_state)?;
        let prepared = {
            let mut manager = manager
                .write()
                .map_err(|e| format!("Failed to acquire write lock: {}", e))?;
            manager
                .prepare_start_terminal(&session_id, width, height)
                .map_err(|e| e.to_string())?
        };
        let started = DefaultConnectionManager::execute_prepared_terminal_start(&app, prepared)
            .map_err(|e| e.to_string())?;
        let mut manager = manager
            .write()
            .map_err(|e| format!("Failed to acquire write lock: {}", e))?;
        manager
            .finish_start_terminal(started)
            .map_err(|e| e.to_string())
    }

    #[command]
    pub fn send_terminal_data(
        db: State<Arc<Mutex<DatabaseManager>>>,
        app_lock_state: State<Arc<Mutex<AppLockRuntimeState>>>,
        manager: State<Arc<RwLock<DefaultConnectionManager>>>,
        session_id: Uuid,
        data: String,
    ) -> Result<(), String> {
        ensure_app_unlocked(&db, &app_lock_state)?;
        let mut manager = manager
            .write()
            .map_err(|e| format!("Failed to acquire write lock: {}", e))?;
        manager
            .send_terminal_data(&session_id, &data)
            .map_err(|e| e.to_string())
    }

    #[command]
    pub fn resize_terminal(
        db: State<Arc<Mutex<DatabaseManager>>>,
        app_lock_state: State<Arc<Mutex<AppLockRuntimeState>>>,
        manager: State<Arc<RwLock<DefaultConnectionManager>>>,
        session_id: Uuid,
        width: u16,
        height: u16,
    ) -> Result<(), String> {
        ensure_app_unlocked(&db, &app_lock_state)?;
        let mut manager = manager
            .write()
            .map_err(|e| format!("Failed to acquire write lock: {}", e))?;
        manager
            .resize_terminal(&session_id, width, height)
            .map_err(|e| e.to_string())
    }

    #[command]
    pub fn close_terminal(
        db: State<Arc<Mutex<DatabaseManager>>>,
        app_lock_state: State<Arc<Mutex<AppLockRuntimeState>>>,
        manager: State<Arc<RwLock<DefaultConnectionManager>>>,
        session_id: Uuid,
    ) -> Result<(), String> {
        ensure_app_unlocked(&db, &app_lock_state)?;
        let mut manager = manager
            .write()
            .map_err(|e| format!("Failed to acquire write lock: {}", e))?;
        manager
            .close_terminal(&session_id)
            .map_err(|e| e.to_string())
    }

    #[command]
    pub fn exec_command(
        db: State<Arc<Mutex<DatabaseManager>>>,
        app_lock_state: State<Arc<Mutex<AppLockRuntimeState>>>,
        manager: State<Arc<RwLock<DefaultConnectionManager>>>,
        session_id: Uuid,
        command: String,
    ) -> Result<String, String> {
        ensure_app_unlocked(&db, &app_lock_state)?;
        let manager = manager
            .read()
            .map_err(|e| format!("Failed to acquire read lock: {}", e))?;
        manager
            .exec_command(&session_id, &command)
            .map_err(|e| e.to_string())
    }

    // Command snippet commands
    #[command]
    pub fn save_command_snippet(
        db: State<Arc<Mutex<DatabaseManager>>>,
        app_lock_state: State<Arc<Mutex<AppLockRuntimeState>>>,
        snippet: crate::modules::db::CommandSnippet,
    ) -> Result<(), String> {
        ensure_app_unlocked(&db, &app_lock_state)?;
        let db = db.lock().map_err(|e| e.to_string())?;
        db.save_command_snippet(&snippet).map_err(|e| e.to_string())
    }

    #[command]
    pub fn get_command_snippets(
        db: State<Arc<Mutex<DatabaseManager>>>,
        app_lock_state: State<Arc<Mutex<AppLockRuntimeState>>>,
    ) -> Result<Vec<crate::modules::db::CommandSnippet>, String> {
        ensure_app_unlocked(&db, &app_lock_state)?;
        let db = db.lock().map_err(|e| e.to_string())?;
        db.get_command_snippets().map_err(|e| e.to_string())
    }

    #[command]
    pub fn get_command_snippet_by_id(
        db: State<Arc<Mutex<DatabaseManager>>>,
        app_lock_state: State<Arc<Mutex<AppLockRuntimeState>>>,
        id: Uuid,
    ) -> Result<Option<crate::modules::db::CommandSnippet>, String> {
        ensure_app_unlocked(&db, &app_lock_state)?;
        let db = db.lock().map_err(|e| e.to_string())?;
        db.get_command_snippet_by_id(&id).map_err(|e| e.to_string())
    }

    #[command]
    pub fn delete_command_snippet(
        db: State<Arc<Mutex<DatabaseManager>>>,
        app_lock_state: State<Arc<Mutex<AppLockRuntimeState>>>,
        id: Uuid,
    ) -> Result<(), String> {
        ensure_app_unlocked(&db, &app_lock_state)?;
        let db = db.lock().map_err(|e| e.to_string())?;
        db.delete_command_snippet(&id).map_err(|e| e.to_string())
    }

    #[command]
    pub fn increment_command_snippet_usage(
        db: State<Arc<Mutex<DatabaseManager>>>,
        app_lock_state: State<Arc<Mutex<AppLockRuntimeState>>>,
        id: Uuid,
    ) -> Result<(), String> {
        ensure_app_unlocked(&db, &app_lock_state)?;
        let db = db.lock().map_err(|e| e.to_string())?;
        db.increment_usage_count(&id).map_err(|e| e.to_string())
    }

    #[command]
    pub fn log_audit_event(
        db: State<Arc<Mutex<DatabaseManager>>>,
        app_lock_state: State<Arc<Mutex<AppLockRuntimeState>>>,
        event: crate::modules::db::AuditEvent,
    ) -> Result<(), String> {
        ensure_app_unlocked(&db, &app_lock_state)?;
        let db = db.lock().map_err(|e| e.to_string())?;
        db.save_audit_event(&event).map_err(|e| e.to_string())
    }

    #[command]
    pub fn get_audit_events(
        db: State<Arc<Mutex<DatabaseManager>>>,
        app_lock_state: State<Arc<Mutex<AppLockRuntimeState>>>,
        limit: Option<u32>,
    ) -> Result<Vec<crate::modules::db::AuditEvent>, String> {
        ensure_app_unlocked(&db, &app_lock_state)?;
        let db = db.lock().map_err(|e| e.to_string())?;
        db.get_audit_events(limit).map_err(|e| e.to_string())
    }

    #[command]
    pub fn clear_audit_events(
        db: State<Arc<Mutex<DatabaseManager>>>,
        app_lock_state: State<Arc<Mutex<AppLockRuntimeState>>>,
    ) -> Result<(), String> {
        ensure_app_unlocked(&db, &app_lock_state)?;
        let db = db.lock().map_err(|e| e.to_string())?;
        db.clear_audit_events().map_err(|e| e.to_string())
    }

    #[command]
    pub fn get_logs(
        db: State<Arc<Mutex<DatabaseManager>>>,
        app_lock_state: State<Arc<Mutex<AppLockRuntimeState>>>,
    ) -> Result<Vec<String>, String> {
        ensure_app_unlocked(&db, &app_lock_state)?;
        Ok(crate::modules::logging::LogManager::get_logs())
    }

    #[command]
    pub fn clear_logs(
        db: State<Arc<Mutex<DatabaseManager>>>,
        app_lock_state: State<Arc<Mutex<AppLockRuntimeState>>>,
    ) -> Result<(), String> {
        ensure_app_unlocked(&db, &app_lock_state)?;
        crate::modules::logging::LogManager::clear_logs();
        Ok(())
    }

    #[command]
    pub fn get_log_file_path(
        db: State<Arc<Mutex<DatabaseManager>>>,
        app_lock_state: State<Arc<Mutex<AppLockRuntimeState>>>,
    ) -> Result<Option<String>, String> {
        ensure_app_unlocked(&db, &app_lock_state)?;
        Ok(crate::modules::logging::LogManager::get_log_file_path())
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Initialize our custom structured logger
    crate::modules::logging::LogManager::init(log::LevelFilter::Info)
        .expect("Failed to initialize logger");

    let connection_manager = Arc::new(RwLock::new(DefaultConnectionManager::new()));
    let connection_manager_for_setup = connection_manager.clone();
    let sftp_manager = crate::modules::sftp::SftpManager::new(connection_manager.clone());

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .setup(move |app| {
            // Remove tauri_plugin_log to avoid logger initialization conflict
            // We use our custom structured logger instead

            let app_handle = app.handle();
            let app_dir = app_handle
                .path()
                .app_data_dir()
                .expect("failed to get app data dir");
            std::fs::create_dir_all(&app_dir).expect("failed to create app data dir");
            let db_path = app_dir.join("app.db");
            let db_manager = crate::modules::db::DatabaseManager::new(db_path.to_str().unwrap())
                .expect("failed to init db");
            let db = Arc::new(Mutex::new(db_manager));
            let lock_enabled = {
                let db_guard = db.lock().expect("failed to lock db for app lock init");
                db_guard
                    .get_setting("app_lock_hash")
                    .expect("failed to read app lock state")
                    .is_some()
            };
            let app_lock_state = Arc::new(Mutex::new(AppLockRuntimeState {
                unlocked: !lock_enabled,
            }));
            app.manage(db.clone());
            app.manage(app_lock_state);

            let mut manager = connection_manager_for_setup
                .write()
                .expect("failed to lock connection manager");
            manager
                .set_db(db)
                .expect("failed to init connection persistence");
            app.manage(manager.keyboard_interactive_coordinator());

            // System Tray
            let quit_i = tauri::menu::MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
            let show_i = tauri::menu::MenuItem::with_id(app, "show", "Show", true, None::<&str>)?;
            let menu = tauri::menu::Menu::with_items(app, &[&show_i, &quit_i])?;

            let _tray = tauri::tray::TrayIconBuilder::new()
                .icon(app.default_window_icon().unwrap().clone())
                .menu(&menu)
                .show_menu_on_left_click(false)
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "quit" => {
                        app.exit(0);
                    }
                    "show" => {
                        if let Some(window) = app.get_webview_window("main") {
                            let _ = window.show();
                            let _ = window.set_focus();
                        }
                    }
                    _ => {}
                })
                .on_tray_icon_event(|tray, event| {
                    if let tauri::tray::TrayIconEvent::Click {
                        button: tauri::tray::MouseButton::Left,
                        ..
                    } = event
                    {
                        let app = tray.app_handle();
                        if let Some(window) = app.get_webview_window("main") {
                            let _ = window.show();
                            let _ = window.set_focus();
                        }
                    }
                })
                .build(app)?;

            Ok(())
        })
        .manage(connection_manager)
        .manage(sftp_manager)
        .invoke_handler(tauri::generate_handler![
            // Connection management commands
            commands::connect,
            commands::disconnect,
            commands::known_hosts_save_host_key,
            commands::get_session,
            commands::get_all_sessions,
            commands::save_connection_config,
            commands::delete_connection_config,
            commands::get_all_connection_configs,
            commands::test_connection,
            commands::launch_rdp,
            commands::keyboard_interactive_respond,
            commands::keyboard_interactive_cancel,
            // Terminal commands
            commands::start_terminal,
            commands::send_terminal_data,
            commands::resize_terminal,
            commands::close_terminal,
            commands::exec_command,
            // App Lock commands
            commands::set_app_lock,
            commands::change_app_lock,
            commands::verify_app_lock,
            commands::is_app_lock_enabled,
            commands::remove_app_lock,
            // SFTP commands
            crate::modules::sftp::sftp_ls,
            crate::modules::sftp::sftp_read,
            crate::modules::sftp::sftp_read_chunk,
            crate::modules::sftp::sftp_write,
            crate::modules::sftp::sftp_mkdir,
            crate::modules::sftp::sftp_rm,
            crate::modules::sftp::sftp_rmdir,
            crate::modules::sftp::sftp_rename,
            crate::modules::sftp::scp_upload,
            crate::modules::sftp::scp_download,
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
            commands::get_logs,
            commands::clear_logs,
            commands::get_log_file_path,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

// Modules
pub mod models;
pub mod modules;
pub mod utils;
