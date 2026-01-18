use super::known_hosts::KnownHostsManager;
use anyhow::anyhow;
use async_trait::async_trait;
use base64::{engine::general_purpose, Engine as _};
use futures::Future;
use log::{debug, error, info};
use russh::client::{Handle, Handler, KeyboardInteractiveAuthResponse, Prompt};
#[cfg(unix)]
use russh_keys::agent::client::AgentClient;
use russh_keys::encoding::Encoding;
use russh_keys::key::PublicKey;
use russh_keys::key::SignatureHash;
use russh_keys::load_secret_key;
use russh_keys::signature::Signature;
use russh_keys::PublicKeyBase64;
use serde_json::json;
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::pin::Pin;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::lookup_host;
#[cfg(unix)]
use tokio::net::UnixStream;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::Mutex;

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

#[derive(Clone)]
struct KeyPairSigner {
    key_pair: Arc<russh_keys::key::KeyPair>,
}

fn append_signature(
    data: &mut russh::CryptoVec,
    signature: &Signature,
) -> Result<(), anyhow::Error> {
    let (t, sig) = match signature {
        Signature::Ed25519(bytes) => (&b"ssh-ed25519"[..], bytes.0.as_slice()),
        Signature::P256(bytes) => (&b"ecdsa-sha2-nistp256"[..], bytes.as_slice()),
        Signature::RSA { hash, bytes } => {
            let t = match hash {
                SignatureHash::SHA2_256 => &b"rsa-sha2-256"[..],
                SignatureHash::SHA2_512 => &b"rsa-sha2-512"[..],
                SignatureHash::SHA1 => &b"ssh-rsa"[..],
            };
            (t, bytes.as_slice())
        }
    };

    data.push_u32_be((t.len() + sig.len() + 8) as u32);
    data.extend_ssh_string(t);
    data.extend_ssh_string(sig);
    Ok(())
}

impl russh::Signer for KeyPairSigner {
    type Error = anyhow::Error;
    type Future =
        Pin<Box<dyn Future<Output = (Self, Result<russh::CryptoVec, Self::Error>)> + Send>>;

    fn auth_publickey_sign(self, _key: &PublicKey, mut to_sign: russh::CryptoVec) -> Self::Future {
        let key_pair = self.key_pair.clone();
        Box::pin(async move {
            let signature = match key_pair.sign_detached(&to_sign) {
                Ok(s) => s,
                Err(e) => {
                    return (self, Err(anyhow!("Failed to sign with private key: {}", e)));
                }
            };
            if let Err(e) = append_signature(&mut to_sign, &signature) {
                return (self, Err(e));
            }
            (self, Ok(to_sign))
        })
    }
}

