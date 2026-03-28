use anyhow::Error;
use log::{debug, error, info};
use russh::client::{Handle, Msg};
use std::collections::HashMap;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

use crate::modules::connection::{LocalForward, RemoteForward};

use super::{SshConnection, SshHandler};

pub(super) async fn forward_remote_connection_to_local(
    channel: russh::Channel<Msg>,
    connected_port: u32,
    local_host: String,
    local_port: u16,
) {
    info!(
        "Forwarding remote connection on port {} to local {}:{}",
        connected_port, local_host, local_port
    );

    match TcpStream::connect(format!("{}:{}", local_host, local_port)).await {
        Ok(mut socket) => {
            let mut channel = channel;
            tokio::spawn(async move {
                let mut buf = vec![0u8; 8192];
                loop {
                    tokio::select! {
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
                                _ => {}
                            }
                        }
                        n = socket.read(&mut buf) => {
                            match n {
                                Ok(n) if n > 0 => {
                                    if let Err(e) = channel.data(&buf[..n]).await {
                                        debug!("Failed to write to remote channel: {:?}", e);
                                        break;
                                    }
                                }
                                _ => break,
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
            let channel = channel;
            channel.close().await.ok();
        }
    }
}

pub(super) async fn setup_port_forwarding(
    connection: &SshConnection,
    local_forwards: &Vec<LocalForward>,
    remote_forwards: &Vec<RemoteForward>,
) -> Result<(), Error> {
    let handle_arc = connection.handle.clone();
    let shutdown_rx = connection.subscribe_shutdown();

    for forward in local_forwards {
        let local_host = forward.local_host.clone();
        let local_port = forward.local_port;
        let remote_host = forward.remote_host.clone();
        let remote_port = forward.remote_port;
        let handle_arc = handle_arc.clone();
        let mut shutdown = shutdown_rx.clone();

        info!(
            "Setting up local port forwarding: {}:{} -> {}:{}",
            local_host, local_port, remote_host, remote_port
        );

        tokio::spawn(async move {
            let listener = match TcpListener::bind(format!("{}:{}", local_host, local_port)).await {
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
                tokio::select! {
                    changed = shutdown.changed() => {
                        if changed.is_err() || *shutdown.borrow() {
                            info!(
                                "Stopping local port forwarding listener on {}:{}",
                                local_host, local_port
                            );
                            break;
                        }
                    }
                    accepted = listener.accept() => match accepted {
                    Ok((mut socket, addr)) => {
                        debug!("Accepted connection from {:?} for forwarding", addr);
                        let handle_arc = handle_arc.clone();
                        let remote_host = remote_host.clone();
                        let mut socket_shutdown = shutdown.clone();

                        tokio::spawn(async move {
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
                                    let mut buf = vec![0u8; 8192];
                                    loop {
                                        tokio::select! {
                                            changed = socket_shutdown.changed() => {
                                                if changed.is_err() || *socket_shutdown.borrow() {
                                                    break;
                                                }
                                            }
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
                                                    _ => {}
                                                }
                                            }
                                            n = socket.read(&mut buf) => {
                                                match n {
                                                    Ok(n) if n > 0 => {
                                                        if let Err(e) = channel.data(&buf[..n]).await {
                                                            debug!("Failed to write to remote channel: {:?}", e);
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
                }}
            }
        });
    }

    {
        let mut handle: tokio::sync::MutexGuard<'_, Handle<SshHandler>> =
            connection.handle.lock().await;

        for forward in remote_forwards {
            info!(
                "Requesting remote port forwarding: {}:{} -> {}:{}",
                forward.remote_host, forward.remote_port, forward.local_host, forward.local_port
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

pub(super) fn local_forward_target_for_port(
    remote_forward_mappings: &HashMap<u16, (String, u16)>,
    connected_port: u32,
) -> Option<(String, u16)> {
    remote_forward_mappings
        .get(&(connected_port as u16))
        .cloned()
}
