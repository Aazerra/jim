use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use super::{EditorContext, InputResult, ModeHandler, Mode};
use crate::edit::EditOperations;

/// Insert mode handler - text editing
pub struct InsertMode;

impl InsertMode {
    pub fn new() -> Self {
        Self
    }
}

impl ModeHandler for InsertMode {
    fn handle_key(&mut self, key: KeyEvent, ctx: EditorContext) -> Result<InputResult> {
        match (key.code, key.modifiers) {
            // Exit insert mode
            (KeyCode::Esc, _) => {
                // Commit any pending edits before leaving insert mode
                ctx.undo_stack.end_group();
                
                // Move cursor back one position when leaving insert mode
                // (Vim behavior: cursor should be on the last inserted character)
                if ctx.cursor.byte_offset > 0 {
                    ctx.cursor.byte_offset = ctx.cursor.byte_offset.saturating_sub(1);
                    // Update line/col from byte offset
                    ctx.cursor.line = ctx.buffer.byte_offset_to_line(ctx.cursor.byte_offset);
                }
                Ok(InputResult::ModeSwitch(Mode::Normal))
            }
            
            // Character insertion
            (KeyCode::Char(c), KeyModifiers::NONE) | (KeyCode::Char(c), KeyModifiers::SHIFT) => {
                // Insert character at cursor position
                let edit = EditOperations::insert(
                    ctx.buffer,
                    ctx.cursor,
                    ctx.cursor.byte_offset,
                    &c.to_string(),
                )?;
                
                // Track edit for undo
                ctx.undo_stack.push(edit);
                
                Ok(InputResult::Handled)
            }
            
            // Enter/newline
            (KeyCode::Enter, _) => {
                // Insert newline at cursor position
                let edit = EditOperations::insert(
                    ctx.buffer,
                    ctx.cursor,
                    ctx.cursor.byte_offset,
                    "\n",
                )?;
                
                // Track edit for undo
                ctx.undo_stack.push(edit);
                
                Ok(InputResult::Handled)
            }
            
            // Tab - insert tab or spaces
            (KeyCode::Tab, _) => {
                // Insert tab character at cursor position
                let edit = EditOperations::insert(
                    ctx.buffer,
                    ctx.cursor,
                    ctx.cursor.byte_offset,
                    "\t",
                )?;
                
                // Track edit for undo
                ctx.undo_stack.push(edit);
                
                Ok(InputResult::Handled)
            }
            
            // Backspace - delete character before cursor
            (KeyCode::Backspace, _) => {
                if ctx.cursor.byte_offset > 0 {
                    // Find the character boundary before cursor
                    let delete_end = ctx.cursor.byte_offset;
                    let delete_start = delete_end.saturating_sub(1);
                    
                    // Delete one character
                    let edit = EditOperations::delete(
                        ctx.buffer,
                        ctx.cursor,
                        delete_start,
                        delete_end,
                    )?;
                    
                    // Track edit for undo
                    ctx.undo_stack.push(edit);
                }
                Ok(InputResult::Handled)
            }
            
            // Delete - delete character at cursor
            (KeyCode::Delete, _) => {
                if ctx.cursor.byte_offset < ctx.buffer.len_bytes() {
                    let delete_start = ctx.cursor.byte_offset;
                    let delete_end = delete_start + 1;
                    
                    let edit = EditOperations::delete(
                        ctx.buffer,
                        ctx.cursor,
                        delete_start,
                        delete_end,
                    )?;
                    
                    // Track edit for undo
                    ctx.undo_stack.push(edit);
                }
                Ok(InputResult::Handled)
            }
            
            // Tab
            (KeyCode::Tab, _) => {
                // TODO: Insert tab/spaces
                Ok(InputResult::Handled)
            }
            
            // Arrow keys (allow navigation in insert mode)
            (KeyCode::Left, _) => {
                ctx.cursor.move_left(ctx.buffer);
                Ok(InputResult::Handled)
            }
            (KeyCode::Right, _) => {
                ctx.cursor.move_right(ctx.buffer);
                Ok(InputResult::Handled)
            }
            (KeyCode::Up, _) => {
                ctx.cursor.move_up(ctx.buffer);
                Ok(InputResult::Handled)
            }
            (KeyCode::Down, _) => {
                ctx.cursor.move_down(ctx.buffer);
                Ok(InputResult::Handled)
            }
            
            _ => Ok(InputResult::NotHandled),
        }
    }
}
