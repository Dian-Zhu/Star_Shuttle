use super::error::TerminalError; use crate::modules::error::Result; use std::collections::VecDeque;

#[derive(Debug, Clone)]
pub struct Cell {
    pub character: char,
    pub foreground: u32,
    pub background: u32,
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub blink: bool,
    pub reverse: bool,
}

impl Default for Cell {
    fn default() -> Self {
        Self {
            character: ' ',
            foreground: 0xffffff,
            background: 0x000000,
            bold: false,
            italic: false,
            underline: false,
            blink: false,
            reverse: false,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Attributes {
    pub foreground: u8,
    pub background: u8,
    pub bold: bool,
    pub dim: bool,
    pub italic: bool,
    pub underline: bool,
    pub blink: bool,
    pub reverse: bool,
}

impl Default for Attributes {
    fn default() -> Self {
        Self {
            foreground: 7, // White
            background: 0, // Black
            bold: false,
            dim: false,
            italic: false,
            underline: false,
            blink: false,
            reverse: false,
        }
    }
}

#[derive(Debug, Clone)]
pub struct TerminalBuffer {
    pub cols: u16,
    pub rows: u16,
    pub buffer: Vec<Vec<Cell>>,
    pub scrollback: VecDeque<Vec<Cell>>,
    pub scrollback_size: usize,
    pub cursor_col: u16,
    pub cursor_row: u16,
    pub cursor_visible: bool,
    pub cursor_blink: bool,
    pub attributes: Attributes,
}

impl TerminalBuffer {

    pub fn new(cols: u16, rows: u16) -> Result<Self> {
        if cols < 1 || rows < 1 {
            return Err(TerminalError::InvalidSize(cols, rows).into());
        }
        
        let buffer = vec![vec![Cell::default(); cols as usize]; rows as usize];
        
        Ok(Self {
            cols,
            rows,
            buffer,
            scrollback: VecDeque::new(),
            scrollback_size: 1000,
            cursor_col: 0,
            cursor_row: 0,
            cursor_visible: true,
            cursor_blink: true,
            attributes: Attributes::default(),
        })
    }
    
    pub fn reset(&mut self) {
        self.buffer = vec![vec![Cell::default(); self.cols as usize]; self.rows as usize];
        self.scrollback.clear();
        self.cursor_col = 0;
        self.cursor_row = 0;
        self.cursor_visible = true;
        self.cursor_blink = true;
        self.attributes = Attributes::default();
    }

    pub fn resize(&mut self, cols: u16, rows: u16) -> Result<()> {
        if cols < 1 || rows < 1 {
            return Err(TerminalError::InvalidSize(cols, rows).into());
        }
        
        // Save current buffer content if needed
        // For simplicity, we'll just recreate the buffer
        let new_buffer = vec![vec![Cell::default(); cols as usize]; rows as usize];
        self.buffer = new_buffer;
        self.cols = cols;
        self.rows = rows;
        
        // Reset cursor position to avoid out-of-bounds
        self.cursor_col = 0;
        self.cursor_row = 0;
        
        Ok(())
    }
    
    pub fn clear(&mut self) {
        for row in &mut self.buffer {
            for cell in row {
                *cell = Cell::default();
            }
        }
    }
    
    pub fn clear_line(&mut self, row: u16) {
        if row < self.rows {
            for cell in &mut self.buffer[row as usize] {
                *cell = Cell::default();
            }
        }
    }
    
    pub fn write_cell(&mut self, col: u16, row: u16, cell: Cell) -> Result<()> {
        if col >= self.cols || row >= self.rows {
            return Err(TerminalError::BufferOverflow.into());
        }
        
        self.buffer[row as usize][col as usize] = cell;
        Ok(())
    }
    
    pub fn get_cell(&self, col: u16, row: u16) -> Option<&Cell> {
        if col < self.cols && row < self.rows {
            Some(&self.buffer[row as usize][col as usize])
        } else {
            None
        }
    }
    
    pub fn scroll_up(&mut self, lines: u16) {
        for _ in 0..lines {
            if let Some(first_row) = self.buffer.first().cloned() {
                self.scrollback.push_front(first_row);
                if self.scrollback.len() > self.scrollback_size {
                    self.scrollback.pop_back();
                }
            }
            self.buffer.remove(0);
            self.buffer.push(vec![Cell::default(); self.cols as usize]);
        }
    }
    
    pub fn scroll_down(&mut self, lines: u16) {
        for _ in 0..lines {
            if let Some(last_row) = self.scrollback.pop_front() {
                self.buffer.insert(0, last_row);
                self.buffer.pop();
            }
        }
    }
    
    pub fn set_cursor_position(&mut self, col: u16, row: u16) -> Result<()> {
        if col < self.cols && row < self.rows {
            self.cursor_col = col;
            self.cursor_row = row;
            Ok(())
        } else {
            Err(TerminalError::InvalidSize(col, row).into())
        }
    }

    // Attribute methods
    pub fn reset_attributes(&mut self) {
        self.attributes = Attributes::default();
    }

    pub fn set_bold(&mut self, value: bool) {
        self.attributes.bold = value;
    }

    pub fn set_dim(&mut self, value: bool) {
        self.attributes.dim = value;
    }

    pub fn set_italic(&mut self, value: bool) {
        self.attributes.italic = value;
    }

    pub fn set_underline(&mut self, value: bool) {
        self.attributes.underline = value;
    }

    pub fn set_blink(&mut self, value: bool) {
        self.attributes.blink = value;
    }

    pub fn set_reverse(&mut self, value: bool) {
        self.attributes.reverse = value;
    }

    pub fn set_foreground(&mut self, color: u8) {
        self.attributes.foreground = color;
    }

    pub fn set_background(&mut self, color: u8) {
        self.attributes.background = color;
    }
}

