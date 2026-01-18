use super::{buffer::TerminalBuffer, parser::TerminalParser, TerminalEmulator};
use uuid::Uuid;

pub struct TerminalEmulatorImpl {
    #[allow(dead_code)]
    session_id: Uuid,
    settings: super::TerminalSettings,
    buffer: TerminalBuffer,
    parser: TerminalParser,
    input_buffer: Vec<u8>,
}

impl super::TerminalEmulator for TerminalEmulatorImpl {
    fn new(session_id: Uuid) -> Self {
        Self {
            session_id,
            settings: super::TerminalSettings {
                theme: "dark".to_string(),
                font_size: 14,
                font_family: "Monaco, 'Courier New', monospace".to_string(),
                scrollback_lines: 1000,
                cursor_style: "block".to_string(),
            },
            buffer: TerminalBuffer::new(80, 24).unwrap(),
            parser: TerminalParser::new(),
            input_buffer: Vec::new(),
        }
    }

    fn write(&mut self, data: &[u8]) -> std::result::Result<(), super::TerminalError> {
        // Parse the data and update the buffer
        self.parser
            .parse(data, &mut self.buffer)
            .map_err(|e| super::TerminalError::Other(e.to_string()))?;
        Ok(())
    }

    fn read(&mut self) -> Vec<u8> {
        // Return any pending input from the input buffer
        let mut result = Vec::new();
        std::mem::swap(&mut self.input_buffer, &mut result);
        result
    }

    fn resize(&mut self, cols: u16, rows: u16) -> std::result::Result<(), super::TerminalError> {
        // Resize the terminal buffer
        self.buffer
            .resize(cols, rows)
            .map_err(|e| super::TerminalError::ResizeError(e.to_string()))?;
        Ok(())
    }

    fn close(&mut self) -> std::result::Result<(), super::TerminalError> {
        // Clean up resources
        Ok(())
    }

    fn get_settings(&self) -> &super::TerminalSettings {
        &self.settings
    }

    fn set_settings(
        &mut self,
        settings: super::TerminalSettings,
    ) -> std::result::Result<(), super::TerminalError> {
        self.settings = settings;
        Ok(())
    }
}

#[derive(Default)]
pub struct TerminalEmulatorManager {
    emulators: std::collections::HashMap<Uuid, TerminalEmulatorImpl>,
}

impl TerminalEmulatorManager {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn create_emulator(&mut self, session_id: Uuid) -> Uuid {
        let emulator = TerminalEmulatorImpl::new(session_id);
        self.emulators.insert(session_id, emulator);
        session_id
    }

    pub fn get_emulator(&self, id: &Uuid) -> Option<&TerminalEmulatorImpl> {
        self.emulators.get(id)
    }

    pub fn get_mut_emulator(&mut self, id: &Uuid) -> Option<&mut TerminalEmulatorImpl> {
        self.emulators.get_mut(id)
    }

    pub fn remove_emulator(&mut self, id: &Uuid) -> std::result::Result<(), super::TerminalError> {
        if self.emulators.remove(id).is_some() {
            Ok(())
        } else {
            Err(super::TerminalError::TerminalNotFound(*id))
        }
    }

    pub fn get_all_emulators(&self) -> Vec<&TerminalEmulatorImpl> {
        self.emulators.values().collect()
    }

    pub fn write_to_emulator(
        &mut self,
        id: &Uuid,
        data: &[u8],
    ) -> std::result::Result<(), super::TerminalError> {
        if let Some(emulator) = self.get_mut_emulator(id) {
            emulator.write(data)
        } else {
            Err(super::TerminalError::TerminalNotFound(*id))
        }
    }

    pub fn read_from_emulator(&mut self, id: &Uuid) -> Vec<u8> {
        if let Some(emulator) = self.get_mut_emulator(id) {
            emulator.read()
        } else {
            Vec::new()
        }
    }

    pub fn resize_emulator(
        &mut self,
        id: &Uuid,
        cols: u16,
        rows: u16,
    ) -> std::result::Result<(), super::TerminalError> {
        if let Some(emulator) = self.get_mut_emulator(id) {
            emulator.resize(cols, rows)
        } else {
            Err(super::TerminalError::TerminalNotFound(*id))
        }
    }
}
