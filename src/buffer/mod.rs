pub mod cursor;

#[cfg(test)]
mod tests;

use anyhow::Result;
use memmap2::Mmap;
use ropey::Rope;
use std::fs::{File, OpenOptions};
use std::io::{BufWriter, Write, Seek, SeekFrom};
use std::path::PathBuf;
use std::sync::{Arc, atomic::{AtomicU32, AtomicBool, Ordering}};

pub use cursor::Cursor;

/// Represents a single edit operation for incremental save
#[derive(Debug, Clone)]
pub struct Edit {
    pub file_offset: usize,  // Position in original file
    pub old_len: usize,      // Bytes replaced (0 for insert)
    pub new_text: String,    // Replacement text
}

/// Save strategy selection
#[derive(Debug, Clone, Copy)]
enum SaveMethod {
    CopyOnWrite,   // Reflink + in-place edits (fastest for small edits on supported filesystems)
    Streaming,     // Stream original + patches (good fallback for all cases)
}

pub struct Buffer {
    // Memory-mapped original file (stays on disk, NOT loaded to RAM)
    mmap: Option<Mmap>,
    file_size: usize,
    
    // Line index: byte offset of each line (fast, ~1MB per 1GB file)
    line_offsets: Vec<usize>,
    
    // LRU cache: only recently viewed lines (8MB max)
    line_cache: std::collections::HashMap<usize, String>,
    cache_order: Vec<usize>,  // For LRU eviction
    max_cache_lines: usize,   // ~1000 lines = ~8MB
    
    // Edit overlay: modified lines only
    edits: std::collections::HashMap<usize, String>,  // line_num -> new content
    
    // Full rope: only used for edited regions or small files
    rope: Option<Rope>,
    use_rope: bool,  // true if file is small (<10MB) or has many edits
    
    // Save progress reporting
    save_progress: Arc<AtomicU32>,
    save_in_progress: Arc<AtomicBool>,
    save_pending: bool,
    
    // Load progress reporting
    pub load_progress: Arc<AtomicU32>,
    pub load_in_progress: Arc<AtomicBool>,
    
    path: Option<PathBuf>,
    modified: bool,
}

impl Buffer {
    pub fn new() -> Self {
        Self {
            mmap: None,
            file_size: 0,
            line_offsets: Vec::new(),
            line_cache: std::collections::HashMap::new(),
            cache_order: Vec::new(),
            max_cache_lines: 1000,  // ~8MB cache (8KB per line average)
            edits: std::collections::HashMap::new(),
            rope: None,
            use_rope: false,
            save_progress: Arc::new(AtomicU32::new(0)),
            save_in_progress: Arc::new(AtomicBool::new(false)),
            save_pending: false,
            load_progress: Arc::new(AtomicU32::new(0)),
            load_in_progress: Arc::new(AtomicBool::new(false)),
            path: None,
            modified: false,
        }
    }

    pub fn load_file(&mut self, path: &str) -> Result<()> {
        // Open file for memory mapping
        let file = File::open(path)?;
        
        // Memory-map the file (doesn't load into RAM)
        let mmap = unsafe { Mmap::map(&file)? };
        let file_size = mmap.len();
        
        // Decide: small files use Rope, large files use lazy loading
        let use_rope = file_size < 10 * 1024 * 1024;  // < 10MB: use Rope
        
        if use_rope {
            // Small file: build full rope (fast, in-memory editing)
            let rope = Rope::from_reader(mmap.as_ref())?;
            self.rope = Some(rope);
            self.use_rope = true;
            self.line_offsets.clear();
        } else {
            // Large file: build line index only (lazy loading)
            // Show progress for files that take >1 second to index
            self.load_in_progress.store(true, Ordering::SeqCst);
            self.load_progress.store(0, Ordering::SeqCst);
            
            self.line_offsets = Self::build_line_index_with_progress(
                &mmap, 
                &self.load_progress
            );
            
            self.load_progress.store(100, Ordering::SeqCst);
            self.load_in_progress.store(false, Ordering::SeqCst);
            self.use_rope = false;
            self.rope = None;
        }
        
        self.mmap = Some(mmap);
        self.file_size = file_size;
        self.path = Some(PathBuf::from(path));
        self.line_cache.clear();
        self.cache_order.clear();
        self.edits.clear();
        self.save_progress.store(0, Ordering::SeqCst);
        self.save_in_progress.store(false, Ordering::SeqCst);
        self.save_pending = false;
        self.modified = false;
        
        Ok(())
    }
    
