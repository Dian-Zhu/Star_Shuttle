use chrono::{DateTime, Utc};
use log::{debug, error, info, warn};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use thiserror::Error;
use uuid::Uuid;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

// Re-export submodules
pub mod auth;
pub mod error;
pub mod known_hosts;
pub mod ssh_impl;
pub mod tracking;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub enum ConnectionProtocol {
    #[default]
    Ssh,
    Rdp,
    Telnet,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ConnectionStatus {
    Disconnected,
    Connecting,
    Connected,
    Disconnecting,
    Error,
}

// Authentication methods
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AuthMethod {
    Password {
        password: String,
        save_password: bool,
    },
    KeyboardInteractive {},
    PrivateKey {
        key_path: String,
        passphrase: Option<String>,
        save_passphrase: bool,
    },
    Agent {
        agent_path: Option<String>,
    },
    Certificate {
        certificate_path: String,
        private_key_path: String,
        passphrase: Option<String>,
        save_passphrase: bool,
    },
}

// Proxy types for jump host support
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub enum ProxyType {
    #[default]
    None,
    Socks5 {
        host: String,
        port: u16,
        username: Option<String>,
        password: Option<String>,
    },
    Http {
        host: String,
        port: u16,
        username: Option<String>,
        password: Option<String>,
    },
    JumpHost {
        host: String,
        port: u16,
        username: String,
        auth_method: AuthMethod,
    },
}

// Connection configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionConfig {
    pub id: Uuid,
    pub name: String,
    #[serde(default)]
    pub protocol: ConnectionProtocol,
    pub host: String,
    pub port: u16,
    pub username: String,
    pub auth_method: AuthMethod,
    pub description: Option<String>,
    pub tags: Vec<String>,
    #[serde(default = "chrono::Utc::now")]
    pub created_at: DateTime<Utc>,
    #[serde(default = "chrono::Utc::now")]
    pub updated_at: DateTime<Utc>,
    pub group_id: Option<Uuid>,
    pub local_forwards: Vec<LocalForward>,
    pub remote_forwards: Vec<RemoteForward>,
    #[serde(default)]
    pub proxy_type: ProxyType,
    #[serde(default)]
    pub socks_proxy_port: Option<u16>, // For SSH dynamic port forwarding (-D)
    #[serde(default)]
    pub auto_reconnect: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocalForward {
    pub local_host: String,
    pub local_port: u16,
    pub remote_host: String,
    pub remote_port: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemoteForward {
    pub remote_host: String,
    pub remote_port: u16,
    pub local_host: String,
    pub local_port: u16,
}

impl Default for ConnectionConfig {
    fn default() -> Self {
        Self {
            id: Uuid::new_v4(),
            name: "Default Connection".to_string(),
            protocol: ConnectionProtocol::Ssh,
            host: String::new(), // 改为空字符串，避免默认连接localhost
            port: 0,             // 改为0，避免默认使用22端口
            username: "".to_string(),
            auth_method: AuthMethod::Password {
                password: "".to_string(),
                save_password: false,
            },
            description: None,
            tags: Vec::new(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
            group_id: None,
            local_forwards: Vec::new(),
            remote_forwards: Vec::new(),
            proxy_type: ProxyType::None,
            socks_proxy_port: None,
            auto_reconnect: None,
        }
    }
}

impl ConnectionConfig {
    pub fn validate(&self) -> Result<(), ConnectionError> {
        if self.protocol == ConnectionProtocol::Rdp || self.protocol == ConnectionProtocol::Telnet {
            if self.host.is_empty() {
                return Err(ConnectionError::InvalidConfig(
                    "Host is required".to_string(),
                ));
            }
            if self.port == 0 {
                return Err(ConnectionError::InvalidConfig(
                    "Port is required".to_string(),
                ));
            }
            return Ok(());
        }

        if self.host.is_empty() {
            return Err(ConnectionError::InvalidConfig(
                "Host is required".to_string(),
            ));
        }

        if self.username.is_empty() {
            return Err(ConnectionError::InvalidConfig(
                "Username is required".to_string(),
            ));
        }

        if self.port == 0 {
            return Err(ConnectionError::InvalidConfig(
                "Port is required".to_string(),
            ));
        }

        match &self.auth_method {
            AuthMethod::Password { password, .. } => {
                if password.is_empty() {
                    return Err(ConnectionError::InvalidConfig(
                        "Password is required".to_string(),
                    ));
                }
            }
            AuthMethod::KeyboardInteractive {} => {}
            AuthMethod::PrivateKey { key_path, .. } => {
                if key_path.is_empty() {
                    return Err(ConnectionError::InvalidConfig(
                        "Private key path is required".to_string(),
                    ));
                }
            }
            AuthMethod::Agent { .. } => {}
            AuthMethod::Certificate {
                certificate_path,
                private_key_path,
                ..
            } => {
                if certificate_path.is_empty() {
                    return Err(ConnectionError::InvalidConfig(
                        "Certificate path is required".to_string(),
                    ));
                }

                if private_key_path.is_empty() {
                    return Err(ConnectionError::InvalidConfig(
                        "Private key path is required for certificate authentication".to_string(),
                    ));
                }
            }
        }

        Ok(())
    }

    pub fn validate_for_save(&self) -> Result<(), ConnectionError> {
        if self.protocol == ConnectionProtocol::Rdp || self.protocol == ConnectionProtocol::Telnet {
            if self.host.is_empty() {
                return Err(ConnectionError::InvalidConfig(
                    "Host is required".to_string(),
                ));
            }
            if self.port == 0 {
                return Err(ConnectionError::InvalidConfig(
                    "Port is required".to_string(),
                ));
            }
            return Ok(());
        }

        if self.host.is_empty() {
            return Err(ConnectionError::InvalidConfig(
                "Host is required".to_string(),
            ));
        }

        if self.username.is_empty() {
            return Err(ConnectionError::InvalidConfig(
                "Username is required".to_string(),
            ));
        }

        if self.port == 0 {
            return Err(ConnectionError::InvalidConfig(
                "Port is required".to_string(),
            ));
        }

        match &self.auth_method {
            AuthMethod::Password {
                password,
                save_password,
            } => {
                if *save_password && password.is_empty() {
                    return Err(ConnectionError::InvalidConfig(
                        "Password is required when save_password is enabled".to_string(),
                    ));
                }
            }
            AuthMethod::KeyboardInteractive {} => {}
            AuthMethod::PrivateKey { key_path, .. } => {
                if key_path.is_empty() {
                    return Err(ConnectionError::InvalidConfig(
                        "Private key path is required".to_string(),
                    ));
                }
            }
            AuthMethod::Agent { .. } => {}
            AuthMethod::Certificate {
                certificate_path,
                private_key_path,
                ..
            } => {
                if certificate_path.is_empty() {
                    return Err(ConnectionError::InvalidConfig(
                        "Certificate path is required".to_string(),
                    ));
                }

                if private_key_path.is_empty() {
                    return Err(ConnectionError::InvalidConfig(
                        "Private key path is required for certificate authentication".to_string(),
                    ));
                }
            }
        }

        if let ProxyType::JumpHost {
            auth_method:
                AuthMethod::Password {
                    password,
                    save_password,
                },
            ..
        } = &self.proxy_type
        {
            if *save_password && password.is_empty() {
                return Err(ConnectionError::InvalidConfig(
                    "Jump host password is required when save_password is enabled".to_string(),
                ));
            }
        }

        Ok(())
    }
}

// Session information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionInfo {
    pub id: Uuid,
    pub connection_id: Uuid,
    pub status: ConnectionStatus,
    pub terminal_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub last_active: DateTime<Utc>,
}

// Connection manager trait
pub trait ConnectionManager {
    fn connect(
        &mut self,
        app: &tauri::AppHandle,
        config: &ConnectionConfig,
    ) -> Result<Uuid, ConnectionError>;
    fn disconnect(&mut self, session_id: &Uuid) -> Result<(), ConnectionError>;
    fn get_session(&self, session_id: &Uuid) -> Option<&SessionInfo>;
    fn get_all_sessions(&self) -> Vec<SessionInfo>;
    fn get_connection_config(&self, connection_id: &Uuid) -> Option<&ConnectionConfig>;
    fn save_connection_config(&mut self, config: ConnectionConfig) -> Result<(), ConnectionError>;
    fn delete_connection_config(&mut self, connection_id: &Uuid) -> Result<(), ConnectionError>;
    fn get_all_connection_configs(&self) -> Vec<ConnectionConfig>;
    fn test_connection(
        &self,
        app: &tauri::AppHandle,
        config: &ConnectionConfig,
    ) -> Result<(), ConnectionError>;

    // Terminal methods
    fn start_terminal(
        &mut self,
        app: &tauri::AppHandle,
        session_id: &Uuid,
        width: u16,
        height: u16,
    ) -> Result<bool, ConnectionError>;
    fn send_terminal_data(&mut self, session_id: &Uuid, data: &str) -> Result<(), ConnectionError>;
    fn resize_terminal(
        &mut self,
        session_id: &Uuid,
        width: u16,
        height: u16,
    ) -> Result<(), ConnectionError>;
    fn close_terminal(&mut self, session_id: &Uuid) -> Result<(), ConnectionError>;
    fn get_terminal_sender(&self, session_id: &Uuid) -> Option<mpsc::Sender<TerminalCommand>>;
    fn log_terminal_data(&self, session_id: &Uuid, data: &[u8], direction: &str);

    // Command execution
    fn exec_command(&self, session_id: &Uuid, command: &str)
        -> Result<String, ConnectionError>;
}

// Connection errors
#[derive(Error, Debug)]
pub enum ConnectionError {
    #[error("Invalid connection configuration: {0}")]
    InvalidConfig(String),
    #[error("Authentication failed: {0}")]
    AuthenticationError(String),
    #[error("Connection failed: {0}")]
    ConnectionFailed(String),
    #[error("Session not found: {0}")]
    SessionNotFound(Uuid),
    #[error("Connection not found: {0}")]
    ConnectionNotFound(Uuid),
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
    #[error("SSH error: {0}")]
    SshError(String),
    #[error("Database error: {0}")]
    DatabaseError(String),
    #[error("Credential error: {0}")]
    CredentialError(String),
    #[error("Other error: {0}")]
    Other(String),
}

use crate::modules::connection::ssh_impl::SshConnection;
use crate::modules::connection::tracking::ChannelTracker;
use crate::modules::credential::CredentialManager;
use crate::modules::db::DatabaseManager;
use async_trait::async_trait;
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter};
use tokio::runtime::Runtime;
use tokio::sync::{mpsc, oneshot};

pub const SSH_KEYBOARD_INTERACTIVE_EVENT: &str = "ssh-keyboard-interactive-request";

type KeyboardInteractivePending =
    Arc<Mutex<HashMap<String, oneshot::Sender<Result<Vec<String>, String>>>>>;

#[derive(Clone)]
pub struct KeyboardInteractiveCoordinator {
    pending: KeyboardInteractivePending,
}

impl Default for KeyboardInteractiveCoordinator {
    fn default() -> Self {
        Self::new()
    }
}

impl KeyboardInteractiveCoordinator {
    pub fn new() -> Self {
        Self {
            pending: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn respond(&self, request_id: String, responses: Vec<String>) -> Result<(), String> {
        let tx = self
            .pending
            .lock()
            .map_err(|e| e.to_string())?
            .remove(&request_id)
            .ok_or_else(|| "unknown request_id".to_string())?;
        let _ = tx.send(Ok(responses));
        Ok(())
    }

    pub fn cancel(&self, request_id: String) -> Result<(), String> {
        let tx = self
            .pending
            .lock()
            .map_err(|e| e.to_string())?
            .remove(&request_id)
            .ok_or_else(|| "unknown request_id".to_string())?;
        let _ = tx.send(Err("canceled".to_string()));
        Ok(())
    }

    pub async fn request(
        &self,
        app: &AppHandle,
        request: ssh_impl::KeyboardInteractivePromptRequest,
    ) -> Result<Vec<String>, anyhow::Error> {
        let request_id = Uuid::new_v4().to_string();
        let (tx, rx) = oneshot::channel::<Result<Vec<String>, String>>();
        {
            let mut guard = self
                .pending
                .lock()
                .map_err(|e| anyhow::anyhow!(e.to_string()))?;
            guard.insert(request_id.clone(), tx);
        }

        app.emit(
            SSH_KEYBOARD_INTERACTIVE_EVENT,
            serde_json::json!({
                "request_id": request_id,
                "host": request.host,
                "port": request.port,
                "username": request.username,
                "name": request.name,
                "instructions": request.instructions,
                "prompts": request.prompts.iter().map(|p| serde_json::json!({
                    "prompt": p.prompt,
                    "echo": p.echo
                })).collect::<Vec<_>>()
            }),
        )?;

        let res = tokio::time::timeout(std::time::Duration::from_secs(300), rx).await;
        match res {
            Ok(Ok(Ok(v))) => Ok(v),
            Ok(Ok(Err(e))) => Err(anyhow::anyhow!(e)),
            Ok(Err(_)) => Err(anyhow::anyhow!(
                "keyboard-interactive response channel closed"
            )),
            Err(_) => {
                let _ = self
                    .pending
                    .lock()
                    .map_err(|e| anyhow::anyhow!(e.to_string()))?
                    .remove(&request_id);
                Err(anyhow::anyhow!("keyboard-interactive prompt timeout"))
            }
        }
    }
}

#[derive(Clone)]
struct TauriKeyboardInteractivePrompter {
    app: AppHandle,
    coordinator: KeyboardInteractiveCoordinator,
}

#[async_trait]
impl ssh_impl::KeyboardInteractivePrompter for TauriKeyboardInteractivePrompter {
    async fn prompt(
        &self,
        request: ssh_impl::KeyboardInteractivePromptRequest,
    ) -> Result<Vec<String>, anyhow::Error> {
        self.coordinator.request(&self.app, request).await
    }
}

// Terminal session data
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

struct TelnetConnection {
    read: tokio::net::tcp::OwnedReadHalf,
    write: tokio::net::tcp::OwnedWriteHalf,
}

// Default connection manager implementation
pub struct DefaultConnectionManager {
    connections: HashMap<Uuid, ConnectionConfig>,
    sessions: HashMap<Uuid, SessionInfo>,
    ssh_connections: HashMap<Uuid, SshConnection>,
    jump_ssh_connections: HashMap<Uuid, SshConnection>,
    telnet_connections: HashMap<Uuid, TelnetConnection>,
    terminals: HashMap<Uuid, TerminalSession>,
    tracker: Arc<Mutex<ChannelTracker>>,
    runtime: Arc<Runtime>,
    db: Option<Arc<Mutex<DatabaseManager>>>,
    credential_manager: CredentialManager,
    keyboard_interactive: KeyboardInteractiveCoordinator,
}

// Manual Debug implementation to handle the non-Debug ssh_connections and runtime fields
impl std::fmt::Debug for DefaultConnectionManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DefaultConnectionManager")
            .field("connections", &self.connections)
            .field("sessions", &self.sessions)
            .field("ssh_connections_count", &self.ssh_connections.len())
            .field("telnet_connections_count", &self.telnet_connections.len())
            .field("terminals_count", &self.terminals.len())
            .finish()
    }
}

impl Default for DefaultConnectionManager {
    fn default() -> Self {
        Self::new()
    }
}

impl DefaultConnectionManager {
    pub fn new() -> Self {
        let runtime = Runtime::new().expect("Failed to create Tokio runtime");
        Self {
            connections: HashMap::new(),
            sessions: HashMap::new(),
            ssh_connections: HashMap::new(),
            jump_ssh_connections: HashMap::new(),
            telnet_connections: HashMap::new(),
            terminals: HashMap::new(),
            tracker: Arc::new(Mutex::new(ChannelTracker::new())),
            runtime: Arc::new(runtime),
            db: None,
            credential_manager: CredentialManager::new(),
            keyboard_interactive: KeyboardInteractiveCoordinator::new(),
        }
    }

