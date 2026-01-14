use serde::{Deserialize, Serialize}; use uuid::Uuid; use chrono::{DateTime, Utc}; use std::collections::HashMap; use thiserror::Error;

// Re-export submodules
pub mod manager; pub mod auth; pub mod error; pub mod ssh_impl;

// Connection status
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
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub group_id: Option<Uuid>,
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
use crate::modules::connection::ssh_impl::connect_ssh;

// Default connection manager implementation
#[derive(Default)]
pub struct DefaultConnectionManager {
    connections: HashMap<Uuid, ConnectionConfig>,
    sessions: HashMap<Uuid, SessionInfo>,
    ssh_handles: HashMap<Uuid, Arc<Mutex<russh::client::Handle<ssh_impl::SshHandler>>>>,
}

// Manual Debug implementation to handle the non-Debug ssh_handles field
impl std::fmt::Debug for DefaultConnectionManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DefaultConnectionManager")
            .field("connections", &self.connections)
            .field("sessions", &self.sessions)
            .field("ssh_handles_count", &self.ssh_handles.len())
            .finish()
    }
}

impl DefaultConnectionManager {
    pub fn new() -> Self {
        Default::default()
    }
}

impl ConnectionManager for DefaultConnectionManager {
    fn connect(&mut self, config: &ConnectionConfig) -> Result<Uuid, ConnectionError> {
        // Log connection attempt start
        println!("Starting connection attempt for config: {:?}", config.id);
        
        // 新增：验证配置合法性，确保不使用默认空配置
        println!("Validating connection configuration...");
        config.validate()?;
        println!("Connection configuration validated successfully");
        
        // Create a new session ID
        let session_id = Uuid::new_v4();
        println!("Created new session ID: {}", session_id);
        
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
        println!("Stored session info with status: Connecting");
        
        // Clone necessary fields from config to move into thread
        let host = config.host.clone();
        let port = config.port;
        let username = config.username.clone();
        
        // Get password from auth method if using password authentication
        let password = match &config.auth_method {
            AuthMethod::Password { password, .. } => Some(password.clone()),
            _ => None,
        };
        
        // Log connection details (excluding sensitive info)
        println!("Attempting to connect to {}:{} as {}", host, port, username);
        
        // Use the real SSH connection function from ssh_impl.rs
        // Note: This is a blocking call, but in a real application, we would spawn a separate thread or use async/await
        match std::thread::spawn(move || {
            let runtime = tokio::runtime::Runtime::new().unwrap();
            runtime.block_on(async {
                ssh_impl::connect_ssh(&host, port, &username, password).await
            })
        }).join() {
            Ok(Ok(handle)) => {
                // Connection successful
                println!("Connection successful for session: {}", session_id);
                session_info.status = ConnectionStatus::Connected;
                self.sessions.insert(session_id, session_info);
                self.ssh_handles.insert(session_id, handle);
                Ok(session_id)
            },
            Ok(Err(e)) => {
                // SSH connection error
                println!("SSH connection error for session {}: {:?}", session_id, e);
                session_info.status = ConnectionStatus::Error;
                self.sessions.insert(session_id, session_info);
                Err(ConnectionError::ConnectionFailed(format!("{:?}", e)))
            },
            Err(e) => {
                // Thread join error
                println!("Thread join error for session {}: {:?}", session_id, e);
                session_info.status = ConnectionStatus::Error;
                self.sessions.insert(session_id, session_info);
                Err(ConnectionError::ConnectionFailed(format!("{:?}", e)))
            },
        }
    }

    fn disconnect(&mut self, session_id: &Uuid) -> Result<(), ConnectionError> {
        let session = self.sessions.get_mut(session_id).ok_or(ConnectionError::SessionNotFound(*session_id))?;
        
        session.status = ConnectionStatus::Disconnecting;
        
        // TODO: Implement actual SSH disconnection logic
        // For now, just simulate disconnection
        session.status = ConnectionStatus::Disconnected;
        
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
        // Validate the connection configuration before saving
        config.validate()?;
        
        // If the connection doesn't have an ID, generate a new one
        let id = if config.id == Uuid::nil() {
            let new_id = Uuid::new_v4();
            config.id = new_id;
            new_id
        } else {
            config.id
        };
        
        // Update timestamps
        let now = Utc::now();
        if config.created_at == DateTime::UNIX_EPOCH {
            config.created_at = now;
        }
        config.updated_at = now;
        
        self.connections.insert(id, config);
        Ok(())
    }

    fn delete_connection_config(&mut self, connection_id: &Uuid) -> Result<(), ConnectionError> {
        if self.connections.remove(connection_id).is_some() {
            Ok(())
        } else {
            Err(ConnectionError::ConnectionNotFound(*connection_id))
        }
    }

    fn get_all_connection_configs(&self) -> Vec<ConnectionConfig> {
        self.connections.values().cloned().collect()
    }

    fn test_connection(&self, config: &ConnectionConfig) -> Result<(), ConnectionError> {
        // 新增：验证配置合法性
        config.validate()?;
        
        // TODO: Implement actual connection testing logic
        // For now, just simulate a successful test
        Ok(())
    }
}