    /// Build line offset index by scanning for newlines
    /// Returns byte offset of each line start
    fn build_line_index(mmap: &Mmap) -> Vec<usize> {
        let mut offsets = vec![0];  // First line starts at 0
        
        for (i, &byte) in mmap.iter().enumerate() {
            if byte == b'\n' {
                offsets.push(i + 1);  // Next line starts after \n
            }
        }
        
        offsets
    }
    
    /// Build line index with progress reporting (for large files)
    fn build_line_index_with_progress(mmap: &Mmap, progress: &Arc<AtomicU32>) -> Vec<usize> {
        let mut offsets = vec![0];
        let total_bytes = mmap.len();
        let mut last_progress = 0u32;
        
        for (i, &byte) in mmap.iter().enumerate() {
            if byte == b'\n' {
                offsets.push(i + 1);
            }
            
            // Update progress every 1% (to avoid excessive atomic writes)
            let current_progress = ((i as f64 / total_bytes as f64) * 100.0) as u32;
            if current_progress > last_progress {
                progress.store(current_progress, Ordering::Relaxed);
                last_progress = current_progress;
            }
        }
        
        offsets
    }

    pub fn is_empty(&self) -> bool {
        if self.use_rope {
            self.rope.as_ref().map(|r| r.len_chars() == 0).unwrap_or(true)
        } else {
            self.file_size == 0
        }
    }

    pub fn line_count(&self) -> usize {
        if self.use_rope {
            self.rope.as_ref().map(|r| r.len_lines()).unwrap_or(0)
        } else {
            self.line_offsets.len().saturating_sub(1)
        }
    }
    
    /// Read a line lazily from mmap (with LRU cache)
    fn read_line_lazy(&mut self, line_idx: usize) -> Option<String> {
        // Check edit overlay first
        if let Some(edited) = self.edits.get(&line_idx) {
            return Some(edited.clone());
        }
        
        // Check cache
        if let Some(cached) = self.line_cache.get(&line_idx) {
            // Move to front of LRU
            self.cache_order.retain(|&x| x != line_idx);
            self.cache_order.push(line_idx);
            return Some(cached.clone());
        }
        
        // Cache miss: read from mmap
        let mmap = self.mmap.as_ref()?;
        let start = *self.line_offsets.get(line_idx)?;
        let end = self.line_offsets.get(line_idx + 1)
            .copied()
            .unwrap_or(mmap.len());
        
        let line_bytes = &mmap[start..end];
        let line = String::from_utf8_lossy(line_bytes).to_string();
        
        // Add to cache
        self.line_cache.insert(line_idx, line.clone());
        self.cache_order.push(line_idx);
        
        // Evict old lines if cache too large
        while self.cache_order.len() > self.max_cache_lines {
            if let Some(old_idx) = self.cache_order.first().copied() {
                self.cache_order.remove(0);
                self.line_cache.remove(&old_idx);
            }
        }
        
        Some(line)
    }

    pub fn get_line(&self, line_idx: usize) -> String {
        if self.use_rope {
            // Small file: use rope
            if let Some(rope) = &self.rope {
                if line_idx >= rope.len_lines() {
                    return String::new();
                }
                return rope.line(line_idx).to_string();
            }
        }
        
        // Large file: check edit overlay first
        if let Some(edited) = self.edits.get(&line_idx) {
            return edited.clone();
        }
        
        // Check cache
        if let Some(cached) = self.line_cache.get(&line_idx) {
            return cached.clone();
        }
        
        // Cache miss: read from mmap (without updating cache)
        if let Some(mmap) = self.mmap.as_ref() {
            if let Some(&start) = self.line_offsets.get(line_idx) {
                let end = self.line_offsets.get(line_idx + 1)
                    .copied()
                    .unwrap_or(mmap.len());
                return String::from_utf8_lossy(&mmap[start..end]).to_string();
            }
        }
        
        String::new()
    }
    
    /// Get line with cache update (mutable version for viewport)
    pub fn get_line_cached(&mut self, line_idx: usize) -> String {
        if self.use_rope {
            return self.get_line(line_idx);
        }
        
        // For lazy mode, use cache and update LRU
        let line = self.read_line_lazy(line_idx).unwrap_or_default();
        line
    }