    pub fn keyboard_interactive_coordinator(&self) -> KeyboardInteractiveCoordinator {
        self.keyboard_interactive.clone()
    }

    pub fn set_db(&mut self, db: Arc<Mutex<DatabaseManager>>) -> Result<(), ConnectionError> {
        self.db = Some(db);
        if let Some(db) = self.db.as_ref() {
            self.credential_manager.set_db(db.clone());
        }
        self.load_connection_configs_from_db()?;
        Ok(())
    }

    fn load_connection_configs_from_db(&mut self) -> Result<(), ConnectionError> {
        let Some(db) = self.db.as_ref() else {
            return Ok(());
        };

        let db = db
            .lock()
            .map_err(|e| ConnectionError::DatabaseError(e.to_string()))?;
        let raw = db
            .get_setting("connection_configs")
            .map_err(|e| ConnectionError::DatabaseError(e.to_string()))?;

        self.connections.clear();

        let Some(raw) = raw else {
            return Ok(());
        };

        let configs: Vec<ConnectionConfig> = serde_json::from_str(&raw)
            .map_err(|e| ConnectionError::DatabaseError(e.to_string()))?;

        for config in configs {
            self.connections.insert(config.id, config);
        }

        Ok(())
    }

    fn persist_connection_configs_to_db(&self) -> Result<(), ConnectionError> {
        let Some(db) = self.db.as_ref() else {
            return Ok(());
        };

        let mut configs: Vec<ConnectionConfig> = self.connections.values().cloned().collect();
        configs.sort_by_key(|c| c.updated_at);

        let raw = serde_json::to_string(&configs)
            .map_err(|e| ConnectionError::DatabaseError(e.to_string()))?;

        let db = db
            .lock()
            .map_err(|e| ConnectionError::DatabaseError(e.to_string()))?;
        db.save_setting("connection_configs", &raw)
            .map_err(|e| ConnectionError::DatabaseError(e.to_string()))?;

        Ok(())
    }

