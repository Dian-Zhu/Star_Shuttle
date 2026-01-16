use super::error::TerminalError; use crate::modules::error::Result; use super::buffer::{TerminalBuffer, Cell}; use log::warn;

#[derive(Debug, Clone, PartialEq)]
pub enum ControlSequence {
    CursorUp(u16),
    CursorDown(u16),
    CursorForward(u16),
    CursorBack(u16),
    CursorNextLine(u16),
    CursorPrecedingLine(u16),
    CursorHorizontalAbsolute(u16),
    CursorPosition(u16, u16), // row, col
    EraseInDisplay(u8),
    EraseInLine(u8),
    ScrollUp(u16),
    ScrollDown(u16),
    SetGraphicsRendition(Vec<u8>),
    Reset,
    Unknown(String),
}

pub struct TerminalParser {
    state: ParserState,
    params: Vec<u16>,
    intermediate: Vec<char>,
    final_char: Option<char>,
    escape_sequence: String,
}

#[derive(Debug, Clone, PartialEq)]
enum ParserState {
    Ground,
    Escape,
    Csi,
    Osc,
    Sgr,
}

impl Default for TerminalParser {
    fn default() -> Self {
        Self {
            state: ParserState::Ground,
            params: Vec::new(),
            intermediate: Vec::new(),
            final_char: None,
            escape_sequence: String::new(),
        }
    }
}

impl TerminalParser {
    pub fn new() -> Self {
        Self::default()
    }
    
    pub fn parse(&mut self, data: &[u8], buffer: &mut TerminalBuffer) -> Result<()> {
        for byte in data {
            self.process_byte(*byte, buffer)?;
        }
        Ok(())
    }
    
    fn process_byte(&mut self, byte: u8, buffer: &mut TerminalBuffer) -> Result<()> {
        match self.state {
            ParserState::Ground => self.process_ground(byte, buffer),
            ParserState::Escape => self.process_escape(byte, buffer),
            ParserState::Csi => self.process_csi(byte, buffer),
            ParserState::Osc => self.process_osc(byte, buffer),
            ParserState::Sgr => self.process_sgr(byte, buffer),
        }
    }
    
    fn process_ground(&mut self, byte: u8, buffer: &mut TerminalBuffer) -> Result<()> {
        match byte {
            0x1b => {
                self.state = ParserState::Escape;
                self.escape_sequence.push(0x1b as char);
            },
            // Control characters
            0x00 => self.handle_null(buffer),
            0x07 => self.handle_bell(buffer),
            0x08 => self.handle_backspace(buffer),
            0x09 => self.handle_tab(buffer),
            0x0a => self.handle_line_feed(buffer),
            0x0d => self.handle_carriage_return(buffer),
            0x0c => self.handle_form_feed(buffer),
            0x0e => self.handle_shift_out(buffer),
            0x0f => self.handle_shift_in(buffer),
            // Printable characters
            0x20..=0x7e => self.handle_printable(byte as char, buffer),
            _ => {},
        }
        Ok(())
    }
    
    fn process_escape(&mut self, byte: u8, buffer: &mut TerminalBuffer) -> Result<()> {
        self.escape_sequence.push(byte as char);
        
        match byte {
            b'[' => {
                self.state = ParserState::Csi;
                self.params.clear();
                self.intermediate.clear();
            },
            b']' => {
                self.state = ParserState::Osc;
            },
            // Other escape sequences
            _ => {
                // Handle simple escape sequences
                self.state = ParserState::Ground;
                self.handle_escape_sequence(&self.escape_sequence, buffer)?;
                self.escape_sequence.clear();
            },
        }
        Ok(())
    }
    
    fn process_csi(&mut self, byte: u8, buffer: &mut TerminalBuffer) -> Result<()> {
        self.escape_sequence.push(byte as char);
        
        match byte {
            b'0'..=b'9' => {
                // Parse parameter digits
                let last_param = if self.params.is_empty() {
                    self.params.push(0);
                    self.params.last_mut().unwrap()
                } else {
                    self.params.last_mut().unwrap()
                };
                *last_param = *last_param * 10 + (byte - b'0') as u16;
            },
            b';' => {
                // End of parameter, start new one
                self.params.push(0);
            },
            b'A'..=b'~' => {
                // Final character, process the CSI sequence
                let seq = self.escape_sequence.clone();
                let params = self.params.clone();
                self.handle_csi_sequence(byte as char, params, buffer)?;
                
                // Reset state
                self.state = ParserState::Ground;
                self.params.clear();
                self.intermediate.clear();
                self.escape_sequence.clear();
            },
            _ => {
                // Intermediate characters
                self.intermediate.push(byte as char);
            },
        }
        Ok(())
    }
    
