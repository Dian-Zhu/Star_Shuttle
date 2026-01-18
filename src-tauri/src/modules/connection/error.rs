use thiserror::Error;

#[derive(Error, Debug, Clone)]
pub enum ConnectionError {
    #[error("Connection timed out after {0} seconds")]
    Timeout(u64),

    #[error("Authentication failed: {0}")]
    AuthFailed(String),

    #[error("Host key verification failed: {0}")]
    HostKeyVerificationFailed(String),

    #[error("Connection refused")]
    ConnectionRefused,

    #[error("Host not found: {0}")]
    HostNotFound(String),

    #[error("Invalid configuration: {0}")]
    InvalidConfig(String),

    #[error("Connection not found: {0}")]
    NotFound(String),

    #[error("Not connected")]
    NotConnected,

    #[error("Channel error: {0}")]
    ChannelError(String),

    #[error("SSH protocol error: {0}")]
    ProtocolError(String),

    #[error("IO error: {0}")]
    IoError(String),

    #[error("Unknown error: {0}")]
    Unknown(String),
}

impl From<std::io::Error> for ConnectionError {
    fn from(error: std::io::Error) -> Self {
        Self::IoError(error.to_string())
    }
}

impl From<russh::Error> for ConnectionError {
    fn from(error: russh::Error) -> Self {
        // Map all russh errors to ProtocolError for now
        Self::ProtocolError(error.to_string())
    }
}