fn load_openssh_public_key(path: &str) -> Result<PublicKey, anyhow::Error> {
    let content = fs::read_to_string(path)
        .map_err(|e| anyhow!("Failed to read public key file {}: {}", path, e))?;
    let line = content
        .lines()
        .map(str::trim)
        .find(|l| !l.is_empty() && !l.starts_with('#'))
        .ok_or_else(|| anyhow!("Public key file has no key line: {}", path))?;

    let mut parts = line.split_whitespace();
    let key_type = parts
        .next()
        .ok_or_else(|| anyhow!("Invalid public key line (missing type): {}", path))?;
    if key_type.contains("-cert-v01@openssh.com") {
        return Err(anyhow!(
            "OpenSSH certificate public keys are not supported: {}",
            key_type
        ));
    }
    let key_base64 = parts
        .next()
        .ok_or_else(|| anyhow!("Invalid public key line (missing base64): {}", path))?;

    let raw = general_purpose::STANDARD
        .decode(key_base64)
        .map_err(|e| anyhow!("Invalid base64 in public key file {}: {}", path, e))?;

    if raw.len() < 4 {
        return Err(anyhow!("Invalid SSH public key blob (too short): {}", path));
    }
    let algo_len = u32::from_be_bytes([raw[0], raw[1], raw[2], raw[3]]) as usize;
    if raw.len() < 4 + algo_len {
        return Err(anyhow!(
            "Invalid SSH public key blob (bad algo length): {}",
            path
        ));
    }
    let algo = &raw[4..4 + algo_len];
    let pubkey = &raw[4 + algo_len..];

    let pk = PublicKey::parse(algo, pubkey)
        .map_err(|e| anyhow!("Failed to parse public key {}: {}", path, e))?;
    if pk.name() != key_type {
        debug!(
            "Public key type mismatch: file says {}, parsed says {}",
            key_type,
            pk.name()
        );
    }

    Ok(pk)
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

        // Check if we have a mapping for this port
        if let Some((local_host, local_port)) =
            self.remote_forward_mappings.get(&(connected_port as u16))
        {
            let local_host = local_host.clone();
            let local_port = *local_port;

            info!(
                "Forwarding remote connection on port {} to local {}:{}",
                connected_port, local_host, local_port
            );

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
                }
                Err(e) => {
                    error!(
                        "Failed to connect to local target {}:{}: {}",
                        local_host, local_port, e
                    );
                    channel.close().await.ok();
                }
            }
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
        // Clone the Arc containing the Handle for local forwarding tasks
        let handle_arc = self.handle.clone();

        // Setup local forwards (client -> server -> remote)
        for forward in local_forwards {
            let local_host = forward.local_host.clone();
            let local_port = forward.local_port;
            let remote_host = forward.remote_host.clone();
            let remote_port = forward.remote_port;
            let handle_arc = handle_arc.clone();

            info!(
                "Setting up local port forwarding: {}:{} -> {}:{}",
                local_host, local_port, remote_host, remote_port
            );

            tokio::spawn(async move {
                let listener =
                    match TcpListener::bind(format!("{}:{}", local_host, local_port)).await {
                        Ok(l) => l,
                        Err(e) => {
                            error!("Failed to bind local port {}: {}", local_port, e);
                            return;
                        }
                    };

                info!(
                    "Listening on {}:{} for forwarding to {}:{}",
                    local_host, local_port, remote_host, remote_port
                );

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
                                    guard
                                        .channel_open_direct_tcpip(
                                            &remote_host,
                                            remote_port as u32,
                                            "127.0.0.1",
                                            0,
                                        )
                                        .await
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
                                    }
                                    Err(e) => {
                                        error!("Failed to open direct-tcpip channel: {:?}", e);
                                    }
                                }
                            });
                        }
                        Err(e) => {
                            error!(
                                "Failed to accept connection on {}:{}: {}",
                                local_host, local_port, e
                            );
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
                info!(
                    "Requesting remote port forwarding: {}:{} -> {}:{}",
                    forward.remote_host,
                    forward.remote_port,
                    forward.local_host,
                    forward.local_port
                );
                match handle
                    .tcpip_forward(&forward.remote_host, forward.remote_port as u32)
                    .await
                {
                    Ok(true) => info!(
                        "Remote port forwarding request accepted for port {}",
                        forward.remote_port
                    ),
                    Ok(false) => error!(
                        "Remote port forwarding request rejected for port {}",
                        forward.remote_port
                    ),
                    Err(e) => error!("Failed to request remote port forwarding: {:?}", e),
                }
            }
        }

        Ok(())
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
            let key_pair = match load_secret_key(key_path, passphrase.as_deref()) {
                Ok(k) => k,
                Err(e) => return Err(anyhow!("Failed to load private key: {:?}", e)),
            };
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
                return Err(anyhow!(
                    "Agent authentication not supported on this platform"
                ));
            }
        }
        AuthType::Certificate(cert_path, key_path, passphrase) => {
            let cert_path_obj = Path::new(&cert_path);
            if !cert_path_obj.exists() {
                return Err(anyhow!(
                    "Certificate file not found: {}",
                    cert_path_obj.display()
                ));
            }
            let cert_meta = fs::metadata(cert_path_obj)
                .map_err(|e| anyhow!("Failed to read certificate metadata: {}", e))?;
            if cert_meta.len() == 0 {
                return Err(anyhow!(
                    "Certificate file is empty: {}",
                    cert_path_obj.display()
                ));
            }

            let key_path_obj = Path::new(&key_path);
            if !key_path_obj.exists() {
                return Err(anyhow!(
                    "Private key file not found: {}",
                    key_path_obj.display()
                ));
            }
            let key_meta = fs::metadata(key_path_obj)
                .map_err(|e| anyhow!("Failed to read private key metadata: {}", e))?;
            if key_meta.len() == 0 {
                return Err(anyhow!(
                    "Private key file is empty: {}",
                    key_path_obj.display()
                ));
            }

            let cert_public_key = load_openssh_public_key(&cert_path)?;

            // Load the private key
            let key_pair = match load_secret_key(&key_path, passphrase.as_deref()) {
                Ok(k) => k,
                Err(e) => return Err(anyhow!("Failed to load private key: {:?}", e)),
            };
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
        start_socks5_proxy(connection.handle.clone(), port).await?;
    }

    Ok(connection)
}

