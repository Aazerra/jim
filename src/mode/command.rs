use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use super::{EditorContext, InputResult, ModeHandler, Mode};

/// Command mode handler - ex-style commands
pub struct CommandMode {
    /// Current command being typed
    pub command_line: String,
}

impl CommandMode {
    pub fn new() -> Self {
        Self {
            command_line: String::new(),
        }
    }
    
    /// Execute a command
    fn execute_command(&mut self, cmd: &str, ctx: &mut EditorContext) -> Result<InputResult> {
        let cmd = cmd.trim();
        
        // Check for :w <filename> pattern
        if cmd.starts_with("w ") || cmd.starts_with("write ") {
            let filename = if cmd.starts_with("w ") {
                &cmd[2..].trim()
            } else {
                &cmd[6..].trim()
            };
            
            ctx.buffer.save_as(filename)?;
            return Ok(InputResult::ModeSwitch(Mode::Normal));
        }
        
        match cmd {
            "q" | "quit" => {
                // Check if buffer is modified
                if ctx.buffer.is_modified() {
                    return Ok(InputResult::Message("No write since last change (use :q! to override)".to_string()));
                }
                Ok(InputResult::Quit)
            }
            "w" | "write" => {
                // Save file
                ctx.buffer.save()?;
                Ok(InputResult::ModeSwitch(Mode::Normal))
            }
            "wq" | "x" => {
                // Save and quit
                ctx.buffer.save()?;
                Ok(InputResult::Quit)
            }
            "q!" => {
                // Force quit without saving
                Ok(InputResult::Quit)
            }
            "" => {
                // Empty command, just return to normal
                Ok(InputResult::ModeSwitch(Mode::Normal))
            }
            _ => {
                // Unknown command
                Ok(InputResult::Message(format!("Unknown command: {}", cmd)))
            }
        }
    }
}

impl ModeHandler for CommandMode {
    fn handle_key(&mut self, key: KeyEvent, mut ctx: EditorContext) -> Result<InputResult> {
        match (key.code, key.modifiers) {
            // Escape - cancel command mode
            (KeyCode::Esc, _) => {
                self.command_line.clear();
                Ok(InputResult::ModeSwitch(Mode::Normal))
            }
            
            // Enter - execute command
            (KeyCode::Enter, _) => {
                let cmd = self.command_line.clone();
                self.command_line.clear();
                self.execute_command(&cmd, &mut ctx)
            }
            
            // Backspace - delete character
            (KeyCode::Backspace, _) => {
                self.command_line.pop();
                if self.command_line.is_empty() {
                    // If command line becomes empty, return to normal mode
                    Ok(InputResult::ModeSwitch(Mode::Normal))
                } else {
                    Ok(InputResult::Handled)
                }
            }
            
            // Type character
            (KeyCode::Char(c), KeyModifiers::NONE) | (KeyCode::Char(c), KeyModifiers::SHIFT) => {
                self.command_line.push(c);
                Ok(InputResult::Handled)
            }
            
            _ => Ok(InputResult::NotHandled),
        }
    }
}
