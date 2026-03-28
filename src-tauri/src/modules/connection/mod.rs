use chrono::{DateTime, Utc};
use log::{debug, error, info, warn};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use thiserror::Error;
use uuid::Uuid;

// Re-export submodules
pub mod auth;
mod auth_mapping;
mod command_exec;
mod config;
mod config_store;
mod connect_helpers;
mod connect_probe;
mod credential_sync;
pub mod error;
mod flow;
mod keyboard_interactive;
pub(crate) mod known_hosts;
mod manager_shell;
mod ssh_connect;
pub(crate) mod ssh_impl;
mod telnet;
mod terminal_control;
mod terminal_exec;
#[cfg(test)]
mod tests;
pub(crate) mod tracking;
mod types;
use self::auth_mapping::auth_method_to_auth_type;
pub use self::config::{
    AuthMethod, ConnectionConfig, ConnectionProtocol, ConnectionStatus, LocalForward, ProxyType,
    RemoteForward,
};
use self::connect_helpers::{connect_telnet, immediate_hop, preflight_connectivity_check};
use self::credential_sync::fill_saved_credentials;
use self::flow::{
    clear_session_terminal_if_matches as clear_session_terminal_if_matches_flow,
    finish_connect_failure as finish_connect_failure_flow,
    finish_connect_success as finish_connect_success_flow,
    finish_disconnect as finish_disconnect_flow,
    finish_start_terminal as finish_start_terminal_flow, new_connecting_session,
    prepare_disconnect as prepare_disconnect_flow,
    prepare_start_terminal as prepare_start_terminal_flow,
};
pub use self::keyboard_interactive::{
    KeyboardInteractiveCoordinator, SSH_KEYBOARD_INTERACTIVE_EVENT,
};
use self::manager_shell::{
    delete_connection_config as manager_delete_connection_config,
    get_all_connection_configs as manager_get_all_connection_configs,
    get_all_sessions as manager_get_all_sessions,
    get_connection_config as manager_get_connection_config, get_session as manager_get_session,
    save_connection_config as manager_save_connection_config,
};
use self::ssh_connect::connect_ssh_via_proxy;
use self::telnet::TelnetConnection;
pub use self::terminal_control::{
    try_send_terminal_command, PreparedTerminalClose, TerminalCommand, TerminalSession,
};
use self::terminal_exec::execute_prepared_terminal_start as execute_prepared_terminal_start_impl;
use self::types::{
    ConnectArtifacts, ConnectCompletion, PreparedConnect, PreparedConnectOperation,
    PreparedTerminalStart, PreparedTerminalStartOperation, StartedTerminal,
};

// Session information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionInfo {
    pub id: Uuid,
    pub connection_id: Uuid,
    pub status: ConnectionStatus,
    pub terminal_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub last_active: DateTime<Utc>,
}

// Connection manager trait
pub trait ConnectionManager {
    fn connect(
        &mut self,
        app: &tauri::AppHandle,
        config: &ConnectionConfig,
    ) -> Result<Uuid, ConnectionError>;
    fn disconnect(&mut self, session_id: &Uuid) -> Result<(), ConnectionError>;
    fn get_session(&self, session_id: &Uuid) -> Option<&SessionInfo>;
    fn get_all_sessions(&self) -> Vec<SessionInfo>;
    fn get_connection_config(&self, connection_id: &Uuid) -> Option<&ConnectionConfig>;
    fn save_connection_config(&mut self, config: ConnectionConfig) -> Result<(), ConnectionError>;
    fn delete_connection_config(&mut self, connection_id: &Uuid) -> Result<(), ConnectionError>;
    fn get_all_connection_configs(&self) -> Vec<ConnectionConfig>;
    fn test_connection(
        &self,
        app: &tauri::AppHandle,
        config: &ConnectionConfig,
    ) -> Result<(), ConnectionError>;

    // Terminal methods
    fn start_terminal(
        &mut self,
        app: &tauri::AppHandle,
        session_id: &Uuid,
        width: u16,
        height: u16,
    ) -> Result<bool, ConnectionError>;
    fn send_terminal_data(&mut self, session_id: &Uuid, data: &str) -> Result<(), ConnectionError>;
    fn resize_terminal(
        &mut self,
        session_id: &Uuid,
        width: u16,
        height: u16,
    ) -> Result<(), ConnectionError>;
    fn close_terminal(&mut self, session_id: &Uuid) -> Result<(), ConnectionError>;
    fn get_terminal_sender(&self, session_id: &Uuid) -> Option<mpsc::Sender<TerminalCommand>>;
    fn log_terminal_data(&self, session_id: &Uuid, data: &[u8], direction: &str);

