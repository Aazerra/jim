use anyhow::Result;

/// Vim-style operators
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Operator {
    Delete,
    Change,
    Yank,
}

/// Vim-style motions
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Motion {
    /// Move by characters: h, l
    Char(Direction, usize),
    /// Move by lines: j, k
    Line(Direction, usize),
    /// Move by words: w, b, e
    Word(WordMotion, usize),
    /// Move to line position: 0, $, ^
    LinePosition(LinePosition),
    /// Text object: iw, aw, i", a"
    TextObject(TextObject),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    Forward,
    Backward,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WordMotion {
    Start,      // w - next word start
    End,        // e - next word end
    BackStart,  // b - previous word start
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinePosition {
    Start,          // 0 - line start
    FirstNonBlank,  // ^ - first non-whitespace
    End,            // $ - line end
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextObject {
    Word { inner: bool },       // iw, aw
    Quotes { inner: bool },     // i", a"
    SingleQuotes { inner: bool }, // i', a'
    Braces { inner: bool },     // i{, a{ (JSON object)
    Brackets { inner: bool },   // i[, a[ (JSON array)
}

/// Result of applying an operator to a motion
#[derive(Debug, Clone)]
pub struct OperatorResult {
    pub range: std::ops::Range<usize>,
    pub text: String,
}

/// A pending operator waiting for a motion
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PendingOperator {
    pub operator: Operator,
    pub count: usize,
}

impl Motion {
    /// Calculate the byte range this motion covers from a starting position
    pub fn calculate_range(&self, buffer: &crate::buffer::Buffer, start_offset: usize) -> Result<std::ops::Range<usize>> {
        match self {
            Motion::Char(dir, count) => {
                let end = match dir {
                    Direction::Forward => (start_offset + count).min(buffer.len_bytes()),
                    Direction::Backward => start_offset.saturating_sub(*count),
                };
                Ok(start_offset.min(end)..start_offset.max(end))
            }
            Motion::Line(dir, count) => {
                let start_line = buffer.byte_offset_to_line(start_offset);
                let target_line = match dir {
                    Direction::Forward => (start_line + count).min(buffer.line_count().saturating_sub(1)),
                    Direction::Backward => start_line.saturating_sub(*count),
                };
                
                let line_start = buffer.line_to_byte_offset(start_line.min(target_line));
                let line_end = buffer.line_to_byte_offset(start_line.max(target_line) + 1);
                Ok(line_start..line_end)
            }
            Motion::Word(motion, count) => {
                Self::calculate_word_range(buffer, start_offset, motion, *count)
            }
            Motion::LinePosition(pos) => {
                let line = buffer.byte_offset_to_line(start_offset);
                let line_start = buffer.line_to_byte_offset(line);
                let line_end = buffer.line_to_byte_offset(line + 1);
                
                match pos {
                    LinePosition::Start => Ok(start_offset..line_start.max(start_offset)),
                    LinePosition::End => Ok(start_offset..line_end.saturating_sub(1).max(start_offset)),
                    LinePosition::FirstNonBlank => {
                        // Find first non-whitespace
                        let line_text = buffer.get_line(line);
                        let first_non_blank = line_text.chars()
                            .position(|c| !c.is_whitespace())
                            .unwrap_or(0);
                        Ok(start_offset..line_start + first_non_blank)
                    }
                }
            }
            Motion::TextObject(obj) => {
                Self::calculate_text_object_range(buffer, start_offset, obj)
            }
        }
    }
    
    fn calculate_word_range(buffer: &crate::buffer::Buffer, start: usize, motion: &WordMotion, count: usize) -> Result<std::ops::Range<usize>> {
        let text = buffer.slice(0..buffer.len_bytes());
        let chars: Vec<char> = text.chars().collect();
        let mut byte_pos = start;
        let mut char_idx = text[..start].chars().count();
        
        for _ in 0..count {
            match motion {
                WordMotion::Start => {
                    // Find next word start
                    while char_idx < chars.len() && !chars[char_idx].is_alphanumeric() && chars[char_idx] != '_' {
                        byte_pos += chars[char_idx].len_utf8();
                        char_idx += 1;
                    }
                    while char_idx < chars.len() && (chars[char_idx].is_alphanumeric() || chars[char_idx] == '_') {
                        byte_pos += chars[char_idx].len_utf8();
                        char_idx += 1;
                    }
                }
                WordMotion::End => {
                    // Find next word end
                    if char_idx < chars.len() {
                        byte_pos += chars[char_idx].len_utf8();
                        char_idx += 1;
                    }
                    while char_idx < chars.len() && !chars[char_idx].is_alphanumeric() && chars[char_idx] != '_' {
                        byte_pos += chars[char_idx].len_utf8();
                        char_idx += 1;
                    }
                    while char_idx < chars.len() && (chars[char_idx].is_alphanumeric() || chars[char_idx] == '_') {
                        byte_pos += chars[char_idx].len_utf8();
                        char_idx += 1;
                    }
                    byte_pos = byte_pos.saturating_sub(1);
                }
                WordMotion::BackStart => {
                    // Find previous word start
                    if char_idx > 0 {
                        char_idx -= 1;
                        byte_pos = byte_pos.saturating_sub(chars[char_idx].len_utf8());
                    }
                    while char_idx > 0 && !chars[char_idx].is_alphanumeric() && chars[char_idx] != '_' {
                        char_idx -= 1;
                        byte_pos = byte_pos.saturating_sub(chars[char_idx].len_utf8());
                    }
                    while char_idx > 0 && (chars[char_idx].is_alphanumeric() || chars[char_idx] == '_') {
                        char_idx -= 1;
                        byte_pos = byte_pos.saturating_sub(chars[char_idx].len_utf8());
                    }
                }
            }
        }
        
        Ok(start.min(byte_pos)..start.max(byte_pos))
    }
    
    fn calculate_text_object_range(buffer: &crate::buffer::Buffer, start: usize, obj: &TextObject) -> Result<std::ops::Range<usize>> {
        match obj {
            TextObject::Word { inner } => {
                let text = buffer.slice(0..buffer.len_bytes());
                let chars: Vec<char> = text.chars().collect();
                let char_idx = text[..start].chars().count();
                
                // Find word boundaries
                let mut word_start = char_idx;
                while word_start > 0 && (chars[word_start - 1].is_alphanumeric() || chars[word_start - 1] == '_') {
                    word_start -= 1;
                }
                
                let mut word_end = char_idx;
                while word_end < chars.len() && (chars[word_end].is_alphanumeric() || chars[word_end] == '_') {
                    word_end += 1;
                }
                
                if !inner {
                    // 'aw' includes trailing whitespace
                    while word_end < chars.len() && chars[word_end].is_whitespace() {
                        word_end += 1;
                    }
                }
                
                let byte_start = text[..word_start].len();
                let byte_end = text[..word_end].len();
                Ok(byte_start..byte_end)
            }
            TextObject::Quotes { inner } => {
                let text = buffer.slice(0..buffer.len_bytes());
                // Find enclosing quotes
                if let Some(range) = Self::find_enclosing_quotes(&text, start, '"', *inner) {
                    Ok(range)
                } else {
                    Ok(start..start)
                }
            }
            _ => Ok(start..start), // TODO: Implement other text objects
        }
    }
    
    fn find_enclosing_quotes(text: &str, pos: usize, quote: char, inner: bool) -> Option<std::ops::Range<usize>> {
        let bytes: Vec<u8> = text.bytes().collect();
        
        // Find opening quote before pos
        let mut start = pos;
        while start > 0 && bytes[start - 1] != quote as u8 {
            start -= 1;
        }
        if start == 0 || bytes[start - 1] != quote as u8 {
            return None;
        }
        start -= 1; // Include opening quote
        
        // Find closing quote after pos
        let mut end = pos;
        while end < bytes.len() && bytes[end] != quote as u8 {
            end += 1;
        }
        if end >= bytes.len() || bytes[end] != quote as u8 {
            return None;
        }
        end += 1; // Include closing quote
        
        if inner {
            // Exclude quotes
            Some((start + 1)..(end - 1))
        } else {
            Some(start..end)
        }
    }
}
