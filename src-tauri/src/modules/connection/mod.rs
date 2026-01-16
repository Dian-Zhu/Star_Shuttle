use serde::{Deserialize, Serialize}; use uuid::Uuid; use chrono::{DateTime, Utc}; use std::collections::HashMap; use thiserror::Error; use log::{info, debug, error, warn};

// Re-export submodules
pub mod auth; pub mod error; pub mod ssh_impl; pub mod known_hosts; pub mod tracking;
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

// Connection configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionConfig {
    pub id: Uuid,
    pub name: String,
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
            host: String::new(), // 改为空字符串，避免默认连接localhost
            port: 0, // 改为0，避免默认使用22端口
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
        }
    }
}

impl ConnectionConfig {
    pub fn validate(&self) -> Result<(), ConnectionError> {
        if self.host.is_empty() {
            return Err(ConnectionError::InvalidConfig("Host is required".to_string()));
        }
        
        if self.username.is_empty() {
            return Err(ConnectionError::InvalidConfig("Username is required".to_string()));
        }
        
        if self.port == 0 {
            return Err(ConnectionError::InvalidConfig("Port is required".to_string()));
        }
        
        if self.port < 1 || self.port > 65535 {
            return Err(ConnectionError::InvalidConfig("Port must be between 1 and 65535".to_string()));
        }
        
        match &self.auth_method {
            AuthMethod::Password { password, .. } => {
                if password.is_empty() {
                    return Err(ConnectionError::InvalidConfig("Password is required".to_string()));
                }
            },
            AuthMethod::PrivateKey { key_path, .. } => {
                if key_path.is_empty() {
                    return Err(ConnectionError::InvalidConfig("Private key path is required".to_string()));
                }
            },
            AuthMethod::Agent { .. } => {},
            AuthMethod::Certificate { certificate_path, private_key_path, .. } => {
                if certificate_path.is_empty() {
                    return Err(ConnectionError::InvalidConfig("Certificate path is required".to_string()));
                }
                
                if private_key_path.is_empty() {
                    return Err(ConnectionError::InvalidConfig("Private key path is required for certificate authentication".to_string()));
                }
            },
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
    fn connect(&mut self, config: &ConnectionConfig) -> Result<Uuid, ConnectionError>;
    fn disconnect(&mut self, session_id: &Uuid) -> Result<(), ConnectionError>;
    fn get_session(&self, session_id: &Uuid) -> Option<&SessionInfo>;
    fn get_all_sessions(&self) -> Vec<SessionInfo>;
    fn get_connection_config(&self, connection_id: &Uuid) -> Option<&ConnectionConfig>;
    fn save_connection_config(&mut self, config: ConnectionConfig) -> Result<(), ConnectionError>;
    fn delete_connection_config(&mut self, connection_id: &Uuid) -> Result<(), ConnectionError>;
    fn get_all_connection_configs(&self) -> Vec<ConnectionConfig>;
    fn test_connection(&self, config: &ConnectionConfig) -> Result<(), ConnectionError>;

    // Terminal methods
    fn start_terminal(&mut self, app: &tauri::AppHandle, session_id: &Uuid, width: u16, height: u16) -> Result<bool, ConnectionError>;
    fn send_terminal_data(&mut self, session_id: &Uuid, data: &str) -> Result<(), ConnectionError>;
    fn resize_terminal(&mut self, session_id: &Uuid, width: u16, height: u16) -> Result<(), ConnectionError>;
    fn close_terminal(&mut self, session_id: &Uuid) -> Result<(), ConnectionError>;
    
    // Command execution
    fn exec_command(&mut self, session_id: &Uuid, command: &str) -> Result<String, ConnectionError>;
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

use std::sync::{Arc, Mutex};
use crate::modules::connection::ssh_impl::{connect_ssh, SshConnection};
use crate::modules::connection::tracking::ChannelTracker;
use tauri::Emitter;
use tokio::sync::mpsc;
use tokio::runtime::Runtime;

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

// Default connection manager implementation
pub struct DefaultConnectionManager {
    connections: HashMap<Uuid, ConnectionConfig>,
    sessions: HashMap<Uuid, SessionInfo>,
    ssh_connections: HashMap<Uuid, SshConnection>,
    terminals: HashMap<Uuid, TerminalSession>,
    tracker: Arc<Mutex<ChannelTracker>>,
    runtime: Arc<Runtime>,
}

// Manual Debug implementation to handle the non-Debug ssh_connections and runtime fields
impl std::fmt::Debug for DefaultConnectionManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DefaultConnectionManager")
            .field("connections", &self.connections)
            .field("sessions", &self.sessions)
            .field("ssh_connections_count", &self.ssh_connections.len())
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
            terminals: HashMap::new(),
            tracker: Arc::new(Mutex::new(ChannelTracker::new())),
            runtime: Arc::new(runtime),
        }
    }