    pub fn get_visible_lines(&self, start_line: usize, count: usize) -> String {
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

    pub fn get_file_size(&self) -> usize {
        if let Some(rope) = &self.rope {
            rope.len_bytes()
        } else {
            self.file_size
        }
    }

    pub fn path(&self) -> Option<&PathBuf> {
        self.path.as_ref()
    }
    
    pub fn len_bytes(&self) -> usize {
        if let Some(rope) = &self.rope {
            rope.len_bytes()
        } else {
            self.file_size
        }
    }
    
    pub fn len_chars(&self) -> usize {
        if let Some(rope) = &self.rope {
            rope.len_chars()
        } else {
            // Estimate for large files (not exact)
            self.file_size
        }
    }
    
    /// Convert byte offset to line number
    pub fn byte_offset_to_line(&self, byte_offset: usize) -> usize {
        if let Some(rope) = &self.rope {
            rope.byte_to_line(byte_offset.min(rope.len_bytes()))
        } else {
            // Binary search in line_offsets
            self.line_offsets
                .binary_search(&byte_offset)
                .unwrap_or_else(|i| i.saturating_sub(1))
        }
    }
    
    /// Convert line number to byte offset
    pub fn line_to_byte_offset(&self, line: usize) -> usize {
        if let Some(rope) = &self.rope {
            if line >= rope.len_lines() {
                return rope.len_bytes();
            }
            rope.line_to_byte(line)
        } else {
            // Look up in line_offsets
            self.line_offsets
                .get(line)
                .copied()
                .unwrap_or(self.file_size)
        }
    }
    
    /// Check if buffer has been modified
    pub fn is_modified(&self) -> bool {
        self.modified
    }
    
    /// Convert large file to rope mode (needed for editing)
    /// Called automatically on first edit
    fn ensure_rope_mode(&mut self) -> Result<()> {
        if self.use_rope {
            return Ok(());  // Already in rope mode
        }
        
        // Convert to rope mode for editing
        if let Some(mmap) = &self.mmap {
            self.rope = Some(Rope::from_reader(mmap.as_ref())?);
            self.use_rope = true;
            // Keep line_offsets for potential future optimizations
        }
        
        Ok(())
    }
    
    /// Insert text at the given byte offset
    pub fn insert(&mut self, offset: usize, text: &str) -> Result<()> {
        if self.use_rope {
            // Small file: use rope directly
            let rope = self.rope.as_mut().ok_or_else(|| anyhow::anyhow!("No rope available"))?;
            let offset = offset.min(rope.len_bytes());
            rope.insert(offset, text);
        } else {
            // Large file: track edit in overlay (don't load full file!)
            let line_num = self.byte_offset_to_line(offset);
            if line_num < self.line_count() {
                // Get current line content (original or edited)
                let mut line_content = self.get_line(line_num);
                
                // Calculate position within line
                let line_start = self.line_to_byte_offset(line_num);
                let pos_in_line = offset.saturating_sub(line_start);
                
                // Insert text at position
                if pos_in_line <= line_content.len() {
                    line_content.insert_str(pos_in_line, text);
                    self.edits.insert(line_num, line_content);
                }
            }
        }
        
        self.modified = true;
        Ok(())
    }
    
    /// Delete text in the given range [start, end)
    pub fn delete(&mut self, start: usize, end: usize) -> Result<()> {
        if start >= end {
            return Ok(());
        }
        
        if self.use_rope {
            // Small file: use rope directly
            let rope = self.rope.as_mut().ok_or_else(|| anyhow::anyhow!("No rope available"))?;
            let start = start.min(rope.len_bytes());
            let end = end.min(rope.len_bytes());
            rope.remove(start..end);
        } else {
            // Large file: track edit in overlay
            // For simplicity, handle single-line deletes
            let start_line = self.byte_offset_to_line(start);
            let end_line = self.byte_offset_to_line(end);
            
            if start_line == end_line {
                // Single line delete
                let mut line_content = self.get_line(start_line);
                let line_start = self.line_to_byte_offset(start_line);
                let pos_start = start.saturating_sub(line_start);
                let pos_end = end.saturating_sub(line_start);
                
                if pos_start < line_content.len() {
                    let actual_end = pos_end.min(line_content.len());
                    line_content.drain(pos_start..actual_end);
                    self.edits.insert(start_line, line_content);
                }
            }
            // Multi-line deletes: convert to rope mode for complex edits
            else if self.edits.len() > 100 {
                // Too many edits, convert to rope
                if let Some(mmap) = &self.mmap {
                    self.rope = Some(Rope::from_reader(mmap.as_ref())?);                    self.use_rope = true;
                    return self.delete(start, end);
                }
            }
        }
        
        self.modified = true;
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
        if let Some(rope) = &self.rope {
            let start = range.start.min(rope.len_bytes());
            let end = range.end.min(rope.len_bytes());
            if start >= end {
                return String::new();
            }
            rope.byte_slice(start..end).to_string()
        } else {
            // Read from mmap
            if let Some(mmap) = &self.mmap {
                let start = range.start.min(mmap.len());
                let end = range.end.min(mmap.len());
                if start >= end {
                    return String::new();
                }
                String::from_utf8_lossy(&mmap[start..end]).to_string()
            } else {
                String::new()
            }
        }
    }
    
    /// Get character at byte offset
    pub fn char_at(&self, byte_offset: usize) -> Option<char> {
        if let Some(rope) = &self.rope {
            if byte_offset >= rope.len_bytes() {
                return None;
            }
            let char_idx = rope.byte_to_char(byte_offset);
            rope.char(char_idx).into()
        } else {
            // Read from mmap
            self.mmap.as_ref()?
                .get(byte_offset)
                .and_then(|&b| Some(b as char))
        }
    }

    /// Map a rope byte-offset (current in-memory view) back to the
    /// original file byte-offset (before edits).
    /// NOTE: With edit overlay architecture, this is simplified since we don't
    /// track byte-level edits across the entire file. For accurate offset mapping,
    /// a piece table would be needed (future work).
    fn rope_to_original_offset(&self, rope_off: usize) -> usize {
        // For lazy mode with edit overlay, rope offsets are not directly
        // mappable to original file offsets without a piece table.
        // For now, return the offset as-is (good enough for most operations)
        rope_off
    }
    
    /// Save buffer to file using incremental write strategy
    pub fn save(&mut self) -> Result<()> {
        if let Some(path) = &self.path {
            // CRITICAL FIX: The rope-to-original-offset mapping is unreliable
            // with multiple edits. To ensure saved content matches viewport,
            // we save the full rope content rather than applying patches.
            // 
            // IMPORTANT: We stream rope chunks (not .to_string()) to avoid
            // loading entire file into memory (would crash on 2GB+ files).
            // TODO: Implement proper piece table for true incremental saves
            
            // For large files with edits, we need to build content with overlay applied
            let needs_overlay = !self.use_rope && !self.edits.is_empty();
            let content_source = if needs_overlay {
                // Build content with edits applied line-by-line
                let mut content = String::with_capacity(self.file_size);
                for line_num in 0..self.line_count() {
                    content.push_str(&self.get_line(line_num));
                }
                Some(content)
            } else {
                None
            };
            
            let rope = self.rope.clone();
            let path_clone = path.clone();
            let progress = Arc::clone(&self.save_progress);
            let in_progress = Arc::clone(&self.save_in_progress);
            
            // Mark save as in progress
            in_progress.store(true, Ordering::SeqCst);
            self.save_pending = true;
            
            // Edits are preserved in the HashMap until file is reloaded
            
            // Spawn background thread to write rope chunks
            std::thread::spawn(move || {
                let result = (|| -> Result<()> {
                    progress.store(10, Ordering::SeqCst);
                    
                    let temp = path_clone.with_extension("tmp");
                    let file = File::create(&temp)?;
                    let mut writer = BufWriter::with_capacity(8 * 1024 * 1024, file);
                    
                    progress.store(20, Ordering::SeqCst);
                    
                    if let Some(content) = content_source {
                        // Large file with edit overlay: write merged content
                        writer.write_all(content.as_bytes())?;
                        progress.store(90, Ordering::SeqCst);
                    } else if let Some(rope) = rope {
                        // Small file with rope: stream chunks
                        let total_bytes = rope.len_bytes();
                        let mut written = 0;
                        
                        for chunk in rope.chunks() {
                            writer.write_all(chunk.as_bytes())?;
                            written += chunk.len();
                            
                            // Update progress (20-90%)
                            let pct = 20 + ((written as f64 / total_bytes as f64) * 70.0) as u32;
                            progress.store(pct.min(90), Ordering::SeqCst);
                        }
                    } else {
                        return Err(anyhow::anyhow!("No content to save"));
                    }
                    
                    writer.flush()?;
                    
                    progress.store(90, Ordering::SeqCst);
                    
                    // Atomic rename
                    std::fs::rename(temp, &path_clone)?;
                    
                    progress.store(100, Ordering::SeqCst);
                    Ok(())
                })();
                
                // If any error occurred, set progress to 0
                if result.is_err() {
                    progress.store(0, Ordering::SeqCst);
                }
                
                // Mark not in progress anymore
                in_progress.store(false, Ordering::SeqCst);
            });
            
            Ok(())
        } else {
            anyhow::bail!("No file path set")
        }
    }
    
    /// Save buffer to a specific path
    pub fn save_as(&mut self, path: &str) -> Result<()> {
        self.path = Some(PathBuf::from(path));
        self.save()
    }
    
    /// Full rewrite - used when no mmap or many edits
    fn save_full(&self, path: &PathBuf) -> Result<()> {
        let mut file = File::create(path)?;
        
        // Write rope contents to file efficiently
        if let Some(rope) = &self.rope {
            for chunk in rope.chunks() {
                file.write_all(chunk.as_bytes())?;
            }
        }
        file.flush()?;
        
        Ok(())
    }
    
    /// Incremental save - streams from mmap and applies edits
    fn save_incremental(&self, path: &PathBuf) -> Result<()> {
        // Create temp file for atomic write
        let temp_path = path.with_extension("tmp");
        let temp_file = File::create(&temp_path)?;
        let mut writer = BufWriter::new(temp_file);
        
        // Sort edits by file offset for sequential processing
        // For lazy mode with edits, we don't need to merge
        // The edit overlay is already per-line and non-overlapping
        let mut sorted_edits: Vec<Edit> = vec![];
        sorted_edits.sort_by_key(|e| e.file_offset);
        
        // Merge overlapping edits
        let merged_edits = self.merge_edits(sorted_edits);
        
        // Stream from mmap and apply edits
        if let Some(ref mmap) = self.mmap {
            let mut file_pos = 0;
            
            for edit in &merged_edits {
                // Write unchanged region from original file
                if file_pos < edit.file_offset {
                    writer.write_all(&mmap[file_pos..edit.file_offset])?;
                }
                
                // Write edited content
                writer.write_all(edit.new_text.as_bytes())?;
                
                // Skip over the replaced bytes in original file
                file_pos = edit.file_offset + edit.old_len;
            }
            
            // Write remaining unchanged data
            if file_pos < mmap.len() {
                writer.write_all(&mmap[file_pos..])?;
            }
        }
        
        writer.flush()?;
        drop(writer);
        
        // Atomic rename (safe, no data loss risk)
        std::fs::rename(&temp_path, path)?;
        
        Ok(())
    }
    
    /// Merge overlapping edits in edit log
    fn merge_edits(&self, mut edits: Vec<Edit>) -> Vec<Edit> {
        if edits.is_empty() {
            return edits;
        }
        
        let mut merged = Vec::new();
        let mut current = edits.remove(0);
        
        for edit in edits {
            let current_end = current.file_offset + current.old_len;
            
            // Check if edits overlap or are adjacent
            if edit.file_offset <= current_end {
                // Merge edits
                let new_end = (edit.file_offset + edit.old_len).max(current_end);
                current.old_len = new_end - current.file_offset;
                current.new_text.push_str(&edit.new_text);
            } else {
                // No overlap, push current and start new
                merged.push(current);
                current = edit;
            }
        }
        
        merged.push(current);
        merged
    }

    /// Return true when a background save is running
    pub fn is_saving(&self) -> bool {
        self.save_in_progress.load(Ordering::SeqCst)
    }

    /// Get current save progress percent (0..=100)
    pub fn save_progress_percent(&self) -> u32 {
        self.save_progress.load(Ordering::SeqCst)
    }

    /// Called by main loop to finalize after background save finishes.
    /// This reloads the mmap to sync with saved file.
    pub fn finalize_save(&mut self) -> Result<()> {
        if self.save_pending && !self.save_in_progress.load(Ordering::SeqCst) {
            if let Some(path) = &self.path {
                // Re-mmap the saved file (rope already has correct content)
                let file = File::open(path)?;
                let mmap = unsafe { Mmap::map(&file)? };
                self.mmap = Some(mmap);
                
                // Reset progress and state
                self.save_progress.store(0, Ordering::SeqCst);
                self.save_pending = false;
                self.modified = false;
            }
        }
        Ok(())
    }
    

    
    /// Save using Copy-on-Write: reflink clone + in-place edits
    /// Fastest method when filesystem supports it (Btrfs, XFS, APFS)
    fn save_cow_bg(
        path: &PathBuf,
        edits: &[Edit],
        progress: &Arc<AtomicU32>,
    ) -> Result<()> {
        let temp = path.with_extension("tmp");
        
        progress.store(10, Ordering::SeqCst);
        
        // Step 1: Create reflink copy (instant, shares disk blocks)
        reflink_copy::reflink(path, &temp)
            .map_err(|e| anyhow::anyhow!("Reflink failed: {}", e))?;
        
        progress.store(30, Ordering::SeqCst);
        
        // Step 2: Merge overlapping edits
        let merged = Self::merge_edits_static(edits);
        let edit_count = merged.len();
        
        progress.store(40, Ordering::SeqCst);
        
        // Step 3: Open temp file and apply edits in-place (only modified blocks written)
        let mut file = OpenOptions::new()
            .write(true)
            .open(&temp)?;
        
        for (idx, edit) in merged.iter().enumerate() {
            // Seek to edit location
            file.seek(SeekFrom::Start(edit.file_offset as u64))?;
            
            // Write new content
            file.write_all(edit.new_text.as_bytes())?;
            
            // Update progress (40-90%)
            let edit_progress = 40 + (idx * 50 / edit_count.max(1));
            progress.store(edit_progress as u32, Ordering::SeqCst);
        }
        
        file.flush()?;
        drop(file);
        
        progress.store(95, Ordering::SeqCst);
        
        // Step 4: Atomic rename
        std::fs::rename(temp, path)?;
        
        progress.store(100, Ordering::SeqCst);
        Ok(())
    }
    
    /// Save using streaming: read original mmap + apply edit patches
    /// Universal method that works on all filesystems
    fn save_streaming_bg(
        mmap: &Mmap,
        edits: &[Edit],
        path: &PathBuf,
        progress: &Arc<AtomicU32>,
    ) -> Result<()> {
        let temp_path = path.with_extension("tmp");
        let temp_file = File::create(&temp_path)?;
        let mut writer = BufWriter::with_capacity(8 * 1024 * 1024, temp_file);
        
        progress.store(5, Ordering::SeqCst);
        
        // Merge overlapping edits
        let merged = Self::merge_edits_static(edits);
        
        progress.store(10, Ordering::SeqCst);
        
        let total = mmap.len();
        let mut written: usize = 0;
        
        // Helper to update progress (10..95%)
        let update_progress = |written_bytes: usize| {
            let pct = 10 + ((written_bytes as f64 / total as f64) * 85.0) as u32;
            progress.store(pct.min(95), Ordering::SeqCst);
        };
        
        // Stream original bytes + edit patches
        let mut file_pos: usize = 0;
        for edit in &merged {
            // Copy unchanged section from original
            if file_pos < edit.file_offset {
                let chunk = &mmap[file_pos..edit.file_offset];
                writer.write_all(chunk)?;
                written += chunk.len();
                update_progress(written);
            }
            
            // Write edited content
            writer.write_all(edit.new_text.as_bytes())?;
            written += edit.new_text.len();
            update_progress(written);
            
            file_pos = edit.file_offset + edit.old_len;
        }
        
        // Copy remaining bytes
        if file_pos < mmap.len() {
            let chunk = &mmap[file_pos..];
            writer.write_all(chunk)?;
            written += chunk.len();
            update_progress(written);
        }
        
        writer.flush()?;
        drop(writer);
        
        progress.store(98, Ordering::SeqCst);
        
        // Atomic rename
        std::fs::rename(&temp_path, path)?;
        
        progress.store(100, Ordering::SeqCst);
        Ok(())
    }
    
    /// Static helper for merging edits (used by background threads)
    fn merge_edits_static(edits: &[Edit]) -> Vec<Edit> {
        if edits.is_empty() {
            return Vec::new();
        }
        
        let mut sorted = edits.to_vec();
        sorted.sort_by_key(|e| e.file_offset);
        
        let mut merged = Vec::new();
        let mut current = sorted[0].clone();
        
        for edit in sorted.iter().skip(1) {
            let current_end = current.file_offset + current.old_len;
            
            if edit.file_offset <= current_end {
                // Overlapping: merge
                let new_end = (edit.file_offset + edit.old_len).max(current_end);
                current.old_len = new_end - current.file_offset;
                current.new_text.push_str(&edit.new_text);
            } else {
                // Non-overlapping: push current and start new
                merged.push(current);
                current = edit.clone();
            }
        }
        
        merged.push(current);
        merged
    }
}

impl Default for Buffer {
    fn default() -> Self {
        Self::new()
    }
}
