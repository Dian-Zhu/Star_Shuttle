#![cfg(test)]

use super::*;
use russh::client::Prompt;
use std::thread;
use std::time::Duration;
use tokio::net::{TcpListener, TcpStream};

fn telnet_artifacts() -> TelnetConnection {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_io()
        .build()
        .expect("failed to create runtime");
    runtime.block_on(async {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("failed to bind listener");
        let addr = listener.local_addr().expect("failed to read listener addr");
        let client = TcpStream::connect(addr);
        let server = listener.accept();
        let (client, server) = tokio::join!(client, server);
        let client = client.expect("failed to connect client");
        let (server, _) = server.expect("failed to accept client");
        let _client = client;
        let (read, write) = server.into_split();
        TelnetConnection { read, write }
    })
}

#[test]
fn test_connection_config_validation() {
    let mut config = ConnectionConfig::default();
    assert!(config.validate().is_err());

    config.host = "localhost".to_string();
    config.port = 22;
    config.username = "user".to_string();
    config.auth_method = AuthMethod::Password {
        password: "password".to_string(),
        save_password: false,
    };
    assert!(config.validate().is_ok());

    config.port = 0;
    assert!(config.validate().is_err());
}

#[test]
fn test_rdp_config_validation() {
    let mut config = ConnectionConfig::default();
    config.protocol = ConnectionProtocol::Rdp;
    config.host = "192.168.1.10".to_string();
    config.port = 3389;
    config.username = "".to_string();
    config.auth_method = AuthMethod::Password {
        password: "".to_string(),
        save_password: false,
    };
    assert!(config.validate().is_ok());
    assert!(config.validate_for_save().is_ok());
}

#[test]
fn test_telnet_config_validation() {
    let mut config = ConnectionConfig::default();
    config.protocol = ConnectionProtocol::Telnet;
    config.host = "192.168.1.10".to_string();
    config.port = 23;
    config.username = "".to_string();
    config.auth_method = AuthMethod::Password {
        password: "".to_string(),
        save_password: false,
    };
    assert!(config.validate().is_ok());
    assert!(config.validate_for_save().is_ok());
}

#[test]
fn test_default_connection_manager_new() {
    let manager = DefaultConnectionManager::new();
    assert!(manager.connections.is_empty());
    assert!(manager.sessions.is_empty());
}

#[test]
fn test_save_and_get_connection_config() {
    let mut manager = DefaultConnectionManager::new();
    let mut config = ConnectionConfig::default();
    config.host = "192.168.1.1".to_string();
    config.port = 22;
    config.username = "admin".to_string();
    config.auth_method = AuthMethod::Password {
        password: "admin".to_string(),
        save_password: false,
    };

    assert!(manager.save_connection_config(config.clone()).is_ok());

    let configs = manager.get_all_connection_configs();
    assert_eq!(configs.len(), 1);
    assert_eq!(configs[0].host, "192.168.1.1");

    let id = configs[0].id;
    assert!(manager.delete_connection_config(&id).is_ok());
    assert!(manager.get_all_connection_configs().is_empty());
}

#[test]
fn test_keyboard_interactive_request_emit_failure_cleans_pending() {
    let coordinator = KeyboardInteractiveCoordinator::new();
    let request = ssh_impl::KeyboardInteractivePromptRequest {
        host: "127.0.0.1".to_string(),
        port: 22,
        username: "user".to_string(),
        name: "auth".to_string(),
        instructions: "instructions".to_string(),
        prompts: vec![Prompt {
            prompt: "Password:".to_string(),
            echo: false,
        }],
    };

    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_time()
        .build()
        .expect("failed to create runtime");
    let result = runtime.block_on(async {
        coordinator
            .request_with_emit(request, |_payload| Err(anyhow::anyhow!("emit failed")))
            .await
    });

    assert!(result.is_err());
    assert_eq!(coordinator.pending_len_for_test(), 0);
}