    pub fn get_ssh_connection(&self, id: &Uuid) -> Option<SshConnection> {
        self.ssh_connections.get(id).cloned()
    }
}

impl ConnectionManager for DefaultConnectionManager {
    fn connect(&mut self, config: &ConnectionConfig) -> Result<Uuid, ConnectionError> {
        // Log connection attempt start
        info!("Starting connection attempt for config: {:?}", config.id);
        
        // 新增：验证配置合法性，确保不使用默认空配置
        info!("Validating connection configuration...");
        config.validate()?;
        info!("Connection configuration validated successfully");
        
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
        debug!("Stored session info with status: Connecting for session {}", session_id);
        
        // Clone necessary fields from config to move into thread
        let host = config.host.clone();
        let port = config.port;
        let username = config.username.clone();
        let auth_method = config.auth_method.clone();
        let local_forwards = config.local_forwards.clone();
        let remote_forwards = config.remote_forwards.clone();
        
        // Convert our AuthMethod to ssh_impl::AuthType
        let auth_type = match auth_method {
            AuthMethod::Password { password, .. } => ssh_impl::AuthType::Password(Some(password)),
            AuthMethod::PrivateKey { key_path, passphrase, .. } => {
                ssh_impl::AuthType::PrivateKey(key_path, passphrase)
            },
            AuthMethod::Agent { agent_path } => ssh_impl::AuthType::Agent(agent_path),
            AuthMethod::Certificate { certificate_path, private_key_path, passphrase, .. } => {
                ssh_impl::AuthType::Certificate(certificate_path, private_key_path, passphrase)
            },
        };
        
        // Log connection details (excluding sensitive info)
        info!("Attempting to connect to {}:{} as {}", host, port, username);
        
        // Use the persistent runtime to establish SSH connection
        // This ensures the connection task remains alive as long as the manager exists
        match self.runtime.block_on(async {
            ssh_impl::connect_ssh(&host, port, &username, auth_type, &local_forwards, &remote_forwards).await
        }) {
            Ok(ssh_connection) => {
                // Connection successful
                info!("Connection successful for session: {}", session_id);
                session_info.status = ConnectionStatus::Connected;
                self.sessions.insert(session_id, session_info);
                self.ssh_connections.insert(session_id, ssh_connection);
                Ok(session_id)
            },
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
        let session = self.sessions.get_mut(session_id).ok_or(ConnectionError::SessionNotFound(*session_id))?;
        
        session.status = ConnectionStatus::Disconnecting;
        info!("Disconnecting session: {}", session_id);
        
        // Remove SSH connection if it exists
        if self.ssh_connections.remove(session_id).is_some() {
            debug!("SSH connection removed for session: {}", session_id);
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

    fn save_connection_config(&mut self, mut config: ConnectionConfig) -> Result<(), ConnectionError> {
        // Log save connection attempt
        info!("Saving connection configuration for id: {:?}", config.id);
        
        // Validate the connection configuration before saving
        debug!("Validating connection configuration for id: {:?}", config.id);
        config.validate()?;
        debug!("Connection configuration validation passed for id: {:?}", config.id);
        
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
        
        // Store connection config
        self.connections.insert(id, config);
        info!("Successfully saved connection configuration with id: {:?}", id);
        Ok(())
    }

    fn delete_connection_config(&mut self, connection_id: &Uuid) -> Result<(), ConnectionError> {
        // Log delete connection attempt
        info!("Deleting connection configuration with id: {:?}", connection_id);
        
        if self.connections.remove(connection_id).is_some() {
            info!("Successfully deleted connection configuration with id: {:?}", connection_id);
            Ok(())
        } else {
            error!("Failed to delete connection configuration: connection not found for id: {:?}", connection_id);
            Err(ConnectionError::ConnectionNotFound(*connection_id))
        }
    }

    fn get_all_connection_configs(&self) -> Vec<ConnectionConfig> {
        self.connections.values().cloned().collect()
    }

    fn test_connection(&self, config: &ConnectionConfig) -> Result<(), ConnectionError> {
        // 新增：验证配置合法性
        config.validate()?;
        
        let host = config.host.clone();
        let port = config.port;
        let username = config.username.clone();
        let auth_method = config.auth_method.clone();
        
        // Log connection test details (excluding sensitive info)
        info!("Testing connection to {}:{} as {}", host, port, username);
        
        // Convert our AuthMethod to ssh_impl::AuthType
        let auth_type = match auth_method {
            AuthMethod::Password { password, .. } => ssh_impl::AuthType::Password(Some(password)),
            AuthMethod::PrivateKey { key_path, passphrase, .. } => {
                ssh_impl::AuthType::PrivateKey(key_path, passphrase)
            },
            AuthMethod::Agent { agent_path } => ssh_impl::AuthType::Agent(agent_path),
            AuthMethod::Certificate { certificate_path, private_key_path, passphrase, .. } => {
                ssh_impl::AuthType::Certificate(certificate_path, private_key_path, passphrase)
            },
        };
        
        // Clone variables for logging
        let host_clone = host.clone();
        let port_clone = port.clone();
        let local_forwards = config.local_forwards.clone();
        let remote_forwards = config.remote_forwards.clone();
        
        // Use the persistent runtime to test the connection
        match self.runtime.block_on(async {
            ssh_impl::connect_ssh(&host, port, &username, auth_type, &local_forwards, &remote_forwards).await
        }) {
            Ok(_ssh_connection) => {
                // Connection test successful
                info!("Connection test successful: {}:{}", host_clone, port_clone);
                Ok(())
            },
            Err(e) => {
                // SSH connection error
                error!("Connection test failed: {:?}", e);
                Err(ConnectionError::ConnectionFailed(format!("{:?}", e)))
            }
        }
    }

    fn start_terminal(&mut self, app: &tauri::AppHandle, session_id: &Uuid, width: u16, height: u16) -> Result<bool, ConnectionError> {
        // Check if session exists
        let session = self.sessions.get(session_id).ok_or(ConnectionError::SessionNotFound(*session_id))?;

        // Check if session is connected
        if session.status != ConnectionStatus::Connected {
            return Err(ConnectionError::ConnectionFailed("Session is not connected".to_string()));
        }

        // Get SSH connection
        let ssh_connection = self.ssh_connections.get(session_id).ok_or(ConnectionError::SessionNotFound(*session_id))?;

        // Check if SSH connection is still valid
        debug!("Skipping synchronous health check for session: {}", session_id);

        info!("Starting terminal for session: {}", session_id);

        // Create terminal ID
        let terminal_id = Uuid::new_v4();
        
        // Create command channel
        let (tx, mut rx) = mpsc::channel::<TerminalCommand>(32);

        let session_id_clone = *session_id;
        let app_clone = app.clone();
        let ssh_handle_clone = Arc::clone(&ssh_connection.handle);
        
        // Tracker clone
        let tracker_clone = Arc::clone(&self.tracker);
        
        // Register session in tracker
        tracker_clone.lock().unwrap().register_session(session_id_clone);

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
                    let mut exit_reason = "unknown";
                    
                    loop {
                        tokio::select! {
                            // Handle incoming SSH data
                            msg = channel.wait() => {
                                match msg {
                                    Some(russh::ChannelMsg::Data { ref data }) => {
                                        // Update last activity time
                                        last_activity = tokio::time::Instant::now();
                                        
                                        // Log received data
                                        tracker_clone.lock().unwrap().log_data(session_id_clone, data, "received");
                                        
                                        let data_str = String::from_utf8_lossy(data).to_string();
                                        let event_name = format!("terminal-output-{}", session_id_clone);
                                        let _ = app_clone.emit(&event_name, serde_json::json!({ "data": data_str }));
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
        self.terminals.insert(terminal_id, TerminalSession {
            id: terminal_id,
            session_id: *session_id,
            sender: tx,
        });

        // Update session info
        if let Some(session) = self.sessions.get_mut(session_id) {
            session.terminal_id = Some(terminal_id);
        }

        Ok(true)
    }

    fn send_terminal_data(&mut self, session_id: &Uuid, data: &str) -> Result<(), ConnectionError> {
        // Find terminal for this session
        let terminal = self.terminals.values()
            .find(|t| &t.session_id == session_id)
            .ok_or_else(|| ConnectionError::SessionNotFound(*session_id))?;

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
        match sender.blocking_send(TerminalCommand::Data(data_bytes)) {
            Ok(_) => Ok(()),
            Err(e) => {
                error!("Failed to send terminal data: {}", e);
                Err(ConnectionError::ConnectionFailed(format!("Failed to send data: {}", e)))
            }
        }
    }

    fn resize_terminal(&mut self, session_id: &Uuid, width: u16, height: u16) -> Result<(), ConnectionError> {
        // Find terminal for this session
        let terminal = self.terminals.values()
            .find(|t| &t.session_id == session_id)
            .ok_or_else(|| ConnectionError::SessionNotFound(*session_id))?;

        // Send resize command
        let sender = terminal.sender.clone();
        let _ = sender.blocking_send(TerminalCommand::Resize(width as u32, height as u32));

        debug!("Resizing terminal to {}x{}", width, height);
        Ok(())
    }

    fn close_terminal(&mut self, session_id: &Uuid) -> Result<(), ConnectionError> {
        // Find and remove terminal for this session
        let terminal_id = self.terminals.values()
            .find(|t| &t.session_id == session_id)
            .map(|t| t.id)
            .ok_or_else(|| ConnectionError::SessionNotFound(*session_id))?;

        let terminal = self.terminals.remove(&terminal_id)
            .ok_or_else(|| ConnectionError::SessionNotFound(*session_id))?;

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

    fn exec_command(&mut self, session_id: &Uuid, command: &str) -> Result<String, ConnectionError> {
        let ssh_connection = self.ssh_connections.get(session_id)
            .ok_or_else(|| ConnectionError::SessionNotFound(*session_id))?
            .clone();

        let command = command.to_string();
        
        // Execute command in a blocking way using runtime
        let result = self.runtime.block_on(async move {
            let handle = ssh_connection.handle.lock().await;
            let mut channel = handle.channel_open_session().await
                .map_err(|e| ConnectionError::SshError(format!("Failed to open channel: {:?}", e)))?;
            
            channel.exec(true, command.as_bytes().to_vec()).await
                .map_err(|e| ConnectionError::SshError(format!("Failed to execute command: {:?}", e)))?;
            
            let mut output = String::new();
            while let Some(msg) = channel.wait().await {
                match msg {
                    russh::ChannelMsg::Data { ref data } => {
                        output.push_str(&String::from_utf8_lossy(data));
                    },
                    russh::ChannelMsg::ExtendedData { ref data, .. } => {
                        output.push_str(&String::from_utf8_lossy(data));
                    },
                    russh::ChannelMsg::Eof => {
                        break;
                    },
                    _ => {}
                }
            }
            channel.close().await.ok();
            Ok(output)
        });

        result
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