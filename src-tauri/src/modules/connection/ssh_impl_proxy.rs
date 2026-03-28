use anyhow::anyhow;
use base64::{engine::general_purpose, Engine as _};
use log::{debug, info};
use russh::client::Handle;
use std::fs;
use std::net::{SocketAddr, SocketAddrV4};
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{oneshot, watch, Mutex};
use uuid::Uuid;

use super::SshHandler;

const EPHEMERAL_ACTIVATION_TIMEOUT: Duration = Duration::from_secs(10);
const EPHEMERAL_ACCEPT_TIMEOUT: Duration = Duration::from_secs(5);
const EPHEMERAL_AUTH_TIMEOUT: Duration = Duration::from_secs(2);
const EPHEMERAL_AUTH_PREFIX: &str = "STAR_SHUTTLE_AUTH ";
const EPHEMERAL_AUTH_MAX_LINE_BYTES: usize = 256;

pub struct EphemeralListenerLease {
    local_port: u16,
    auth_token: String,
    auth_tx: Option<oneshot::Sender<String>>,
}

pub struct ActivatedEphemeralListener {
    local_port: u16,
    auth_token: String,
}

impl EphemeralListenerLease {
    pub fn activate(mut self) -> Result<ActivatedEphemeralListener, anyhow::Error> {
        let tx = self
            .auth_tx
            .take()
            .ok_or_else(|| anyhow!("Ephemeral listener lease is already activated"))?;
        let auth_token = self.auth_token;
        tx.send(auth_token.clone())
            .map_err(|_| anyhow!("Ephemeral listener activation channel is closed"))?;
        Ok(ActivatedEphemeralListener {
            local_port: self.local_port,
            auth_token,
        })
    }
}

impl ActivatedEphemeralListener {
    #[cfg(test)]
    pub fn local_port(&self) -> u16 {
        self.local_port
    }

    pub async fn connect(self) -> Result<TcpStream, anyhow::Error> {
        let mut stream = TcpStream::connect(("127.0.0.1", self.local_port)).await?;
        write_ephemeral_auth_line(&mut stream, &self.auth_token).await?;
        Ok(stream)
    }
}

