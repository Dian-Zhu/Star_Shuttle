#[path = "ssh_impl_auth.rs"]
mod auth_helpers;
#[path = "ssh_impl_forwarding.rs"]
mod forwarding_helpers;
#[path = "ssh_impl_proxy.rs"]
mod proxy_helpers;

use super::known_hosts::KnownHostsManager;
use anyhow::anyhow;
use async_trait::async_trait;
use log::{debug, error, info};
use russh::client::{Handle, Handler, Prompt};
use russh_keys::PublicKeyBase64;
use serde_json::json;
use std::collections::HashMap;
use std::sync::Arc;
#[cfg(test)]
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::lookup_host;
#[cfg(test)]
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{watch, Mutex};

#[cfg(unix)]
use self::auth_helpers::authenticate_agent;
use self::auth_helpers::{
    authenticate_keyboard_interactive, load_openssh_public_key, load_private_key,
    validate_certificate_paths, KeyPairSigner,
};
use self::forwarding_helpers::{
    forward_remote_connection_to_local, local_forward_target_for_port,
    setup_port_forwarding as setup_port_forwarding_impl,
};
use self::proxy_helpers::start_socks5_proxy;
#[cfg(test)]
use self::proxy_helpers::{http_proxy_connect, socks5_client_handshake};
pub use self::proxy_helpers::{
    start_ephemeral_direct_tcpip_listener, start_ephemeral_http_proxy_dial_listener,
    start_ephemeral_socks5_proxy_dial_listener,
};

#[derive(Clone)]
pub enum AuthType {
    Password(Option<String>),
    PrivateKey(String, Option<String>),
    Agent(Option<String>),
    Certificate(String, String, Option<String>),
    KeyboardInteractive,
}

#[derive(Debug)]
pub struct KeyboardInteractivePromptRequest {
    pub host: String,
    pub port: u16,
    pub username: String,
    pub name: String,
    pub instructions: String,
    pub prompts: Vec<Prompt>,
}

#[async_trait]
pub trait KeyboardInteractivePrompter: Send + Sync {
    async fn prompt(
        &self,
        request: KeyboardInteractivePromptRequest,
    ) -> Result<Vec<String>, anyhow::Error>;
}

#[derive(Clone)]
pub struct SshConnection {
    pub handle: Arc<Mutex<Handle<SshHandler>>>,
    lifecycle: Arc<ConnectionLifecycle>,
}

struct ConnectionLifecycle {
    shutdown_tx: watch::Sender<bool>,
}

impl ConnectionLifecycle {
    fn new() -> Self {
        let (shutdown_tx, _shutdown_rx) = watch::channel(false);
        Self { shutdown_tx }
    }

    fn subscribe(&self) -> watch::Receiver<bool> {
        self.shutdown_tx.subscribe()
    }

    fn shutdown(&self) {
        let _ = self.shutdown_tx.send(true);
    }

    #[cfg(test)]
    fn is_shutdown(&self) -> bool {
        *self.shutdown_tx.borrow()
    }
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

        let key_type = server_public_key.name().to_string();
        let key_base64 = server_public_key.public_key_base64();
        info!("Server key algorithm: {}", key_type);

        // Validate server key using known_hosts manager

        if let Some(known_hosts) = &mut self.known_hosts_manager {
            match known_hosts.check_host_key(&self.host, self.port, server_public_key) {
                Ok(is_known) => {
                    if is_known {
                        info!("Host key is known and valid");
                    } else {
                        return Err(anyhow!(
                            "HOST_KEY_UNKNOWN|{}",
                            json!({
                                "host": self.host,
                                "port": self.port,
                                "fingerprint": fingerprint,
                                "key_type": key_type,
                                "key_base64": key_base64
                            })
                        ));
                    }
                }
                Err(e) => {
                    return Err(anyhow!(
                        "HOST_KEY_MISMATCH|{}",
                        json!({
                            "host": self.host,
                            "port": self.port,
                            "fingerprint": fingerprint,
                            "key_type": key_type,
                            "key_base64": key_base64,
                            "reason": e.to_string()
                        })
                    ));
                }
            }
        } else {
            return Err(anyhow!(
                "HOST_KEY_UNAVAILABLE|{}",
                json!({
                    "host": self.host,
                    "port": self.port,
                    "fingerprint": fingerprint,
                    "key_type": key_type,
                    "key_base64": key_base64
                })
            ));
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

        if let Some((local_host, local_port)) =
            local_forward_target_for_port(&self.remote_forward_mappings, connected_port)
        {
            forward_remote_connection_to_local(channel, connected_port, local_host, local_port)
                .await;
        } else {
            error!(
                "No forwarding mapping found for remote port {}",
                connected_port
            );
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
        debug!(
            "Channel open confirmation: id={}, max_packet_size={}, window_size={}",
            id, max_packet_size, window_size
        );
        Ok((self, session))
    }

    async fn data(
        self,
        channel: russh::ChannelId,
        data: &[u8],
        session: russh::client::Session,
    ) -> Result<(Self, russh::client::Session), Self::Error> {
        debug!(
            "Received data on channel {}: {:?}",
            channel,
            String::from_utf8_lossy(data)
        );
        Ok((self, session))
    }
}

impl SshConnection {
    pub async fn setup_port_forwarding(
        &self,
        local_forwards: &Vec<super::LocalForward>,
        remote_forwards: &Vec<super::RemoteForward>,
    ) -> Result<(), anyhow::Error> {
        setup_port_forwarding_impl(self, local_forwards, remote_forwards).await
    }

