use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokenKind {
    BraceOpen,      // {
    BraceClose,     // }
    BracketOpen,    // [
    BracketClose,   // ]
    Colon,          // :
    Comma,          // ,
    String,         // "..."
    Number,         // 123, 12.34, -5, 1e10
    True,           // true
    False,          // false
    Null,           // null
    Whitespace,     // spaces, newlines, tabs
    Invalid,        // parse error
}

impl fmt::Display for TokenKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TokenKind::BraceOpen => write!(f, "{{"),
            TokenKind::BraceClose => write!(f, "}}"),
            TokenKind::BracketOpen => write!(f, "["),
            TokenKind::BracketClose => write!(f, "]"),
            TokenKind::Colon => write!(f, ":"),
            TokenKind::Comma => write!(f, ","),
            TokenKind::String => write!(f, "String"),
            TokenKind::Number => write!(f, "Number"),
            TokenKind::True => write!(f, "true"),
            TokenKind::False => write!(f, "false"),
            TokenKind::Null => write!(f, "null"),
            TokenKind::Whitespace => write!(f, "Whitespace"),
            TokenKind::Invalid => write!(f, "Invalid"),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Token {
    pub kind: TokenKind,
    pub start: usize,    // byte offset
    pub end: usize,      // byte offset (exclusive)
    pub depth: u32,      // nesting depth
}

impl Token {
    pub fn new(kind: TokenKind, start: usize, end: usize, depth: u32) -> Self {
        Self {
            kind,
            start,
            end,
            depth,
        }
    }

    pub fn len(&self) -> usize {
        self.end - self.start
    }

    pub fn is_empty(&self) -> bool {
        self.start >= self.end
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_token_creation() {
        let token = Token::new(TokenKind::String, 0, 10, 1);
        assert_eq!(token.kind, TokenKind::String);
        assert_eq!(token.start, 0);
        assert_eq!(token.end, 10);
        assert_eq!(token.depth, 1);
        assert_eq!(token.len(), 10);
    }

    #[test]
    fn test_token_display() {
        assert_eq!(format!("{}", TokenKind::BraceOpen), "{");
        assert_eq!(format!("{}", TokenKind::String), "String");
        assert_eq!(format!("{}", TokenKind::Number), "Number");
    }
}