async fn authenticate_keyboard_interactive(
    handle: &mut Handle<SshHandler>,
    host: &str,
    port: u16,
    username: &str,
    prompter: Arc<dyn KeyboardInteractivePrompter>,
) -> Result<bool, anyhow::Error> {
    let mut response = handle
        .authenticate_keyboard_interactive_start(username, None::<String>)
        .await
        .map_err(anyhow::Error::from)?;

    loop {
        match response {
            KeyboardInteractiveAuthResponse::Success => return Ok(true),
            KeyboardInteractiveAuthResponse::Failure => return Ok(false),
            KeyboardInteractiveAuthResponse::InfoRequest {
                name,
                instructions,
                prompts,
            } => {
                let replies = prompter
                    .prompt(KeyboardInteractivePromptRequest {
                        host: host.to_string(),
                        port,
                        username: username.to_string(),
                        name,
                        instructions,
                        prompts,
                    })
                    .await?;
                response = handle
                    .authenticate_keyboard_interactive_respond(replies)
                    .await
                    .map_err(anyhow::Error::from)?;
            }
        }
    }
}

pub async fn start_ephemeral_direct_tcpip_listener(
    handle: Arc<Mutex<Handle<SshHandler>>>,
    remote_host: String,
    remote_port: u16,
) -> Result<u16, anyhow::Error> {
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let local_port = listener.local_addr()?.port();

    info!(
        "Listening on 127.0.0.1:{} for forwarding to {}:{}",
        local_port, remote_host, remote_port
    );

    tokio::spawn(async move {
        loop {
            match listener.accept().await {
                Ok((mut socket, addr)) => {
                    debug!("Accepted connection from {:?} for jump forwarding", addr);
                    let handle = handle.clone();
                    let remote_host = remote_host.clone();

                    tokio::spawn(async move {
                        let channel_result = {
                            let guard = handle.lock().await;
                            guard
                                .channel_open_direct_tcpip(
                                    &remote_host,
                                    remote_port as u32,
                                    "127.0.0.1",
                                    0,
                                )
                                .await
                        };

                        match channel_result {
                            Ok(mut channel) => {
                                let mut buf = vec![0u8; 8192];
                                loop {
                                    tokio::select! {
                                        msg = channel.wait() => {
                                            match msg {
                                                Some(russh::ChannelMsg::Data { data }) => {
                                                    if socket.write_all(&data).await.is_err() {
                                                        break;
                                                    }
                                                }
                                                Some(russh::ChannelMsg::Eof) | None => {
                                                    break;
                                                }
                                                _ => {}
                                            }
                                        }
                                        n = socket.read(&mut buf) => {
                                            match n {
                                                Ok(n) if n > 0 => {
                                                    if channel.data(&buf[..n]).await.is_err() {
                                                        break;
                                                    }
                                                }
                                                _ => break,
                                            }
                                        }
                                    }
                                }

                                channel.close().await.ok();
                            }
                            Err(e) => {
                                debug!("Failed to open direct-tcpip channel: {:?}", e);
                            }
                        }
                    });
                }
                Err(e) => {
                    debug!("Failed to accept jump forwarding connection: {:?}", e);
                    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                }
            }
        }
    });

    Ok(local_port)
}

