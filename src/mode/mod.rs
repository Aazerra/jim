use anyhow::Result;
use crossterm::event::KeyEvent;

pub mod normal;
pub mod insert;
pub mod visual;
pub mod command;
pub mod operator;
pub mod register;

use crate::buffer::Buffer;
use crate::buffer::cursor::Cursor;
use crate::edit::undo::UndoStack;

pub use operator::{Operator, Motion, PendingOperator, OperatorResult};
pub use register::RegisterMap;

/// Editor mode states
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Normal,
    Insert,
    Visual { line_wise: bool },
    Command,
}

impl Mode {
    /// Returns a display string for the mode
    pub fn display(&self) -> &str {
        match self {
            Mode::Normal => "",
            Mode::Insert => "-- INSERT --",
            Mode::Visual { line_wise: false } => "-- VISUAL --",
            Mode::Visual { line_wise: true } => "-- VISUAL LINE --",
            Mode::Command => "-- COMMAND --",
        }
    }
}

/// Context passed to mode handlers
pub struct EditorContext<'a> {
    pub buffer: &'a mut Buffer,
    pub cursor: &'a mut Cursor,
    pub mode: &'a mut Mode,
    pub undo_stack: &'a mut UndoStack,
    pub register_map: &'a mut RegisterMap,
    pub pending_operator: &'a mut Option<PendingOperator>,
    pub structural_index: Option<&'a crate::parser::StructuralIndex>,
}

/// Result of handling an input event
#[derive(Debug)]
pub enum InputResult {
    /// Input was handled, continue
    Handled,
    /// Request mode change
    ModeSwitch(Mode),
    /// Request quit
    Quit,
    /// Input not handled, pass to next handler
    NotHandled,
    /// Request structural navigation
    StructuralNav(StructuralNavAction),
    /// Request to clear node tracking (cursor moved manually)
    ClearNodeTracking,
    /// Display a message to the user
    Message(String),
}

/// Structural navigation actions
#[derive(Debug)]
pub enum StructuralNavAction {
    NextSibling,
    PrevSibling,
    Parent,
    FirstChild,
    NextKey,
    PrevKey,
    NextValue,
    PrevValue,
}

/// Trait for mode-specific input handlers
pub trait ModeHandler {
    fn handle_key(&mut self, key: KeyEvent, ctx: EditorContext) -> Result<InputResult>;
}
