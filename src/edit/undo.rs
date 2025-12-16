use anyhow::Result;
use std::time::{SystemTime, Instant};

use super::Edit;
use crate::buffer::Buffer;
use crate::buffer::cursor::Cursor;

/// Undo/redo stack with transaction grouping
pub struct UndoStack {
    /// Stack of undo operations
    stack: Vec<EditGroup>,
    /// Stack of redo operations
    redo_stack: Vec<EditGroup>,
    /// Current edit group (for grouping multiple edits)
    current_group: Vec<Edit>,
    /// Maximum number of undo levels
    max_size: usize,
    /// When the current group was started
    group_start_time: Option<Instant>,
    /// Timeout for auto-grouping edits (milliseconds)
    group_timeout_ms: u64,
}

/// A group of edits that are undone/redone together
#[derive(Debug, Clone)]
pub struct EditGroup {
    edits: Vec<Edit>,
    timestamp: SystemTime,
}

impl EditGroup {
    fn new(edits: Vec<Edit>) -> Self {
        Self {
            edits,
            timestamp: SystemTime::now(),
        }
    }
    
    /// Apply all edits in the group
    fn apply(&self, buffer: &mut Buffer, cursor: &mut Cursor) -> Result<()> {
        for edit in &self.edits {
            // Apply the edit
            if !edit.new_text.is_empty() {
                buffer.insert(edit.offset, &edit.new_text)?;
            }
            if !edit.old_text.is_empty() {
                buffer.delete(edit.offset, edit.offset + edit.old_text.len())?;
            }
        }
        
        // Restore cursor to final position
        if let Some(last_edit) = self.edits.last() {
            cursor.byte_offset = last_edit.cursor_after.byte_offset;
            cursor.line = last_edit.cursor_after.line;
            cursor.col = last_edit.cursor_after.col;
        }
        
        Ok(())
    }
    
    /// Apply all edits in reverse (for undo)
    fn apply_reverse(&self, buffer: &mut Buffer, cursor: &mut Cursor) -> Result<()> {
        for edit in self.edits.iter().rev() {
            let reversed = edit.reverse();
            
            // Apply the reversed edit
            if !reversed.new_text.is_empty() {
                buffer.insert(reversed.offset, &reversed.new_text)?;
            }
            if !reversed.old_text.is_empty() {
                buffer.delete(reversed.offset, reversed.offset + reversed.old_text.len())?;
            }
        }
        
        // Restore cursor to initial position
        if let Some(first_edit) = self.edits.first() {
            cursor.byte_offset = first_edit.cursor_before.byte_offset;
            cursor.line = first_edit.cursor_before.line;
            cursor.col = first_edit.cursor_before.col;
        }
        
        Ok(())
    }
}

impl UndoStack {
    pub fn new() -> Self {
        Self {
            stack: Vec::new(),
            redo_stack: Vec::new(),
            current_group: Vec::new(),
            max_size: 1000,
            group_start_time: None,
            group_timeout_ms: 1000, // 1 second
        }
    }
    
    /// Add an edit to the current group
    pub fn push(&mut self, edit: Edit) {
        // Check if we should start a new group
        if let Some(start_time) = self.group_start_time {
            if start_time.elapsed().as_millis() > self.group_timeout_ms as u128 {
                self.commit_group();
            }
        }
        
        if self.group_start_time.is_none() {
            self.group_start_time = Some(Instant::now());
        }
        
        self.current_group.push(edit);
        
        // Clear redo stack when new edit is made
        self.redo_stack.clear();
    }
    
    /// Start a new edit group (for explicit transaction boundaries)
    pub fn begin_group(&mut self) {
        if !self.current_group.is_empty() {
            self.commit_group();
        }
        self.group_start_time = Some(Instant::now());
    }
    
    /// Commit the current edit group to the undo stack
    pub fn end_group(&mut self) {
        self.commit_group();
    }
    
    /// Commit current group to undo stack
    fn commit_group(&mut self) {
        if self.current_group.is_empty() {
            return;
        }
        
        let group = EditGroup::new(std::mem::take(&mut self.current_group));
        self.stack.push(group);
        self.group_start_time = None;
        
        // Enforce max size
        if self.stack.len() > self.max_size {
            self.stack.remove(0);
        }
    }
    
    /// Undo the last edit group
    pub fn undo(&mut self, buffer: &mut Buffer, cursor: &mut Cursor) -> Result<bool> {
        // Commit any pending edits first
        self.commit_group();
        
        if let Some(group) = self.stack.pop() {
            group.apply_reverse(buffer, cursor)?;
            self.redo_stack.push(group);
            Ok(true)
        } else {
            Ok(false)
        }
    }
    
    /// Redo the last undone edit group
    pub fn redo(&mut self, buffer: &mut Buffer, cursor: &mut Cursor) -> Result<bool> {
        if let Some(group) = self.redo_stack.pop() {
            group.apply(buffer, cursor)?;
            self.stack.push(group);
            Ok(true)
        } else {
            Ok(false)
        }
    }
    
    /// Check if undo is available
    pub fn can_undo(&self) -> bool {
        !self.stack.is_empty() || !self.current_group.is_empty()
    }
    
    /// Check if redo is available
    pub fn can_redo(&self) -> bool {
        !self.redo_stack.is_empty()
    }
    
    /// Get the number of undo levels available
    pub fn undo_count(&self) -> usize {
        self.stack.len() + if self.current_group.is_empty() { 0 } else { 1 }
    }
}

impl Default for UndoStack {
    fn default() -> Self {
        Self::new()
    }
}
