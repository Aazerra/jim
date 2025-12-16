pub mod cursor;

use anyhow::Result;
use ropey::Rope;
use std::fs::File;
use std::io::BufReader;
use std::path::PathBuf;

pub use cursor::Cursor;

pub struct Buffer {
    rope: Rope,
    path: Option<PathBuf>,
    modified: bool,
}

impl Buffer {
    pub fn new() -> Self {
        Self {
            rope: Rope::new(),
            path: None,
            modified: false,
        }
    }

    pub fn load_file(&mut self, path: &str) -> Result<()> {
        // Load entire file into rope
        self.rope = Rope::from_reader(BufReader::new(File::open(path)?))?;
        self.path = Some(PathBuf::from(path));
        self.modified = false;
        
        Ok(())
    }

    pub fn is_empty(&self) -> bool {
        self.rope.len_chars() == 0
    }

    pub fn line_count(&self) -> usize {
        self.rope.len_lines()
    }

    pub fn get_line(&self, line_idx: usize) -> String {
        if line_idx >= self.rope.len_lines() {
            return String::new();
        }
        
        self.rope.line(line_idx).to_string()
    }

    pub fn get_visible_lines(&mut self, start_line: usize, count: usize) -> String {
        let mut result = String::new();
        let max_line = self.line_count();
        
        for i in 0..count {
            let line_idx = start_line + i;
            if line_idx >= max_line {
                break;
            }
            result.push_str(&self.get_line(line_idx));
        }
        
        result
    }

    pub fn file_size(&self) -> usize {
        self.rope.len_bytes()
    }

    pub fn path(&self) -> Option<&PathBuf> {
        self.path.as_ref()
    }
    
    pub fn len_bytes(&self) -> usize {
        self.rope.len_bytes()
    }
    
    pub fn len_chars(&self) -> usize {
        self.rope.len_chars()
    }
    
    /// Convert byte offset to line number
    pub fn byte_offset_to_line(&self, byte_offset: usize) -> usize {
        self.rope.byte_to_line(byte_offset.min(self.rope.len_bytes()))
    }
    
    /// Convert line number to byte offset
    pub fn line_to_byte_offset(&self, line: usize) -> usize {
        if line >= self.rope.len_lines() {
            return self.rope.len_bytes();
        }
        self.rope.line_to_byte(line)
    }
    
    /// Check if buffer has been modified
    pub fn is_modified(&self) -> bool {
        self.modified
    }
    
    /// Insert text at the given byte offset
    pub fn insert(&mut self, offset: usize, text: &str) -> Result<()> {
        let offset = offset.min(self.rope.len_bytes());
        self.rope.insert(offset, text);
        self.modified = true;
        Ok(())
    }
    
    /// Delete text in the given range [start, end)
    pub fn delete(&mut self, start: usize, end: usize) -> Result<()> {
        let start = start.min(self.rope.len_bytes());
        let end = end.min(self.rope.len_bytes());
        if start < end {
            self.rope.remove(start..end);
            self.modified = true;
        }
        Ok(())
    }
    
    /// Replace text in range [start, end) with new_text
    pub fn replace(&mut self, start: usize, end: usize, new_text: &str) -> Result<()> {
        self.delete(start, end)?;
        self.insert(start, new_text)?;
        Ok(())
    }
    
    /// Get a slice of text from the buffer
    pub fn slice(&self, range: std::ops::Range<usize>) -> String {
        let start = range.start.min(self.rope.len_bytes());
        let end = range.end.min(self.rope.len_bytes());
        if start >= end {
            return String::new();
        }
        self.rope.byte_slice(start..end).to_string()
    }
    
    /// Get character at byte offset
    pub fn char_at(&self, byte_offset: usize) -> Option<char> {
        if byte_offset >= self.rope.len_bytes() {
            return None;
        }
        let char_idx = self.rope.byte_to_char(byte_offset);
        self.rope.char(char_idx).into()
    }
}

impl Default for Buffer {
    fn default() -> Self {
        Self::new()
    }
}
