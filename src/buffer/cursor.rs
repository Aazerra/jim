use anyhow::Result;

/// Cursor position in the buffer
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Cursor {
    /// Byte offset in buffer
    pub byte_offset: usize,
    /// Line number (0-indexed)
    pub line: usize,
    /// Column number (0-indexed, character position not byte)
    pub col: usize,
}

impl Cursor {
    pub fn new() -> Self {
        Self {
            byte_offset: 0,
            line: 0,
            col: 0,
        }
    }
    
    pub fn from_line_col(line: usize, col: usize) -> Self {
        Self {
            byte_offset: 0, // Will be calculated from buffer
            line,
            col,
        }
    }
    
    pub fn from_byte_offset(byte_offset: usize) -> Self {
        Self {
            byte_offset,
            line: 0, // Will be calculated from buffer
            col: 0,
        }
    }
    
    /// Move cursor to the next line
    pub fn move_down(&mut self) {
        self.line = self.line.saturating_add(1);
        // Note: byte_offset should be recalculated by buffer
    }
    
    /// Move cursor to the previous line
    pub fn move_up(&mut self) {
        self.line = self.line.saturating_sub(1);
        // Note: byte_offset should be recalculated by buffer
    }
    
    /// Move cursor right one character
    pub fn move_right(&mut self) {
        self.col = self.col.saturating_add(1);
    }
    
    /// Move cursor left one character
    pub fn move_left(&mut self) {
        self.col = self.col.saturating_sub(1);
    }
    
    /// Move to start of line
    pub fn move_to_line_start(&mut self) {
        self.col = 0;
    }
    
    /// Move to end of line
    pub fn move_to_line_end(&mut self, line_length: usize) {
        self.col = line_length.saturating_sub(1);
    }
    
    /// Set cursor to specific line and column
    pub fn set_position(&mut self, line: usize, col: usize) {
        self.line = line;
        self.col = col;
    }
    
    /// Set byte offset and mark line/col for recalculation
    pub fn set_byte_offset(&mut self, offset: usize) {
        self.byte_offset = offset;
    }
}

impl Default for Cursor {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_cursor_creation() {
        let cursor = Cursor::new();
        assert_eq!(cursor.line, 0);
        assert_eq!(cursor.col, 0);
        assert_eq!(cursor.byte_offset, 0);
    }
    
    #[test]
    fn test_cursor_movement() {
        let mut cursor = Cursor::new();
        cursor.move_down();
        assert_eq!(cursor.line, 1);
        
        cursor.move_right();
        cursor.move_right();
        assert_eq!(cursor.col, 2);
        
        cursor.move_up();
        assert_eq!(cursor.line, 0);
        
        cursor.move_left();
        assert_eq!(cursor.col, 1);
    }
    
    #[test]
    fn test_cursor_boundaries() {
        let mut cursor = Cursor::new();
        cursor.move_up(); // Should saturate at 0
        assert_eq!(cursor.line, 0);
        
        cursor.move_left(); // Should saturate at 0
        assert_eq!(cursor.col, 0);
    }
    
    #[test]
    fn test_cursor_line_navigation() {
        let mut cursor = Cursor::new();
        cursor.col = 5;
        cursor.move_to_line_start();
        assert_eq!(cursor.col, 0);
        
        cursor.move_to_line_end(20);
        assert_eq!(cursor.col, 19);
    }
}
