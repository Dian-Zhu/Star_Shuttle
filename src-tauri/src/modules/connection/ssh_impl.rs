use russh::client::{Handle, Handler}; use std::sync::Arc; use tokio::sync::Mutex; use tokio::net::lookup_host; use anyhow::anyhow; use super::known_hosts::KnownHostsManager; use log::{info, debug, error, warn};
use russh_keys::load_secret_key;
use tokio::net::{TcpListener, TcpStream};
use std::collections::HashMap;
use async_trait::async_trait;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

#[derive(Clone)]
pub enum AuthType {
    Password(Option<String>),
    PrivateKey(String, Option<String>),
    Agent(Option<String>),
    Certificate(String, String, Option<String>),
}

#[derive(Clone)]
pub struct SshConnection {
    pub handle: Arc<Mutex<Handle<SshHandler>>>,
}

#[derive(Clone)]
pub struct SshHandler {
    #[allow(dead_code)]
    username: String,
    #[allow(dead_code)]
    auth_type: AuthType,
    #[allow(dead_code)]
    auth_complete: bool,
    known_hosts_manager: Option<KnownHostsManager>,
    host: String,
    port: u16,
    // Map remote_port -> (local_host, local_port) for remote forwarding
    remote_forward_mappings: HashMap<u16, (String, u16)>,
}

#[async_trait]
impl Handler for SshHandler {
    type Error = anyhow::Error;

    // Server key validation
    async fn check_server_key(
        mut self,
        server_public_key: &russh_keys::key::PublicKey,
    ) -> Result<(Self, bool), Self::Error> {
        // Calculate and log the key fingerprint
        let fingerprint = server_public_key.fingerprint();
        info!("Server key fingerprint: {}", fingerprint);
        
        // Get key algorithm
        let algorithm = match server_public_key {
            russh_keys::key::PublicKey::Ed25519(_) => "Ed25519",
            _ => "Unknown",
        };
        info!("Server key algorithm: {}", algorithm);
        
        // Validate server key using known_hosts manager
        
        if let Some(known_hosts) = &mut self.known_hosts_manager {
            match known_hosts.check_host_key(&self.host, self.port, server_public_key) {
                Ok(is_known) => {
                    if is_known {
                        info!("Host key is known and valid");
                    } else {
                        info!("Host key is not known, accepting for development purposes");
                        // For development purposes, we'll accept new keys automatically
                        // In production, we should prompt the user
                        if let Err(e) = known_hosts.add_host_key(&self.host, self.port, server_public_key) {
                            error!("Failed to add host key to known_hosts: {:?}", e);
                        }
                    }
                },
                Err(e) => {
                    error!("Error checking host key: {:?}", e);
                    // For development purposes, we'll accept keys even if there's an error
                }
            }
        } else {
            // No known_hosts manager, accept for development purposes
            info!("No known_hosts manager, accepting key for development purposes");
        }
        
        Ok((self, true))
    }

    async fn server_channel_open_forwarded_tcpip(
        self,
        channel: russh::Channel<russh::client::Msg>,
        connected_address: &str,
        connected_port: u32,
        originator_address: &str,
        originator_port: u32,
        session: russh::client::Session,
    ) -> Result<(Self, russh::client::Session), Self::Error> {
        info!(
            "Remote forwarded connection from {}:{} to {}:{}",
            originator_address, originator_port, connected_address, connected_port
        );
        
        // Check if we have a mapping for this port
        if let Some((local_host, local_port)) = self.remote_forward_mappings.get(&(connected_port as u16)) {
            let local_host = local_host.clone();
            let local_port = *local_port;
            
            info!("Forwarding remote connection on port {} to local {}:{}", connected_port, local_host, local_port);
            
            // Connect to local target
            match TcpStream::connect(format!("{}:{}", local_host, local_port)).await {
                Ok(mut socket) => {
                     let mut channel = channel;
                     // Spawn proxy task
                     tokio::spawn(async move {
                         let mut buf = vec![0u8; 8192];
                         loop {
                             tokio::select! {
                                 // Read from channel (remote) -> Write to socket (local)
                                 msg = channel.wait() => {
                                     match msg {
                                         Some(russh::ChannelMsg::Data { data }) => {
                                             if let Err(e) = socket.write_all(&data).await {
                                                 debug!("Failed to write to local socket: {:?}", e);
                                                 break;
                                             }
                                         }
                                         Some(russh::ChannelMsg::Eof) | None => {
                                             break;
                                         }
                                         _ => {} // Ignore other messages
                                     }
                                 }
                                 // Read from socket (local) -> Write to channel (remote)
                                 n = socket.read(&mut buf) => {
                                     match n {
                                         Ok(n) if n > 0 => {
                                             if let Err(e) = channel.data(&buf[..n]).await {
                                                 debug!("Failed to write to remote channel: {:?}", e);
                                                 break;
                                             }
                                         }
                                         _ => break, // EOF or Error
                                     }
                                 }
                             }
                         }
                         channel.close().await.ok();
                     });
                },
                Err(e) => {
                    error!("Failed to connect to local target {}:{}: {}", local_host, local_port, e);
                    channel.close().await.ok();
                }
            }
        } else {
            error!("No forwarding mapping found for remote port {}", connected_port);
            channel.close().await.ok();
        }
        
        Ok((self, session))
    }

