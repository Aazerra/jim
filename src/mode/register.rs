use std::collections::HashMap;

/// Register system for yank/delete/paste operations
#[derive(Debug, Clone)]
pub struct RegisterMap {
    /// Named registers a-z
    registers: HashMap<char, String>,
    /// Unnamed register (default for d, c, y)
    unnamed: String,
    /// Last yank (0 register)
    last_yank: String,
    /// Small delete register (- register, <1 line)
    small_delete: String,
}

impl RegisterMap {
    pub fn new() -> Self {
        Self {
            registers: HashMap::new(),
            unnamed: String::new(),
            last_yank: String::new(),
            small_delete: String::new(),
        }
    }
    
    /// Store text in a register
    pub fn set(&mut self, register: Option<char>, text: String, is_yank: bool) {
        match register {
            Some(reg) if ('a'..='z').contains(&reg) || ('A'..='Z').contains(&reg) => {
                if ('A'..='Z').contains(&reg) {
                    // Uppercase appends to register
                    let lower = reg.to_ascii_lowercase();
                    let existing = self.registers.get(&lower).cloned().unwrap_or_default();
                    self.registers.insert(lower, existing + &text);
                } else {
                    self.registers.insert(reg, text.clone());
                }
            }
            _ => {
                // Default unnamed register
                self.unnamed = text.clone();
            }
        }
        
        if is_yank {
            self.last_yank = text.clone();
        }
        
        // Store small deletes (less than 1 line)
        if !is_yank && !text.contains('\n') {
            self.small_delete = text;
        }
    }
    
    /// Get text from a register
    pub fn get(&self, register: Option<char>) -> Option<String> {
        match register {
            Some('0') => Some(self.last_yank.clone()),
            Some('-') => Some(self.small_delete.clone()),
            Some(reg) if ('a'..='z').contains(&reg) || ('A'..='Z').contains(&reg) => {
                let lower = reg.to_ascii_lowercase();
                self.registers.get(&lower).cloned()
            }
            _ => Some(self.unnamed.clone()),
        }
    }
    
    /// Get the unnamed register (default)
    pub fn get_unnamed(&self) -> String {
        self.unnamed.clone()
    }
}

impl Default for RegisterMap {
    fn default() -> Self {
        Self::new()
    }
}
