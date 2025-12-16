pub mod cursor;

use anyhow::Result;
use std::collections::HashMap;
use std::fs::File;
use std::io::{BufRead, BufReader, Seek, SeekFrom};
use std::path::PathBuf;

pub use cursor::Cursor;

pub struct Buffer {
    file: Option<File>,
    path: Option<PathBuf>,
    file_size: usize,
    line_offsets: Vec<u64>,  // Byte offset of each line start
    line_cache: HashMap<usize, String>,  // LRU cache for lines
    cache_size: usize,
}

impl Buffer {
    pub fn new() -> Self {
        Self {
            file: None,
            path: None,
            file_size: 0,
            line_offsets: vec![0],  // First line starts at 0
            line_cache: HashMap::new(),
            cache_size: 1000,  // Cache up to 1000 lines
        }
    }

    pub fn load_file(&mut self, path: &str) -> Result<()> {
        let file = File::open(path)?;
        let metadata = file.metadata()?;
        self.file_size = metadata.len() as usize;
        
        // Build line offset index by scanning file
        self.build_line_index(path)?;
        
        // Keep file handle open for lazy reading
        self.file = Some(File::open(path)?);
        self.path = Some(PathBuf::from(path));
        
        Ok(())
    }
    
    fn build_line_index(&mut self, path: &str) -> Result<()> {
        let file = File::open(path)?;
        let reader = BufReader::new(file);
        
        self.line_offsets.clear();
        self.line_offsets.push(0);
        
        let mut offset = 0u64;
        for line_result in reader.lines() {
            let line = line_result?;
            // +1 for newline character
            offset += line.len() as u64 + 1;
            self.line_offsets.push(offset);
        }
        
        Ok(())
    }

    pub fn is_empty(&self) -> bool {
        self.line_offsets.len() <= 1
    }

    pub fn line_count(&self) -> usize {
        self.line_offsets.len().saturating_sub(1)
    }

    pub fn get_line(&mut self, line_idx: usize) -> String {
        // Check cache first
        if let Some(cached) = self.line_cache.get(&line_idx) {
            return cached.clone();
        }
        
        if line_idx >= self.line_offsets.len() - 1 {
            return String::new();
        }
        
        // Read from file
        let start_offset = self.line_offsets[line_idx];
        
        let line = if let Some(ref mut file) = self.file {
            if let Err(_) = file.seek(SeekFrom::Start(start_offset)) {
                return String::new();
            }
            
            let mut buf_reader = BufReader::new(file.try_clone().unwrap_or_else(|_| {
                File::open(self.path.as_ref().unwrap()).unwrap()
            }));
            
            let mut line_str = String::new();
            match buf_reader.read_line(&mut line_str) {
                Ok(_) => line_str,
                Err(_) => return String::new(),
            }
        } else {
            return String::new();
        };
        
        // Cache the line
        if self.line_cache.len() >= self.cache_size {
            // Simple cache eviction: clear when full
            self.line_cache.clear();
        }
        self.line_cache.insert(line_idx, line.clone());
        line
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
        self.file_size
    }

    pub fn path(&self) -> Option<&PathBuf> {
        self.path.as_ref()
    }
    
    pub fn len_bytes(&self) -> usize {
        self.file_size
    }
    
    /// Convert byte offset to line number using binary search
    pub fn byte_offset_to_line(&self, byte_offset: usize) -> usize {
        let offset = byte_offset as u64;
        
        // Binary search to find the line containing this byte offset
        match self.line_offsets.binary_search(&offset) {
            Ok(idx) => idx,  // Exact match
            Err(idx) => idx.saturating_sub(1),  // Between two lines, take previous
        }
    }
}

impl Default for Buffer {
    fn default() -> Self {
        Self::new()
    }
}
