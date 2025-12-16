use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use super::{EditorContext, InputResult, ModeHandler, Mode};
use crate::edit::EditOperations;

/// Visual mode handler - visual selection
pub struct VisualMode {
    /// Start position of selection (byte offset)
    pub selection_start: usize,
    /// Whether this is line-wise visual mode
    pub line_wise: bool,
}

impl VisualMode {
    pub fn new(start_offset: usize, line_wise: bool) -> Self {
        Self {
            selection_start: start_offset,
            line_wise,
        }
    }
    
    /// Get the selection range (start, end) in byte offsets
    /// Returns (min, max) regardless of selection direction
    pub fn get_selection_range(&self, cursor_offset: usize) -> (usize, usize) {
        if self.selection_start < cursor_offset {
            (self.selection_start, cursor_offset)
        } else {
            (cursor_offset, self.selection_start)
        }
    }
}

impl ModeHandler for VisualMode {
    fn handle_key(&mut self, key: KeyEvent, ctx: EditorContext) -> Result<InputResult> {
        match (key.code, key.modifiers) {
            // Escape - return to normal mode
            (KeyCode::Esc, _) => {
                Ok(InputResult::ModeSwitch(Mode::Normal))
            }
            
            // Movement keys - extend selection
            (KeyCode::Char('h'), KeyModifiers::NONE) | (KeyCode::Left, _) => {
                ctx.cursor.move_left(ctx.buffer);
                ctx.cursor.sync_byte_offset(ctx.buffer);
                Ok(InputResult::Handled)
            }
            (KeyCode::Char('j'), KeyModifiers::NONE) | (KeyCode::Down, _) => {
                ctx.cursor.move_down(ctx.buffer);
                ctx.cursor.sync_byte_offset(ctx.buffer);
                Ok(InputResult::Handled)
            }
            (KeyCode::Char('k'), KeyModifiers::NONE) | (KeyCode::Up, _) => {
                ctx.cursor.move_up(ctx.buffer);
                ctx.cursor.sync_byte_offset(ctx.buffer);
                Ok(InputResult::Handled)
            }
            (KeyCode::Char('l'), KeyModifiers::NONE) | (KeyCode::Right, _) => {
                ctx.cursor.move_right(ctx.buffer);
                ctx.cursor.sync_byte_offset(ctx.buffer);
                Ok(InputResult::Handled)
            }
            
            // Line motions
            (KeyCode::Char('0'), KeyModifiers::NONE) => {
                ctx.cursor.move_to_line_start();
                ctx.cursor.sync_byte_offset(ctx.buffer);
                Ok(InputResult::Handled)
            }
            (KeyCode::Char('$'), KeyModifiers::NONE) => {
                let line_length = ctx.buffer.get_line(ctx.cursor.line).chars().count();
                ctx.cursor.move_to_line_end(line_length);
                ctx.cursor.sync_byte_offset(ctx.buffer);
                Ok(InputResult::Handled)
            }
            
            // Toggle line-wise mode
            (KeyCode::Char('V'), KeyModifiers::SHIFT) => {
                self.line_wise = !self.line_wise;
                *ctx.mode = Mode::Visual { line_wise: self.line_wise };
                Ok(InputResult::Handled)
            }
            
            // Operators on selection
            (KeyCode::Char('d'), KeyModifiers::NONE) | (KeyCode::Char('x'), KeyModifiers::NONE) => {
                let cursor_offset = ctx.cursor.byte_offset;
                let (start, end) = self.get_selection_range(cursor_offset);
                let text = ctx.buffer.slice(start..end);
                
                let edit = EditOperations::delete(ctx.buffer, ctx.cursor, start, end)?;
                ctx.register_map.set(None, text, false);
                ctx.undo_stack.push(edit);
                
                // Move cursor to start of selection
                ctx.cursor.byte_offset = start;
                let line = ctx.buffer.byte_offset_to_line(start);
                ctx.cursor.line = line;
                let line_start = ctx.buffer.line_to_byte_offset(line);
                let offset_in_line = start - line_start;
                let line_text = ctx.buffer.get_line(line);
                ctx.cursor.col = line_text.chars()
                    .take_while(|_| {
                        let bytes: usize = line_text.chars()
                            .take(ctx.cursor.col)
                            .map(|c| c.len_utf8())
                            .sum();
                        bytes < offset_in_line
                    })
                    .count();
                
                Ok(InputResult::ModeSwitch(Mode::Normal))
            }
            
            (KeyCode::Char('y'), KeyModifiers::NONE) => {
                let cursor_offset = ctx.cursor.byte_offset;
                let (start, end) = self.get_selection_range(cursor_offset);
                let text = ctx.buffer.slice(start..end);
                ctx.register_map.set(None, text, true);
                
                // Move cursor to start of selection
                ctx.cursor.byte_offset = start;
                let line = ctx.buffer.byte_offset_to_line(start);
                ctx.cursor.line = line;
                let line_start = ctx.buffer.line_to_byte_offset(line);
                let offset_in_line = start - line_start;
                let line_text = ctx.buffer.get_line(line);
                ctx.cursor.col = line_text.chars()
                    .take_while(|_| {
                        let bytes: usize = line_text.chars()
                            .take(ctx.cursor.col)
                            .map(|c| c.len_utf8())
                            .sum();
                        bytes < offset_in_line
                    })
                    .count();
                
                Ok(InputResult::ModeSwitch(Mode::Normal))
            }
            
            (KeyCode::Char('c'), KeyModifiers::NONE) => {
                let cursor_offset = ctx.cursor.byte_offset;
                let (start, end) = self.get_selection_range(cursor_offset);
                let text = ctx.buffer.slice(start..end);
                
                let edit = EditOperations::delete(ctx.buffer, ctx.cursor, start, end)?;
                ctx.register_map.set(None, text, false);
                ctx.undo_stack.push(edit);
                
                // Move cursor to start and enter insert mode
                ctx.cursor.byte_offset = start;
                let line = ctx.buffer.byte_offset_to_line(start);
                ctx.cursor.line = line;
                let line_start = ctx.buffer.line_to_byte_offset(line);
                let offset_in_line = start - line_start;
                let line_text = ctx.buffer.get_line(line);
                ctx.cursor.col = line_text.chars()
                    .take_while(|_| {
                        let bytes: usize = line_text.chars()
                            .take(ctx.cursor.col)
                            .map(|c| c.len_utf8())
                            .sum();
                        bytes < offset_in_line
                    })
                    .count();
                
                Ok(InputResult::ModeSwitch(Mode::Insert))
            }
            
            _ => Ok(InputResult::NotHandled),
        }
    }
}
