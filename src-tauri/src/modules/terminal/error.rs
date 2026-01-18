use thiserror::Error;
use uuid::Uuid;

#[derive(Error, Debug, Clone)]
pub enum TerminalError {
    #[error("Invalid terminal size: cols={0}, rows={1}")]
    InvalidSize(u16, u16),

    #[error("Buffer overflow")]
    BufferOverflow,

    #[error("Invalid escape sequence: {0}")]
    InvalidEscapeSequence(String),

    #[error("Not connected")]
    NotConnected,

    #[error("IO error: {0}")]
    IoError(String),

    #[error("Terminal not found: {0}")]
    TerminalNotFound(Uuid),

    #[error("Invalid terminal operation: {0}")]
    InvalidOperation(String),

    #[error("Terminal resize error: {0}")]
    ResizeError(String),

    #[error("Other error: {0}")]
    Other(String),
}

impl From<std::io::Error> for TerminalError {
    fn from(error: std::io::Error) -> Self {
        Self::IoError(error.to_string())
    }
}
