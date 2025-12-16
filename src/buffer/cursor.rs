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
    
    /// Sync byte_offset from current line and col position
    pub fn sync_byte_offset(&mut self, buffer: &crate::buffer::Buffer) {
        // Use buffer's rope to get correct byte offset for line start
        let line_start_offset = buffer.line_to_byte_offset(self.line);
        
        // Get line text and clamp col to line length
        let line_text = buffer.get_line(self.line);
        let line_len_chars = line_text.chars().count();
        if line_len_chars > 0 {
            self.col = self.col.min(line_len_chars.saturating_sub(1));
        } else {
            self.col = 0;
        }
        
        // Calculate byte offset within line (handle UTF-8)
        let col_bytes: usize = line_text.chars().take(self.col).map(|c| c.len_utf8()).sum();
        self.byte_offset = line_start_offset + col_bytes;
    }
    
    /// Move cursor to the next line
    pub fn move_down(&mut self, buffer: &crate::buffer::Buffer) {
        if self.line < buffer.line_count().saturating_sub(1) {
            self.line = self.line.saturating_add(1);
            self.sync_byte_offset(buffer);
        }
    }
    
    /// Move cursor to the previous line
    pub fn move_up(&mut self, buffer: &crate::buffer::Buffer) {
        if self.line > 0 {
            self.line = self.line.saturating_sub(1);
            self.sync_byte_offset(buffer);
        }
    }
    
    /// Move cursor right one character
    pub fn move_right(&mut self, buffer: &crate::buffer::Buffer) {
        let line_text = buffer.get_line(self.line);
        let line_len = line_text.chars().count();
        if self.col < line_len.saturating_sub(1) {
            self.col = self.col.saturating_add(1);
            self.sync_byte_offset(buffer);
        }
    }
    
    /// Move cursor left one character
    pub fn move_left(&mut self, buffer: &crate::buffer::Buffer) {
        if self.col > 0 {
            self.col = self.col.saturating_sub(1);
            self.sync_byte_offset(buffer);
        }
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
    
    /// Move cursor to start of line (alias for mode handlers)
    pub fn move_start_of_line(&mut self, buffer: &crate::buffer::Buffer) {
        self.col = 0;
        self.sync_byte_offset(buffer);
    }
    
    /// Move cursor to end of line (alias for mode handlers)
    pub fn move_end_of_line(&mut self, buffer: &crate::buffer::Buffer) {
        let line_text = buffer.get_line(self.line);
        let line_len = line_text.chars().count();
        self.col = line_len.saturating_sub(1).max(0);
        self.sync_byte_offset(buffer);
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
