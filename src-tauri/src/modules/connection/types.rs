use crate::modules::connection::ssh_impl::SshConnection;
use crate::modules::connection::tracking::ChannelTracker;
use crate::modules::connection::{
    ssh_impl, LocalForward, ProxyType, RemoteForward, TelnetConnection, TerminalSession,
};
use std::sync::{Arc, Mutex};
use tokio::runtime::Runtime;
use uuid::Uuid;

pub(crate) enum PreparedConnectOperation {
    Telnet {
        addr: String,
    },
    Ssh {
        host: String,
        port: u16,
        username: String,
        auth_type: ssh_impl::AuthType,
        local_forwards: Vec<LocalForward>,
        remote_forwards: Vec<RemoteForward>,
        proxy_type: ProxyType,
        socks_proxy_port: Option<u16>,
        keyboard_interactive_prompter: Option<Arc<dyn ssh_impl::KeyboardInteractivePrompter>>,
    },
}

pub(crate) struct PreparedConnect {
    pub(crate) session_id: Uuid,
    pub(crate) connection_id: Uuid,
    pub(crate) runtime: Arc<Runtime>,
    pub(crate) operation: PreparedConnectOperation,
}

pub(crate) enum ConnectArtifacts {
    Telnet(TelnetConnection),
    Ssh {
        connection: SshConnection,
        jump_connection: Option<SshConnection>,
    },
}

pub(crate) struct ConnectCompletion {
    pub(crate) session_id: Uuid,
    pub(crate) connection_id: Uuid,
    pub(crate) artifacts: ConnectArtifacts,
}

pub(crate) enum PreparedTerminalStartOperation {
    Telnet {
        telnet: TelnetConnection,
    },
    Ssh {
        ssh_connection: SshConnection,
        width: u16,
        height: u16,
    },
}

pub(crate) struct PreparedTerminalStart {
    pub(crate) session_id: Uuid,
    pub(crate) runtime: Arc<Runtime>,
    pub(crate) tracker: Arc<Mutex<ChannelTracker>>,
    pub(crate) operation: PreparedTerminalStartOperation,
}

pub(crate) struct StartedTerminal {
    pub(crate) session_id: Uuid,
    pub(crate) terminal_id: Uuid,
    pub(crate) terminal: TerminalSession,
}