    fn sanitize_auth_method(auth_method: &AuthMethod) -> AuthMethod {
        match auth_method {
            AuthMethod::Password { save_password, .. } => AuthMethod::Password {
                password: String::new(),
                save_password: *save_password,
            },
            AuthMethod::KeyboardInteractive {} => AuthMethod::KeyboardInteractive {},
            AuthMethod::PrivateKey {
                key_path,
                save_passphrase,
                ..
            } => AuthMethod::PrivateKey {
                key_path: key_path.clone(),
                passphrase: None,
                save_passphrase: *save_passphrase,
            },
            AuthMethod::Agent { agent_path } => AuthMethod::Agent {
                agent_path: agent_path.clone(),
            },
            AuthMethod::Certificate {
                certificate_path,
                private_key_path,
                save_passphrase,
                ..
            } => AuthMethod::Certificate {
                certificate_path: certificate_path.clone(),
                private_key_path: private_key_path.clone(),
                passphrase: None,
                save_passphrase: *save_passphrase,
            },
        }
    }

    fn sanitize_proxy_type(proxy_type: &ProxyType) -> ProxyType {
        match proxy_type {
            ProxyType::None => ProxyType::None,
            ProxyType::Socks5 {
                host,
                port,
                username,
                password,
            } => ProxyType::Socks5 {
                host: host.clone(),
                port: *port,
                username: username.clone(),
                password: password.clone(),
            },
            ProxyType::Http {
                host,
                port,
                username,
                password,
            } => ProxyType::Http {
                host: host.clone(),
                port: *port,
                username: username.clone(),
                password: password.clone(),
            },
            ProxyType::JumpHost {
                host,
                port,
                username,
                auth_method,
            } => ProxyType::JumpHost {
                host: host.clone(),
                port: *port,
                username: username.clone(),
                auth_method: Self::sanitize_auth_method(auth_method),
            },
        }
    }

    fn sanitize_config_for_storage(config: &ConnectionConfig) -> ConnectionConfig {
        let mut out = config.clone();
        out.auth_method = Self::sanitize_auth_method(&out.auth_method);
        out.proxy_type = Self::sanitize_proxy_type(&out.proxy_type);
        out
    }

    fn fill_saved_credentials(&self, config: &mut ConnectionConfig) -> Result<(), ConnectionError> {
        if config.protocol != ConnectionProtocol::Ssh {
            return Ok(());
        }

        match &mut config.auth_method {
            AuthMethod::Password {
                password,
                save_password,
            } => {
                if *save_password && password.is_empty() {
                    match self.credential_manager.get_password(&config.id) {
                        Ok(Some(v)) => *password = v,
                        Ok(None) => {}
                        Err(e) => return Err(ConnectionError::CredentialError(e.to_string())),
                    }
                }
            }
            AuthMethod::KeyboardInteractive {} => {}
            AuthMethod::PrivateKey {
                passphrase,
                save_passphrase,
                ..
            } => {
                if *save_passphrase && passphrase.as_deref().unwrap_or_default().is_empty() {
                    match self.credential_manager.get_passphrase(&config.id) {
                        Ok(Some(v)) => *passphrase = Some(v),
                        Ok(None) => {}
                        Err(e) => return Err(ConnectionError::CredentialError(e.to_string())),
                    }
                }
            }
            AuthMethod::Agent { .. } => {}
            AuthMethod::Certificate {
                passphrase,
                save_passphrase,
                ..
            } => {
                if *save_passphrase && passphrase.as_deref().unwrap_or_default().is_empty() {
                    match self.credential_manager.get_passphrase(&config.id) {
                        Ok(Some(v)) => *passphrase = Some(v),
                        Ok(None) => {}
                        Err(e) => return Err(ConnectionError::CredentialError(e.to_string())),
                    }
                }
            }
        }

        if let ProxyType::JumpHost { auth_method, .. } = &mut config.proxy_type {
            match auth_method {
                AuthMethod::Password {
                    password,
                    save_password,
                } => {
                    if *save_password && password.is_empty() {
                        match self
                            .credential_manager
                            .get_password_kind(&config.id, "jump_password")
                        {
                            Ok(Some(v)) => *password = v,
                            Ok(None) => {}
                            Err(e) => return Err(ConnectionError::CredentialError(e.to_string())),
                        }
                    }
                }
                AuthMethod::KeyboardInteractive {} => {}
                AuthMethod::PrivateKey {
                    passphrase,
                    save_passphrase,
                    ..
                } => {
                    if *save_passphrase && passphrase.as_deref().unwrap_or_default().is_empty() {
                        match self
                            .credential_manager
                            .get_password_kind(&config.id, "jump_passphrase")
                        {
                            Ok(Some(v)) => *passphrase = Some(v),
                            Ok(None) => {}
                            Err(e) => return Err(ConnectionError::CredentialError(e.to_string())),
                        }
                    }
                }
                AuthMethod::Agent { .. } => {}
                AuthMethod::Certificate {
                    passphrase,
                    save_passphrase,
                    ..
                } => {
                    if *save_passphrase && passphrase.as_deref().unwrap_or_default().is_empty() {
                        match self
                            .credential_manager
                            .get_password_kind(&config.id, "jump_passphrase")
                        {
                            Ok(Some(v)) => *passphrase = Some(v),
                            Ok(None) => {}
                            Err(e) => return Err(ConnectionError::CredentialError(e.to_string())),
                        }
                    }
                }
            }
        }
        Ok(())
    }

    fn sync_credentials_for_save(&self, config: &ConnectionConfig) -> Result<(), ConnectionError> {
        if config.protocol != ConnectionProtocol::Ssh {
            return Ok(());
        }

        match &config.auth_method {
            AuthMethod::Password {
                password,
                save_password,
            } => {
                if *save_password {
                    if !password.is_empty() {
                        self.credential_manager
                            .save_password(&config.id, password)
                            .map_err(|e| ConnectionError::CredentialError(e.to_string()))?;
                    }
                } else {
                    self.credential_manager
                        .delete_password(&config.id)
                        .map_err(|e| ConnectionError::CredentialError(e.to_string()))?;
                }

                self.credential_manager
                    .delete_passphrase(&config.id)
                    .map_err(|e| ConnectionError::CredentialError(e.to_string()))?;
            }
            AuthMethod::KeyboardInteractive {} => {
                self.credential_manager
                    .delete_password(&config.id)
                    .map_err(|e| ConnectionError::CredentialError(e.to_string()))?;
                self.credential_manager
                    .delete_passphrase(&config.id)
                    .map_err(|e| ConnectionError::CredentialError(e.to_string()))?;
            }
            AuthMethod::PrivateKey {
                passphrase,
                save_passphrase,
                ..
            } => {
                if *save_passphrase {
                    if let Some(p) = passphrase.as_ref().filter(|p| !p.is_empty()) {
                        self.credential_manager
                            .save_passphrase(&config.id, p)
                            .map_err(|e| ConnectionError::CredentialError(e.to_string()))?;
                    }
                } else {
                    self.credential_manager
                        .delete_passphrase(&config.id)
                        .map_err(|e| ConnectionError::CredentialError(e.to_string()))?;
                }

                self.credential_manager
                    .delete_password(&config.id)
                    .map_err(|e| ConnectionError::CredentialError(e.to_string()))?;
            }
            AuthMethod::Agent { .. } => {
                self.credential_manager
                    .delete_password(&config.id)
                    .map_err(|e| ConnectionError::CredentialError(e.to_string()))?;
                self.credential_manager
                    .delete_passphrase(&config.id)
                    .map_err(|e| ConnectionError::CredentialError(e.to_string()))?;
            }
            AuthMethod::Certificate {
                passphrase,
                save_passphrase,
                ..
            } => {
                if *save_passphrase {
                    if let Some(p) = passphrase.as_ref().filter(|p| !p.is_empty()) {
                        self.credential_manager
                            .save_passphrase(&config.id, p)
                            .map_err(|e| ConnectionError::CredentialError(e.to_string()))?;
                    }
                } else {
                    self.credential_manager
                        .delete_passphrase(&config.id)
                        .map_err(|e| ConnectionError::CredentialError(e.to_string()))?;
                }

                self.credential_manager
                    .delete_password(&config.id)
                    .map_err(|e| ConnectionError::CredentialError(e.to_string()))?;
            }
        }

        if let ProxyType::JumpHost { auth_method, .. } = &config.proxy_type {
            match auth_method {
                AuthMethod::Password {
                    password,
                    save_password,
                } => {
                    if *save_password {
                        if !password.is_empty() {
                            self.credential_manager
                                .save_password_kind(&config.id, "jump_password", password)
                                .map_err(|e| ConnectionError::CredentialError(e.to_string()))?;
                        }
                    } else {
                        self.credential_manager
                            .delete_password_kind(&config.id, "jump_password")
                            .map_err(|e| ConnectionError::CredentialError(e.to_string()))?;
                    }

                    self.credential_manager
                        .delete_password_kind(&config.id, "jump_passphrase")
                        .map_err(|e| ConnectionError::CredentialError(e.to_string()))?;
                }
                AuthMethod::KeyboardInteractive {} => {
                    self.credential_manager
                        .delete_password_kind(&config.id, "jump_password")
                        .map_err(|e| ConnectionError::CredentialError(e.to_string()))?;
                    self.credential_manager
                        .delete_password_kind(&config.id, "jump_passphrase")
                        .map_err(|e| ConnectionError::CredentialError(e.to_string()))?;
                }
                AuthMethod::PrivateKey {
                    passphrase,
                    save_passphrase,
                    ..
                } => {
                    if *save_passphrase {
                        if let Some(p) = passphrase.as_ref().filter(|p| !p.is_empty()) {
                            self.credential_manager
                                .save_password_kind(&config.id, "jump_passphrase", p)
                                .map_err(|e| ConnectionError::CredentialError(e.to_string()))?;
                        }
                    } else {
                        self.credential_manager
                            .delete_password_kind(&config.id, "jump_passphrase")
                            .map_err(|e| ConnectionError::CredentialError(e.to_string()))?;
                    }

                    self.credential_manager
                        .delete_password_kind(&config.id, "jump_password")
                        .map_err(|e| ConnectionError::CredentialError(e.to_string()))?;
                }
                AuthMethod::Agent { .. } => {
                    self.credential_manager
                        .delete_password_kind(&config.id, "jump_password")
                        .map_err(|e| ConnectionError::CredentialError(e.to_string()))?;
                    self.credential_manager
                        .delete_password_kind(&config.id, "jump_passphrase")
                        .map_err(|e| ConnectionError::CredentialError(e.to_string()))?;
                }
                AuthMethod::Certificate {
                    passphrase,
                    save_passphrase,
                    ..
                } => {
                    if *save_passphrase {
                        if let Some(p) = passphrase.as_ref().filter(|p| !p.is_empty()) {
                            self.credential_manager
                                .save_password_kind(&config.id, "jump_passphrase", p)
                                .map_err(|e| ConnectionError::CredentialError(e.to_string()))?;
                        }
                    } else {
                        self.credential_manager
                            .delete_password_kind(&config.id, "jump_passphrase")
                            .map_err(|e| ConnectionError::CredentialError(e.to_string()))?;
                    }

                    self.credential_manager
                        .delete_password_kind(&config.id, "jump_password")
                        .map_err(|e| ConnectionError::CredentialError(e.to_string()))?;
                }
            }
        }

        Ok(())
    }

