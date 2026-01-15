use russh::client::{Config, Handle, Handler}; use std::sync::{Arc, Mutex}; use std::net::SocketAddr; use std::str::FromStr; use tokio::net::lookup_host; use anyhow::anyhow; use super::known_hosts::KnownHostsManager; use log::{info, debug, error};
use russh_keys::load_secret_key; // 新增导入

#[derive(Clone)]
pub enum AuthType {
    Password(Option<String>),
    PrivateKey(String, Option<String>),
    Agent(Option<String>),
    Certificate(String, String, Option<String>),
}

pub struct SshConnection {
    pub handle: Arc<Mutex<Handle<SshHandler>>>,
    // 移除session_task字段，russh的Handle内部已经管理会话任务
}

pub struct SshHandler {
    username: String,
    auth_type: AuthType,
    auth_complete: bool,
    known_hosts_manager: Option<KnownHostsManager>,
    host: String,
    port: u16,
}

impl Handler for SshHandler {
    type Error = anyhow::Error;

    // Server key validation
    fn check_server_key<'life0, 'async_trait>(
        mut self,
        server_public_key: &'life0 russh_keys::key::PublicKey,
    ) -> ::core::pin::Pin<Box<dyn std::future::Future<Output = Result<(Self, bool), Self::Error>> + Send + 'async_trait>>
    where
        'life0: 'async_trait,
        Self: Send + 'async_trait,
    {
        Box::pin(async move {
            // Calculate and log the key fingerprint
            let fingerprint = server_public_key.fingerprint();
            println!("Server key fingerprint: {}", fingerprint);
            
            // Get key algorithm
            let algorithm = match server_public_key {
                russh_keys::key::PublicKey::Ed25519(_) => "Ed25519",
                _ => "Unknown",
            };
            println!("Server key algorithm: {}", algorithm);
            
            // Validate server key using known_hosts manager
            let mut accept_key = false;
            
            if let Some(known_hosts) = &mut self.known_hosts_manager {
                match known_hosts.check_host_key(&self.host, self.port, server_public_key) {
                    Ok(is_known) => {
                        if is_known {
                            println!("Host key is known and valid");
                            accept_key = true;
                        } else {
                            println!("Host key is not known, accepting for development purposes");
                            // For development purposes, we'll accept new keys automatically
                            // In production, we should prompt the user
                            if let Err(e) = known_hosts.add_host_key(&self.host, self.port, server_public_key) {
                                println!("Failed to add host key to known_hosts: {:?}", e);
                            }
                            accept_key = true;
                        }
                    },
                    Err(e) => {
                        println!("Error checking host key: {:?}", e);
                        // For development purposes, we'll accept keys even if there's an error
                        accept_key = true;
                    }
                }
            } else {
                // No known_hosts manager, accept for development purposes
                println!("No known_hosts manager, accepting key for development purposes");
                accept_key = true;
            }
            
            Ok((self, accept_key))
        })
    }



    fn auth_banner<'life0, 'async_trait>(
        self,
        banner: &'life0 str,
        session: russh::client::Session,
    ) -> ::core::pin::Pin<Box<dyn std::future::Future<Output = Result<(Self, russh::client::Session), Self::Error>> + Send + 'async_trait>>
    where
        'life0: 'async_trait,
        Self: Send + 'async_trait,
    {
        Box::pin(async move {
            println!("SSH Banner: {}", banner);
            Ok((self, session))
        })
    }

    fn channel_open_confirmation<'async_trait>(
        self,
        id: russh::ChannelId,
        max_packet_size: u32,
        window_size: u32,
        session: russh::client::Session,
    ) -> ::core::pin::Pin<Box<dyn std::future::Future<Output = Result<(Self, russh::client::Session), Self::Error>> + Send + 'async_trait>>
    where
        Self: Send + 'async_trait,
    {
        Box::pin(async move {
            println!("Channel open confirmation: id={}, max_packet_size={}, window_size={}", id, max_packet_size, window_size);
            Ok((self, session))
        })
    }

    fn data<'life0, 'async_trait>(
        self,
        channel: russh::ChannelId,
        data: &'life0 [u8],
        session: russh::client::Session,
    ) -> ::core::pin::Pin<Box<dyn std::future::Future<Output = Result<(Self, russh::client::Session), Self::Error>> + Send + 'async_trait>>
    where
        'life0: 'async_trait,
        Self: Send + 'async_trait,
    {
        Box::pin(async move {
            println!("Received data on channel {}: {:?}", channel, String::from_utf8_lossy(data));
            Ok((self, session))
        })
    }
}

