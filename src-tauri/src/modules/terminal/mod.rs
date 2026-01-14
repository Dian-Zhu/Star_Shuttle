use serde::{Deserialize, Serialize};
use uuid::Uuid;
use thiserror::Error;

// Re-export submodules
pub mod emulator;
pub mod parser;
pub mod buffer;
pub mod error;

// Terminal settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TerminalSettings {
    pub theme: String,
    pub font_size: u32,
    pub font_family: String,
    pub scrollback_lines: u32,
    pub cursor_style: String,
}

// Terminal emulator trait
pub trait TerminalEmulator {
    fn new(session_id: Uuid) -> Self;
    fn write(&mut self, data: &[u8]) -> Result<(), TerminalError>;
    fn read(&mut self) -> Vec<u8>;
    fn resize(&mut self, cols: u16, rows: u16) -> Result<(), TerminalError>;
    fn close(&mut self) -> Result<(), TerminalError>;
    fn get_settings(&self) -> &TerminalSettings;
    fn set_settings(&mut self, settings: TerminalSettings) -> Result<(), TerminalError>;
}

// Re-export error types from error module
pub use error::TerminalError;

// Default terminal emulator implementation
#[derive(Debug)]
pub struct DefaultTerminalEmulator {
    session_id: Uuid,
    settings: TerminalSettings,
    // Implementation-specific fields will be added here
}

impl TerminalEmulator for DefaultTerminalEmulator {
    fn new(session_id: Uuid) -> Self {
        DefaultTerminalEmulator {
            session_id,
            settings: TerminalSettings {
                theme: "dark".to_string(),
                font_size: 14,
                font_family: "Monaco, 'Courier New', monospace".to_string(),
                scrollback_lines: 1000,
                cursor_style: "block".to_string(),
            },
        }
    }

    fn write(&mut self, data: &[u8]) -> Result<(), TerminalError> {
        // Implementation will be added here
        Ok(())
    }

    fn read(&mut self) -> Vec<u8> {
        // Implementation will be added here
        Vec::new()
    }

    fn resize(&mut self, cols: u16, rows: u16) -> Result<(), TerminalError> {
        // Implementation will be added here
        Ok(())
    }

    fn close(&mut self) -> Result<(), TerminalError> {
        // Implementation will be added here
        Ok(())
    }

    fn get_settings(&self) -> &TerminalSettings {
        &self.settings
    }

    fn set_settings(&mut self, settings: TerminalSettings) -> Result<(), TerminalError> {
        self.settings = settings;
        Ok(())
    }
}