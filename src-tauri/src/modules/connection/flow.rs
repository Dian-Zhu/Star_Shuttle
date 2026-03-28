use crate::modules::connection::tracking::ChannelTracker;
use crate::modules::connection::{
    try_send_terminal_command, ConnectArtifacts, ConnectCompletion, ConnectionConfig,
    ConnectionError, ConnectionProtocol, ConnectionStatus, PreparedTerminalStart,
    PreparedTerminalStartOperation, SessionInfo, SshConnection, StartedTerminal, TelnetConnection,
    TerminalCommand, TerminalSession,
};
use chrono::Utc;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tokio::runtime::Runtime;
use uuid::Uuid;

fn ensure_terminal_start_allowed(session: &SessionInfo) -> Result<(), ConnectionError> {
    if session.status != ConnectionStatus::Connected {
        return Err(ConnectionError::ConnectionFailed(
            "Session is not connected".to_string(),
        ));
    }
    if session.terminal_id.is_some() {
        return Err(ConnectionError::ConnectionFailed(
            "Terminal is already started for this session".to_string(),
        ));
    }
    Ok(())
}

pub(crate) fn clear_session_terminal_if_matches(
    sessions: &mut HashMap<Uuid, SessionInfo>,
    session_id: Uuid,
    expected_terminal_id: Option<Uuid>,
) -> bool {
    let Some(session) = sessions.get_mut(&session_id) else {
        return false;
    };

    if let Some(expected) = expected_terminal_id {
        if session.terminal_id != Some(expected) {
            return false;
        }
    } else if session.terminal_id.is_none() {
        return false;
    }

    session.terminal_id = None;
    true
}

pub(crate) fn new_connecting_session(session_id: Uuid, connection_id: Uuid) -> SessionInfo {
    SessionInfo {
        id: session_id,
        connection_id,
        status: ConnectionStatus::Connecting,
        terminal_id: None,
        created_at: Utc::now(),
        last_active: Utc::now(),
    }
}

pub(crate) fn finish_connect_success(
    sessions: &mut HashMap<Uuid, SessionInfo>,
    telnet_connections: &mut HashMap<Uuid, TelnetConnection>,
    ssh_connections: &mut HashMap<Uuid, SshConnection>,
    jump_ssh_connections: &mut HashMap<Uuid, SshConnection>,
    completion: ConnectCompletion,
) -> Result<Uuid, ConnectionError> {
    let mut session = sessions
        .get(&completion.session_id)
        .cloned()
        .ok_or(ConnectionError::SessionNotFound(completion.session_id))?;
    if session.status != ConnectionStatus::Connecting {
        return Err(ConnectionError::ConnectionFailed(
            "Connection attempt was canceled".to_string(),
        ));
    }
    session.connection_id = completion.connection_id;
    session.status = ConnectionStatus::Connected;
    sessions.insert(completion.session_id, session);

    match completion.artifacts {
        ConnectArtifacts::Telnet(telnet) => {
            telnet_connections.insert(completion.session_id, telnet);
        }
        ConnectArtifacts::Ssh {
            connection,
            jump_connection,
        } => {
            ssh_connections.insert(completion.session_id, connection);
            if let Some(jump) = jump_connection {
                jump_ssh_connections.insert(completion.session_id, jump);
            }
        }
    }

    Ok(completion.session_id)
}

pub(crate) fn finish_connect_failure(sessions: &mut HashMap<Uuid, SessionInfo>, session_id: Uuid) {
    if matches!(
        sessions.get(&session_id).map(|session| &session.status),
        Some(ConnectionStatus::Connecting)
    ) {
        sessions.remove(&session_id);
    }
}

pub(crate) fn prepare_disconnect(
    sessions: &mut HashMap<Uuid, SessionInfo>,
    session_id: Uuid,
) -> Result<(), ConnectionError> {
    let session = sessions
        .get_mut(&session_id)
        .ok_or(ConnectionError::SessionNotFound(session_id))?;

    if !matches!(
        session.status,
        ConnectionStatus::Disconnecting | ConnectionStatus::Disconnected
    ) {
        session.status = ConnectionStatus::Disconnecting;
    }

    Ok(())
}

pub(crate) fn finish_disconnect(
    sessions: &mut HashMap<Uuid, SessionInfo>,
    ssh_connections: &mut HashMap<Uuid, SshConnection>,
    jump_ssh_connections: &mut HashMap<Uuid, SshConnection>,
    telnet_connections: &mut HashMap<Uuid, TelnetConnection>,
    session_id: Uuid,
) {
    if let Some(connection) = ssh_connections.remove(&session_id) {
        connection.shutdown_background_tasks();
    }
    if let Some(connection) = jump_ssh_connections.remove(&session_id) {
        connection.shutdown_background_tasks();
    }
    let _ = telnet_connections.remove(&session_id);
    sessions.remove(&session_id);
}

pub(crate) fn prepare_start_terminal(
    sessions: &HashMap<Uuid, SessionInfo>,
    connections: &HashMap<Uuid, ConnectionConfig>,
    telnet_connections: &mut HashMap<Uuid, TelnetConnection>,
    ssh_connections: &HashMap<Uuid, SshConnection>,
    runtime: &Arc<Runtime>,
    tracker: &Arc<Mutex<ChannelTracker>>,
    session_id: &Uuid,
    width: u16,
    height: u16,
) -> Result<PreparedTerminalStart, ConnectionError> {
    let session = sessions
        .get(session_id)
        .ok_or(ConnectionError::SessionNotFound(*session_id))?;
    ensure_terminal_start_allowed(session)?;

    let protocol = connections
        .get(&session.connection_id)
        .map(|c| c.protocol.clone())
        .unwrap_or_default();

    let operation = if protocol == ConnectionProtocol::Telnet {
        let telnet = telnet_connections
            .remove(session_id)
            .ok_or(ConnectionError::SessionNotFound(*session_id))?;
        PreparedTerminalStartOperation::Telnet { telnet }
    } else {
        let ssh_connection = ssh_connections
            .get(session_id)
            .ok_or(ConnectionError::SessionNotFound(*session_id))?
            .clone();
        PreparedTerminalStartOperation::Ssh {
            ssh_connection,
            width,
            height,
        }
    };

    Ok(PreparedTerminalStart {
        session_id: *session_id,
        runtime: Arc::clone(runtime),
        tracker: Arc::clone(tracker),
        operation,
    })
}

pub(crate) fn finish_start_terminal(
    sessions: &mut HashMap<Uuid, SessionInfo>,
    terminals: &mut HashMap<Uuid, TerminalSession>,
    started: StartedTerminal,
) -> Result<bool, ConnectionError> {
    let session = sessions
        .get(&started.session_id)
        .ok_or(ConnectionError::SessionNotFound(started.session_id))?;
    if let Err(err) = ensure_terminal_start_allowed(session) {
        let _ = try_send_terminal_command(&started.terminal.sender, TerminalCommand::Close);
        return Err(err);
    }

    terminals.insert(started.terminal_id, started.terminal);

    if let Some(session) = sessions.get_mut(&started.session_id) {
        session.terminal_id = Some(started.terminal_id);
    }

    Ok(true)
}