    pub fn get_ssh_connection(&self, id: &Uuid) -> Option<SshConnection> {
        self.ssh_connections.get(id).cloned()
    }
}

impl ConnectionManager for DefaultConnectionManager {
    fn connect(
        &mut self,
        app: &tauri::AppHandle,
        config: &ConnectionConfig,
    ) -> Result<Uuid, ConnectionError> {
        if config.protocol == ConnectionProtocol::Rdp {
            return Err(ConnectionError::InvalidConfig(
                "RDP does not support in-app sessions".to_string(),
            ));
        }

        // Log connection attempt start
        info!("Starting connection attempt for config: {:?}", config.id);

        // 新增：验证配置合法性，确保不使用默认空配置
        info!("Validating connection configuration...");
        let mut effective_config = config.clone();
        self.fill_saved_credentials(&mut effective_config)?;
        effective_config.validate()?;
        info!("Connection configuration validated successfully");

        // Pre-flight network connectivity check
        // Check connectivity to the immediate next hop (target host or proxy)
        let (check_host, check_port) = match &effective_config.proxy_type {
            ProxyType::None => (effective_config.host.clone(), effective_config.port),
            ProxyType::Socks5 { host, port, .. } => (host.clone(), *port),
            ProxyType::Http { host, port, .. } => (host.clone(), *port),
            ProxyType::JumpHost { host, port, .. } => (host.clone(), *port),
        };
        
        info!("Checking network connectivity to {}:{} before connection...", check_host, check_port);
        let addr = format!("{}:{}", check_host, check_port);
        // Use a short timeout (3 seconds) for the connectivity check
        let check_res = self.runtime.block_on(async {
            tokio::time::timeout(
                std::time::Duration::from_secs(3),
                TcpStream::connect(&addr),
            )
            .await
        });

        match check_res {
            Ok(Ok(_)) => {
                debug!("Network connectivity check passed for {}:{}", check_host, check_port);
            }
            Ok(Err(e)) => {
                let msg = format!("网络不可达: 无法连接到 {}:{} ({})", check_host, check_port, e);
                error!("{}", msg);
                return Err(ConnectionError::ConnectionFailed(msg));
            }
            Err(_) => {
                let msg = format!("网络不可达: 连接 {}:{} 超时 (3秒)", check_host, check_port);
                error!("{}", msg);
                return Err(ConnectionError::ConnectionFailed(msg));
            }
        }

        if effective_config.protocol == ConnectionProtocol::Telnet {
            let session_id = Uuid::new_v4();
            let mut session_info = SessionInfo {
                id: session_id,
                connection_id: config.id,
                status: ConnectionStatus::Connecting,
                terminal_id: None,
                created_at: Utc::now(),
                last_active: Utc::now(),
            };
            self.sessions.insert(session_id, session_info.clone());

            let host = effective_config.host.clone();
            let port = effective_config.port;
            let addr = format!("{}:{}", host, port);
            info!("Attempting to connect Telnet to {}", addr);

            let connect_res = self.runtime.block_on(async move {
                tokio::time::timeout(std::time::Duration::from_secs(10), TcpStream::connect(addr))
                    .await
            });

            return match connect_res {
                Ok(Ok(stream)) => {
                    let (read, write) = stream.into_split();
                    self.telnet_connections
                        .insert(session_id, TelnetConnection { read, write });
                    session_info.status = ConnectionStatus::Connected;
                    self.sessions.insert(session_id, session_info);
                    Ok(session_id)
                }
                Ok(Err(e)) => {
                    session_info.status = ConnectionStatus::Error;
                    self.sessions.insert(session_id, session_info);
                    Err(ConnectionError::ConnectionFailed(e.to_string()))
                }
                Err(_) => {
                    session_info.status = ConnectionStatus::Error;
                    self.sessions.insert(session_id, session_info);
                    Err(ConnectionError::ConnectionFailed(
                        "Telnet connection timed out".to_string(),
                    ))
                }
            };
        }

        // Create a new session ID
        let session_id = Uuid::new_v4();
        info!("Created new session ID: {}", session_id);

        // Update session status to connecting
        let mut session_info = SessionInfo {
            id: session_id,
            connection_id: config.id,
            status: ConnectionStatus::Connecting,
            terminal_id: None,
            created_at: Utc::now(),
            last_active: Utc::now(),
        };

        // Store session info
        self.sessions.insert(session_id, session_info.clone());
        debug!(
            "Stored session info with status: Connecting for session {}",
            session_id
        );

        // Clone necessary fields from config to move into thread
        let host = effective_config.host.clone();
        let port = effective_config.port;
        let username = effective_config.username.clone();
        let auth_method = effective_config.auth_method.clone();
        let local_forwards = effective_config.local_forwards.clone();
        let remote_forwards = effective_config.remote_forwards.clone();
        let proxy_type = effective_config.proxy_type.clone();
        let socks_proxy_port = effective_config.socks_proxy_port;

        let auth_type = auth_method_to_auth_type(auth_method);
        let keyboard_interactive_prompter: Option<Arc<dyn ssh_impl::KeyboardInteractivePrompter>> =
            Some(Arc::new(TauriKeyboardInteractivePrompter {
                app: app.clone(),
                coordinator: self.keyboard_interactive.clone(),
            }));

        // Log connection details (excluding sensitive info)
        info!("Attempting to connect to {}:{} as {}", host, port, username);

        // Use the persistent runtime to establish SSH connection
        // This ensures the connection task remains alive as long as the manager exists
        info!("Starting blocking connection task for session {}", session_id);
        let start_time = std::time::Instant::now();
        let connect_res: Result<(SshConnection, Option<SshConnection>), anyhow::Error> =
            self.runtime.block_on(async {
                match proxy_type {
                    ProxyType::JumpHost {
                        host: jump_host,
                        port: jump_port,
                        username: jump_username,
                        auth_method: jump_auth_method,
                    } => {
                        let jump_auth_type = auth_method_to_auth_type(jump_auth_method);
                        let jump_connection = ssh_impl::connect_ssh(
                            &jump_host,
                            jump_port,
                            &jump_username,
                            jump_auth_type,
                            &Vec::new(),
                            &Vec::new(),
                            None,
                            keyboard_interactive_prompter.clone(),
                        )
                        .await?;

                        let local_port = ssh_impl::start_ephemeral_direct_tcpip_listener(
                            jump_connection.handle.clone(),
                            host.clone(),
                            port,
                        )
                        .await?;

                        let target_connection = ssh_impl::connect_ssh_with_known_host(
                            "127.0.0.1",
                            local_port,
                            &host,
                            port,
                            &username,
                            auth_type,
                            &local_forwards,
                            &remote_forwards,
                            socks_proxy_port,
                            keyboard_interactive_prompter.clone(),
                        )
                        .await?;

                        Ok((target_connection, Some(jump_connection)))
                    }
                    ProxyType::Socks5 {
                        host: proxy_host,
                        port: proxy_port,
                        username: proxy_username,
                        password: proxy_password,
                    } => {
                        let local_port = ssh_impl::start_ephemeral_socks5_proxy_dial_listener(
                            proxy_host,
                            proxy_port,
                            proxy_username,
                            proxy_password,
                            host.clone(),
                            port,
                        )
                        .await?;

                        let target_connection = ssh_impl::connect_ssh_with_known_host(
                            "127.0.0.1",
                            local_port,
                            &host,
                            port,
                            &username,
                            auth_type,
                            &local_forwards,
                            &remote_forwards,
                            socks_proxy_port,
                            keyboard_interactive_prompter.clone(),
                        )
                        .await?;

                        Ok((target_connection, None))
                    }
                    ProxyType::Http {
                        host: proxy_host,
                        port: proxy_port,
                        username: proxy_username,
                        password: proxy_password,
                    } => {
                        let local_port = ssh_impl::start_ephemeral_http_proxy_dial_listener(
                            proxy_host,
                            proxy_port,
                            proxy_username,
                            proxy_password,
                            host.clone(),
                            port,
                        )
                        .await?;

                        let target_connection = ssh_impl::connect_ssh_with_known_host(
                            "127.0.0.1",
                            local_port,
                            &host,
                            port,
                            &username,
                            auth_type,
                            &local_forwards,
                            &remote_forwards,
                            socks_proxy_port,
                            keyboard_interactive_prompter.clone(),
                        )
                        .await?;

                        Ok((target_connection, None))
                    }
                    _ => {
                        let target_connection = ssh_impl::connect_ssh(
                            &host,
                            port,
                            &username,
                            auth_type,
                            &local_forwards,
                            &remote_forwards,
                            socks_proxy_port,
                            keyboard_interactive_prompter.clone(),
                        )
                        .await?;
                        Ok((target_connection, None))
                    }
                }
            });
        
        info!("Blocking connection task finished in {:?}", start_time.elapsed());

        match connect_res {
            Ok(ssh_connection) => {
                // Connection successful
                info!("Connection successful for session: {}", session_id);
                session_info.status = ConnectionStatus::Connected;
                self.sessions.insert(session_id, session_info);
                self.ssh_connections.insert(session_id, ssh_connection.0);
                if let Some(jump) = ssh_connection.1 {
                    self.jump_ssh_connections.insert(session_id, jump);
                }
                Ok(session_id)
            }
            Err(e) => {
                // SSH connection error
                error!("SSH connection error for session {}: {:?}", session_id, e);
                session_info.status = ConnectionStatus::Error;
                self.sessions.insert(session_id, session_info);
                Err(ConnectionError::ConnectionFailed(format!("{:?}", e)))
            }
        }
    }