    // Command execution
    fn exec_command(&self, session_id: &Uuid, command: &str) -> Result<String, ConnectionError>;
}

// Connection errors
#[derive(Error, Debug)]
pub enum ConnectionError {
    #[error("Invalid connection configuration: {0}")]
    InvalidConfig(String),
    #[error("Authentication failed: {0}")]
    AuthenticationError(String),
    #[error("Connection failed: {0}")]
    ConnectionFailed(String),
    #[error("Session not found: {0}")]
    SessionNotFound(Uuid),
    #[error("Connection not found: {0}")]
    ConnectionNotFound(Uuid),
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
    #[error("SSH error: {0}")]
    SshError(String),
    #[error("Database error: {0}")]
    DatabaseError(String),
    #[error("Credential error: {0}")]
    CredentialError(String),
    #[error("Other error: {0}")]
    Other(String),
}

use self::keyboard_interactive::TauriKeyboardInteractivePrompter;
use crate::modules::connection::ssh_impl::SshConnection;
use crate::modules::connection::tracking::ChannelTracker;
use crate::modules::credential::CredentialManager;
use crate::modules::db::DatabaseManager;
use std::sync::{Arc, Mutex};
use tokio::runtime::Runtime;
use tokio::sync::mpsc;

// Default connection manager implementation
pub struct DefaultConnectionManager {
    connections: HashMap<Uuid, ConnectionConfig>,
    sessions: HashMap<Uuid, SessionInfo>,
    ssh_connections: HashMap<Uuid, SshConnection>,
    jump_ssh_connections: HashMap<Uuid, SshConnection>,
    telnet_connections: HashMap<Uuid, TelnetConnection>,
    terminals: HashMap<Uuid, TerminalSession>,
    tracker: Arc<Mutex<ChannelTracker>>,
    runtime: Arc<Runtime>,
    db: Option<Arc<Mutex<DatabaseManager>>>,
    credential_manager: CredentialManager,
    keyboard_interactive: KeyboardInteractiveCoordinator,
}

// Manual Debug implementation to handle the non-Debug ssh_connections and runtime fields
impl std::fmt::Debug for DefaultConnectionManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DefaultConnectionManager")
            .field("connections", &self.connections)
            .field("sessions", &self.sessions)
            .field("ssh_connections_count", &self.ssh_connections.len())
            .field("telnet_connections_count", &self.telnet_connections.len())
            .field("terminals_count", &self.terminals.len())
            .finish()
    }
}

impl Default for DefaultConnectionManager {
    fn default() -> Self {
        Self::new()
    }
}

impl DefaultConnectionManager {
    fn should_disconnect_after_terminal_exit(exit_reason: &str) -> bool {
        matches!(
            exit_reason,
            "connection_lost" | "server_closed" | "keepalive_failed" | "read_error" | "write_error"
        )
    }

    pub fn new() -> Self {
        let runtime = Runtime::new().expect("Failed to create Tokio runtime");
        Self {
            connections: HashMap::new(),
            sessions: HashMap::new(),
            ssh_connections: HashMap::new(),
            jump_ssh_connections: HashMap::new(),
            telnet_connections: HashMap::new(),
            terminals: HashMap::new(),
            tracker: Arc::new(Mutex::new(ChannelTracker::new())),
            runtime: Arc::new(runtime),
            db: None,
            credential_manager: CredentialManager::new(),
            keyboard_interactive: KeyboardInteractiveCoordinator::new(),
        }
    }

    pub fn keyboard_interactive_coordinator(&self) -> KeyboardInteractiveCoordinator {
        self.keyboard_interactive.clone()
    }

    pub fn set_db(&mut self, db: Arc<Mutex<DatabaseManager>>) -> Result<(), ConnectionError> {
        self.db = Some(db);
        if let Some(db) = self.db.as_ref() {
            self.credential_manager.set_db(db.clone());
        }
        config_store::load_connection_configs_from_db(&self.db, &mut self.connections)?;
        Ok(())
    }