pub async fn start_ephemeral_socks5_proxy_dial_listener(
    proxy_host: String,
    proxy_port: u16,
    proxy_username: Option<String>,
    proxy_password: Option<String>,
    remote_host: String,
    remote_port: u16,
) -> Result<u16, anyhow::Error> {
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let local_port = listener.local_addr()?.port();

    info!(
        "Listening on 127.0.0.1:{} for proxying to {}:{} via SOCKS5 {}:{}",
        local_port, remote_host, remote_port, proxy_host, proxy_port
    );

    tokio::spawn(async move {
        let accept = listener.accept().await;
        let Ok((mut local, _addr)) = accept else {
            return;
        };

        let proxy_stream = TcpStream::connect(format!("{}:{}", proxy_host, proxy_port)).await;
        let Ok(mut proxy_stream) = proxy_stream else {
            local.shutdown().await.ok();
            return;
        };

        if socks5_client_handshake(
            &mut proxy_stream,
            proxy_username.as_deref(),
            proxy_password.as_deref(),
            &remote_host,
            remote_port,
        )
        .await
        .is_err()
        {
            local.shutdown().await.ok();
            proxy_stream.shutdown().await.ok();
            return;
        }

        let mut local_buf = vec![0u8; 8192];
        let mut proxy_buf = vec![0u8; 8192];
        loop {
            tokio::select! {
                n = local.read(&mut local_buf) => {
                    match n {
                        Ok(n) if n > 0 => {
                            if proxy_stream.write_all(&local_buf[..n]).await.is_err() {
                                break;
                            }
                        }
                        _ => break,
                    }
                }
                n = proxy_stream.read(&mut proxy_buf) => {
                    match n {
                        Ok(n) if n > 0 => {
                            if local.write_all(&proxy_buf[..n]).await.is_err() {
                                break;
                            }
                        }
                        _ => break,
                    }
                }
            }
        }

        local.shutdown().await.ok();
        proxy_stream.shutdown().await.ok();
    });

    Ok(local_port)
}

pub async fn start_ephemeral_http_proxy_dial_listener(
    proxy_host: String,
    proxy_port: u16,
    proxy_username: Option<String>,
    proxy_password: Option<String>,
    remote_host: String,
    remote_port: u16,
) -> Result<u16, anyhow::Error> {
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let local_port = listener.local_addr()?.port();

    info!(
        "Listening on 127.0.0.1:{} for proxying to {}:{} via HTTP {}:{}",
        local_port, remote_host, remote_port, proxy_host, proxy_port
    );

    tokio::spawn(async move {
        let accept = listener.accept().await;
        let Ok((mut local, _addr)) = accept else {
            return;
        };

        let proxy_stream = TcpStream::connect(format!("{}:{}", proxy_host, proxy_port)).await;
        let Ok(mut proxy_stream) = proxy_stream else {
            local.shutdown().await.ok();
            return;
        };

        if http_proxy_connect(
            &mut proxy_stream,
            proxy_username.as_deref(),
            proxy_password.as_deref(),
            &remote_host,
            remote_port,
        )
        .await
        .is_err()
        {
            local.shutdown().await.ok();
            proxy_stream.shutdown().await.ok();
            return;
        }

        let mut local_buf = vec![0u8; 8192];
        let mut proxy_buf = vec![0u8; 8192];
        loop {
            tokio::select! {
                n = local.read(&mut local_buf) => {
                    match n {
                        Ok(n) if n > 0 => {
                            if proxy_stream.write_all(&local_buf[..n]).await.is_err() {
                                break;
                            }
                        }
                        _ => break,
                    }
                }
                n = proxy_stream.read(&mut proxy_buf) => {
                    match n {
                        Ok(n) if n > 0 => {
                            if local.write_all(&proxy_buf[..n]).await.is_err() {
                                break;
                            }
                        }
                        _ => break,
                    }
                }
            }
        }

        local.shutdown().await.ok();
        proxy_stream.shutdown().await.ok();
    });

    Ok(local_port)
}

async fn start_socks5_proxy(
    handle: Arc<Mutex<Handle<SshHandler>>>,
    port: u16,
) -> Result<(), anyhow::Error> {
    let listener = TcpListener::bind(format!("127.0.0.1:{}", port)).await?;

    tokio::spawn(async move {
        loop {
            match listener.accept().await {
                Ok((stream, addr)) => {
                    debug!("SOCKS5 client connected from {:?}", addr);
                    let handle = handle.clone();
                    tokio::spawn(async move {
                        if let Err(e) = handle_socks5_client(handle, stream).await {
                            debug!("SOCKS5 client handling error: {:?}", e);
                        }
                    });
                }
                Err(e) => {
                    debug!("SOCKS5 accept error: {:?}", e);
                    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                }
            }
        }
    });

    Ok(())
}

#[cfg(unix)]
async fn authenticate_agent(
    handle: &mut Handle<SshHandler>,
    username: &str,
    agent_path: Option<String>,
) -> Result<bool, anyhow::Error> {
    let sock = match agent_path {
        Some(p) => p,
        None => std::env::var("SSH_AUTH_SOCK").map_err(|_| anyhow!("SSH_AUTH_SOCK not set"))?,
    };
    let stream = UnixStream::connect(sock).await?;
    let mut client = AgentClient::connect(stream);
    let keys = client.request_identities().await?;
    let public_key = keys
        .into_iter()
        .next()
        .ok_or(anyhow!("No identities in SSH agent"))?;
    let (_, res) = handle
        .authenticate_future(username, public_key, client)
        .await;
    res.map_err(anyhow::Error::from)
}

