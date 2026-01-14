use thiserror::Error;

#[derive(Error, Debug)]
pub enum AppError {
    #[error("Configuration error: {0}")]
    ConfigError(String),
    
    #[error("Connection error: {0}")]
    ConnectionError(String),
    
    #[error("Authentication error: {0}")]
    AuthError(String),
    
    #[error("Database error: {0}")]
    DatabaseError(#[from] rusqlite::Error),
    
    #[error("Credential error: {0}")]
    CredentialError(String),
    
    #[error("File transfer error: {0}")]
    FileTransferError(String),
    
    #[error("Terminal error: {0}")]
    TerminalError(#[from] crate::modules::terminal::error::TerminalError),
    
    #[error("Session error: {0}")]
    SessionError(String),
    
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
    
    #[error("SSH error: {0}")]
    SshError(String),
    
    #[error("SFTP error: {0}")]
    SftpError(String),
    
    #[error("Keyring error: {0}")]
    KeyringError(#[from] keyring::Error),
    
    #[error("UUID parsing error: {0}")]
    UuidError(#[from] uuid::Error),
    
    #[error("Serialization error: {0}")]
    SerializationError(#[from] serde_json::Error),
    
    #[error("Unknown error: {0}")]
    UnknownError(String),
}

pub type Result<T> = std::result::Result<T, AppError>;