    fn process_osc(&mut self, byte: u8, buffer: &mut TerminalBuffer) -> Result<()> {
        // Simple OSC handling - just ignore for now
        if byte == 0x07 || (byte == 0x1b && self.escape_sequence.len() > 2 && self.escape_sequence.as_bytes()[self.escape_sequence.len() - 2] == b'\\') {
            self.state = ParserState::Ground;
            self.escape_sequence.clear();
        } else {
            self.escape_sequence.push(byte as char);
        }
        Ok(())
    }
    
    fn process_sgr(&mut self, byte: u8, buffer: &mut TerminalBuffer) -> Result<()> {
        // SGR (Select Graphic Rendition) handling
        self.state = ParserState::Ground;
        Ok(())
    }
    
    fn handle_escape_sequence(&self, seq: &str, buffer: &mut TerminalBuffer) -> Result<()> {
        // Handle simple escape sequences
        Ok(())
    }
    
    fn handle_csi_sequence(&self, final_char: char, params: Vec<u16>, buffer: &mut TerminalBuffer) -> Result<()> {
        let sequence = ControlSequence::from_csi(final_char, params);
        self.execute_sequence(sequence, buffer)
    }
    
    fn execute_sequence(&self, sequence: ControlSequence, buffer: &mut TerminalBuffer) -> Result<()> {
        match sequence {
            ControlSequence::CursorUp(n) => buffer.set_cursor_position(buffer.cursor_col, buffer.cursor_row.saturating_sub(n))?,
            ControlSequence::CursorDown(n) => buffer.set_cursor_position(buffer.cursor_col, buffer.cursor_row.saturating_add(n).min(buffer.rows - 1))?,
            ControlSequence::CursorForward(n) => buffer.set_cursor_position(buffer.cursor_col.saturating_add(n).min(buffer.cols - 1), buffer.cursor_row)?,
            ControlSequence::CursorBack(n) => buffer.set_cursor_position(buffer.cursor_col.saturating_sub(n), buffer.cursor_row)?,
            ControlSequence::CursorNextLine(n) => {
                for _ in 0..n {
                    buffer.set_cursor_position(0, buffer.cursor_row.saturating_add(1).min(buffer.rows - 1))?;
                }
            },
            ControlSequence::CursorPrecedingLine(n) => {
                for _ in 0..n {
                    buffer.set_cursor_position(0, buffer.cursor_row.saturating_sub(1))?;
                }
            },
            ControlSequence::CursorHorizontalAbsolute(col) => buffer.set_cursor_position(col.min(buffer.cols - 1), buffer.cursor_row)?,
            ControlSequence::CursorPosition(row, col) => {
                // CSI rows and columns are 1-based
                let row = row.saturating_sub(1).min(buffer.rows - 1);
                let col = col.saturating_sub(1).min(buffer.cols - 1);
                buffer.set_cursor_position(col, row)?;
            },
            ControlSequence::EraseInDisplay(0) => {
                // Erase from cursor to end of display
                for row in buffer.cursor_row..buffer.rows {
                    for col in buffer.cursor_col..buffer.cols {
                        buffer.write_cell(col, row, Cell::default())?;
                    }
                }
            },
            ControlSequence::EraseInDisplay(1) => {
                // Erase from beginning of display to cursor
                for row in 0..=buffer.cursor_row {
                    for col in 0..=buffer.cursor_col {
                        buffer.write_cell(col, row, Cell::default())?;
                    }
                }
            },
            ControlSequence::EraseInDisplay(2) => {
                // Erase entire display
                buffer.clear();
            },
            ControlSequence::EraseInLine(0) => {
                // Erase from cursor to end of line
                for col in buffer.cursor_col..buffer.cols {
                    buffer.write_cell(col, buffer.cursor_row, Cell::default())?;
                }
            },
            ControlSequence::EraseInLine(1) => {
                // Erase from beginning of line to cursor
                for col in 0..=buffer.cursor_col {
                    buffer.write_cell(col, buffer.cursor_row, Cell::default())?;
                }
            },
            ControlSequence::EraseInLine(2) => {
                // Erase entire line
                buffer.clear_line(buffer.cursor_row);
            },
            ControlSequence::ScrollUp(n) => buffer.scroll_up(n),
            ControlSequence::ScrollDown(n) => buffer.scroll_down(n),
            ControlSequence::SetGraphicsRendition(params) => {
                // Handle graphics rendition parameters
                // See: https://en.wikipedia.org/wiki/ANSI_escape_code#SGR_(Select_Graphic_Rendition)
                for param in params {
                    match param {
                        0 => {
                            // Reset all attributes
                            buffer.reset_attributes();
                        },
                        1 => buffer.set_bold(true),
                        2 => buffer.set_dim(true),
                        3 => buffer.set_italic(true),
                        4 => buffer.set_underline(true),
                        5 => buffer.set_blink(true),
                        7 => buffer.set_reverse(true),
                        21 => buffer.set_bold(false),
                        22 => buffer.set_dim(false),
                        23 => buffer.set_italic(false),
                        24 => buffer.set_underline(false),
                        25 => buffer.set_blink(false),
                        27 => buffer.set_reverse(false),
                        // Foreground colors (30-37, 90-97)
                        30..=37 => buffer.set_foreground(param - 30),
                        90..=97 => buffer.set_foreground(param - 90 + 8),
                        // Background colors (40-47, 100-107)
                        40..=47 => buffer.set_background(param - 40),
                        100..=107 => buffer.set_background(param - 100 + 8),
                        _ => {},
                    }
                }
            },
            ControlSequence::Reset => {
                buffer.reset();
            },
            _ => {},
        }
        Ok(())
    }
    
