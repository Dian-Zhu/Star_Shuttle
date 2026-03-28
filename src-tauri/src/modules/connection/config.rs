use crate::modules::connection::ConnectionError;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

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

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub enum ProxyType {
    #[default]
    None,
    Socks5 {
        host: String,
        port: u16,
        username: Option<String>,
        password: Option<String>,
        #[serde(default)]
        has_password: bool,
    },
    Http {
        host: String,
        port: u16,
        username: Option<String>,
        password: Option<String>,
        #[serde(default)]
        has_password: bool,
    },
    JumpHost {
        host: String,
        port: u16,
        username: String,
        auth_method: AuthMethod,
    },
}

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
    pub socks_proxy_port: Option<u16>,
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
            host: String::new(),
            port: 0,
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
