#[cfg(test)]
use crate::modules::app_runtime::parse_host_key_error_payload;
pub(crate) use crate::modules::app_runtime::{
    enrich_host_key_error_with_challenge, ensure_app_unlocked_runtime, AppLockRuntimeState,
    HostKeyChallengeRuntimeState,
};
use crate::modules::connection::{
    ConnectionConfig, ConnectionProtocol, ConnectionStatus, DefaultConnectionManager,
};
use crate::modules::db::DatabaseManager;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, RwLock};
use tauri::{AppHandle, Manager, State};
use uuid::Uuid;

// Create a separate module for commands to avoid macro name conflicts
pub(crate) mod commands {
    use super::*;
    use crate::modules::connection::known_hosts::KnownHostsManager;
    use crate::modules::connection::ConnectionManager;
    use std::fs::OpenOptions;
    use std::io::Write;
    use std::process::Command;
    use std::sync::OnceLock;
    use std::thread;
    use std::time::{Duration, Instant};
    use tauri::command;
    use tauri_plugin_dialog::DialogExt;

    #[derive(Debug, Clone, serde::Deserialize)]
    pub struct LocalFsDialogFilter {
        pub name: String,
        pub extensions: Vec<String>,
    }

    #[derive(Debug, Clone, serde::Serialize)]
    pub struct LocalFsDialogGrant {
        pub path: String,
        pub access_token: String,
        pub size: u64,
    }

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

    const APP_LOCK_MIN_PASSWORD_LEN: usize = 8;
    const APP_LOCK_MAX_FAILED_ATTEMPTS: u32 = 5;
    const APP_LOCK_LOCKOUT_SECONDS: u64 = 15;
    const APP_LOCK_BCRYPT_COST: u32 = 13;
    const APP_LOCK_COMMON_WEAK_PASSWORDS: &[&str] = &[
        "password",
        "password1",
        "password123",
        "12345678",
        "123456789",
        "1234567890",
        "qwerty123",
        "qwerty1234",
        "admin123",
        "admin1234",
        "letmein1",
        "welcome1",
        "monkey123",
        "11111111",
        "22222222",
        "abc12345",
        "abcd1234",
        "iloveyou1",
        "trustno1",
        "sunshine1",
        "princess1",
        "football1",
        "charlie1",
        "access14",
        "master12",
    ];

    #[derive(Default)]
    struct AppLockVerifyThrottleState {
        failed_attempts: u32,
        blocked_until: Option<Instant>,
    }

    static APP_LOCK_VERIFY_THROTTLE: OnceLock<Mutex<AppLockVerifyThrottleState>> = OnceLock::new();

