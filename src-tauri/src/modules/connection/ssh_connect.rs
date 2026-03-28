use crate::modules::connection::ssh_impl::SshConnection;
use crate::modules::connection::{ssh_impl, LocalForward, ProxyType, RemoteForward};
use std::sync::Arc;

pub(crate) async fn connect_ssh_via_proxy(
    host: String,
    port: u16,
    username: String,
    auth_type: ssh_impl::AuthType,
    local_forwards: Vec<LocalForward>,
    remote_forwards: Vec<RemoteForward>,
    proxy_type: ProxyType,
    socks_proxy_port: Option<u16>,
    keyboard_interactive_prompter: Option<Arc<dyn ssh_impl::KeyboardInteractivePrompter>>,
) -> Result<(SshConnection, Option<SshConnection>), anyhow::Error> {
    match proxy_type {
        ProxyType::JumpHost {
            host: jump_host,
            port: jump_port,
            username: jump_username,
            auth_method: jump_auth_method,
        } => {
            let jump_auth_type = super::auth_method_to_auth_type(jump_auth_method);
            let jump_connection = ssh_impl::connect_ssh(
                &jump_host,
                jump_port,
                &jump_username,
                jump_auth_type,
                &Vec::new(),
                &Vec::new(),
                None,
                keyboard_interactive_prompter.clone(),
            )
            .await?;

            let listener_lease = ssh_impl::start_ephemeral_direct_tcpip_listener(
                jump_connection.handle.clone(),
                host.clone(),
                port,
            )
            .await?;
            let activated_listener = listener_lease.activate()?;
            let local_stream = activated_listener.connect().await?;

            let target_connection = ssh_impl::connect_ssh_with_known_host_stream(
                local_stream,
                &host,
                port,
                &host,
                port,
                &username,
                auth_type,
                &local_forwards,
                &remote_forwards,
                socks_proxy_port,
                keyboard_interactive_prompter.clone(),
            )
            .await?;

            Ok((target_connection, Some(jump_connection)))
        }
        ProxyType::Socks5 {
            host: proxy_host,
            port: proxy_port,
            username: proxy_username,
            password: proxy_password,
            ..
        } => {
            let listener_lease = ssh_impl::start_ephemeral_socks5_proxy_dial_listener(
                proxy_host,
                proxy_port,
                proxy_username,
                proxy_password,
                host.clone(),
                port,
            )
            .await?;
            let activated_listener = listener_lease.activate()?;
            let local_stream = activated_listener.connect().await?;

            let target_connection = ssh_impl::connect_ssh_with_known_host_stream(
                local_stream,
                &host,
                port,
                &host,
                port,
                &username,
                auth_type,
                &local_forwards,
                &remote_forwards,
                socks_proxy_port,
                keyboard_interactive_prompter.clone(),
            )
            .await?;

            Ok((target_connection, None))
        }
        ProxyType::Http {
            host: proxy_host,
            port: proxy_port,
            username: proxy_username,
            password: proxy_password,
            ..
        } => {
            let listener_lease = ssh_impl::start_ephemeral_http_proxy_dial_listener(
                proxy_host,
                proxy_port,
                proxy_username,
                proxy_password,
                host.clone(),
                port,
            )
            .await?;
            let activated_listener = listener_lease.activate()?;
            let local_stream = activated_listener.connect().await?;

            let target_connection = ssh_impl::connect_ssh_with_known_host_stream(
                local_stream,
                &host,
                port,
                &host,
                port,
                &username,
                auth_type,
                &local_forwards,
                &remote_forwards,
                socks_proxy_port,
                keyboard_interactive_prompter.clone(),
            )
            .await?;

            Ok((target_connection, None))
        }
        ProxyType::None => {
            let target_connection = ssh_impl::connect_ssh(
                &host,
                port,
                &username,
                auth_type,
                &local_forwards,
                &remote_forwards,
                socks_proxy_port,
                keyboard_interactive_prompter.clone(),
            )
            .await?;
            Ok((target_connection, None))
        }
    }
}