    fn disconnect(&mut self, session_id: &Uuid) -> Result<(), ConnectionError> {
        let session = self
            .sessions
            .get_mut(session_id)
            .ok_or(ConnectionError::SessionNotFound(*session_id))?;

        session.status = ConnectionStatus::Disconnecting;
        info!("Disconnecting session: {}", session_id);

        // Remove SSH connection if it exists
        if self.ssh_connections.remove(session_id).is_some() {
            debug!("SSH connection removed for session: {}", session_id);
        }
        if self.jump_ssh_connections.remove(session_id).is_some() {
            debug!("Jump SSH connection removed for session: {}", session_id);
        }
        if self.telnet_connections.remove(session_id).is_some() {
            debug!("Telnet connection removed for session: {}", session_id);
        }

        // Update session status
        session.status = ConnectionStatus::Disconnected;
        info!("Session disconnected: {}", session_id);

        Ok(())
    }

    fn get_session(&self, session_id: &Uuid) -> Option<&SessionInfo> {
        self.sessions.get(session_id)
    }

    fn get_all_sessions(&self) -> Vec<SessionInfo> {
        self.sessions.values().cloned().collect()
    }

    fn get_connection_config(&self, connection_id: &Uuid) -> Option<&ConnectionConfig> {
        self.connections.get(connection_id)
    }

    fn save_connection_config(
        &mut self,
        mut config: ConnectionConfig,
    ) -> Result<(), ConnectionError> {
        // Log save connection attempt
        info!("Saving connection configuration for id: {:?}", config.id);

        // Validate the connection configuration before saving
        debug!(
            "Validating connection configuration for id: {:?}",
            config.id
        );
        self.fill_saved_credentials(&mut config)?;
        config.validate_for_save()?;
        debug!(
            "Connection configuration validation passed for id: {:?}",
            config.id
        );

        // If the connection doesn't have an ID, generate a new one
        let id = if config.id == Uuid::nil() {
            let new_id = Uuid::new_v4();
            config.id = new_id;
            debug!("Generated new ID {:?} for connection", new_id);
            new_id
        } else {
            config.id
        };

        // Update timestamps
        let now = Utc::now();
        if config.created_at == DateTime::UNIX_EPOCH {
            config.created_at = now;
            debug!("Set created_at timestamp for new connection: {:?}", now);
        }
        config.updated_at = now;
        debug!("Updated updated_at timestamp for connection: {:?}", now);

        self.sync_credentials_for_save(&config)?;

        // Store connection config
        let stored = Self::sanitize_config_for_storage(&config);
        self.connections.insert(id, stored);
        self.persist_connection_configs_to_db()?;
        info!(
            "Successfully saved connection configuration with id: {:?}",
            id
        );
        Ok(())
    }

    fn delete_connection_config(&mut self, connection_id: &Uuid) -> Result<(), ConnectionError> {
        // Log delete connection attempt
        info!(
            "Deleting connection configuration with id: {:?}",
            connection_id
        );

        if self.connections.remove(connection_id).is_some() {
            let _ = self.credential_manager.delete_password(connection_id);
            let _ = self.credential_manager.delete_passphrase(connection_id);
            self.persist_connection_configs_to_db()?;
            info!(
                "Successfully deleted connection configuration with id: {:?}",
                connection_id
            );
            Ok(())
        } else {
            error!(
                "Failed to delete connection configuration: connection not found for id: {:?}",
                connection_id
            );
            Err(ConnectionError::ConnectionNotFound(*connection_id))
        }
    }

    fn get_all_connection_configs(&self) -> Vec<ConnectionConfig> {
        self.connections.values().cloned().collect()
    }