    fn handle_null(&self, buffer: &mut TerminalBuffer) -> () {
        // Do nothing
    }
    
    fn handle_bell(&self, buffer: &mut TerminalBuffer) -> () {
        // Ring bell (ignored in terminal buffer)
    }
    
    fn handle_backspace(&self, buffer: &mut TerminalBuffer) -> () {
        if buffer.cursor_col > 0 {
            if let Err(e) = buffer.set_cursor_position(buffer.cursor_col - 1, buffer.cursor_row) {
                warn!("Failed to backspace: {}", e);
            }
        }
    }
    
    fn handle_tab(&self, buffer: &mut TerminalBuffer) -> () {
        let next_tab_stop = ((buffer.cursor_col / 8) + 1) * 8;
        if let Err(e) = buffer.set_cursor_position(next_tab_stop.min(buffer.cols - 1), buffer.cursor_row) {
            warn!("Failed to tab: {}", e);
        }
    }
    
    fn handle_line_feed(&self, buffer: &mut TerminalBuffer) -> () {
        if buffer.cursor_row < buffer.rows - 1 {
            if let Err(e) = buffer.set_cursor_position(buffer.cursor_col, buffer.cursor_row + 1) {
                warn!("Failed to line feed: {}", e);
            }
        } else {
            buffer.scroll_up(1);
        }
    }
    
    fn handle_carriage_return(&self, buffer: &mut TerminalBuffer) -> () {
        if let Err(e) = buffer.set_cursor_position(0, buffer.cursor_row) {
            warn!("Failed to carriage return: {}", e);
        }
    }
    
    fn handle_form_feed(&self, buffer: &mut TerminalBuffer) -> () {
        // Clear screen (simplified)
        buffer.clear();
    }
    
    fn handle_shift_out(&self, buffer: &mut TerminalBuffer) -> () {
        // Ignored
    }
    
    fn handle_shift_in(&self, buffer: &mut TerminalBuffer) -> () {
        // Ignored
    }
    
