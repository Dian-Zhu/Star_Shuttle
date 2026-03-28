use anyhow::anyhow;
use base64::{engine::general_purpose, Engine as _};
use log::{debug, info};
use russh::client::Handle;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{watch, Mutex};

use super::SshHandler;

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
) -> Result<u16, anyhow::Error> {
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let local_port = listener.local_addr()?.port();

    info!(
        "Listening on 127.0.0.1:{} for forwarding to {}:{}",
        local_port, remote_host, remote_port
    );

    tokio::spawn(async move {
        let accepted = listener.accept().await;
        let Ok((mut socket, addr)) = accepted else {
            return;
        };

        debug!("Accepted connection from {:?} for jump forwarding", addr);
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