    fn test_connection(
        &self,
        app: &tauri::AppHandle,
        config: &ConnectionConfig,
    ) -> Result<(), ConnectionError> {
        if config.protocol == ConnectionProtocol::Rdp || config.protocol == ConnectionProtocol::Telnet {
            let effective_config = config.clone();
            effective_config.validate()?;

            let host = effective_config.host.clone();
            let port = effective_config.port;
            info!("Testing TCP connectivity to {}:{}", host, port);

            let addr = format!("{}:{}", host, port);
            let res = self.runtime.block_on(async move {
                tokio::time::timeout(
                    std::time::Duration::from_secs(5),
                    tokio::net::TcpStream::connect(addr),
                )
                .await
            });

            return match res {
                Ok(Ok(_)) => Ok(()),
                Ok(Err(e)) => Err(ConnectionError::ConnectionFailed(e.to_string())),
                Err(_) => Err(ConnectionError::ConnectionFailed(
                    "Connection test timed out".to_string(),
                )),
            };
        } else if config.protocol != ConnectionProtocol::Ssh {
            return Err(ConnectionError::InvalidConfig(
                "Unsupported protocol for connection test".to_string(),
            ));
        }

        // 新增：验证配置合法性
        let mut effective_config = config.clone();
        self.fill_saved_credentials(&mut effective_config)?;
        effective_config.validate()?;

        let host = effective_config.host.clone();
        let port = effective_config.port;
        let username = effective_config.username.clone();
        let auth_method = effective_config.auth_method.clone();
        let proxy_type = effective_config.proxy_type.clone();

        // Log connection test details (excluding sensitive info)
        info!("Testing connection to {}:{} as {}", host, port, username);

        let auth_type = auth_method_to_auth_type(auth_method);

        // Clone variables for logging
        let host_clone = host.clone();
        let port_clone = port;
        let local_forwards = effective_config.local_forwards.clone();
        let remote_forwards = effective_config.remote_forwards.clone();
        let keyboard_interactive_prompter: Option<Arc<dyn ssh_impl::KeyboardInteractivePrompter>> =
            Some(Arc::new(TauriKeyboardInteractivePrompter {
                app: app.clone(),
                coordinator: self.keyboard_interactive.clone(),
            }));

        // Use the persistent runtime to test the connection
        let res: Result<(), anyhow::Error> = self.runtime.block_on(async {
            match proxy_type {
                ProxyType::JumpHost {
                    host: jump_host,
                    port: jump_port,
                    username: jump_username,
                    auth_method: jump_auth_method,
                } => {
                    let jump_auth_type = auth_method_to_auth_type(jump_auth_method);
                    let jump_connection = ssh_impl::connect_ssh(
                        &jump_host,
                        jump_port,
                        &jump_username,
                        jump_auth_type,
                        &Vec::new(),
                        &Vec::new(),
                        None,
                        keyboard_interactive_prompter.clone(),
                    )
                    .await?;

                    let local_port = ssh_impl::start_ephemeral_direct_tcpip_listener(
                        jump_connection.handle.clone(),
                        host.clone(),
                        port,
                    )
                    .await?;

                    let _target_connection = ssh_impl::connect_ssh_with_known_host(
                        "127.0.0.1",
                        local_port,
                        &host,
                        port,
                        &username,
                        auth_type,
                        &local_forwards,
                        &remote_forwards,
                        effective_config.socks_proxy_port,
                        keyboard_interactive_prompter.clone(),
                    )
                    .await?;

                    Ok(())
                }
                ProxyType::Socks5 {
                    host: proxy_host,
                    port: proxy_port,
                    username: proxy_username,
                    password: proxy_password,
                } => {
                    let local_port = ssh_impl::start_ephemeral_socks5_proxy_dial_listener(
                        proxy_host,
                        proxy_port,
                        proxy_username,
                        proxy_password,
                        host.clone(),
                        port,
                    )
                    .await?;

                    let _target_connection = ssh_impl::connect_ssh_with_known_host(
                        "127.0.0.1",
                        local_port,
                        &host,
                        port,
                        &username,
                        auth_type,
                        &local_forwards,
                        &remote_forwards,
                        effective_config.socks_proxy_port,
                        keyboard_interactive_prompter.clone(),
                    )
                    .await?;

                    Ok(())
                }
                ProxyType::Http {
                    host: proxy_host,
                    port: proxy_port,
                    username: proxy_username,
                    password: proxy_password,
                } => {
                    let local_port = ssh_impl::start_ephemeral_http_proxy_dial_listener(
                        proxy_host,
                        proxy_port,
                        proxy_username,
                        proxy_password,
                        host.clone(),
                        port,
                    )
                    .await?;

                    let _target_connection = ssh_impl::connect_ssh_with_known_host(
                        "127.0.0.1",
                        local_port,
                        &host,
                        port,
                        &username,
                        auth_type,
                        &local_forwards,
                        &remote_forwards,
                        effective_config.socks_proxy_port,
                        keyboard_interactive_prompter.clone(),
                    )
                    .await?;

                    Ok(())
                }
                _ => {
                    let _target_connection = ssh_impl::connect_ssh(
                        &host,
                        port,
                        &username,
                        auth_type,
                        &local_forwards,
                        &remote_forwards,
                        effective_config.socks_proxy_port,
                        keyboard_interactive_prompter.clone(),
                    )
                    .await?;
                    Ok(())
                }
            }
        });

        match res {
            Ok(()) => {
                // Connection test successful
                info!("Connection test successful: {}:{}", host_clone, port_clone);
                Ok(())
            }
            Err(e) => {
                // SSH connection error
                error!("Connection test failed: {:?}", e);
                Err(ConnectionError::ConnectionFailed(format!("{:?}", e)))
            }
        }
    }