    fn handle_printable(&self, c: char, buffer: &mut TerminalBuffer) -> () {
        let cell = super::buffer::Cell {
            character: c,
            foreground: match buffer.attributes.foreground {
                0 => 0x000000, // Black
                1 => 0xff0000, // Red
                2 => 0x00ff00, // Green
                3 => 0xffff00, // Yellow
                4 => 0x0000ff, // Blue
                5 => 0xff00ff, // Magenta
                6 => 0x00ffff, // Cyan
                7 => 0xffffff, // White
                8 => 0x808080, // Bright Black
                9 => 0xff5555, // Bright Red
                10 => 0x55ff55, // Bright Green
                11 => 0xffff55, // Bright Yellow
                12 => 0x5555ff, // Bright Blue
                13 => 0xff55ff, // Bright Magenta
                14 => 0x55ffff, // Bright Cyan
                15 => 0xffffff, // Bright White
                _ => 0xffffff, // Default to white
            },
            background: match buffer.attributes.background {
                0 => 0x000000, // Black
                1 => 0xff0000, // Red
                2 => 0x00ff00, // Green
                3 => 0xffff00, // Yellow
                4 => 0x0000ff, // Blue
                5 => 0xff00ff, // Magenta
                6 => 0x00ffff, // Cyan
                7 => 0xffffff, // White
                8 => 0x808080, // Bright Black
                9 => 0xff5555, // Bright Red
                10 => 0x55ff55, // Bright Green
                11 => 0xffff55, // Bright Yellow
                12 => 0x5555ff, // Bright Blue
                13 => 0xff55ff, // Bright Magenta
                14 => 0x55ffff, // Bright Cyan
                15 => 0xffffff, // Bright White
                _ => 0x000000, // Default to black
            },
            bold: buffer.attributes.bold,
            italic: buffer.attributes.italic,
            underline: buffer.attributes.underline,
            blink: buffer.attributes.blink,
            reverse: buffer.attributes.reverse,
        };
        if let Err(e) = buffer.write_cell(buffer.cursor_col, buffer.cursor_row, cell) {
            warn!("Failed to write cell: {}", e);
        }
        
        if buffer.cursor_col < buffer.cols - 1 {
            if let Err(e) = buffer.set_cursor_position(buffer.cursor_col + 1, buffer.cursor_row) {
                warn!("Failed to advance cursor: {}", e);
            }
        } else {
            if buffer.cursor_row < buffer.rows - 1 {
                if let Err(e) = buffer.set_cursor_position(0, buffer.cursor_row + 1) {
                    warn!("Failed to wrap cursor: {}", e);
                }
            } else {
                buffer.scroll_up(1);
                if let Err(e) = buffer.set_cursor_position(0, buffer.cursor_row) {
                    warn!("Failed to set cursor after scroll: {}", e);
                }
            }
        }
    }
}

impl ControlSequence {
    fn from_csi(final_char: char, params: Vec<u16>) -> Self {
        match final_char {
            'A' => ControlSequence::CursorUp(params.get(0).copied().unwrap_or(1)),
            'B' => ControlSequence::CursorDown(params.get(0).copied().unwrap_or(1)),
            'C' => ControlSequence::CursorForward(params.get(0).copied().unwrap_or(1)),
            'D' => ControlSequence::CursorBack(params.get(0).copied().unwrap_or(1)),
            'E' => ControlSequence::CursorNextLine(params.get(0).copied().unwrap_or(1)),
            'F' => ControlSequence::CursorPrecedingLine(params.get(0).copied().unwrap_or(1)),
            'G' => ControlSequence::CursorHorizontalAbsolute(params.get(0).copied().unwrap_or(1)),
            'H' | 'f' => {
                let row = params.get(0).copied().unwrap_or(1);
                let col = params.get(1).copied().unwrap_or(1);
                ControlSequence::CursorPosition(row, col)
            },
            'J' => ControlSequence::EraseInDisplay(params.get(0).copied().unwrap_or(0) as u8),
            'K' => ControlSequence::EraseInLine(params.get(0).copied().unwrap_or(0) as u8),
            'S' => ControlSequence::ScrollUp(params.get(0).copied().unwrap_or(1)),
            'T' => ControlSequence::ScrollDown(params.get(0).copied().unwrap_or(1)),
            'm' => ControlSequence::SetGraphicsRendition(params.iter().map(|&p| p as u8).collect()),
            _ => ControlSequence::Unknown(format!("CSI {} {}", params.iter().map(|p| p.to_string()).collect::<Vec<_>>().join(";"), final_char)),
        }
    }
}