    async fn auth_banner(
        self,
        banner: &str,
        session: russh::client::Session,
    ) -> Result<(Self, russh::client::Session), Self::Error> {
        info!("SSH Banner: {}", banner);
        Ok((self, session))
    }

    async fn channel_open_confirmation(
        self,
        id: russh::ChannelId,
        max_packet_size: u32,
        window_size: u32,
        session: russh::client::Session,
    ) -> Result<(Self, russh::client::Session), Self::Error> {
        debug!("Channel open confirmation: id={}, max_packet_size={}, window_size={}", id, max_packet_size, window_size);
        Ok((self, session))
    }

    async fn data(
        self,
        channel: russh::ChannelId,
        data: &[u8],
        session: russh::client::Session,
    ) -> Result<(Self, russh::client::Session), Self::Error> {
        debug!("Received data on channel {}: {:?}", channel, String::from_utf8_lossy(data));
        Ok((self, session))
    }
}

impl SshConnection {
    pub async fn setup_port_forwarding(
        &self,
        local_forwards: &Vec<super::LocalForward>,
        remote_forwards: &Vec<super::RemoteForward>,
    ) -> Result<(), anyhow::Error> {
        // Clone the Arc containing the Handle for local forwarding tasks
        let handle_arc = self.handle.clone();

        // Setup local forwards (client -> server -> remote)
        for forward in local_forwards {
            let local_host = forward.local_host.clone();
            let local_port = forward.local_port;
            let remote_host = forward.remote_host.clone();
            let remote_port = forward.remote_port;
            let handle_arc = handle_arc.clone();

            info!("Setting up local port forwarding: {}:{} -> {}:{}", local_host, local_port, remote_host, remote_port);
            
            tokio::spawn(async move {
                let listener = match TcpListener::bind(format!("{}:{}", local_host, local_port)).await {
                    Ok(l) => l,
                    Err(e) => {
                        error!("Failed to bind local port {}: {}", local_port, e);
                        return;
                    }
                };

                info!("Listening on {}:{} for forwarding to {}:{}", local_host, local_port, remote_host, remote_port);

                loop {
                    match listener.accept().await {
                        Ok((mut socket, addr)) => {
                            debug!("Accepted connection from {:?} for forwarding", addr);
                            let handle_arc = handle_arc.clone();
                            let remote_host = remote_host.clone();
                            let remote_port = remote_port;

                            tokio::spawn(async move {
                                // Lock the mutex to get access to the Handle
                                let channel_result = {
                                    let guard = handle_arc.lock().await;
                                    guard.channel_open_direct_tcpip(
                                        &remote_host,
                                        remote_port as u32,
                                        "127.0.0.1",
                                        0,
                                    ).await
                                };
                                
                                match channel_result {
                                    Ok(mut channel) => {
                                         // Use manual proxy loop instead of copy_bidirectional
                                         let mut buf = vec![0u8; 8192];
                                         loop {
                                             tokio::select! {
                                                 // Read from channel (remote) -> Write to socket (local)
                                                 msg = channel.wait() => {
                                                     match msg {
                                                         Some(russh::ChannelMsg::Data { data }) => {
                                                             if let Err(e) = socket.write_all(&data).await {
                                                                 debug!("Failed to write to local socket: {:?}", e);
                                                                 break;
                                                             }
                                                         }
                                                         Some(russh::ChannelMsg::Eof) | None => {
                                                             break;
                                                         }
                                                         _ => {} // Ignore other messages
                                                     }
                                                 }
                                                 // Read from socket (local) -> Write to channel (remote)
                                                 n = socket.read(&mut buf) => {
                                                     match n {
                                                         Ok(n) if n > 0 => {
                                                             if let Err(e) = channel.data(&buf[..n]).await {
                                                                 debug!("Failed to write to remote channel: {:?}", e);
                                                                 break;
                                                             }
                                                         }
                                                         _ => break, // EOF or Error
                                                     }
                                                 }
                                             }
                                         }
                                    },
                                    Err(e) => {
                                        error!("Failed to open direct-tcpip channel: {:?}", e);
                                    }
                                }
                            });
                        },
                        Err(e) => {
                            error!("Failed to accept connection on {}:{}: {}", local_host, local_port, e);
                            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                        }
                    }
                }
            });
        }

        // Setup remote forwards (remote -> server -> client)
        // Lock the mutex to get access to the Handle for remote forwarding
        {
            let mut handle = self.handle.lock().await;
            
            for forward in remote_forwards {
                info!("Requesting remote port forwarding: {}:{} -> {}:{}", forward.remote_host, forward.remote_port, forward.local_host, forward.local_port);
                match handle.tcpip_forward(&forward.remote_host, forward.remote_port as u32).await {
                    Ok(true) => info!("Remote port forwarding request accepted for port {}", forward.remote_port),
                    Ok(false) => error!("Remote port forwarding request rejected for port {}", forward.remote_port),
                    Err(e) => error!("Failed to request remote port forwarding: {:?}", e),
                }
            }
        }

        Ok(())
    }
}