    fn start_terminal(
        &mut self,
        app: &tauri::AppHandle,
        session_id: &Uuid,
        width: u16,
        height: u16,
    ) -> Result<bool, ConnectionError> {
        // Check if session exists
        let session = self
            .sessions
            .get(session_id)
            .ok_or(ConnectionError::SessionNotFound(*session_id))?;

        // Check if session is connected
        if session.status != ConnectionStatus::Connected {
            return Err(ConnectionError::ConnectionFailed(
                "Session is not connected".to_string(),
            ));
        }

        let protocol = self
            .connections
            .get(&session.connection_id)
            .map(|c| c.protocol.clone())
            .unwrap_or_default();

        if protocol == ConnectionProtocol::Telnet {
            let telnet = self
                .telnet_connections
                .remove(session_id)
                .ok_or(ConnectionError::SessionNotFound(*session_id))?;

            info!("Starting Telnet terminal for session: {}", session_id);

            let terminal_id = Uuid::new_v4();
            let (tx, mut rx) = mpsc::channel::<TerminalCommand>(2048);

            let session_id_clone = *session_id;
            let app_clone = app.clone();
            let tracker_clone = Arc::clone(&self.tracker);

            tracker_clone
                .lock()
                .unwrap()
                .register_session(session_id_clone);

            let runtime = Arc::clone(&self.runtime);
            let mut read = telnet.read;
            let mut write = telnet.write;

            runtime.spawn(async move {
                let mut buf = vec![0u8; 8192];
                #[allow(unused_assignments)]
                let mut exit_reason = "unknown";
                let mut output_stats_last = tokio::time::Instant::now();
                let mut output_stats_bytes: usize = 0;
                let mut output_stats_messages: u64 = 0;

                loop {
                    tokio::select! {
                        read_res = read.read(&mut buf) => {
                            match read_res {
                                Ok(0) => {
                                    exit_reason = "connection_lost";
                                    break;
                                }
                                Ok(n) => {
                                    let mut display = Vec::<u8>::new();
                                    let mut replies = Vec::<u8>::new();
                                    telnet_process_incoming(&buf[..n], &mut display, &mut replies);

                                    if !replies.is_empty() {
                                        if let Ok(mut tracker) = tracker_clone.lock() {
                                            tracker.log_data(session_id_clone, &replies, "sent");
                                        }
                                        let _ = write.write_all(&replies).await;
                                    }

                                    if !display.is_empty() {
                                        if let Ok(mut tracker) = tracker_clone.lock() {
                                            tracker.log_data(session_id_clone, &display, "received");
                                        }
                                        let data_str = String::from_utf8_lossy(&display).to_string();
                                        let event_name = format!("terminal-output-{}", session_id_clone);
                                        output_stats_bytes += display.len();
                                        output_stats_messages += 1;
                                        let emit_start = tokio::time::Instant::now();
                                        let _ = app_clone.emit(&event_name, serde_json::json!({ "data": data_str }));
                                        let emit_ms = emit_start.elapsed().as_millis();
                                        if emit_ms > 10 {
                                            warn!(
                                                "Terminal output emit slow (telnet) session {}: {}ms",
                                                session_id_clone, emit_ms
                                            );
                                        }
                                        let elapsed = output_stats_last.elapsed();
                                        if elapsed.as_millis() >= 1000 {
                                            info!(
                                                "Terminal output cadence (telnet) session {}: msgs={}, bytes={}, elapsed_ms={}",
                                                session_id_clone,
                                                output_stats_messages,
                                                output_stats_bytes,
                                                elapsed.as_millis()
                                            );
                                            output_stats_last = tokio::time::Instant::now();
                                            output_stats_bytes = 0;
                                            output_stats_messages = 0;
                                        }
                                    }
                                }
                                Err(e) => {
                                    let error_msg = format!("Telnet read error: {}", e);
                                    let event_name = format!("terminal-error-{}", session_id_clone);
                                    let _ = app_clone.emit(&event_name, serde_json::json!({ "error": error_msg }));
                                    exit_reason = "read_error";
                                    break;
                                }
                            }
                        }
                        cmd = rx.recv() => {
                            match cmd {
                                Some(TerminalCommand::Data(data)) => {
                                    if let Ok(mut tracker) = tracker_clone.lock() {
                                        tracker.log_data(session_id_clone, &data, "sent");
                                    }
                                    if let Err(e) = write.write_all(&data).await {
                                        let error_msg = format!("Telnet write error: {}", e);
                                        let event_name = format!("terminal-error-{}", session_id_clone);
                                        let _ = app_clone.emit(&event_name, serde_json::json!({ "error": error_msg }));
                                        exit_reason = "write_error";
                                        break;
                                    }
                                }
                                Some(TerminalCommand::Resize(_, _)) => {}
                                Some(TerminalCommand::Close) => {
                                    let _ = write.shutdown().await;
                                    exit_reason = "user_closed";
                                    break;
                                }
                                None => {
                                    exit_reason = "command_channel_closed";
                                    break;
                                }
                            }
                        }
                    }
                }

                let event_name = format!("session-closed-{}", session_id_clone);
                let _ = app_clone.emit(&event_name, serde_json::json!({ "reason": exit_reason }));
            });

            self.terminals.insert(
                terminal_id,
                TerminalSession {
                    id: terminal_id,
                    session_id: *session_id,
                    sender: tx,
                },
            );

            if let Some(session) = self.sessions.get_mut(session_id) {
                session.terminal_id = Some(terminal_id);
            }

            return Ok(true);
        }

        // Get SSH connection
        let ssh_connection = self
            .ssh_connections
            .get(session_id)
            .ok_or(ConnectionError::SessionNotFound(*session_id))?;

        // Check if SSH connection is still valid
        debug!(
            "Skipping synchronous health check for session: {}",
            session_id
        );

        info!("Starting terminal for session: {}", session_id);

        // Create terminal ID
        let terminal_id = Uuid::new_v4();

        // Create command channel
        let (tx, mut rx) = mpsc::channel::<TerminalCommand>(2048);

        let session_id_clone = *session_id;
        let app_clone = app.clone();
        let ssh_handle_clone = Arc::clone(&ssh_connection.handle);

        // Tracker clone
        let tracker_clone = Arc::clone(&self.tracker);

        // Register session in tracker
        tracker_clone
            .lock()
            .unwrap()
            .register_session(session_id_clone);

        // Spawn a task to handle the terminal channel on the persistent runtime
        let runtime = Arc::clone(&self.runtime);
        runtime.spawn(async move {
            // Debug: Log connection state before opening channel
            debug!("Attempting to open terminal channel for session: {}", session_id_clone);

            // Open channel
            // We need to lock the mutex to access the handle, but we should drop the lock
            // as soon as we have the channel to allow other operations on the connection.
            let channel_result = {
                let handle = ssh_handle_clone.lock().await;
                handle.channel_open_session().await
            };

            match channel_result {
                Ok(mut channel) => {
                    debug!("Channel opened for session: {}", session_id_clone);

                    // Request PTY with proper terminal modes
                    // russh uses a different approach for terminal modes - use empty array for now
                    if let Err(e) = channel.request_pty(true, "xterm-256color", width as u32, height as u32, 0, 0, &[]).await {
                        error!("Failed to request PTY: {:?}", e);
                        // Send error to frontend
                        let error_msg = format!("Failed to request PTY: {}", e);
                        let event_name = format!("terminal-error-{}", session_id_clone);
                        let _ = app_clone.emit(&event_name, serde_json::json!({ "error": error_msg }));
                        return;
                    }

                    // Start shell
                    if let Err(e) = channel.request_shell(true).await {
                        error!("Failed to start shell: {:?}", e);
                        // Send error to frontend
                        let error_msg = format!("Failed to start shell: {}", e);
                        let event_name = format!("terminal-error-{}", session_id_clone);
                        let _ = app_clone.emit(&event_name, serde_json::json!({ "error": error_msg }));
                        return;
                    }

                    info!("Terminal started for session: {}", session_id_clone);

                    // Send initial newline to trigger shell prompt
                    let newline_data = b"\r\n";
                    if let Err(e) = channel.data(&newline_data[..]).await {
                        error!("Failed to send initial newline: {:?}", e);
                    }

                    // Event loop for channel and commands
                    let mut last_activity = tokio::time::Instant::now();
                    #[allow(unused_assignments)]
                    let mut exit_reason = "unknown";
                    let mut output_stats_last = tokio::time::Instant::now();
                    let mut output_stats_bytes: usize = 0;
                    let mut output_stats_messages: u64 = 0;

                    let mut output_buffer = Vec::new();
                    let mut flush_deadline: Option<tokio::time::Instant> = None;

                    loop {
                        tokio::select! {
                            // Flush buffer if deadline reached
                            _ = async {
                                if let Some(deadline) = flush_deadline {
                                    tokio::time::sleep_until(deadline).await
                                } else {
                                    std::future::pending().await
                                }
                            }, if flush_deadline.is_some() => {
                                if !output_buffer.is_empty() {
                                    if let Ok(mut tracker) = tracker_clone.lock() {
                                        tracker.log_data(session_id_clone, &output_buffer, "received");
                                    }

                                    let data_str = String::from_utf8_lossy(&output_buffer).to_string();
                                    let event_name = format!("terminal-output-{}", session_id_clone);
                                    output_stats_bytes += output_buffer.len();
                                    output_stats_messages += 1;
                                    
                                    let emit_start = tokio::time::Instant::now();
                                    let _ = app_clone.emit(&event_name, serde_json::json!({ "data": data_str }));
                                    let emit_ms = emit_start.elapsed().as_millis();
                                    if emit_ms > 10 {
                                        warn!("Terminal output emit slow (ssh) session {}: {}ms", session_id_clone, emit_ms);
                                    }
                                    
                                    output_buffer.clear();
                                }
                                flush_deadline = None;
                            }

                            // Handle incoming SSH data
                            msg = channel.wait() => {
                                match msg {
                                    Some(russh::ChannelMsg::Data { ref data }) => {
                                        // Update last activity time
                                        last_activity = tokio::time::Instant::now();

                                        output_buffer.extend_from_slice(data);

                                        // Increase buffer threshold to 64KB to reduce IPC frequency and CPU usage
                                        if output_buffer.len() >= 65536 {
                                            // Flush immediately
                                            if let Ok(mut tracker) = tracker_clone.lock() {
                                                tracker.log_data(session_id_clone, &output_buffer, "received");
                                            }

                                            let data_str = String::from_utf8_lossy(&output_buffer).to_string();
                                            let event_name = format!("terminal-output-{}", session_id_clone);
                                            output_stats_bytes += output_buffer.len();
                                            output_stats_messages += 1;
                                            
                                            let emit_start = tokio::time::Instant::now();
                                            let _ = app_clone.emit(&event_name, serde_json::json!({ "data": data_str }));
                                            let emit_ms = emit_start.elapsed().as_millis();
                                            if emit_ms > 10 {
                                                warn!("Terminal output emit slow (ssh) session {}: {}ms", session_id_clone, emit_ms);
                                            }
                                            
                                            output_buffer.clear();
                                            flush_deadline = None;
                                        } else if flush_deadline.is_none() {
                                            flush_deadline = Some(tokio::time::Instant::now() + tokio::time::Duration::from_millis(15));
                                        }

                                        let elapsed = output_stats_last.elapsed();
                                        if elapsed.as_millis() >= 1000 {
                                            info!(
                                                "Terminal output cadence (ssh) session {}: msgs={}, bytes={}, elapsed_ms={}",
                                                session_id_clone,
                                                output_stats_messages,
                                                output_stats_bytes,
                                                elapsed.as_millis()
                                            );
                                            output_stats_last = tokio::time::Instant::now();
                                            output_stats_bytes = 0;
                                            output_stats_messages = 0;
                                        }
                                    },
                                    Some(russh::ChannelMsg::ExitStatus { exit_status }) => {
                                        info!("Terminal exited with status: {}", exit_status);
                                        exit_reason = "normal";
                                        break;
                                    },
                                    Some(russh::ChannelMsg::Close) => {
                                        info!("Channel closed by server");
                                        exit_reason = "server_closed";
                                        break;
                                    },
                                    None => {
                                        debug!("Channel closed (connection lost)");
                                        exit_reason = "connection_lost";
                                        break;
                                    },
                                    _ => {}
                                }
                            }
                            // Handle outgoing commands
                            cmd = rx.recv() => {
                                match cmd {
                                    Some(TerminalCommand::Data(data)) => {
                                        // Update last activity time
                                        last_activity = tokio::time::Instant::now();

                                        // russh::Channel::data takes AsyncRead
                                        let _ = channel.data(&data[..]).await;
                                    },
                                    Some(TerminalCommand::Resize(w, h)) => {
                                        let _ = channel.window_change(w, h, 0, 0).await;
                                    },
                                    Some(TerminalCommand::Close) => {
                                        let _ = channel.close().await;
                                        exit_reason = "user_closed";
                                        break;
                                    },
                                    None => {
                                        debug!("Command channel closed");
                                        exit_reason = "command_channel_closed";
                                        break;
                                    }
                                }
                            }
                            // Heartbeat check (every 30 seconds)
                            _ = tokio::time::sleep(tokio::time::Duration::from_secs(30)) => {
                                // Check if we've had activity in the last 60 seconds
                                if last_activity.elapsed() > tokio::time::Duration::from_secs(60) {
                                    // Send a keepalive by sending a null byte
                                    let null_byte = b"\0";
                                    if let Err(e) = channel.data(&null_byte[..]).await {
                                        debug!("Keepalive failed, connection may be dead: {:?}", e);
                                        exit_reason = "keepalive_failed";
                                        break;
                                    }
                                    debug!("Sent keepalive to session: {}", session_id_clone);
                                }
                            }
                        }
                    }

                    // Emit session closed event
                    let event_name = format!("session-closed-{}", session_id_clone);
                    info!("Emitting session closed event: {} (reason: {})", event_name, exit_reason);
                    let _ = app_clone.emit(&event_name, serde_json::json!({ "reason": exit_reason }));
                },
                Err(e) => {
                    error!("Failed to open terminal channel: {:?}", e);
                    // Send error to frontend
                    let error_msg = format!("Failed to open terminal channel: {}", e);
                    let event_name = format!("terminal-error-{}", session_id_clone);
                    let _ = app_clone.emit(&event_name, serde_json::json!({ "error": error_msg }));
                }
            }
        });

        // Store terminal session
        self.terminals.insert(
            terminal_id,
            TerminalSession {
                id: terminal_id,
                session_id: *session_id,
                sender: tx,
            },
        );

        // Update session info
        if let Some(session) = self.sessions.get_mut(session_id) {
            session.terminal_id = Some(terminal_id);
        }

        Ok(true)
    }