    fn app_lock_verify_throttle_state() -> &'static Mutex<AppLockVerifyThrottleState> {
        APP_LOCK_VERIFY_THROTTLE.get_or_init(|| Mutex::new(AppLockVerifyThrottleState::default()))
    }

    fn enforce_app_lock_verify_throttle() -> Result<(), String> {
        let mut state = app_lock_verify_throttle_state()
            .lock()
            .map_err(|e| e.to_string())?;

        if let Some(until) = state.blocked_until {
            let now = Instant::now();
            if now < until {
                let wait_seconds = (until - now).as_secs().max(1);
                return Err(format!(
                    "Too many failed attempts. Please retry in {} seconds.",
                    wait_seconds
                ));
            }
            state.blocked_until = None;
            state.failed_attempts = 0;
        }
        Ok(())
    }

    fn update_app_lock_verify_throttle(success: bool) {
        let Ok(mut state) = app_lock_verify_throttle_state().lock() else {
            return;
        };

        if success {
            state.failed_attempts = 0;
            state.blocked_until = None;
            return;
        }

        state.failed_attempts = state.failed_attempts.saturating_add(1);
        if state.failed_attempts >= APP_LOCK_MAX_FAILED_ATTEMPTS {
            state.failed_attempts = 0;
            state.blocked_until =
                Some(Instant::now() + Duration::from_secs(APP_LOCK_LOCKOUT_SECONDS));
        }
    }

    pub(crate) fn validate_app_lock_password_strength(password: &str) -> Result<(), String> {
        let trimmed = password.trim();
        if trimmed.is_empty() {
            return Err("Password cannot be empty".to_string());
        }

        if trimmed.chars().count() < APP_LOCK_MIN_PASSWORD_LEN {
            return Err(format!(
                "Password must be at least {} characters",
                APP_LOCK_MIN_PASSWORD_LEN
            ));
        }

        let has_letter = trimmed.chars().any(|c| c.is_ascii_alphabetic());
        let has_digit = trimmed.chars().any(|c| c.is_ascii_digit());
        if !has_letter || !has_digit {
            return Err("Password must include both letters and digits".to_string());
        }

        let normalized = trimmed.to_ascii_lowercase();
        if APP_LOCK_COMMON_WEAK_PASSWORDS.contains(&normalized.as_str()) {
            return Err("Password is too weak".to_string());
        }

        // Reject 3+ consecutive identical characters (e.g. "aaa", "111")
        let chars: Vec<char> = trimmed.chars().collect();
        for window in chars.windows(3) {
            if window[0] == window[1] && window[1] == window[2] {
                return Err("Password must not contain 3 or more repeated characters".to_string());
            }
        }

        // Reject 4+ sequential ascending/descending characters (e.g. "1234", "abcd", "dcba")
        for window in chars.windows(4) {
            let a = window[0] as i32;
            let b = window[1] as i32;
            let c = window[2] as i32;
            let d = window[3] as i32;
            if b - a == 1 && c - b == 1 && d - c == 1 {
                return Err("Password must not contain sequential characters".to_string());
            }
            if a - b == 1 && b - c == 1 && c - d == 1 {
                return Err("Password must not contain sequential characters".to_string());
            }
        }

        Ok(())
    }

    pub(crate) fn sanitize_rdp_field(name: &str, value: &str) -> Result<(), String> {
        if value.contains('\n') || value.contains('\r') {
            return Err(format!("{} contains invalid line breaks", name));
        }
        Ok(())
    }

    fn schedule_rdp_file_cleanup(path: PathBuf) {
        thread::spawn(move || {
            thread::sleep(Duration::from_secs(120));
            let _ = std::fs::remove_file(path);
        });
    }

    fn apply_dialog_filters<R: tauri::Runtime>(
        mut dialog: tauri_plugin_dialog::FileDialogBuilder<R>,
        filters: Vec<LocalFsDialogFilter>,
    ) -> tauri_plugin_dialog::FileDialogBuilder<R> {
        for filter in filters {
            let name = filter.name.trim();
            let extensions: Vec<&str> = filter
                .extensions
                .iter()
                .map(|ext| ext.trim())
                .filter(|ext| !ext.is_empty())
                .collect();
            if name.is_empty() || extensions.is_empty() {
                continue;
            }
            dialog = dialog.add_filter(name.to_string(), &extensions);
        }
        dialog
    }

    fn to_grant_response(
        local_fs_state: &State<'_, crate::modules::local_fs::LocalFsState>,
        path: PathBuf,
        mode: &str,
        size: u64,
    ) -> Result<LocalFsDialogGrant, String> {
        let access_token =
            crate::modules::local_fs::issue_dialog_path_grant(local_fs_state.inner(), &path, mode)?;
        Ok(LocalFsDialogGrant {
            path: path.to_string_lossy().to_string(),
            access_token,
            size,
        })
    }

    #[command]
    pub fn local_fs_pick_file_for_read(
        app: AppHandle,
        db: State<Arc<Mutex<DatabaseManager>>>,
        app_lock_state: State<Arc<Mutex<AppLockRuntimeState>>>,
        local_fs_state: State<crate::modules::local_fs::LocalFsState>,
        filters: Option<Vec<LocalFsDialogFilter>>,
    ) -> Result<Option<LocalFsDialogGrant>, String> {
        ensure_app_unlocked(&db, &app_lock_state)?;
        let dialog = app.dialog().file();
        let dialog = apply_dialog_filters(dialog, filters.unwrap_or_default());
        let Some(file_path) = dialog.blocking_pick_file() else {
            return Ok(None);
        };
        let path = file_path.into_path().map_err(|e| e.to_string())?;
        if !path.is_absolute() {
            return Err(format!(
                "Dialog returned a non-absolute path: {}",
                path.display()
            ));
        }
        let size = std::fs::metadata(&path).map_err(|e| e.to_string())?.len();
        to_grant_response(&local_fs_state, path, "read", size).map(Some)
    }

    #[command]
    pub fn local_fs_pick_file_for_write(
        app: AppHandle,
        db: State<Arc<Mutex<DatabaseManager>>>,
        app_lock_state: State<Arc<Mutex<AppLockRuntimeState>>>,
        local_fs_state: State<crate::modules::local_fs::LocalFsState>,
        default_file_name: Option<String>,
        filters: Option<Vec<LocalFsDialogFilter>>,
    ) -> Result<Option<LocalFsDialogGrant>, String> {
        ensure_app_unlocked(&db, &app_lock_state)?;
        let mut dialog = app.dialog().file();
        if let Some(name) = default_file_name {
            let trimmed = name.trim();
            if !trimmed.is_empty() {
                dialog = dialog.set_file_name(trimmed.to_string());
            }
        }
        dialog = apply_dialog_filters(dialog, filters.unwrap_or_default());
        let Some(file_path) = dialog.blocking_save_file() else {
            return Ok(None);
        };
        let path = file_path.into_path().map_err(|e| e.to_string())?;
        if !path.is_absolute() {
            return Err(format!(
                "Dialog returned a non-absolute path: {}",
                path.display()
            ));
        }
        let size = std::fs::metadata(&path).map(|meta| meta.len()).unwrap_or(0);
        to_grant_response(&local_fs_state, path, "write", size).map(Some)
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

        validate_app_lock_password_strength(&password)?;
        let hash = bcrypt::hash(password, APP_LOCK_BCRYPT_COST).map_err(|e| e.to_string())?;
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

        validate_app_lock_password_strength(&new_password)?;
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
            drop(db);
            enforce_app_lock_verify_throttle()?;
            let ok = bcrypt::verify(password, &hash).map_err(|e| e.to_string())?;
            update_app_lock_verify_throttle(ok);
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
    pub fn lock_app(
        db: State<Arc<Mutex<DatabaseManager>>>,
        app_lock_state: State<Arc<Mutex<AppLockRuntimeState>>>,
    ) -> Result<(), String> {
        let db = db.lock().map_err(|e| e.to_string())?;
        let lock_enabled = db
            .get_setting("app_lock_hash")
            .map_err(|e| e.to_string())?
            .is_some();
        drop(db);

        if !lock_enabled {
            return Ok(());
        }

        set_unlock_state(&app_lock_state, false)
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
        host_key_challenge_state: State<Arc<Mutex<HostKeyChallengeRuntimeState>>>,
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
                return Err(enrich_host_key_error_with_challenge(
                    error.to_string(),
                    host_key_challenge_state.inner(),
                ));
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
        let temp_dir = std::env::temp_dir().join("starshuttle-rdp");
        std::fs::create_dir_all(&temp_dir).map_err(|e| {
            format!("Failed to create RDP temp directory: {}", e)
        })?;

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
        sanitize_rdp_field("Host", host)?;
        sanitize_rdp_field("Username", username)?;

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
        host_key_challenge_state: State<Arc<Mutex<HostKeyChallengeRuntimeState>>>,
        challenge_token: String,
        host: String,
        port: u16,
        key_type: String,
        key_base64: String,
        replace: Option<bool>,
    ) -> Result<(), String> {
        ensure_app_unlocked(&db, &app_lock_state)?;
        let replace = replace.unwrap_or(false);
        {
            let mut challenge_state = host_key_challenge_state.lock().map_err(|e| e.to_string())?;
            challenge_state.consume(
                &challenge_token,
                &host,
                port,
                &key_type,
                &key_base64,
                replace,
            )?;
        }
        let mut manager = KnownHostsManager::new().map_err(|e| e.to_string())?;
        manager
            .upsert_host_key_parts(&host, port, key_type, key_base64, replace)
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
        let prepared_close = {
            let mut manager = manager
                .write()
                .map_err(|e| format!("Failed to acquire write lock: {}", e))?;
            manager
                .prepare_disconnect(&session_id)
                .map_err(|e| e.to_string())?;
            manager.prepare_close_terminal(&session_id).ok()
        };

        if let Some(prepared) = prepared_close.as_ref() {
            if let Err(err) = DefaultConnectionManager::execute_prepared_terminal_close(prepared) {
                log::warn!(
                    "Failed to send close command while disconnecting session {}: {}",
                    session_id,
                    err
                );
            }
        }

        {
            let mut manager = manager
                .write()
                .map_err(|e| format!("Failed to acquire write lock: {}", e))?;
            if let Some(prepared) = prepared_close.as_ref() {
                manager.finish_close_terminal(prepared);
            }
            manager.finish_disconnect(&session_id);
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
        let has_active_session = manager.get_all_sessions().into_iter().any(|session| {
            session.connection_id == connection_id
                && session.status != ConnectionStatus::Disconnected
        });
        if has_active_session {
            return Err(
                "Cannot delete connection config while sessions are still active".to_string(),
            );
        }
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
        host_key_challenge_state: State<Arc<Mutex<HostKeyChallengeRuntimeState>>>,
        manager: State<Arc<RwLock<DefaultConnectionManager>>>,
        config: ConnectionConfig,
    ) -> Result<(), String> {
        ensure_app_unlocked(&db, &app_lock_state)?;
        let manager = manager
            .read()
            .map_err(|e| format!("Failed to acquire read lock: {}", e))?;
        manager.test_connection(&app, &config).map_err(|e| {
            enrich_host_key_error_with_challenge(e.to_string(), host_key_challenge_state.inner())
        })
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
        let prepared = {
            let manager = manager
                .write()
                .map_err(|e| format!("Failed to acquire write lock: {}", e))?;
            manager.prepare_close_terminal(&session_id)
        }
        .map_err(|e| e.to_string())?;

        DefaultConnectionManager::execute_prepared_terminal_close(&prepared)
            .map_err(|e| e.to_string())?;

        let mut manager = manager
            .write()
            .map_err(|e| format!("Failed to acquire write lock: {}", e))?;
        manager.finish_close_terminal(&prepared);
        Ok(())
    }

    #[command]
    pub fn exec_audited_command(
        db: State<Arc<Mutex<DatabaseManager>>>,
        app_lock_state: State<Arc<Mutex<AppLockRuntimeState>>>,
        manager: State<Arc<RwLock<DefaultConnectionManager>>>,
        session_id: Uuid,
        command: String,
        audit_event: crate::modules::db::AuditEvent,
        execute: bool,
    ) -> Result<String, String> {
        ensure_app_unlocked(&db, &app_lock_state)?;
        let mut audit_event = audit_event;
        let action = audit_event.action.clone();
        if !matches!(action.as_str(), "ALLOWED" | "WARNED" | "BLOCKED") {
            return Err(format!("Unsupported audit action: {}", action));
        }
        if execute && action == "BLOCKED" {
            return Err("Blocked audit action cannot execute a command".to_string());
        }

        audit_event.session_id = Some(session_id);
        {
            let db = db.lock().map_err(|e| e.to_string())?;
            db.save_audit_event(&audit_event).map_err(|e| e.to_string())?;
        }

        if !execute {
            return Ok(String::new());
        }

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
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    if cfg!(debug_assertions) {
        crate::modules::logging::LogManager::init(log::LevelFilter::Info)
            .expect("Failed to initialize logger");
    } else {
        log::set_max_level(log::LevelFilter::Off);
    }

    let connection_manager = Arc::new(RwLock::new(DefaultConnectionManager::new()));
    let connection_manager_for_setup = connection_manager.clone();
    let sftp_manager = crate::modules::sftp::SftpManager::new(connection_manager.clone());
    let local_fs_state = crate::modules::local_fs::LocalFsState::default();
    let host_key_challenge_state = Arc::new(Mutex::new(HostKeyChallengeRuntimeState::default()));

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
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

            // Initialize AI ChatManager
            let chat_manager = Arc::new(crate::modules::ai::chat::ChatManager::new(db.clone()));
            app.manage(chat_manager);

            // Initialize AI AgentManager
            let agent_manager = Arc::new(crate::modules::ai::agent::AgentManager::new(
                db.clone(),
                connection_manager_for_setup.clone(),
            ));
            app.manage(agent_manager);

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
        .manage(local_fs_state)
        .manage(host_key_challenge_state)
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
            commands::exec_audited_command,
            // App Lock commands
            commands::set_app_lock,
            commands::change_app_lock,
            commands::verify_app_lock,
            commands::is_app_lock_enabled,
            commands::lock_app,
            commands::remove_app_lock,
            commands::local_fs_pick_file_for_read,
            commands::local_fs_pick_file_for_write,
            // Local filesystem commands
            crate::modules::local_fs::local_fs_open_read,
            crate::modules::local_fs::local_fs_stat,
            crate::modules::local_fs::local_fs_read_chunk,
            crate::modules::local_fs::local_fs_open_write,
            crate::modules::local_fs::local_fs_write_chunk,
            crate::modules::local_fs::local_fs_seek,
            crate::modules::local_fs::local_fs_close,
            crate::modules::local_fs::local_fs_read_text,
            crate::modules::local_fs::local_fs_write_text,
            // SFTP commands
            crate::modules::sftp::tauri_commands::sftp_ls,
            crate::modules::sftp::tauri_commands::sftp_read,
            crate::modules::sftp::tauri_commands::sftp_read_chunk,
            crate::modules::sftp::tauri_commands::sftp_write,
            crate::modules::sftp::tauri_commands::sftp_mkdir,
            crate::modules::sftp::tauri_commands::sftp_rm,
            crate::modules::sftp::tauri_commands::sftp_rmdir,
            crate::modules::sftp::tauri_commands::sftp_rename,
            crate::modules::sftp::tauri_commands::scp_upload,
            crate::modules::sftp::tauri_commands::scp_download,
            // Command snippet commands
            commands::save_command_snippet,
            commands::get_command_snippets,
            commands::get_command_snippet_by_id,
            commands::delete_command_snippet,
            commands::increment_command_snippet_usage,
            // Audit logging commands
            commands::log_audit_event,
            // AI commands
            crate::modules::ai::ai_get_config,
            crate::modules::ai::ai_save_config,
            crate::modules::ai::ai_get_provider_defaults,
            crate::modules::ai::ai_test_connection,
            crate::modules::ai::ai_chat_new,
            crate::modules::ai::ai_chat_list,
            crate::modules::ai::ai_chat_messages,
            crate::modules::ai::ai_chat_send,
            crate::modules::ai::ai_chat_clear,
            crate::modules::ai::ai_chat_delete,
            crate::modules::ai::ai_get_terminal_context,
            // Agent commands
            crate::modules::ai::ai_agent_start,
            crate::modules::ai::ai_agent_confirm,
            crate::modules::ai::ai_agent_cancel,
            crate::modules::ai::ai_agent_status,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

// Modules
pub mod models;
pub mod modules;
pub mod utils;

#[cfg(test)]
mod tests {
    use super::{
        commands::{sanitize_rdp_field, validate_app_lock_password_strength},
        enrich_host_key_error_with_challenge, parse_host_key_error_payload,
        HostKeyChallengeRuntimeState,
    };
    use std::sync::{Arc, Mutex};

    #[test]
    fn rejects_rdp_fields_with_line_breaks() {
        assert!(sanitize_rdp_field("Host", "rdp-host").is_ok());
        assert!(sanitize_rdp_field("Host", "bad\nhost").is_err());
        assert!(sanitize_rdp_field("Username", "bad\ruser").is_err());
    }

    #[test]
    fn app_lock_password_strength_validation_rejects_weak_passwords() {
        assert!(validate_app_lock_password_strength("").is_err());
        assert!(validate_app_lock_password_strength("12345678").is_err());
        assert!(validate_app_lock_password_strength("password123").is_err());
        assert!(validate_app_lock_password_strength("abcdefgh").is_err());
    }

    #[test]
    fn app_lock_password_strength_validation_accepts_reasonable_password() {
        assert!(validate_app_lock_password_strength("A1secureLock").is_ok());
    }

    #[test]
    fn host_key_error_enrichment_issues_one_time_token() {
        let state = Arc::new(Mutex::new(HostKeyChallengeRuntimeState::default()));
        let raw = "HOST_KEY_UNKNOWN|{\"host\":\"example.com\",\"port\":22,\"fingerprint\":\"fp\",\"key_type\":\"ssh-ed25519\",\"key_base64\":\"AAAA\"}".to_string();
        let enriched = enrich_host_key_error_with_challenge(raw, &state);

        let (_, payload) = parse_host_key_error_payload(&enriched).expect("payload should parse");
        let token = payload
            .challenge_token
            .clone()
            .expect("challenge token should exist");

        let mut guard = state.lock().expect("state lock should work");
        assert!(guard
            .consume(&token, "example.com", 22, "ssh-ed25519", "AAAA", false)
            .is_ok());
        assert!(guard
            .consume(&token, "example.com", 22, "ssh-ed25519", "AAAA", false)
            .is_err());
    }

    #[test]
    fn host_key_challenge_rejects_payload_tampering() {
        let state = Arc::new(Mutex::new(HostKeyChallengeRuntimeState::default()));
        let raw = "HOST_KEY_UNKNOWN|{\"host\":\"example.com\",\"port\":22,\"fingerprint\":\"fp\",\"key_type\":\"ssh-ed25519\",\"key_base64\":\"AAAA\"}".to_string();
        let enriched = enrich_host_key_error_with_challenge(raw, &state);

        let (_, payload) = parse_host_key_error_payload(&enriched).expect("payload should parse");
        let token = payload
            .challenge_token
            .expect("challenge token should exist");

        let mut guard = state.lock().expect("state lock should work");
        let result = guard.consume(&token, "example.com", 22, "ssh-ed25519", "BBBB", false);
        assert!(result.is_err());
    }

    #[test]
    fn host_key_mismatch_challenge_requires_replace_true() {
        let state = Arc::new(Mutex::new(HostKeyChallengeRuntimeState::default()));
        let raw = "HOST_KEY_MISMATCH|{\"host\":\"example.com\",\"port\":22,\"fingerprint\":\"fp\",\"key_type\":\"ssh-ed25519\",\"key_base64\":\"AAAA\",\"reason\":\"changed\"}".to_string();
        let enriched = enrich_host_key_error_with_challenge(raw, &state);

        let (_, payload) = parse_host_key_error_payload(&enriched).expect("payload should parse");
        let token = payload
            .challenge_token
            .expect("challenge token should exist");

        let mut guard = state.lock().expect("state lock should work");
        let result = guard.consume(&token, "example.com", 22, "ssh-ed25519", "AAAA", false);
        assert!(result.is_err());
    }

    #[test]
    fn host_key_unavailable_enrichment_parses_and_issues_token() {
        let state = Arc::new(Mutex::new(HostKeyChallengeRuntimeState::default()));
        let raw = "HOST_KEY_UNAVAILABLE|{\"host\":\"example.com\",\"port\":22,\"fingerprint\":\"fp\",\"key_type\":\"ssh-ed25519\",\"key_base64\":\"AAAA\"}".to_string();
        let enriched = enrich_host_key_error_with_challenge(raw, &state);

        let (_, payload) = parse_host_key_error_payload(&enriched).expect("payload should parse");
        let token = payload
            .challenge_token
            .expect("challenge token should exist");

        let mut guard = state.lock().expect("state lock should work");
        assert!(guard
            .consume(&token, "example.com", 22, "ssh-ed25519", "AAAA", false)
            .is_ok());
    }
}
