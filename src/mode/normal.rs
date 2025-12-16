use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, Event};

use super::{EditorContext, InputResult, ModeHandler, Mode, StructuralNavAction};
use super::operator::{Operator, Motion, Direction, WordMotion, PendingOperator};
use crate::edit::EditOperations;

/// Normal mode handler - navigation and commands
pub struct NormalMode {
    /// Register to use for next operation (if specified with ")
    selected_register: Option<char>,
}

impl NormalMode {
    pub fn new() -> Self {
        Self {
            selected_register: None,
        }
    }
    
    /// Execute an operator with a motion
    fn execute_operator(&mut self, op: Operator, motion: Motion, ctx: &mut EditorContext) -> Result<()> {
        let range = motion.calculate_range(ctx.buffer, ctx.cursor.byte_offset)?;
        let text = ctx.buffer.slice(range.clone());
        
        match op {
            Operator::Delete => {
                // Delete and store in register
                let edit = EditOperations::delete(ctx.buffer, ctx.cursor, range.start, range.end)?;
                ctx.register_map.set(self.selected_register, text, false);
                ctx.undo_stack.push(edit);
            }
            Operator::Change => {
                // Delete and enter insert mode
                let edit = EditOperations::delete(ctx.buffer, ctx.cursor, range.start, range.end)?;
                ctx.register_map.set(self.selected_register, text, false);
                ctx.undo_stack.push(edit);
                *ctx.mode = Mode::Insert;
            }
            Operator::Yank => {
                // Copy to register without deleting
                ctx.register_map.set(self.selected_register, text, true);
            }
        }
        
        self.selected_register = None;
        Ok(())
    }
}