    pub fn get_ssh_connection(&self, id: &Uuid) -> Option<SshConnection> {
        self.ssh_connections.get(id).cloned()
    }

    pub(crate) fn prepare_connect(
        &mut self,
        app: &tauri::AppHandle,
        config: &ConnectionConfig,
    ) -> Result<PreparedConnect, ConnectionError> {
        if config.protocol == ConnectionProtocol::Rdp {
            return Err(ConnectionError::InvalidConfig(
                "RDP does not support in-app sessions".to_string(),
            ));
        }

        info!("Starting connection attempt for config: {:?}", config.id);
        info!("Validating connection configuration...");
        let mut effective_config = config.clone();
        fill_saved_credentials(&self.credential_manager, &mut effective_config)?;
        effective_config.validate()?;
        info!("Connection configuration validated successfully");

        let session_id = Uuid::new_v4();
        let session_info = new_connecting_session(session_id, config.id);
        self.sessions.insert(session_id, session_info);

        if effective_config.protocol == ConnectionProtocol::Telnet {
            let addr = format!("{}:{}", effective_config.host, effective_config.port);
            return Ok(PreparedConnect {
                session_id,
                connection_id: config.id,
                runtime: Arc::clone(&self.runtime),
                operation: PreparedConnectOperation::Telnet { addr },
            });
        }

        let auth_type = auth_method_to_auth_type(effective_config.auth_method.clone());
        let keyboard_interactive_prompter: Option<Arc<dyn ssh_impl::KeyboardInteractivePrompter>> =
            Some(Arc::new(TauriKeyboardInteractivePrompter {
                app: app.clone(),
                coordinator: self.keyboard_interactive.clone(),
            }));

        info!(
            "Prepared SSH connection to {}:{} as {}",
            effective_config.host, effective_config.port, effective_config.username
        );

        Ok(PreparedConnect {
            session_id,
            connection_id: config.id,
            runtime: Arc::clone(&self.runtime),
            operation: PreparedConnectOperation::Ssh {
                host: effective_config.host,
                port: effective_config.port,
                username: effective_config.username,
                auth_type,
                local_forwards: effective_config.local_forwards,
                remote_forwards: effective_config.remote_forwards,
                proxy_type: effective_config.proxy_type,
                socks_proxy_port: effective_config.socks_proxy_port,
                keyboard_interactive_prompter,
            },
        })
    }

    pub(crate) fn execute_prepared_connect(
        prepared: PreparedConnect,
    ) -> Result<ConnectCompletion, ConnectionError> {
        match prepared.operation {
            PreparedConnectOperation::Telnet { addr } => {
                let telnet = connect_telnet(&prepared.runtime, &addr)?;
                Ok(ConnectCompletion {
                    session_id: prepared.session_id,
                    connection_id: prepared.connection_id,
                    artifacts: ConnectArtifacts::Telnet(telnet),
                })
            }
            PreparedConnectOperation::Ssh {
                host,
                port,
                username,
                auth_type,
                local_forwards,
                remote_forwards,
                proxy_type,
                socks_proxy_port,
                keyboard_interactive_prompter,
            } => {
                let (check_host, check_port) = immediate_hop(&proxy_type, &host, port);
                preflight_connectivity_check(&prepared.runtime, &check_host, check_port)?;

                info!(
                    "Starting blocking connection task for session {}",
                    prepared.session_id
                );
                let start_time = std::time::Instant::now();
                let connect_res: Result<(SshConnection, Option<SshConnection>), anyhow::Error> =
                    prepared.runtime.block_on(connect_ssh_via_proxy(
                        host,
                        port,
                        username,
                        auth_type,
                        local_forwards,
                        remote_forwards,
                        proxy_type,
                        socks_proxy_port,
                        keyboard_interactive_prompter,
                    ));

                info!(
                    "Blocking connection task finished in {:?}",
                    start_time.elapsed()
                );

                match connect_res {
                    Ok((connection, jump_connection)) => Ok(ConnectCompletion {
                        session_id: prepared.session_id,
                        connection_id: prepared.connection_id,
                        artifacts: ConnectArtifacts::Ssh {
                            connection,
                            jump_connection,
                        },
                    }),
                    Err(e) => {
                        error!(
                            "SSH connection error for session {}: {:?}",
                            prepared.session_id, e
                        );
                        Err(ConnectionError::ConnectionFailed(format!("{:?}", e)))
                    }
                }
            }
        }
    }