    pub fn subscribe_shutdown(&self) -> watch::Receiver<bool> {
        self.lifecycle.subscribe()
    }

    pub fn shutdown_background_tasks(&self) {
        self.lifecycle.shutdown();
    }

    #[cfg(test)]
    pub fn is_shutdown_for_test(&self) -> bool {
        self.lifecycle.is_shutdown()
    }
}

#[allow(clippy::too_many_arguments)]
pub async fn connect_ssh(
    host: &str,
    port: u16,
    username: &str,
    auth_type: AuthType,
    local_forwards: &Vec<super::LocalForward>,
    remote_forwards: &Vec<super::RemoteForward>,
    socks_proxy_port: Option<u16>,
    keyboard_interactive_prompter: Option<Arc<dyn KeyboardInteractivePrompter>>,
) -> Result<SshConnection, anyhow::Error> {
    connect_ssh_with_known_host(
        host,
        port,
        host,
        port,
        username,
        auth_type,
        local_forwards,
        remote_forwards,
        socks_proxy_port,
        keyboard_interactive_prompter,
    )
    .await
}

#[allow(clippy::too_many_arguments)]
pub async fn connect_ssh_with_known_host(
    connect_host: &str,
    connect_port: u16,
    known_hosts_host: &str,
    known_hosts_port: u16,
    username: &str,
    auth_type: AuthType,
    local_forwards: &Vec<super::LocalForward>,
    remote_forwards: &Vec<super::RemoteForward>,
    socks_proxy_port: Option<u16>,
    keyboard_interactive_prompter: Option<Arc<dyn KeyboardInteractivePrompter>>,
) -> Result<SshConnection, anyhow::Error> {
    info!(
        "Starting SSH connection to {}:{} as {}",
        connect_host, connect_port, username
    );

    // Validate input parameters
    if connect_host.is_empty() {
        error!("Host is required");
        return Err(anyhow!("Host is required"));
    }

    if username.is_empty() {
        error!("Username is required");
        return Err(anyhow!("Username is required"));
    }

    if connect_port == 0 {
        error!("Port must be between 1 and 65535, got {}", connect_port);
        return Err(anyhow!(
            "Port must be between 1 and 65535, got {}",
            connect_port
        ));
    }

    // Parse address
    let addr_str = format!("{}:{}", connect_host, connect_port);
    let addr = match lookup_host(&addr_str).await {
        Ok(mut addrs) => match addrs.next() {
            Some(addr) => addr,
            None => return Err(anyhow!("Could not resolve host: {}", connect_host)),
        },
        Err(e) => return Err(anyhow!("Could not resolve host {}: {}", connect_host, e)),
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
        ..<russh::client::Config as Default>::default()
    };

    let config = Arc::new(config);

    // Initialize handler
    let handler = SshHandler {
        username: username.to_string(),
        auth_type: auth_type.clone(),
        auth_complete: false,
        known_hosts_manager,
        host: known_hosts_host.to_string(),
        port: known_hosts_port,
        remote_forward_mappings: remote_forwards
            .iter()
            .map(|f| (f.remote_port, (f.local_host.clone(), f.local_port)))
            .collect(),
    };

    // Connect
    info!("Connecting to {}", addr);
    let mut handle = match russh::client::connect(config, addr, handler).await {
        Ok(h) => h,
        Err(e) => {
            let display = e.to_string();
            if let Some(idx) = display.find("HOST_KEY_") {
                return Err(anyhow!(display[idx..].to_string()));
            }
            let debug = format!("{:?}", e);
            if let Some(idx) = debug.find("HOST_KEY_") {
                return Err(anyhow!(debug[idx..].to_string()));
            }
            return Err(anyhow!("Connection failed: {}", display));
        }
    };

    info!("Connected to {}, authenticating...", addr);

    // Authentication
    let auth_res: Result<bool, anyhow::Error> = match auth_type {
        AuthType::Password(password) => {
            if let Some(pwd) = password {
                handle
                    .authenticate_password(username, pwd)
                    .await
                    .map_err(anyhow::Error::from)
            } else {
                handle
                    .authenticate_none(username)
                    .await
                    .map_err(anyhow::Error::from)
            }
        }
        AuthType::PrivateKey(key_path, passphrase) => {
            let key_pair = load_private_key(&key_path, passphrase.as_deref())?;
            handle
                .authenticate_publickey(username, Arc::new(key_pair))
                .await
                .map_err(anyhow::Error::from)
        }
        AuthType::Agent(agent_path) => {
            #[cfg(unix)]
            {
                authenticate_agent(&mut handle, username, agent_path).await
            }
            #[cfg(not(unix))]
            {
                // Suppress unused variable warning on non-unix platforms
                let _ = agent_path;
                return Err(anyhow!(
                    "Agent authentication not supported on this platform"
                ));
            }
        }
        AuthType::Certificate(cert_path, key_path, passphrase) => {
            validate_certificate_paths(&cert_path, &key_path)?;
            let cert_public_key = load_openssh_public_key(&cert_path)?;

            let key_pair = load_private_key(&key_path, passphrase.as_deref())?;
            let signer = KeyPairSigner {
                key_pair: Arc::new(key_pair),
            };

            info!(
                "Attempting certificate authentication with private key: {} and certificate file: {}",
                key_path,
                cert_path
            );
            let (_, res) = handle
                .authenticate_future(username, cert_public_key, signer)
                .await;
            res
        }
        AuthType::KeyboardInteractive => {
            let Some(prompter) = keyboard_interactive_prompter.clone() else {
                return Err(anyhow!(
                    "Keyboard-interactive authentication requires a prompter"
                ));
            };
            authenticate_keyboard_interactive(
                &mut handle,
                connect_host,
                connect_port,
                username,
                prompter,
            )
            .await
        }
    };

    let authenticated = match auth_res {
        Ok(true) => true,
        Ok(false) => {
            if let Some(prompter) = keyboard_interactive_prompter {
                authenticate_keyboard_interactive(
                    &mut handle,
                    connect_host,
                    connect_port,
                    username,
                    prompter,
                )
                .await?
            } else {
                false
            }
        }
        Err(e) => {
            error!("Authentication error: {:?}", e);
            return Err(anyhow!("Authentication error: {:?}", e));
        }
    };

    if authenticated {
        info!("Authentication successful");
    } else {
        error!("Authentication failed");
        return Err(anyhow!("Authentication failed"));
    }

    // Create connection object
    let connection = SshConnection {
        handle: Arc::new(Mutex::new(handle)),
        lifecycle: Arc::new(ConnectionLifecycle::new()),
    };

    // Setup port forwarding if configured
    if !local_forwards.is_empty() || !remote_forwards.is_empty() {
        info!("Setting up port forwarding...");
        if let Err(e) = connection
            .setup_port_forwarding(local_forwards, remote_forwards)
            .await
        {
            error!("Failed to setup port forwarding: {:?}", e);
            // We don't fail the whole connection if port forwarding fails, just log error
        }
    }

    // Start SOCKS5 proxy if configured
    if let Some(port) = socks_proxy_port {
        info!("Starting SOCKS5 proxy on port {}...", port);
        start_socks5_proxy(
            connection.handle.clone(),
            port,
            connection.subscribe_shutdown(),
        )
        .await?;
    }

    Ok(connection)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_openssh_cert_public_key_files() {
        let mut path = std::env::temp_dir();
        path.push(format!("star-shuttle-test-{}.pub", uuid::Uuid::new_v4()));
        std::fs::write(
            &path,
            "ssh-ed25519-cert-v01@openssh.com AAAA test@example\n",
        )
        .unwrap();
        let res = load_openssh_public_key(path.to_string_lossy().as_ref());
        assert!(res.is_err());
    }

    #[test]
    fn test_http_proxy_connect_with_basic_auth() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
            let port = listener.local_addr().unwrap().port();

            let server = tokio::spawn(async move {
                let (mut stream, _) = listener.accept().await.unwrap();
                let mut buf = Vec::new();
                let mut tmp = [0u8; 128];
                loop {
                    let n = stream.read(&mut tmp).await.unwrap();
                    if n == 0 {
                        break;
                    }
                    buf.extend_from_slice(&tmp[..n]);
                    if buf.windows(4).any(|w| w == b"\r\n\r\n") {
                        break;
                    }
                    if buf.len() > 32 * 1024 {
                        break;
                    }
                }

                let req = String::from_utf8_lossy(&buf);
                assert!(req.starts_with("CONNECT example.com:22 HTTP/1.1\r\n"));
                assert!(req.contains("\r\nHost: example.com:22\r\n"));
                assert!(req.contains("\r\nProxy-Connection: Keep-Alive\r\n"));
                assert!(req.contains("\r\nProxy-Authorization: Basic "));

                stream
                    .write_all(b"HTTP/1.1 200 Connection established\r\n\r\n")
                    .await
                    .unwrap();
            });

            let mut client = TcpStream::connect(("127.0.0.1", port)).await.unwrap();
            http_proxy_connect(&mut client, Some("user"), Some("pass"), "example.com", 22)
                .await
                .unwrap();

            server.await.unwrap();
        });
    }

    #[test]
    fn test_socks5_client_handshake_no_auth() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
            let port = listener.local_addr().unwrap().port();

            let server = tokio::spawn(async move {
                let (mut stream, _) = listener.accept().await.unwrap();

                let mut head = [0u8; 2];
                stream.read_exact(&mut head).await.unwrap();
                assert_eq!(head[0], 0x05);
                let nmethods = head[1] as usize;
                let mut methods = vec![0u8; nmethods];
                stream.read_exact(&mut methods).await.unwrap();
                assert!(methods.contains(&0x00));
                stream.write_all(&[0x05, 0x00]).await.unwrap();

                let mut req_head = [0u8; 4];
                stream.read_exact(&mut req_head).await.unwrap();
                assert_eq!(&req_head, &[0x05, 0x01, 0x00, 0x03]);
                let mut len = [0u8; 1];
                stream.read_exact(&mut len).await.unwrap();
                let mut host = vec![0u8; len[0] as usize];
                stream.read_exact(&mut host).await.unwrap();
                assert_eq!(String::from_utf8(host).unwrap(), "example.com");
                let mut port_buf = [0u8; 2];
                stream.read_exact(&mut port_buf).await.unwrap();
                assert_eq!(u16::from_be_bytes(port_buf), 22);

                stream
                    .write_all(&[0x05, 0x00, 0x00, 0x01, 0, 0, 0, 0, 0, 0])
                    .await
                    .unwrap();
            });

            let mut client = TcpStream::connect(("127.0.0.1", port)).await.unwrap();
            socks5_client_handshake(&mut client, None, None, "example.com", 22)
                .await
                .unwrap();

            server.await.unwrap();
        });
    }

    #[test]
    fn test_socks5_client_handshake_username_password() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
            let port = listener.local_addr().unwrap().port();

            let server = tokio::spawn(async move {
                let (mut stream, _) = listener.accept().await.unwrap();

                let mut head = [0u8; 2];
                stream.read_exact(&mut head).await.unwrap();
                assert_eq!(head[0], 0x05);
                let nmethods = head[1] as usize;
                let mut methods = vec![0u8; nmethods];
                stream.read_exact(&mut methods).await.unwrap();
                assert!(methods.contains(&0x02));
                stream.write_all(&[0x05, 0x02]).await.unwrap();

                let mut auth_ver = [0u8; 2];
                stream.read_exact(&mut auth_ver).await.unwrap();
                assert_eq!(auth_ver[0], 0x01);
                let ulen = auth_ver[1] as usize;
                let mut u = vec![0u8; ulen];
                stream.read_exact(&mut u).await.unwrap();
                let mut plen = [0u8; 1];
                stream.read_exact(&mut plen).await.unwrap();
                let mut p = vec![0u8; plen[0] as usize];
                stream.read_exact(&mut p).await.unwrap();
                assert_eq!(String::from_utf8(u).unwrap(), "user");
                assert_eq!(String::from_utf8(p).unwrap(), "pass");
                stream.write_all(&[0x01, 0x00]).await.unwrap();

                let mut req_head = [0u8; 4];
                stream.read_exact(&mut req_head).await.unwrap();
                assert_eq!(&req_head, &[0x05, 0x01, 0x00, 0x03]);
                let mut len = [0u8; 1];
                stream.read_exact(&mut len).await.unwrap();
                let mut host = vec![0u8; len[0] as usize];
                stream.read_exact(&mut host).await.unwrap();
                assert_eq!(String::from_utf8(host).unwrap(), "example.com");
                let mut port_buf = [0u8; 2];
                stream.read_exact(&mut port_buf).await.unwrap();
                assert_eq!(u16::from_be_bytes(port_buf), 22);

                stream
                    .write_all(&[0x05, 0x00, 0x00, 0x01, 0, 0, 0, 0, 0, 0])
                    .await
                    .unwrap();
            });

            let mut client = TcpStream::connect(("127.0.0.1", port)).await.unwrap();
            socks5_client_handshake(&mut client, Some("user"), Some("pass"), "example.com", 22)
                .await
                .unwrap();

            server.await.unwrap();
        });
    }
}