fn create_ephemeral_listener_lease(
    local_port: u16,
) -> (EphemeralListenerLease, oneshot::Receiver<String>, String) {
    let auth_token = format!("{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple());
    let (auth_tx, auth_rx) = oneshot::channel();
    (
        EphemeralListenerLease {
            local_port,
            auth_token: auth_token.clone(),
            auth_tx: Some(auth_tx),
        },
        auth_rx,
        auth_token,
    )
}

async fn wait_for_ephemeral_activation(
    auth_rx: oneshot::Receiver<String>,
    expected_token: &str,
    context: &str,
) -> Result<(), anyhow::Error> {
    let received_token = tokio::time::timeout(EPHEMERAL_ACTIVATION_TIMEOUT, auth_rx)
        .await
        .map_err(|_| anyhow!("Timed out waiting for activation: {}", context))?
        .map_err(|_| anyhow!("Activation channel closed: {}", context))?;

    if received_token != expected_token {
        return Err(anyhow!("Invalid activation token: {}", context));
    }

    Ok(())
}

async fn accept_authorized_local_connection(
    listener: &TcpListener,
    context: &str,
    expected_token: &str,
) -> Result<TcpStream, anyhow::Error> {
    let deadline = tokio::time::Instant::now() + EPHEMERAL_ACCEPT_TIMEOUT;

    loop {
        let now = tokio::time::Instant::now();
        if now >= deadline {
            return Err(anyhow!(
                "Timed out waiting for authorized local connection: {}",
                context
            ));
        }

        let remaining = deadline.duration_since(now);
        let accepted = tokio::time::timeout(remaining, listener.accept())
            .await
            .map_err(|_| anyhow!("Timed out accepting local connection: {}", context))?;
        let (mut stream, addr) = accepted.map_err(|e| anyhow!(e.to_string()))?;

        if !is_connection_owned_by_current_process(&stream) {
            debug!(
                "Rejected unauthorized local client {:?} for {}",
                addr, context
            );
            let _ = stream.shutdown().await;
            continue;
        }

        if !authenticate_ephemeral_local_client(&mut stream, expected_token, context).await? {
            debug!(
                "Rejected client with invalid ephemeral auth {:?} for {}",
                addr, context
            );
            let _ = stream.shutdown().await;
            continue;
        }

        debug!(
            "Accepted authorized local client {:?} for {}",
            addr, context
        );
        return Ok(stream);
    }
}

async fn write_ephemeral_auth_line(
    stream: &mut TcpStream,
    auth_token: &str,
) -> Result<(), anyhow::Error> {
    let line = format!("{}{}\n", EPHEMERAL_AUTH_PREFIX, auth_token);
    stream.write_all(line.as_bytes()).await?;
    stream.flush().await?;
    Ok(())
}

async fn authenticate_ephemeral_local_client(
    stream: &mut TcpStream,
    expected_token: &str,
    context: &str,
) -> Result<bool, anyhow::Error> {
    let auth_future = async {
        let mut received = Vec::with_capacity(EPHEMERAL_AUTH_MAX_LINE_BYTES);
        let mut byte = [0u8; 1];
        while received.len() < EPHEMERAL_AUTH_MAX_LINE_BYTES {
            let n = stream.read(&mut byte).await?;
            if n == 0 {
                return Ok::<bool, anyhow::Error>(false);
            }
            if byte[0] == b'\n' {
                break;
            }
            if byte[0] != b'\r' {
                received.push(byte[0]);
            }
        }

        if received.len() >= EPHEMERAL_AUTH_MAX_LINE_BYTES {
            return Ok(false);
        }

        let line = String::from_utf8(received).map_err(|e| anyhow!(e.to_string()))?;
        let Some(token) = line.strip_prefix(EPHEMERAL_AUTH_PREFIX) else {
            return Ok(false);
        };
        Ok(token == expected_token)
    };

    tokio::time::timeout(EPHEMERAL_AUTH_TIMEOUT, auth_future)
        .await
        .map_err(|_| anyhow!("Timed out authenticating local client: {}", context))?
}

#[cfg(target_os = "linux")]
fn is_connection_owned_by_current_process(stream: &TcpStream) -> bool {
    let local_addr = match stream.local_addr() {
        Ok(addr) => addr,
        Err(_) => return false,
    };
    let peer_addr = match stream.peer_addr() {
        Ok(addr) => addr,
        Err(_) => return false,
    };

    let (local_v4, peer_v4) = match (local_addr, peer_addr) {
        (SocketAddr::V4(local), SocketAddr::V4(peer)) => (local, peer),
        _ => return false,
    };

    let client_inode = match find_client_socket_inode(peer_v4, local_v4) {
        Some(inode) => inode,
        None => return false,
    };

    current_process_has_socket_inode(&client_inode)
}

#[cfg(not(target_os = "linux"))]
fn is_connection_owned_by_current_process(_stream: &TcpStream) -> bool {
    true
}

#[cfg(target_os = "linux")]
fn find_client_socket_inode(
    client_local: SocketAddrV4,
    client_remote: SocketAddrV4,
) -> Option<String> {
    let content = fs::read_to_string("/proc/net/tcp").ok()?;
    let local_repr = socket_addr_to_proc_repr(client_local);
    let remote_repr = socket_addr_to_proc_repr(client_remote);

    for line in content.lines().skip(1) {
        let fields: Vec<&str> = line.split_whitespace().collect();
        if fields.len() <= 9 {
            continue;
        }

        if fields[1] == local_repr && fields[2] == remote_repr {
            return Some(fields[9].to_string());
        }
    }

    None
}

#[cfg(target_os = "linux")]
fn socket_addr_to_proc_repr(addr: SocketAddrV4) -> String {
    let ip = addr.ip().octets();
    format!(
        "{:02X}{:02X}{:02X}{:02X}:{:04X}",
        ip[3],
        ip[2],
        ip[1],
        ip[0],
        addr.port()
    )
}

#[cfg(target_os = "linux")]
fn current_process_has_socket_inode(inode: &str) -> bool {
    let expected = format!("socket:[{}]", inode);
    let entries = match fs::read_dir("/proc/self/fd") {
        Ok(entries) => entries,
        Err(_) => return false,
    };

    for entry in entries.flatten() {
        if let Ok(target) = fs::read_link(entry.path()) {
            if target.to_string_lossy() == expected {
                return true;
            }
        }
    }

    false
}

pub(super) async fn start_socks5_proxy(
    handle: Arc<Mutex<Handle<SshHandler>>>,
    port: u16,
    mut shutdown: watch::Receiver<bool>,
) -> Result<(), anyhow::Error> {
    let listener = TcpListener::bind(format!("127.0.0.1:{}", port)).await?;

    tokio::spawn(async move {
        loop {
            tokio::select! {
                changed = shutdown.changed() => {
                    if changed.is_err() || *shutdown.borrow() {
                        debug!("Stopping SOCKS5 listener on port {}", port);
                        break;
                    }
                }
                accepted = listener.accept() => match accepted {
                Ok((stream, addr)) => {
                    debug!("SOCKS5 client connected from {:?}", addr);
                    let handle = handle.clone();
                    let client_shutdown = shutdown.clone();
                    tokio::spawn(async move {
                        if let Err(e) = handle_socks5_client(handle, stream, client_shutdown).await {
                            debug!("SOCKS5 client handling error: {:?}", e);
                        }
                    });
                }
                Err(e) => {
                    debug!("SOCKS5 accept error: {:?}", e);
                    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                }
            }}
        }
    });

    Ok(())
}

pub async fn start_ephemeral_direct_tcpip_listener(
    handle: Arc<Mutex<Handle<SshHandler>>>,
    remote_host: String,
    remote_port: u16,
) -> Result<EphemeralListenerLease, anyhow::Error> {
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let local_port = listener.local_addr()?.port();
    let (lease, auth_rx, expected_token) = create_ephemeral_listener_lease(local_port);

    info!(
        "Listening on 127.0.0.1:{} for forwarding to {}:{}",
        local_port, remote_host, remote_port
    );

    tokio::spawn(async move {
        if let Err(e) =
            wait_for_ephemeral_activation(auth_rx, &expected_token, "direct-tcpip listener").await
        {
            debug!("Direct-tcpip listener activation failed: {:?}", e);
            return;
        }

        let mut socket = match accept_authorized_local_connection(
            &listener,
            "direct-tcpip listener",
            &expected_token,
        )
        .await
        {
            Ok(socket) => socket,
            Err(e) => {
                debug!("Direct-tcpip listener accept failed: {:?}", e);
                return;
            }
        };

        debug!("Accepted authorized connection for jump forwarding");
        let channel_result = {
            let guard = handle.lock().await;
            guard
                .channel_open_direct_tcpip(&remote_host, remote_port as u32, "127.0.0.1", 0)
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

                socket.shutdown().await.ok();
                channel.close().await.ok();
            }
            Err(e) => {
                debug!("Failed to open direct-tcpip channel: {:?}", e);
            }
        }
    });

    Ok(lease)
}