    pub(crate) fn finish_connect_success(
        &mut self,
        completion: ConnectCompletion,
    ) -> Result<Uuid, ConnectionError> {
        finish_connect_success_flow(
            &mut self.sessions,
            &mut self.telnet_connections,
            &mut self.ssh_connections,
            &mut self.jump_ssh_connections,
            completion,
        )
    }

    pub(crate) fn finish_connect_failure(&mut self, session_id: Uuid) {
        finish_connect_failure_flow(&mut self.sessions, session_id);
    }

    pub(crate) fn prepare_disconnect(&mut self, session_id: &Uuid) -> Result<(), ConnectionError> {
        prepare_disconnect_flow(&mut self.sessions, *session_id)
    }

    pub(crate) fn finish_disconnect(&mut self, session_id: &Uuid) {
        finish_disconnect_flow(
            &mut self.sessions,
            &mut self.ssh_connections,
            &mut self.jump_ssh_connections,
            &mut self.telnet_connections,
            *session_id,
        );
        if let Ok(mut tracker) = self.tracker.lock() {
            tracker.unregister_session(session_id);
        }
    }

    pub(crate) fn prepare_start_terminal(
        &mut self,
        session_id: &Uuid,
        width: u16,
        height: u16,
    ) -> Result<PreparedTerminalStart, ConnectionError> {
        prepare_start_terminal_flow(
            &self.sessions,
            &self.connections,
            &mut self.telnet_connections,
            &self.ssh_connections,
            &self.runtime,
            &self.tracker,
            session_id,
            width,
            height,
        )
    }

    pub(crate) fn execute_prepared_terminal_start(
        app: &tauri::AppHandle,
        prepared: PreparedTerminalStart,
    ) -> Result<StartedTerminal, ConnectionError> {
        execute_prepared_terminal_start_impl(app, prepared)
    }

    pub(crate) fn finish_start_terminal(
        &mut self,
        started: StartedTerminal,
    ) -> Result<bool, ConnectionError> {
        finish_start_terminal_flow(&mut self.sessions, &mut self.terminals, started)
    }

    pub(crate) fn prepare_close_terminal(
        &self,
        session_id: &Uuid,
    ) -> Result<PreparedTerminalClose, ConnectionError> {
        terminal_control::prepare_terminal_close(&self.terminals, session_id)
    }

    pub(crate) fn execute_prepared_terminal_close(
        prepared: &PreparedTerminalClose,
    ) -> Result<(), ConnectionError> {
        terminal_control::execute_prepared_terminal_close(prepared)
    }

    pub(crate) fn finish_close_terminal(&mut self, prepared: &PreparedTerminalClose) {
        terminal_control::finish_terminal_close(&mut self.terminals, prepared);
        let _ = clear_session_terminal_if_matches_flow(
            &mut self.sessions,
            prepared.session_id,
            Some(prepared.terminal_id),
        );
    }

    pub(crate) fn handle_terminal_exit(&mut self, session_id: &Uuid, exit_reason: &str) {
        if let Some(terminal_id) =
            terminal_control::remove_terminal_by_session(&mut self.terminals, session_id)
        {
            let _ = clear_session_terminal_if_matches_flow(
                &mut self.sessions,
                *session_id,
                Some(terminal_id),
            );
        }

        if Self::should_disconnect_after_terminal_exit(exit_reason) {
            match self.prepare_disconnect(session_id) {
                Ok(()) => {
                    self.finish_disconnect(session_id);
                    info!(
                        "Session {} disconnected after terminal exit ({})",
                        session_id, exit_reason
                    );
                    return;
                }
                Err(ConnectionError::SessionNotFound(_)) => {
                    return;
                }
                Err(err) => {
                    warn!(
                        "Failed to prepare disconnect for session {} after terminal exit ({}): {}",
                        session_id, exit_reason, err
                    );
                }
            }
        }

        if let Ok(mut tracker) = self.tracker.lock() {
            tracker.unregister_session(session_id);
        }
    }
}