    fn send_terminal_data(&mut self, session_id: &Uuid, data: &str) -> Result<(), ConnectionError> {
        // Find terminal for this session
        let terminal = self
            .terminals
            .values()
            .find(|t| &t.session_id == session_id)
            .ok_or(ConnectionError::SessionNotFound(*session_id))?;

        let data_bytes = data.as_bytes().to_vec();

        // Log sent data
        // Log sent data
        if let Ok(mut tracker) = self.tracker.lock() {
            tracker.log_data(*session_id, &data_bytes, "sent");
        } else {
            error!("Failed to lock channel tracker for logging sent data");
        }

        // Send data command
        let sender = terminal.sender.clone();
        let send_start = std::time::Instant::now();
        match sender.blocking_send(TerminalCommand::Data(data_bytes)) {
            Ok(_) => {
                let send_ms = send_start.elapsed().as_millis();
                if send_ms > 10 {
                    warn!(
                        "Terminal input send slow session {}: {}ms",
                        session_id, send_ms
                    );
                }
                Ok(())
            },
            Err(e) => {
                error!("Failed to send terminal data: {}", e);
                Err(ConnectionError::ConnectionFailed(format!(
                    "Failed to send data: {}",
                    e
                )))
            }
        }
    }

    fn resize_terminal(
        &mut self,
        session_id: &Uuid,
        width: u16,
        height: u16,
    ) -> Result<(), ConnectionError> {
        // Find terminal for this session
        let terminal = self
            .terminals
            .values()
            .find(|t| &t.session_id == session_id)
            .ok_or(ConnectionError::SessionNotFound(*session_id))?;

        // Send resize command
        let sender = terminal.sender.clone();
        let _ = sender.blocking_send(TerminalCommand::Resize(width as u32, height as u32));

        debug!("Resizing terminal to {}x{}", width, height);
        Ok(())
    }

    fn close_terminal(&mut self, session_id: &Uuid) -> Result<(), ConnectionError> {
        println!("[ConnectionManager] close_terminal called for session: {}", session_id);
        // Find and remove terminal for this session
        let terminal_id = self
            .terminals
            .values()
            .find(|t| &t.session_id == session_id)
            .map(|t| t.id)
            .ok_or(ConnectionError::SessionNotFound(*session_id))?;

        let terminal = self
            .terminals
            .remove(&terminal_id)
            .ok_or(ConnectionError::SessionNotFound(*session_id))?;

        // Send close command
        let sender = terminal.sender.clone();
        let _ = sender.blocking_send(TerminalCommand::Close);

        // Update session info
        if let Some(session) = self.sessions.get_mut(session_id) {
            session.terminal_id = None;
        }

        info!("Terminal closed for session: {}", session_id);
        Ok(())
    }

    fn get_terminal_sender(&self, session_id: &Uuid) -> Option<mpsc::Sender<TerminalCommand>> {
        self.terminals
            .values()
            .find(|t| &t.session_id == session_id)
            .map(|t| t.sender.clone())
    }

    fn log_terminal_data(&self, session_id: &Uuid, data: &[u8], direction: &str) {
        if let Ok(mut tracker) = self.tracker.lock() {
            tracker.log_data(*session_id, data, direction);
        } else {
            error!("Failed to lock channel tracker for logging data");
        }
    }

    fn exec_command(
        &self,
        session_id: &Uuid,
        command: &str,
    ) -> Result<String, ConnectionError> {
        let ssh_connection = self
            .ssh_connections
            .get(session_id)
            .ok_or(ConnectionError::SessionNotFound(*session_id))?
            .clone();

        let command = command.to_string();

        // Execute command in a blocking way using runtime
        let result = self.runtime.block_on(async move {
            let handle = ssh_connection.handle.lock().await;
            let mut channel = handle.channel_open_session().await.map_err(|e| {
                ConnectionError::SshError(format!("Failed to open channel: {:?}", e))
            })?;

            channel
                .exec(true, command.as_bytes().to_vec())
                .await
                .map_err(|e| {
                    ConnectionError::SshError(format!("Failed to execute command: {:?}", e))
                })?;

            let mut output = String::new();
            while let Some(msg) = channel.wait().await {
                match msg {
                    russh::ChannelMsg::Data { ref data } => {
                        output.push_str(&String::from_utf8_lossy(data));
                    }
                    russh::ChannelMsg::ExtendedData { ref data, .. } => {
                        output.push_str(&String::from_utf8_lossy(data));
                    }
                    russh::ChannelMsg::Eof => {
                        break;
                    }
                    _ => {}
                }
            }
            channel.close().await.ok();
            Ok(output)
        });

        result
    }
}

fn telnet_process_incoming(input: &[u8], display: &mut Vec<u8>, replies: &mut Vec<u8>) {
    const IAC: u8 = 255;
    const DONT: u8 = 254;
    const DO: u8 = 253;
    const WONT: u8 = 252;
    const WILL: u8 = 251;
    const SB: u8 = 250;
    const SE: u8 = 240;

    let mut i = 0usize;
    while i < input.len() {
        if input[i] != IAC {
            display.push(input[i]);
            i += 1;
            continue;
        }

        if i + 1 >= input.len() {
            break;
        }

        let cmd = input[i + 1];
        if cmd == IAC {
            display.push(IAC);
            i += 2;
            continue;
        }

        if cmd == SB {
            i += 2;
            while i + 1 < input.len() {
                if input[i] == IAC && input[i + 1] == SE {
                    i += 2;
                    break;
                }
                i += 1;
            }
            continue;
        }

        if cmd == DO || cmd == DONT || cmd == WILL || cmd == WONT {
            if i + 2 >= input.len() {
                break;
            }
            let opt = input[i + 2];
            match cmd {
                DO | DONT => {
                    replies.extend_from_slice(&[IAC, WONT, opt]);
                }
                WILL | WONT => {
                    replies.extend_from_slice(&[IAC, DONT, opt]);
                }
                _ => {}
            }
            i += 3;
            continue;
        }

        i += 2;
    }
}

fn auth_method_to_auth_type(auth_method: AuthMethod) -> ssh_impl::AuthType {
    match auth_method {
        AuthMethod::Password { password, .. } => ssh_impl::AuthType::Password(Some(password)),
        AuthMethod::KeyboardInteractive {} => ssh_impl::AuthType::KeyboardInteractive,
        AuthMethod::PrivateKey {
            key_path,
            passphrase,
            ..
        } => ssh_impl::AuthType::PrivateKey(key_path, passphrase),
        AuthMethod::Agent { agent_path } => ssh_impl::AuthType::Agent(agent_path),
        AuthMethod::Certificate {
            certificate_path,
            private_key_path,
            passphrase,
            ..
        } => ssh_impl::AuthType::Certificate(certificate_path, private_key_path, passphrase),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_connection_config_validation() {
        let mut config = ConnectionConfig::default();
        assert!(config.validate().is_err()); // Default is empty, should fail

        config.host = "localhost".to_string();
        config.port = 22;
        config.username = "user".to_string();
        config.auth_method = AuthMethod::Password {
            password: "password".to_string(),
            save_password: false,
        };
        assert!(config.validate().is_ok());

        config.port = 0;
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_rdp_config_validation() {
        let mut config = ConnectionConfig::default();
        config.protocol = ConnectionProtocol::Rdp;
        config.host = "192.168.1.10".to_string();
        config.port = 3389;
        config.username = "".to_string();
        config.auth_method = AuthMethod::Password {
            password: "".to_string(),
            save_password: false,
        };
        assert!(config.validate().is_ok());
        assert!(config.validate_for_save().is_ok());
    }

    #[test]
    fn test_telnet_config_validation() {
        let mut config = ConnectionConfig::default();
        config.protocol = ConnectionProtocol::Telnet;
        config.host = "192.168.1.10".to_string();
        config.port = 23;
        config.username = "".to_string();
        config.auth_method = AuthMethod::Password {
            password: "".to_string(),
            save_password: false,
        };
        assert!(config.validate().is_ok());
        assert!(config.validate_for_save().is_ok());
    }

    #[test]
    fn test_default_connection_manager_new() {
        let manager = DefaultConnectionManager::new();
        assert!(manager.connections.is_empty());
        assert!(manager.sessions.is_empty());
    }

    #[test]
    fn test_save_and_get_connection_config() {
        let mut manager = DefaultConnectionManager::new();
        let mut config = ConnectionConfig::default();
        config.host = "192.168.1.1".to_string();
        config.port = 22;
        config.username = "admin".to_string();
        config.auth_method = AuthMethod::Password {
            password: "admin".to_string(),
            save_password: false,
        };

        // Save
        assert!(manager.save_connection_config(config.clone()).is_ok());

        // Retrieve
        let configs = manager.get_all_connection_configs();
        assert_eq!(configs.len(), 1);
        assert_eq!(configs[0].host, "192.168.1.1");

        // Delete
        let id = configs[0].id;
        assert!(manager.delete_connection_config(&id).is_ok());
        assert!(manager.get_all_connection_configs().is_empty());
    }
}