pub async fn start_ephemeral_socks5_proxy_dial_listener(
    proxy_host: String,
    proxy_port: u16,
    proxy_username: Option<String>,
    proxy_password: Option<String>,
    remote_host: String,
    remote_port: u16,
) -> Result<EphemeralListenerLease, anyhow::Error> {
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let local_port = listener.local_addr()?.port();
    let (lease, auth_rx, expected_token) = create_ephemeral_listener_lease(local_port);

    info!(
        "Listening on 127.0.0.1:{} for proxying to {}:{} via SOCKS5 {}:{}",
        local_port, remote_host, remote_port, proxy_host, proxy_port
    );

    tokio::spawn(async move {
        if let Err(e) =
            wait_for_ephemeral_activation(auth_rx, &expected_token, "socks5 dial listener").await
        {
            debug!("SOCKS5 dial listener activation failed: {:?}", e);
            return;
        }

        let mut local = match accept_authorized_local_connection(
            &listener,
            "socks5 dial listener",
            &expected_token,
        )
        .await
        {
            Ok(local) => local,
            Err(e) => {
                debug!("SOCKS5 dial listener accept failed: {:?}", e);
                return;
            }
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

    Ok(lease)
}

pub async fn start_ephemeral_http_proxy_dial_listener(
    proxy_host: String,
    proxy_port: u16,
    proxy_username: Option<String>,
    proxy_password: Option<String>,
    remote_host: String,
    remote_port: u16,
) -> Result<EphemeralListenerLease, anyhow::Error> {
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let local_port = listener.local_addr()?.port();
    let (lease, auth_rx, expected_token) = create_ephemeral_listener_lease(local_port);

    info!(
        "Listening on 127.0.0.1:{} for proxying to {}:{} via HTTP {}:{}",
        local_port, remote_host, remote_port, proxy_host, proxy_port
    );

    tokio::spawn(async move {
        if let Err(e) =
            wait_for_ephemeral_activation(auth_rx, &expected_token, "http dial listener").await
        {
            debug!("HTTP dial listener activation failed: {:?}", e);
            return;
        }

        let mut local = match accept_authorized_local_connection(
            &listener,
            "http dial listener",
            &expected_token,
        )
        .await
        {
            Ok(local) => local,
            Err(e) => {
                debug!("HTTP dial listener accept failed: {:?}", e);
                return;
            }
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

    Ok(lease)
}

async fn handle_socks5_client(
    handle: Arc<Mutex<Handle<SshHandler>>>,
    mut stream: TcpStream,
    mut shutdown: watch::Receiver<bool>,
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
            changed = shutdown.changed() => {
                if changed.is_err() || *shutdown.borrow() {
                    break;
                }
            }
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

    stream.shutdown().await.ok();
    channel.close().await.ok();
    Ok(())
}

pub(super) async fn socks5_client_handshake(
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

pub(super) async fn http_proxy_connect(
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
    fn ephemeral_lease_activation_succeeds_with_matching_token() {
        let rt = tokio::runtime::Runtime::new().expect("runtime should initialize");
        rt.block_on(async {
            let (lease, auth_rx, expected_token) = create_ephemeral_listener_lease(31337);
            let activated = lease.activate().expect("activation should succeed");
            assert_eq!(activated.local_port(), 31337);
            wait_for_ephemeral_activation(auth_rx, &expected_token, "test")
                .await
                .expect("token should match");
        });
    }

    #[test]
    fn ephemeral_activation_fails_when_not_activated() {
        let rt = tokio::runtime::Runtime::new().expect("runtime should initialize");
        rt.block_on(async {
            let (lease, auth_rx, expected_token) = create_ephemeral_listener_lease(31338);
            drop(lease);
            let result = wait_for_ephemeral_activation(auth_rx, &expected_token, "test").await;
            assert!(
                result.is_err(),
                "activation should fail when sender is dropped"
            );
        });
    }

    #[test]
    fn accept_authorized_local_connection_accepts_current_process_client() {
        let rt = tokio::runtime::Runtime::new().expect("runtime should initialize");
        rt.block_on(async {
            let listener = TcpListener::bind("127.0.0.1:0")
                .await
                .expect("listener should bind");
            let port = listener
                .local_addr()
                .expect("listener should have local addr")
                .port();
            let expected_token = "token-123";

            let accept_task = tokio::spawn(async move {
                let stream =
                    accept_authorized_local_connection(&listener, "test-listener", expected_token)
                        .await
                        .expect("self connection should be accepted");
                stream
                    .peer_addr()
                    .expect("accepted stream should have peer addr")
            });

            let mut client = TcpStream::connect(("127.0.0.1", port))
                .await
                .expect("client should connect");
            write_ephemeral_auth_line(&mut client, expected_token)
                .await
                .expect("client should authenticate");

            let peer_addr = accept_task.await.expect("accept task should finish");
            assert_eq!(peer_addr.ip().to_string(), "127.0.0.1");
        });
    }

    #[test]
    fn activated_listener_connect_sends_auth_line() {
        let rt = tokio::runtime::Runtime::new().expect("runtime should initialize");
        rt.block_on(async {
            let listener = TcpListener::bind("127.0.0.1:0")
                .await
                .expect("listener should bind");
            let port = listener
                .local_addr()
                .expect("listener should have local addr")
                .port();

            let server = tokio::spawn(async move {
                let (mut stream, _) = listener.accept().await.expect("accept should succeed");
                let mut buf = vec![0u8; 128];
                let n = stream.read(&mut buf).await.expect("read should succeed");
                String::from_utf8(buf[..n].to_vec()).expect("auth line should be utf8")
            });

            let activated = ActivatedEphemeralListener {
                local_port: port,
                auth_token: "token-456".to_string(),
            };
            let _stream = activated.connect().await.expect("connect should succeed");
            let auth_line = server.await.expect("server should finish");
            assert_eq!(auth_line, "STAR_SHUTTLE_AUTH token-456\n");
        });
    }
}
