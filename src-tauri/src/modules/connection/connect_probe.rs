use crate::modules::connection::{
    auth_method_to_auth_type,
    connect_helpers::{immediate_hop, preflight_connectivity_check},
    credential_sync::fill_saved_credentials,
    keyboard_interactive::TauriKeyboardInteractivePrompter, ssh_connect::connect_ssh_via_proxy,
    ssh_impl, ConnectionConfig, ConnectionError, ConnectionProtocol,
    KeyboardInteractiveCoordinator,
};
use crate::modules::credential::CredentialManager;
use log::{error, info};
use std::sync::Arc;
use tauri::AppHandle;
use tokio::runtime::Runtime;

pub fn test_connection(
    runtime: &Runtime,
    app: &AppHandle,
    config: &ConnectionConfig,
    credential_manager: &CredentialManager,
    keyboard_interactive: &KeyboardInteractiveCoordinator,
) -> Result<(), ConnectionError> {
    match config.protocol {
        ConnectionProtocol::Rdp | ConnectionProtocol::Telnet => {
            test_tcp_connection(runtime, config)
        }
        ConnectionProtocol::Ssh => test_ssh_connection(
            runtime,
            app,
            config,
            credential_manager,
            keyboard_interactive,
        ),
    }
}

fn test_tcp_connection(
    runtime: &Runtime,
    config: &ConnectionConfig,
) -> Result<(), ConnectionError> {
    let effective_config = config.clone();
    effective_config.validate()?;

    let host = effective_config.host.clone();
    let port = effective_config.port;
    info!("Testing TCP connectivity to {}:{}", host, port);

    let addr = format!("{}:{}", host, port);
    let res = runtime.block_on(async move {
        tokio::time::timeout(
            std::time::Duration::from_secs(5),
            tokio::net::TcpStream::connect(addr),
        )
        .await
    });

    match res {
        Ok(Ok(_)) => Ok(()),
        Ok(Err(e)) => Err(ConnectionError::ConnectionFailed(e.to_string())),
        Err(_) => Err(ConnectionError::ConnectionFailed(
            "Connection test timed out".to_string(),
        )),
    }
}

fn test_ssh_connection(
    runtime: &Runtime,
    app: &tauri::AppHandle,
    config: &ConnectionConfig,
    credential_manager: &CredentialManager,
    keyboard_interactive: &KeyboardInteractiveCoordinator,
) -> Result<(), ConnectionError> {
    let mut effective_config = config.clone();
    fill_saved_credentials(credential_manager, &mut effective_config)?;
    effective_config.validate()?;

    let host = effective_config.host.clone();
    let port = effective_config.port;
    let username = effective_config.username.clone();
    let auth_method = effective_config.auth_method.clone();
    let proxy_type = effective_config.proxy_type.clone();

    info!("Testing connection to {}:{} as {}", host, port, username);

    let (check_host, check_port) = immediate_hop(&proxy_type, &host, port);
    preflight_connectivity_check(runtime, &check_host, check_port)?;

    let auth_type = auth_method_to_auth_type(auth_method);
    let keyboard_interactive_prompter: Option<Arc<dyn ssh_impl::KeyboardInteractivePrompter>> =
        Some(Arc::new(TauriKeyboardInteractivePrompter {
            app: app.clone(),
            coordinator: keyboard_interactive.clone(),
        }));

    let host_clone = host.clone();
    let port_clone = port;

    let res: Result<(), anyhow::Error> = runtime.block_on(async {
        connect_ssh_via_proxy(
            host,
            port,
            username,
            auth_type,
            Vec::new(),
            Vec::new(),
            proxy_type,
            None,
            keyboard_interactive_prompter,
        )
        .await?;
        Ok(())
    });

    match res {
        Ok(()) => {
            info!("Connection test successful: {}:{}", host_clone, port_clone);
            Ok(())
        }
        Err(e) => {
            error!("Connection test failed: {:?}", e);
            Err(ConnectionError::ConnectionFailed(format!("{:?}", e)))
        }
    }
}