impl ConnectionManager for DefaultConnectionManager {
    fn connect(
        &mut self,
        app: &tauri::AppHandle,
        config: &ConnectionConfig,
    ) -> Result<Uuid, ConnectionError> {
        let prepared = self.prepare_connect(app, config)?;
        let session_id = prepared.session_id;

        match Self::execute_prepared_connect(prepared) {
            Ok(completion) => self.finish_connect_success(completion),
            Err(err) => {
                self.finish_connect_failure(session_id);
                Err(err)
            }
        }
    }

    fn disconnect(&mut self, session_id: &Uuid) -> Result<(), ConnectionError> {
        self.prepare_disconnect(session_id)?;
        info!("Disconnecting session: {}", session_id);

        if let Err(err) = self.close_terminal(session_id) {
            if !matches!(err, ConnectionError::SessionNotFound(_)) {
                warn!(
                    "Failed to close terminal while disconnecting session {}: {}",
                    session_id, err
                );
            }
        }

        self.finish_disconnect(session_id);
        info!("Session disconnected: {}", session_id);

        Ok(())
    }

    fn get_session(&self, session_id: &Uuid) -> Option<&SessionInfo> {
        manager_get_session(&self.sessions, session_id)
    }

    fn get_all_sessions(&self) -> Vec<SessionInfo> {
        manager_get_all_sessions(&self.sessions)
    }

    fn get_connection_config(&self, connection_id: &Uuid) -> Option<&ConnectionConfig> {
        manager_get_connection_config(&self.connections, connection_id)
    }

    fn save_connection_config(&mut self, config: ConnectionConfig) -> Result<(), ConnectionError> {
        manager_save_connection_config(
            &mut self.connections,
            &self.credential_manager,
            &self.db,
            config,
        )
    }

    fn delete_connection_config(&mut self, connection_id: &Uuid) -> Result<(), ConnectionError> {
        manager_delete_connection_config(
            &mut self.connections,
            &self.credential_manager,
            &self.db,
            connection_id,
        )
    }

    fn get_all_connection_configs(&self) -> Vec<ConnectionConfig> {
        manager_get_all_connection_configs(&self.connections)
    }

    fn test_connection(
        &self,
        app: &tauri::AppHandle,
        config: &ConnectionConfig,
    ) -> Result<(), ConnectionError> {
        connect_probe::test_connection(
            &self.runtime,
            app,
            config,
            &self.credential_manager,
            &self.keyboard_interactive,
        )
    }

    fn start_terminal(
        &mut self,
        app: &tauri::AppHandle,
        session_id: &Uuid,
        width: u16,
        height: u16,
    ) -> Result<bool, ConnectionError> {
        let prepared = self.prepare_start_terminal(session_id, width, height)?;
        let started = Self::execute_prepared_terminal_start(app, prepared)?;
        self.finish_start_terminal(started)
    }

    fn send_terminal_data(&mut self, session_id: &Uuid, data: &str) -> Result<(), ConnectionError> {
        terminal_control::send_terminal_data(&self.terminals, &self.tracker, session_id, data)
    }

    fn resize_terminal(
        &mut self,
        session_id: &Uuid,
        width: u16,
        height: u16,
    ) -> Result<(), ConnectionError> {
        terminal_control::resize_terminal(&self.terminals, session_id, width, height)?;
        debug!("Resizing terminal to {}x{}", width, height);
        Ok(())
    }

    fn close_terminal(&mut self, session_id: &Uuid) -> Result<(), ConnectionError> {
        terminal_control::close_terminal(&mut self.terminals, session_id)?;
        let _ = clear_session_terminal_if_matches_flow(&mut self.sessions, *session_id, None);

        info!("Terminal closed for session: {}", session_id);
        Ok(())
    }

    fn get_terminal_sender(&self, session_id: &Uuid) -> Option<mpsc::Sender<TerminalCommand>> {
        terminal_control::get_terminal_sender(&self.terminals, session_id)
    }

    fn log_terminal_data(&self, session_id: &Uuid, data: &[u8], direction: &str) {
        terminal_control::log_terminal_data(&self.tracker, session_id, data, direction);
    }

    fn exec_command(&self, session_id: &Uuid, command: &str) -> Result<String, ConnectionError> {
        let ssh_connection = self
            .ssh_connections
            .get(session_id)
            .ok_or(ConnectionError::SessionNotFound(*session_id))?
            .clone();

        command_exec::exec_command(&self.runtime, ssh_connection, command)
    }
}