#[test]
fn test_finish_connect_success_does_not_revive_disconnected_session() {
    let mut manager = DefaultConnectionManager::new();
    let config_id = Uuid::new_v4();
    let session_id = Uuid::new_v4();
    manager.sessions.insert(
        session_id,
        SessionInfo {
            id: session_id,
            connection_id: config_id,
            status: ConnectionStatus::Disconnected,
            terminal_id: None,
            created_at: Utc::now(),
            last_active: Utc::now(),
        },
    );

    let result = manager.finish_connect_success(ConnectCompletion {
        session_id,
        connection_id: config_id,
        artifacts: ConnectArtifacts::Telnet(telnet_artifacts()),
    });

    assert!(result.is_err());
    assert_eq!(
        manager.sessions.get(&session_id).map(|s| &s.status),
        Some(&ConnectionStatus::Disconnected)
    );
    assert!(!manager.telnet_connections.contains_key(&session_id));
}

#[test]
fn test_finish_start_terminal_does_not_attach_to_disconnected_session() {
    let mut manager = DefaultConnectionManager::new();
    let session_id = Uuid::new_v4();
    manager.sessions.insert(
        session_id,
        SessionInfo {
            id: session_id,
            connection_id: Uuid::new_v4(),
            status: ConnectionStatus::Disconnected,
            terminal_id: None,
            created_at: Utc::now(),
            last_active: Utc::now(),
        },
    );

    let terminal_id = Uuid::new_v4();
    let (sender, _receiver) = mpsc::channel(1);
    let result = manager.finish_start_terminal(StartedTerminal {
        session_id,
        terminal_id,
        terminal: TerminalSession {
            id: terminal_id,
            session_id,
            sender,
        },
    });

    assert!(result.is_err());
    assert!(!manager.terminals.contains_key(&terminal_id));
    assert_eq!(
        manager
            .sessions
            .get(&session_id)
            .and_then(|s| s.terminal_id),
        None
    );
}

#[test]
fn test_send_terminal_data_fails_fast_when_queue_is_full() {
    let mut manager = DefaultConnectionManager::new();
    let session_id = Uuid::new_v4();
    let terminal_id = Uuid::new_v4();
    let (sender, _receiver) = mpsc::channel(1);
    sender
        .try_send(TerminalCommand::Data(vec![1]))
        .expect("failed to seed queue");
    manager.terminals.insert(
        terminal_id,
        TerminalSession {
            id: terminal_id,
            session_id,
            sender,
        },
    );

    let result = manager.send_terminal_data(&session_id, "queued");

    assert!(
        matches!(result, Err(ConnectionError::ConnectionFailed(message)) if message.contains("queue is full"))
    );
}

#[test]
fn test_finish_connect_failure_removes_connecting_session() {
    let mut manager = DefaultConnectionManager::new();
    let session_id = Uuid::new_v4();
    manager.sessions.insert(
        session_id,
        SessionInfo {
            id: session_id,
            connection_id: Uuid::new_v4(),
            status: ConnectionStatus::Connecting,
            terminal_id: None,
            created_at: Utc::now(),
            last_active: Utc::now(),
        },
    );

    manager.finish_connect_failure(session_id);

    assert!(!manager.sessions.contains_key(&session_id));
}

#[test]
fn test_disconnect_removes_session_entry() {
    let mut manager = DefaultConnectionManager::new();
    let session_id = Uuid::new_v4();
    manager.sessions.insert(
        session_id,
        SessionInfo {
            id: session_id,
            connection_id: Uuid::new_v4(),
            status: ConnectionStatus::Connected,
            terminal_id: None,
            created_at: Utc::now(),
            last_active: Utc::now(),
        },
    );

    assert!(manager.disconnect(&session_id).is_ok());
    assert!(!manager.sessions.contains_key(&session_id));
}

#[test]
fn test_close_terminal_waits_for_full_queue_before_removing_terminal() {
    let mut manager = DefaultConnectionManager::new();
    let session_id = Uuid::new_v4();
    let terminal_id = Uuid::new_v4();
    let (sender, mut receiver) = mpsc::channel(1);

    sender
        .try_send(TerminalCommand::Data(vec![1]))
        .expect("failed to seed queue");
    manager.terminals.insert(
        terminal_id,
        TerminalSession {
            id: terminal_id,
            session_id,
            sender,
        },
    );

    let drain = thread::spawn(move || {
        thread::sleep(Duration::from_millis(50));
        let first = receiver.blocking_recv();
        let second = receiver.blocking_recv();
        (first, second)
    });

    assert!(manager.close_terminal(&session_id).is_ok());
    assert!(manager.terminals.is_empty());

    let (_first, second) = drain.join().expect("failed to join drain thread");
    assert!(matches!(second, Some(TerminalCommand::Close)));
}