pub async fn connect_ssh(
    host: &str,
    port: u16,
    username: &str,
    auth_type: AuthType,
    local_forwards: &Vec<super::LocalForward>,
    remote_forwards: &Vec<super::RemoteForward>,
    socks_proxy_port: Option<u16>,
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
    
    if port == 0 {
        error!("Port must be between 1 and 65535, got {}", port);
        return Err(anyhow!("Port must be between 1 and 65535, got {}", port));
    }
    
    // Parse address
    let addr_str = format!("{}:{}", host, port);
    let addr = match lookup_host(&addr_str).await {
        Ok(mut addrs) => match addrs.next() {
            Some(addr) => addr,
            None => return Err(anyhow!("Could not resolve host: {}", host)),
        },
        Err(e) => return Err(anyhow!("Could not resolve host {}: {}", host, e)),
    };
    
    // Load config (known_hosts)
    let known_hosts_manager = match KnownHostsManager::new() {
        Ok(manager) => Some(manager),
        Err(e) => {
            error!("Failed to initialize known_hosts manager: {:?}", e);
            None
        }
    };
    
    let config = russh::client::Config {
        inactivity_timeout: Some(std::time::Duration::from_secs(3600)),
        .. <russh::client::Config as Default>::default()
    };
    
    let config = Arc::new(config);
    
    // Initialize handler
    let handler = SshHandler {
        username: username.to_string(),
        auth_type: auth_type.clone(),
        auth_complete: false,
        known_hosts_manager,
        host: host.to_string(),
        port,
        remote_forward_mappings: remote_forwards.iter().map(|f| (f.remote_port, (f.local_host.clone(), f.local_port))).collect(),
    };
    
    // Connect
    info!("Connecting to {}", addr);
    let mut handle = match russh::client::connect(config, addr, handler).await {
        Ok(h) => h,
        Err(e) => return Err(anyhow!("Connection failed: {:?}", e)),
    };
    
    info!("Connected to {}, authenticating...", addr);
    
    // Authentication
    let auth_res = match auth_type {
        AuthType::Password(password) => {
            if let Some(pwd) = password {
                handle.authenticate_password(username, pwd).await
            } else {
                handle.authenticate_none(username).await
            }
        },
        AuthType::PrivateKey(key_path, passphrase) => {
            let key_pair = match load_secret_key(key_path, passphrase.as_deref()) {
                Ok(k) => k,
                Err(e) => return Err(anyhow!("Failed to load private key: {:?}", e)),
            };
            handle.authenticate_publickey(username, Arc::new(key_pair)).await
        },
        AuthType::Agent(_) => {
            return Err(anyhow!("Agent authentication not supported yet"));
        },
        AuthType::Certificate(cert_path, key_path, passphrase) => {
            // Load the private key
            let key_pair = match load_secret_key(&key_path, passphrase.as_deref()) {
                Ok(k) => k,
                Err(e) => return Err(anyhow!("Failed to load private key: {:?}", e)),
            };
            
            // Load certificate (if russh supports certificate authentication)
            // For now, we'll try to authenticate with the key pair
            // Note: russh 0.40.0 may not have explicit certificate support in the public API
            // This is a simplified implementation
            info!("Attempting certificate authentication with key: {} and certificate: {}", key_path, cert_path);
            handle.authenticate_publickey(username, Arc::new(key_pair)).await
        }
    };
    
    match auth_res {
        Ok(true) => {
            info!("Authentication successful");
        },
        Ok(false) => {
            error!("Authentication failed");
            return Err(anyhow!("Authentication failed"));
        },
        Err(e) => {
            error!("Authentication error: {:?}", e);
            return Err(anyhow!("Authentication error: {:?}", e));
        }
    }
    
    // Create connection object
    let connection = SshConnection {
        handle: Arc::new(Mutex::new(handle)),
    };
    
    // Setup port forwarding if configured
    if !local_forwards.is_empty() || !remote_forwards.is_empty() {
        info!("Setting up port forwarding...");
        if let Err(e) = connection.setup_port_forwarding(local_forwards, remote_forwards).await {
            error!("Failed to setup port forwarding: {:?}", e);
            // We don't fail the whole connection if port forwarding fails, just log error
        }
    }

    // Start SOCKS5 proxy if configured
    if let Some(port) = socks_proxy_port {
        info!("Starting SOCKS5 proxy on port {}...", port);
        // TODO: Implement actual SOCKS5 proxy server
        // For now, just log that the feature is recognized
        warn!("SOCKS5 proxy feature recognized but not yet implemented (port: {})", port);
    }

    Ok(connection)
}
