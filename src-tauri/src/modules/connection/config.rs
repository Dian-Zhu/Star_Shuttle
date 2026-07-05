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

#[derive(Clone, Serialize, Deserialize)]
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

#[derive(Clone, Serialize, Deserialize, Default)]
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

// 手写 Debug，对口令/密码等秘密字段脱敏。
//
// 这些结构会随连接流程在多处被 `{:?}` 记入日志（tauri-plugin-log 落盘）。
// 派生 Debug 会把明文密码、私钥口令、证书口令、代理密码原样打印到日志，
// 造成凭证泄露。此处只输出「是否存在」而非内容。
impl std::fmt::Debug for AuthMethod {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AuthMethod::Password { save_password, .. } => f
                .debug_struct("Password")
                .field("password", &"***")
                .field("save_password", save_password)
                .finish(),
            AuthMethod::KeyboardInteractive {} => f.debug_struct("KeyboardInteractive").finish(),
            AuthMethod::PrivateKey {
                key_path,
                passphrase,
                save_passphrase,
            } => f
                .debug_struct("PrivateKey")
                .field("key_path", key_path)
                .field("passphrase", &passphrase.as_ref().map(|_| "***"))
                .field("save_passphrase", save_passphrase)
                .finish(),
            AuthMethod::Agent { agent_path } => {
                f.debug_struct("Agent").field("agent_path", agent_path).finish()
            }
            AuthMethod::Certificate {
                certificate_path,
                private_key_path,
                passphrase,
                save_passphrase,
            } => f
                .debug_struct("Certificate")
                .field("certificate_path", certificate_path)
                .field("private_key_path", private_key_path)
                .field("passphrase", &passphrase.as_ref().map(|_| "***"))
                .field("save_passphrase", save_passphrase)
                .finish(),
        }
    }
}

impl std::fmt::Debug for ProxyType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProxyType::None => f.write_str("None"),
            ProxyType::Socks5 {
                host,
                port,
                username,
                password,
                has_password,
            } => f
                .debug_struct("Socks5")
                .field("host", host)
                .field("port", port)
                .field("username", username)
                .field("password", &password.as_ref().map(|_| "***"))
                .field("has_password", has_password)
                .finish(),
            ProxyType::Http {
                host,
                port,
                username,
                password,
                has_password,
            } => f
                .debug_struct("Http")
                .field("host", host)
                .field("port", port)
                .field("username", username)
                .field("password", &password.as_ref().map(|_| "***"))
                .field("has_password", has_password)
                .finish(),
            ProxyType::JumpHost {
                host,
                port,
                username,
                auth_method,
            } => f
                .debug_struct("JumpHost")
                .field("host", host)
                .field("port", port)
                .field("username", username)
                .field("auth_method", auth_method)
                .finish(),
        }
    }
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
    const MAX_HOST_LENGTH: usize = 255;
    const MAX_USERNAME_LENGTH: usize = 128;

    pub fn validate(&self) -> Result<(), ConnectionError> {
        if self.host.len() > Self::MAX_HOST_LENGTH {
            return Err(ConnectionError::InvalidConfig(
                "Host exceeds maximum length".to_string(),
            ));
        }
        if self.host.contains('\0') {
            return Err(ConnectionError::InvalidConfig(
                "Host contains invalid characters".to_string(),
            ));
        }
        if self.username.len() > Self::MAX_USERNAME_LENGTH {
            return Err(ConnectionError::InvalidConfig(
                "Username exceeds maximum length".to_string(),
            ));
        }
        if self.username.contains('\0') {
            return Err(ConnectionError::InvalidConfig(
                "Username contains invalid characters".to_string(),
            ));
        }

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

#[cfg(test)]
mod redaction_tests {
    use super::*;

    #[test]
    fn auth_method_debug_redacts_password() {
        let auth = AuthMethod::Password {
            password: "s3cr3t-pw".to_string(),
            save_password: true,
        };
        let dbg = format!("{:?}", auth);
        assert!(!dbg.contains("s3cr3t-pw"), "password leaked: {dbg}");
        assert!(dbg.contains("***"));
        assert!(dbg.contains("save_password: true"));
    }

    #[test]
    fn auth_method_debug_redacts_private_key_and_cert_passphrase() {
        let key = AuthMethod::PrivateKey {
            key_path: "/home/u/.ssh/id_ed25519".to_string(),
            passphrase: Some("key-pass".to_string()),
            save_passphrase: true,
        };
        let dbg = format!("{:?}", key);
        assert!(!dbg.contains("key-pass"), "passphrase leaked: {dbg}");
        assert!(dbg.contains("/home/u/.ssh/id_ed25519"), "key_path should remain");

        let cert = AuthMethod::Certificate {
            certificate_path: "/c/cert.pub".to_string(),
            private_key_path: "/c/key".to_string(),
            passphrase: Some("cert-pass".to_string()),
            save_passphrase: false,
        };
        let dbg = format!("{:?}", cert);
        assert!(!dbg.contains("cert-pass"), "cert passphrase leaked: {dbg}");
    }

    #[test]
    fn proxy_type_debug_redacts_password_but_keeps_host() {
        let proxy = ProxyType::Socks5 {
            host: "proxy.example".to_string(),
            port: 1080,
            username: Some("user".to_string()),
            password: Some("proxy-pw".to_string()),
            has_password: true,
        };
        let dbg = format!("{:?}", proxy);
        assert!(!dbg.contains("proxy-pw"), "proxy password leaked: {dbg}");
        assert!(dbg.contains("proxy.example"));
        assert!(dbg.contains("has_password: true"));
    }

    #[test]
    fn connection_config_debug_redacts_nested_secrets() {
        let config = ConnectionConfig {
            id: Uuid::nil(),
            name: "test".to_string(),
            protocol: ConnectionProtocol::Ssh,
            host: "host".to_string(),
            port: 22,
            username: "root".to_string(),
            auth_method: AuthMethod::Password {
                password: "top-secret".to_string(),
                save_password: true,
            },
            description: None,
            tags: vec![],
            created_at: Utc::now(),
            updated_at: Utc::now(),
            group_id: None,
            local_forwards: vec![],
            remote_forwards: vec![],
            proxy_type: ProxyType::None,
            socks_proxy_port: None,
            auto_reconnect: None,
        };
        let dbg = format!("{:?}", config);
        assert!(!dbg.contains("top-secret"), "nested password leaked: {dbg}");
    }
}
