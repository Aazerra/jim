use anyhow::Result;

pub mod undo;

use crate::buffer::Buffer;
use crate::buffer::cursor::Cursor;

/// Represents a single edit operation
#[derive(Debug, Clone)]
pub struct Edit {
    /// Byte offset where edit occurred
    pub offset: usize,
    /// Text before the edit
    pub old_text: String,
    /// Text after the edit
    pub new_text: String,
    /// Cursor position before edit
    pub cursor_before: CursorState,
    /// Cursor position after edit
    pub cursor_after: CursorState,
}

/// Snapshot of cursor state for undo/redo
#[derive(Debug, Clone, Copy)]
pub struct CursorState {
    pub line: usize,
    pub col: usize,
    pub byte_offset: usize,
}

impl From<&Cursor> for CursorState {
    fn from(cursor: &Cursor) -> Self {
        Self {
            line: cursor.line,
            col: cursor.col,
            byte_offset: cursor.byte_offset,
        }
    }
}

impl Edit {
    /// Create a new edit
    pub fn new(
        offset: usize,
        old_text: String,
        new_text: String,
        cursor_before: CursorState,
        cursor_after: CursorState,
    ) -> Self {
        Self {
            offset,
            old_text,
            new_text,
            cursor_before,
            cursor_after,
        }
    }
    
    /// Calculate the range affected by this edit
    pub fn range(&self) -> std::ops::Range<usize> {
        self.offset..self.offset + self.old_text.len()
    }
    
    /// Get the reverse edit (for undo)
    pub fn reverse(&self) -> Self {
        Self {
            offset: self.offset,
            old_text: self.new_text.clone(),
            new_text: self.old_text.clone(),
            cursor_before: self.cursor_after,
            cursor_after: self.cursor_before,
        }
    }
}

/// Basic edit operations on buffer
pub struct EditOperations;

impl EditOperations {
    /// Insert text at the given position
    pub fn insert(
        buffer: &mut Buffer,
        cursor: &mut Cursor,
        offset: usize,
        text: &str,
    ) -> Result<Edit> {
        let cursor_before = CursorState::from(&*cursor);
        
        // Perform the insertion
        buffer.insert(offset, text)?;
        
        // Update cursor position - move cursor after inserted text
        cursor.byte_offset = offset + text.len();
        cursor.line = buffer.byte_offset_to_line(cursor.byte_offset);
        
        // Calculate column position
        let line_start = buffer.line_to_byte_offset(cursor.line);
        cursor.col = cursor.byte_offset.saturating_sub(line_start);
        
        let cursor_after = CursorState::from(&*cursor);
        
        Ok(Edit::new(
            offset,
            String::new(),
            text.to_string(),
            cursor_before,
            cursor_after,
        ))
    }
    
    /// Delete text in the given range
    pub fn delete(
        buffer: &mut Buffer,
        cursor: &mut Cursor,
        start: usize,
        end: usize,
    ) -> Result<Edit> {
        let cursor_before = CursorState::from(&*cursor);
        
        // Get the text being deleted
        let deleted_text = buffer.slice(start..end);
        
        // Perform the deletion
        buffer.delete(start, end)?;
        
        // Update cursor position - move to start of deleted region
        cursor.byte_offset = start;
        cursor.line = buffer.byte_offset_to_line(cursor.byte_offset);
        
        // Calculate column position
        let line_start = buffer.line_to_byte_offset(cursor.line);
        cursor.col = cursor.byte_offset.saturating_sub(line_start);
        
        let cursor_after = CursorState::from(&*cursor);
        
        Ok(Edit::new(
            start,
            deleted_text,
            String::new(),
            cursor_before,
            cursor_after,
        ))
    }
    
    /// Replace text in the given range
    pub fn replace(
        buffer: &mut Buffer,
        cursor: &mut Cursor,
        start: usize,
        end: usize,
        new_text: &str,
    ) -> Result<Edit> {
        let cursor_before = CursorState::from(&*cursor);
        
        // Get the text being replaced
        let old_text = buffer.slice(start..end);
        
        // Perform the replacement
        buffer.replace(start, end, new_text)?;
        
        // Update cursor position
        cursor.byte_offset = start + new_text.len();
        // TODO: Update cursor line and col
        
        let cursor_after = CursorState::from(&*cursor);
        
        Ok(Edit::new(
            start,
            old_text,
            new_text.to_string(),
            cursor_before,
            cursor_after,
        ))
    }
}