impl ModeHandler for NormalMode {
    fn handle_key(&mut self, key: KeyEvent, mut ctx: EditorContext) -> Result<InputResult> {
        match (key.code, key.modifiers) {
            // Quit commands
            (KeyCode::Char('q'), KeyModifiers::NONE) => {
                return Ok(InputResult::Quit);
            }
            
            // Navigation (handled by existing cursor logic, but we acknowledge it here)
            (KeyCode::Char('h'), KeyModifiers::NONE) => {
                ctx.cursor.move_left(ctx.buffer);
                Ok(InputResult::ClearNodeTracking)
            }
            (KeyCode::Char('j'), KeyModifiers::NONE) => {
                ctx.cursor.move_down(ctx.buffer);
                Ok(InputResult::ClearNodeTracking)
            }
            (KeyCode::Char('k'), KeyModifiers::NONE) => {
                ctx.cursor.move_up(ctx.buffer);
                Ok(InputResult::ClearNodeTracking)
            }
            (KeyCode::Char('l'), KeyModifiers::NONE) => {
                ctx.cursor.move_right(ctx.buffer);
                Ok(InputResult::ClearNodeTracking)
            }
            
            // Page navigation - Ctrl+d (half page down) and Ctrl+u (half page up)
            (KeyCode::Char('d'), KeyModifiers::CONTROL) => {
                // Move down half a page (approximately 20 lines)
                for _ in 0..20 {
                    ctx.cursor.move_down(ctx.buffer);
                }
                Ok(InputResult::ClearNodeTracking)
            }
            (KeyCode::Char('u'), KeyModifiers::CONTROL) => {
                // Move up half a page (approximately 20 lines)
                for _ in 0..20 {
                    ctx.cursor.move_up(ctx.buffer);
                }
                Ok(InputResult::ClearNodeTracking)
            }
            
            // Command mode
            (KeyCode::Char(':'), _) => {
                Ok(InputResult::ModeSwitch(Mode::Command))
            }
            
            // Text object handlers (must be before 'i' and 'a' insert mode handlers)
            (KeyCode::Char('i'), KeyModifiers::NONE) if ctx.pending_operator.is_some() => {
                // Inner text object - wait for next key
                self.handle_text_object(true, &mut ctx)?;
                Ok(InputResult::Handled)
            }
            (KeyCode::Char('a'), KeyModifiers::NONE) if ctx.pending_operator.is_some() => {
                // Around text object - wait for next key
                self.handle_text_object(false, &mut ctx)?;
                Ok(InputResult::Handled)
            }
            
            // Visual mode
            (KeyCode::Char('v'), KeyModifiers::NONE) => {
                Ok(InputResult::ModeSwitch(Mode::Visual { line_wise: false }))
            }
            (KeyCode::Char('V'), KeyModifiers::SHIFT) => {
                Ok(InputResult::ModeSwitch(Mode::Visual { line_wise: true }))
            }
            
            // Enter insert mode commands
            (KeyCode::Char('i'), KeyModifiers::NONE) => {
                // Insert before cursor - no cursor movement needed
                Ok(InputResult::ModeSwitch(Mode::Insert))
            }
            (KeyCode::Char('a'), KeyModifiers::NONE) => {
                // Insert after cursor - move cursor right
                ctx.cursor.move_right(ctx.buffer);
                Ok(InputResult::ModeSwitch(Mode::Insert))
            }
            (KeyCode::Char('o'), KeyModifiers::NONE) => {
                // Open line below
                ctx.cursor.move_end_of_line(ctx.buffer);
                // TODO: Insert newline and move cursor to new line
                // For now, just enter insert mode
                Ok(InputResult::ModeSwitch(Mode::Insert))
            }
            (KeyCode::Char('O'), KeyModifiers::SHIFT) => {
                // Open line above
                ctx.cursor.move_start_of_line(ctx.buffer);
                // TODO: Insert newline before current line and move cursor
                // For now, just enter insert mode
                Ok(InputResult::ModeSwitch(Mode::Insert))
            }
            (KeyCode::Char('A'), KeyModifiers::SHIFT) => {
                // Insert at end of line
                ctx.cursor.move_end_of_line(ctx.buffer);
                Ok(InputResult::ModeSwitch(Mode::Insert))
            }
            (KeyCode::Char('I'), KeyModifiers::SHIFT) => {
                // Insert at start of line (first non-whitespace)
                ctx.cursor.move_start_of_line(ctx.buffer);
                Ok(InputResult::ModeSwitch(Mode::Insert))
            }
            
            // Line navigation
            (KeyCode::Char('0'), KeyModifiers::NONE) => {
                ctx.cursor.move_start_of_line(ctx.buffer);
                Ok(InputResult::Handled)
            }
            (KeyCode::Char('$'), KeyModifiers::NONE) => {
                ctx.cursor.move_end_of_line(ctx.buffer);
                Ok(InputResult::Handled)
            }
            (KeyCode::Char('^'), KeyModifiers::NONE) => {
                ctx.cursor.move_start_of_line(ctx.buffer);
                Ok(InputResult::Handled)
            }
            
            // Structural navigation
            (KeyCode::Char('j'), KeyModifiers::CONTROL) => {
                // Ctrl+j - next sibling
                Ok(InputResult::StructuralNav(StructuralNavAction::NextSibling))
            }
            (KeyCode::Char('k'), KeyModifiers::CONTROL) => {
                // Ctrl+k - prev sibling  
                Ok(InputResult::StructuralNav(StructuralNavAction::PrevSibling))
            }
            (KeyCode::Char('h'), KeyModifiers::CONTROL) => {
                // Ctrl+h - parent (move out)
                Ok(InputResult::StructuralNav(StructuralNavAction::Parent))
            }
            (KeyCode::Char('l'), KeyModifiers::CONTROL) => {
                // Ctrl+l - first child (move in)
                Ok(InputResult::StructuralNav(StructuralNavAction::FirstChild))
            }
            
            // Key/Value navigation with two-key sequences
            (KeyCode::Char(']'), KeyModifiers::NONE) => {
                // Wait for second key: 'l' for next key, 'v' for next value
                use crossterm::event;
                use std::time::Duration;
                
                if let Ok(true) = event::poll(Duration::from_millis(500)) {
                    if let Ok(Event::Key(next_key)) = event::read() {
                        match next_key.code {
                            KeyCode::Char('l') => {
                                return Ok(InputResult::StructuralNav(StructuralNavAction::NextKey));
                            }
                            KeyCode::Char('v') => {
                                return Ok(InputResult::StructuralNav(StructuralNavAction::NextValue));
                            }
                            _ => {}
                        }
                    }
                }
                Ok(InputResult::NotHandled)
            }
            (KeyCode::Char('['), KeyModifiers::NONE) => {
                // Wait for second key: 'l' for prev key, 'v' for prev value
                use crossterm::event;
                use std::time::Duration;
                
                if let Ok(true) = event::poll(Duration::from_millis(500)) {
                    if let Ok(Event::Key(next_key)) = event::read() {
                        match next_key.code {
                            KeyCode::Char('l') => {
                                return Ok(InputResult::StructuralNav(StructuralNavAction::PrevKey));
                            }
                            KeyCode::Char('v') => {
                                return Ok(InputResult::StructuralNav(StructuralNavAction::PrevValue));
                            }
                            _ => {}
                        }
                    }
                }
                Ok(InputResult::NotHandled)
            }
            
            // Page navigation
            (KeyCode::Char('g'), KeyModifiers::NONE) => {
                // TODO: Implement gg (go to first line)
                // For now, just go to start
                ctx.cursor.line = 0;
                ctx.cursor.col = 0;
                ctx.cursor.byte_offset = 0;
                Ok(InputResult::Handled)
            }
            (KeyCode::Char('G'), KeyModifiers::SHIFT) => {
                // TODO: Implement G (go to last line)
                Ok(InputResult::Handled)
            }
            
            // Undo/Redo
            (KeyCode::Char('u'), KeyModifiers::NONE) => {
                match ctx.undo_stack.undo(ctx.buffer, ctx.cursor) {
                    Ok(true) => Ok(InputResult::Handled),
                    Ok(false) => Ok(InputResult::Handled),
                    Err(e) => Err(e),
                }
            }
            (KeyCode::Char('r'), KeyModifiers::CONTROL) => {
                match ctx.undo_stack.redo(ctx.buffer, ctx.cursor) {
                    Ok(true) => Ok(InputResult::Handled),
                    Ok(false) => Ok(InputResult::Handled),
                    Err(e) => Err(e),
                }
            }
            
            // Operators - set pending operator
            (KeyCode::Char('d'), KeyModifiers::NONE) if ctx.pending_operator.is_none() => {
                *ctx.pending_operator = Some(PendingOperator {
                    operator: Operator::Delete,
                    count: 1,
                });
                Ok(InputResult::Handled)
            }
            (KeyCode::Char('c'), KeyModifiers::NONE) if ctx.pending_operator.is_none() => {
                *ctx.pending_operator = Some(PendingOperator {
                    operator: Operator::Change,
                    count: 1,
                });
                Ok(InputResult::Handled)
            }
            (KeyCode::Char('y'), KeyModifiers::NONE) if ctx.pending_operator.is_none() => {
                *ctx.pending_operator = Some(PendingOperator {
                    operator: Operator::Yank,
                    count: 1,
                });
                Ok(InputResult::Handled)
            }
            
            // Paste
            (KeyCode::Char('p'), KeyModifiers::NONE) => {
                if let Some(text) = ctx.register_map.get(self.selected_register) {
                    let edit = EditOperations::insert(
                        ctx.buffer,
                        ctx.cursor,
                        ctx.cursor.byte_offset,
                        &text,
                    )?;
                    ctx.undo_stack.push(edit);
                }
                self.selected_register = None;
                Ok(InputResult::Handled)
            }
            (KeyCode::Char('P'), KeyModifiers::SHIFT) => {
                if let Some(text) = ctx.register_map.get(self.selected_register) {
                    // Paste before cursor
                    let paste_pos = if ctx.cursor.byte_offset > 0 {
                        ctx.cursor.byte_offset.saturating_sub(1)
                    } else {
                        0
                    };
                    let edit = EditOperations::insert(
                        ctx.buffer,
                        ctx.cursor,
                        paste_pos,
                        &text,
                    )?;
                    ctx.undo_stack.push(edit);
                }
                self.selected_register = None;
                Ok(InputResult::Handled)
            }
            
            // Word motions - if pending operator, apply it
            (KeyCode::Char('w'), KeyModifiers::NONE) => {
                if let Some(pending) = ctx.pending_operator.take() {
                    let motion = Motion::Word(WordMotion::Start, pending.count);
                    self.execute_operator(pending.operator, motion, &mut ctx)?;
                    Ok(InputResult::Handled)
                } else {
                    // Just move cursor
                    let motion = Motion::Word(WordMotion::Start, 1);
                    if let Ok(range) = motion.calculate_range(ctx.buffer, ctx.cursor.byte_offset) {
                        ctx.cursor.byte_offset = range.end;
                        ctx.cursor.line = ctx.buffer.byte_offset_to_line(ctx.cursor.byte_offset);
                    }
                    Ok(InputResult::Handled)
                }
            }
            (KeyCode::Char('b'), KeyModifiers::NONE) => {
                if let Some(pending) = ctx.pending_operator.take() {
                    let motion = Motion::Word(WordMotion::BackStart, pending.count);
                    self.execute_operator(pending.operator, motion, &mut ctx)?;
                    Ok(InputResult::Handled)
                } else {
                    let motion = Motion::Word(WordMotion::BackStart, 1);
                    if let Ok(range) = motion.calculate_range(ctx.buffer, ctx.cursor.byte_offset) {
                        ctx.cursor.byte_offset = range.start;
                        ctx.cursor.line = ctx.buffer.byte_offset_to_line(ctx.cursor.byte_offset);
                    }
                    Ok(InputResult::Handled)
                }
            }
            (KeyCode::Char('e'), KeyModifiers::NONE) => {
                if let Some(pending) = ctx.pending_operator.take() {
                    let motion = Motion::Word(WordMotion::End, pending.count);
                    self.execute_operator(pending.operator, motion, &mut ctx)?;
                    Ok(InputResult::Handled)
                } else {
                    let motion = Motion::Word(WordMotion::End, 1);
                    if let Ok(range) = motion.calculate_range(ctx.buffer, ctx.cursor.byte_offset) {
                        ctx.cursor.byte_offset = range.end;
                        ctx.cursor.line = ctx.buffer.byte_offset_to_line(ctx.cursor.byte_offset);
                    }
                    Ok(InputResult::Handled)
                }
            }
            
            // Line operations
            (KeyCode::Char('d'), KeyModifiers::NONE) if ctx.pending_operator.is_some() => {
                // dd - delete line
                *ctx.pending_operator = None;
                let motion = Motion::Line(Direction::Forward, 1);
                self.execute_operator(Operator::Delete, motion, &mut ctx)?;
                Ok(InputResult::Handled)
            }
            (KeyCode::Char('c'), KeyModifiers::NONE) if ctx.pending_operator.is_some() => {
                // cc - change line
                *ctx.pending_operator = None;
                let motion = Motion::Line(Direction::Forward, 1);
                self.execute_operator(Operator::Change, motion, &mut ctx)?;
                Ok(InputResult::Handled)
            }
            (KeyCode::Char('y'), KeyModifiers::NONE) if ctx.pending_operator.is_some() => {
                // yy - yank line
                *ctx.pending_operator = None;
                let motion = Motion::Line(Direction::Forward, 1);
                self.execute_operator(Operator::Yank, motion, &mut ctx)?;
                Ok(InputResult::Handled)
            }
            
            // x - delete character (like dl)
            (KeyCode::Char('x'), KeyModifiers::NONE) => {
                let motion = Motion::Char(Direction::Forward, 1);
                self.execute_operator(Operator::Delete, motion, &mut ctx)?;
                Ok(InputResult::Handled)
            }
            
            // X - delete character before cursor (like dh)
            (KeyCode::Char('X'), KeyModifiers::SHIFT) => {
                let motion = Motion::Char(Direction::Backward, 1);
                self.execute_operator(Operator::Delete, motion, &mut ctx)?;
                Ok(InputResult::Handled)
            }
            
            // Register selection with "
            (KeyCode::Char('"'), KeyModifiers::NONE) => {
                // Next character will be the register name
                // For now, just clear pending operator
                *ctx.pending_operator = None;
                Ok(InputResult::Handled)
            }
            
            _ => Ok(InputResult::NotHandled),
        }
    }
}

// Helper methods for NormalMode
impl NormalMode {
    /// Handle text object selection (iw, aw, i{, a{, etc.)
    fn handle_text_object(&mut self, inner: bool, ctx: &mut EditorContext) -> Result<()> {
        use crossterm::event;
        use std::time::Duration;
        
        // Wait for next key to determine which text object
        if let Ok(true) = event::poll(Duration::from_millis(500)) {
            if let Ok(Event::Key(key)) = event::read() {
                let text_object = match key.code {
                    KeyCode::Char('w') => Some(super::operator::TextObject::Word { inner }),
                    KeyCode::Char('"') => Some(super::operator::TextObject::Quotes { inner }),
                    KeyCode::Char('{') | KeyCode::Char('}') => Some(super::operator::TextObject::Braces { inner }),
                    KeyCode::Char('[') | KeyCode::Char(']') => Some(super::operator::TextObject::Brackets { inner }),
                    _ => None,
                };
                
                if let Some(obj) = text_object {
                    if let Some(pending) = ctx.pending_operator.take() {
                        let motion = Motion::TextObject(obj);
                        self.execute_operator(pending.operator, motion, ctx)?;
                    }
                }
            }
        }
        Ok(())
    }
}