pub async fn connect_ssh(
    host: &str,
    port: u16,
    username: &str,
    auth_type: AuthType,
) -> Result<SshConnection, anyhow::Error> {
    info!("Starting SSH connection to {}:{} as {}", host, port, username);
    
    // Validate input parameters
    if host.is_empty() {
        error!("Host is required");
        return Err(anyhow!("Host is required"));
    }
    
    if username.is_empty() {
        error!("Username is required");
        return Err(anyhow!("Username is required"));
    }
    
    if port < 1 || port > 65535 {
        error!("Port must be between 1 and 65535, got {}", port);
        return Err(anyhow!("Port must be between 1 and 65535, got {}", port));
    }
    
    // Log authentication type
    match &auth_type {
        AuthType::Password(_) => debug!("Using password authentication"),
        AuthType::PrivateKey(key_path, _) => debug!("Using private key authentication with key: {}", key_path),
        AuthType::Agent(agent_path) => debug!("Using SSH agent authentication with agent path: {:?}", agent_path),
        AuthType::Certificate(cert_path, key_path, _) => debug!("Using certificate authentication with cert: {} and key: {}", cert_path, key_path),
    }
    
    // Create SSH client config
    let config = Arc::new(Config::default());
    debug!("Created SSH client config");

    // Create known hosts manager
    let known_hosts_manager = KnownHostsManager::new().ok();
    debug!("Created known hosts manager: {:?}", known_hosts_manager.is_some());
    
    // Create handler
    let handler = SshHandler {
        username: username.to_string(),
        auth_type: auth_type.clone(), // 克隆以备后用
        auth_complete: false,
        known_hosts_manager,
        host: host.to_string(),
        port,
    };
    debug!("Created SSH handler");

    // Parse socket address or resolve hostname
    let addr = {
        // Try to parse directly as IP address
        if let Ok(addr) = SocketAddr::from_str(&format!("{}:{}", host, port)) {
            debug!("Directly parsed IP address: {:?}", addr);
            addr
        } else {
            // Resolve hostname to IP address
            debug!("Resolving hostname: {}", host);
            let addrs = lookup_host((host, port)).await
                .map_err(|e| {
                    error!("Failed to resolve hostname {}: {}", host, e);
                    anyhow!("Failed to resolve hostname {}: {}", host, e)
                })?;
            
            let addr = addrs.into_iter().next()
                .ok_or_else(|| {
                    error!("Failed to resolve host: {}, no addresses found", host);
                    anyhow!("Failed to resolve host: {}, no addresses found", host)
                })?;
            
            debug!("Resolved hostname {} to IP address: {:?}", host, addr);
            addr
        }
    };

    // Connect to SSH server
    debug!("Attempting to connect to {}:{} (resolved to {:?}) as {}", host, port, addr, username);
    let mut handle = russh::client::connect(config, addr, handler).await
        .map_err(|e| {
            error!("Failed to establish SSH connection to {}:{}: {}", host, port, e);
            anyhow!("Failed to establish SSH connection to {}:{}: {}", host, port, e)
        })?;

    info!("SSH TCP connection established to {}:{}", host, port);
    
    // 执行身份验证
    match &auth_type {
        AuthType::Password(Some(password)) => {
            debug!("Attempting password authentication for user: {}", username);
            
            // 添加详细的调试信息
            debug!("Password length: {}", password.len());
            debug!("Host: {}:{}", host, port);
            
            // 先尝试none认证来获取服务器支持的认证方法
            debug!("First attempting none authentication to check server capabilities");
            let none_result = handle.authenticate_none(username).await
                .map_err(|e| {
                    debug!("None authentication failed (expected): {:?}", e);
                    e
                }).ok();
            
            if let Some(false) = none_result {
                debug!("Server rejected none authentication (expected)");
            }
            
            // 尝试密码身份验证
            debug!("Now attempting password authentication");
            let auth_result = handle.authenticate_password(username, password).await
                .map_err(|e| {
                    error!("Password authentication failed with error: {:?}", e);
                    anyhow!("Password authentication failed: {:?}", e)
                })?;
            
            if !auth_result {
                error!("Password authentication rejected by server for user: {}@{}:{}", username, host, port);
                
                // 提供更具体的错误信息
                let error_msg = format!(
                    "密码认证被服务器拒绝。请检查：\n1. 用户名和密码是否正确\n2. 服务器是否启用了密码认证\n3. 用户账户是否被锁定或禁用\n服务器: {}:{}, 用户: {}",
                    host, port, username
                );
                
                return Err(anyhow!(error_msg));
            }
            
            info!("Password authentication successful for user: {}@{}:{}", username, host, port);
        }
        AuthType::PrivateKey(key_path, passphrase) => {
            debug!("Attempting public key authentication with key: {}", key_path);
            
            // 加载私钥
            let key = load_secret_key(key_path, passphrase.as_deref())
                .map_err(|e| {
                    error!("Failed to load private key from {}: {}", key_path, e);
                    anyhow!("Failed to load private key: {}", e)
                })?;
            
            let key_pair = Arc::new(key);
            
            // 尝试身份验证
            let auth_result = handle.authenticate_publickey(username, key_pair).await
                .map_err(|e| {
                    error!("Public key authentication failed with error: {:?}", e);
                    anyhow!("Public key authentication failed: {:?}", e)
                })?;
            
            if !auth_result {
                error!("Public key authentication rejected by server for user: {}", username);
                return Err(anyhow!("Public key authentication rejected by server. Please check key file and permissions."));
            }
            
            info!("Public key authentication successful for user: {}", username);
        }
        AuthType::Agent(agent_path) => {
            // TODO: 实现SSH代理身份验证
            error!("SSH agent authentication not yet implemented");
            return Err(anyhow!("SSH agent authentication not yet implemented"));
        }
        AuthType::Certificate(cert_path, key_path, passphrase) => {
            // TODO: 实现证书身份验证
            error!("Certificate authentication not yet implemented");
            return Err(anyhow!("Certificate authentication not yet implemented"));
        }
        AuthType::Password(None) => {
            error!("Password authentication requested but no password provided");
            return Err(anyhow!("Password authentication requested but no password provided"));
        }
    }

    info!("SSH connection fully established and authenticated to {}:{} as {}", host, port, username);
    
    // 创建SSH连接
    Ok(SshConnection {
        handle: Arc::new(Mutex::new(handle)),
    })
}