async fn handle_socks5_client(
    handle: Arc<Mutex<Handle<SshHandler>>>,
    mut stream: TcpStream,
) -> Result<(), anyhow::Error> {
    let mut header = [0u8; 2];
    stream.read_exact(&mut header).await?;
    if header[0] != 0x05 {
        return Err(anyhow!("Unsupported SOCKS version: {}", header[0]));
    }

    let nmethods = header[1] as usize;
    let mut methods = vec![0u8; nmethods];
    stream.read_exact(&mut methods).await?;
    if !methods.contains(&0x00) {
        stream.write_all(&[0x05, 0xff]).await.ok();
        return Ok(());
    }
    stream.write_all(&[0x05, 0x00]).await?;

    let mut req_hdr = [0u8; 4];
    stream.read_exact(&mut req_hdr).await?;
    if req_hdr[0] != 0x05 {
        return Err(anyhow!("Invalid request version: {}", req_hdr[0]));
    }
    if req_hdr[1] != 0x01 {
        stream
            .write_all(&[0x05, 0x07, 0x00, 0x01, 0, 0, 0, 0, 0, 0])
            .await
            .ok();
        return Ok(());
    }

    let atyp = req_hdr[3];
    let dest_host = match atyp {
        0x01 => {
            let mut ip = [0u8; 4];
            stream.read_exact(&mut ip).await?;
            std::net::Ipv4Addr::from(ip).to_string()
        }
        0x03 => {
            let mut len = [0u8; 1];
            stream.read_exact(&mut len).await?;
            let mut name = vec![0u8; len[0] as usize];
            stream.read_exact(&mut name).await?;
            String::from_utf8(name)?
        }
        0x04 => {
            let mut ip = [0u8; 16];
            stream.read_exact(&mut ip).await?;
            std::net::Ipv6Addr::from(ip).to_string()
        }
        _ => {
            stream
                .write_all(&[0x05, 0x08, 0x00, 0x01, 0, 0, 0, 0, 0, 0])
                .await
                .ok();
            return Ok(());
        }
    };

    let mut port_buf = [0u8; 2];
    stream.read_exact(&mut port_buf).await?;
    let dest_port = u16::from_be_bytes(port_buf);

    let channel_result = {
        let guard = handle.lock().await;
        guard
            .channel_open_direct_tcpip(&dest_host, dest_port as u32, "127.0.0.1", 0)
            .await
    };

    let mut channel = match channel_result {
        Ok(ch) => ch,
        Err(_) => {
            stream
                .write_all(&[0x05, 0x05, 0x00, 0x01, 0, 0, 0, 0, 0, 0])
                .await
                .ok();
            return Ok(());
        }
    };

    stream
        .write_all(&[0x05, 0x00, 0x00, 0x01, 0, 0, 0, 0, 0, 0])
        .await?;

    let mut buf = vec![0u8; 8192];
    loop {
        tokio::select! {
            msg = channel.wait() => {
                match msg {
                    Some(russh::ChannelMsg::Data { data }) => {
                        if stream.write_all(&data).await.is_err() {
                            break;
                        }
                    }
                    Some(russh::ChannelMsg::Eof) | None => {
                        break;
                    }
                    _ => {}
                }
            }
            n = stream.read(&mut buf) => {
                match n {
                    Ok(n) if n > 0 => {
                        if channel.data(&buf[..n]).await.is_err() {
                            break;
                        }
                    }
                    _ => break,
                }
            }
        }
    }

    channel.close().await.ok();
    Ok(())
}

async fn socks5_client_handshake(
    stream: &mut TcpStream,
    username: Option<&str>,
    password: Option<&str>,
    dest_host: &str,
    dest_port: u16,
) -> Result<(), anyhow::Error> {
    let mut methods = vec![0x00u8];
    if username.is_some() || password.is_some() {
        methods.push(0x02);
    }

    let mut req = Vec::with_capacity(2 + methods.len());
    req.push(0x05);
    req.push(methods.len() as u8);
    req.extend_from_slice(&methods);
    stream.write_all(&req).await?;

    let mut resp = [0u8; 2];
    stream.read_exact(&mut resp).await?;
    if resp[0] != 0x05 {
        return Err(anyhow!("Invalid SOCKS5 response version: {}", resp[0]));
    }

    match resp[1] {
        0x00 => {}
        0x02 => {
            let u = username.unwrap_or("");
            let p = password.unwrap_or("");
            if u.len() > 255 || p.len() > 255 {
                return Err(anyhow!("SOCKS5 username/password too long"));
            }

            let mut auth = Vec::with_capacity(3 + u.len() + p.len());
            auth.push(0x01);
            auth.push(u.len() as u8);
            auth.extend_from_slice(u.as_bytes());
            auth.push(p.len() as u8);
            auth.extend_from_slice(p.as_bytes());
            stream.write_all(&auth).await?;

            let mut auth_resp = [0u8; 2];
            stream.read_exact(&mut auth_resp).await?;
            if auth_resp[0] != 0x01 || auth_resp[1] != 0x00 {
                return Err(anyhow!("SOCKS5 auth failed"));
            }
        }
        0xff => return Err(anyhow!("SOCKS5 proxy requires unsupported auth")),
        v => return Err(anyhow!("Unsupported SOCKS5 auth method: {}", v)),
    }

    let host_bytes = dest_host.as_bytes();
    let mut conn = vec![0x05, 0x01, 0x00, 0x03, host_bytes.len() as u8];
    conn.extend_from_slice(host_bytes);
    conn.extend_from_slice(&dest_port.to_be_bytes());
    stream.write_all(&conn).await?;

    let mut head = [0u8; 4];
    stream.read_exact(&mut head).await?;
    if head[0] != 0x05 {
        return Err(anyhow!(
            "Invalid SOCKS5 connect response version: {}",
            head[0]
        ));
    }
    if head[1] != 0x00 {
        return Err(anyhow!("SOCKS5 connect failed, rep={}", head[1]));
    }

    match head[3] {
        0x01 => {
            let mut addr = [0u8; 4];
            stream.read_exact(&mut addr).await?;
        }
        0x03 => {
            let mut len = [0u8; 1];
            stream.read_exact(&mut len).await?;
            let mut addr = vec![0u8; len[0] as usize];
            stream.read_exact(&mut addr).await?;
        }
        0x04 => {
            let mut addr = [0u8; 16];
            stream.read_exact(&mut addr).await?;
        }
        v => return Err(anyhow!("Invalid SOCKS5 atyp: {}", v)),
    }
    let mut bnd_port = [0u8; 2];
    stream.read_exact(&mut bnd_port).await?;

    Ok(())
}

async fn http_proxy_connect(
    stream: &mut TcpStream,
    username: Option<&str>,
    password: Option<&str>,
    dest_host: &str,
    dest_port: u16,
) -> Result<(), anyhow::Error> {
    let authority = format!("{}:{}", dest_host, dest_port);
    let mut req = format!(
        "CONNECT {} HTTP/1.1\r\nHost: {}\r\nProxy-Connection: Keep-Alive\r\n",
        authority, authority
    );

    if let (Some(u), Some(p)) = (username, password) {
        let token = general_purpose::STANDARD.encode(format!("{}:{}", u, p));
        req.push_str(&format!("Proxy-Authorization: Basic {}\r\n", token));
    }

    req.push_str("\r\n");
    stream.write_all(req.as_bytes()).await?;

    let mut buf = Vec::with_capacity(1024);
    let mut tmp = [0u8; 256];
    loop {
        let n = stream.read(&mut tmp).await?;
        if n == 0 {
            return Err(anyhow!("HTTP proxy closed connection"));
        }
        buf.extend_from_slice(&tmp[..n]);
        if buf.windows(4).any(|w| w == b"\r\n\r\n") {
            break;
        }
        if buf.len() > 16 * 1024 {
            return Err(anyhow!("HTTP proxy response too large"));
        }
    }

    let header = String::from_utf8_lossy(&buf);
    let first_line = header.lines().next().unwrap_or_default();
    if !first_line.starts_with("HTTP/") {
        return Err(anyhow!("Invalid HTTP proxy response"));
    }
    let mut parts = first_line.split_whitespace();
    let _http = parts.next().unwrap_or_default();
    let code = parts.next().unwrap_or_default();
    if code != "200" {
        return Err(anyhow!("HTTP proxy CONNECT failed: {}", first_line));
    }

    Ok(())
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
