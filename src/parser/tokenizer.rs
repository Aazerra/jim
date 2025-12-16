use super::token::{Token, TokenKind};

#[derive(Debug, Clone, Copy, PartialEq)]
#[allow(dead_code)]
enum State {
    Start,
    InString,
    InStringEscape,
    InNumber,
    InTrue(u8),     // Position in "true"
    InFalse(u8),    // Position in "false"
    InNull(u8),     // Position in "null"
}

pub struct Tokenizer {
    input: Vec<u8>,
    pos: usize,
    #[allow(dead_code)]
    state: State,
    depth: u32,
}

impl Tokenizer {
    pub fn new(input: String) -> Self {
        Self {
            input: input.into_bytes(),
            pos: 0,
            state: State::Start,
            depth: 0,
        }
    }

    pub fn from_bytes(input: Vec<u8>) -> Self {
        Self {
            input,
            pos: 0,
            state: State::Start,
            depth: 0,
        }
    }

    fn peek(&self) -> Option<u8> {
        if self.pos < self.input.len() {
            Some(self.input[self.pos])
        } else {
            None
        }
    }

    fn advance(&mut self) -> Option<u8> {
        if self.pos < self.input.len() {
            let ch = self.input[self.pos];
            self.pos += 1;
            Some(ch)
        } else {
            None
        }
    }

    fn skip_whitespace(&mut self) -> Option<Token> {
        let start = self.pos;
        
        while let Some(ch) = self.peek() {
            if ch == b' ' || ch == b'\n' || ch == b'\r' || ch == b'\t' {
                self.advance();
            } else {
                break;
            }
        }

        if self.pos > start {
            Some(Token::new(TokenKind::Whitespace, start, self.pos, self.depth))
        } else {
            None
        }
    }

    fn tokenize_string(&mut self, start: usize) -> Token {
        // Consume opening quote
        self.advance();
        
        loop {
            match self.advance() {
                Some(b'"') => {
                    // End of string
                    return Token::new(TokenKind::String, start, self.pos, self.depth);
                }
                Some(b'\\') => {
                    // Escape sequence - consume next character
                    self.advance();
                }
                Some(_) => {
                    // Regular character, continue
                }
                None => {
                    // Unexpected end of input
                    return Token::new(TokenKind::Invalid, start, self.pos, self.depth);
                }
            }
        }
    }

    fn tokenize_number(&mut self, start: usize) -> Token {
        // Consume optional minus
        if self.peek() == Some(b'-') {
            self.advance();
        }

        // Consume digits
        let mut has_digits = false;
        while let Some(ch) = self.peek() {
            if ch.is_ascii_digit() {
                self.advance();
                has_digits = true;
            } else {
                break;
            }
        }

        // Consume optional decimal part
        if self.peek() == Some(b'.') {
            self.advance();
            while let Some(ch) = self.peek() {
                if ch.is_ascii_digit() {
                    self.advance();
                } else {
                    break;
                }
            }
        }

        // Consume optional exponent
        if let Some(ch) = self.peek() {
            if ch == b'e' || ch == b'E' {
                self.advance();
                if let Some(sign) = self.peek() {
                    if sign == b'+' || sign == b'-' {
                        self.advance();
                    }
                }
                while let Some(ch) = self.peek() {
                    if ch.is_ascii_digit() {
                        self.advance();
                    } else {
                        break;
                    }
                }
            }
        }

        if has_digits {
            Token::new(TokenKind::Number, start, self.pos, self.depth)
        } else {
            Token::new(TokenKind::Invalid, start, self.pos, self.depth)
        }
    }

    fn match_keyword(&mut self, start: usize, keyword: &[u8], kind: TokenKind) -> Token {
        for &expected in keyword {
            match self.advance() {
                Some(ch) if ch == expected => continue,
                _ => return Token::new(TokenKind::Invalid, start, self.pos, self.depth),
            }
        }
        Token::new(kind, start, self.pos, self.depth)
    }

    pub fn next_token(&mut self) -> Option<Token> {
        // Skip whitespace
        if let Some(ws_token) = self.skip_whitespace() {
            return Some(ws_token);
        }

        let start = self.pos;
        let ch = self.peek()?;

        let token = match ch {
            b'{' => {
                self.advance();
                self.depth += 1;
                Token::new(TokenKind::BraceOpen, start, self.pos, self.depth - 1)
            }
            b'}' => {
                self.advance();
                self.depth = self.depth.saturating_sub(1);
                Token::new(TokenKind::BraceClose, start, self.pos, self.depth)
            }
            b'[' => {
                self.advance();
                self.depth += 1;
                Token::new(TokenKind::BracketOpen, start, self.pos, self.depth - 1)
            }
            b']' => {
                self.advance();
                self.depth = self.depth.saturating_sub(1);
                Token::new(TokenKind::BracketClose, start, self.pos, self.depth)
            }
            b':' => {
                self.advance();
                Token::new(TokenKind::Colon, start, self.pos, self.depth)
            }
            b',' => {
                self.advance();
                Token::new(TokenKind::Comma, start, self.pos, self.depth)
            }
            b'"' => self.tokenize_string(start),
            b'-' | b'0'..=b'9' => self.tokenize_number(start),
            b't' => self.match_keyword(start, b"true", TokenKind::True),
            b'f' => self.match_keyword(start, b"false", TokenKind::False),
            b'n' => self.match_keyword(start, b"null", TokenKind::Null),
            _ => {
                self.advance();
                Token::new(TokenKind::Invalid, start, self.pos, self.depth)
            }
        };

        Some(token)
    }

    pub fn tokenize_all(&mut self) -> Vec<Token> {
        let mut tokens = Vec::new();
        while let Some(token) = self.next_token() {
            tokens.push(token);
        }
        tokens
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tokenize_simple_object() {
        let input = r#"{"key": "value"}"#.to_string();
        let mut tokenizer = Tokenizer::new(input);
        let tokens = tokenizer.tokenize_all();
        
        let kinds: Vec<TokenKind> = tokens.iter().map(|t| t.kind).collect();
        assert_eq!(kinds, vec![
            TokenKind::BraceOpen,
            TokenKind::String,
            TokenKind::Colon,
            TokenKind::Whitespace,
            TokenKind::String,
            TokenKind::BraceClose,
        ]);
    }

    #[test]
    fn test_tokenize_array() {
        let input = r#"[1, 2, 3]"#.to_string();
        let mut tokenizer = Tokenizer::new(input);
        let tokens = tokenizer.tokenize_all();
        
        let kinds: Vec<TokenKind> = tokens.iter().map(|t| t.kind).collect();
        assert_eq!(kinds, vec![
            TokenKind::BracketOpen,
            TokenKind::Number,
            TokenKind::Comma,
            TokenKind::Whitespace,
            TokenKind::Number,
            TokenKind::Comma,
            TokenKind::Whitespace,
            TokenKind::Number,
            TokenKind::BracketClose,
        ]);
    }

    #[test]
    fn test_tokenize_keywords() {
        let input = r#"true false null"#.to_string();
        let mut tokenizer = Tokenizer::new(input);
        let tokens = tokenizer.tokenize_all();
        
        let kinds: Vec<TokenKind> = tokens.iter()
            .filter(|t| t.kind != TokenKind::Whitespace)
            .map(|t| t.kind)
            .collect();
        assert_eq!(kinds, vec![
            TokenKind::True,
            TokenKind::False,
            TokenKind::Null,
        ]);
    }

    #[test]
    fn test_tokenize_depth() {
        let input = r#"{"a": [1, 2]}"#.to_string();
        let mut tokenizer = Tokenizer::new(input);
        let tokens = tokenizer.tokenize_all();
        
        let depths: Vec<u32> = tokens.iter()
            .filter(|t| matches!(t.kind, TokenKind::BraceOpen | TokenKind::BracketOpen))
            .map(|t| t.depth)
            .collect();
        assert_eq!(depths, vec![0, 1]);
    }

    #[test]
    fn test_tokenize_escaped_string() {
        let input = r#""hello \"world\"""#.to_string();
        let mut tokenizer = Tokenizer::new(input);
        let tokens = tokenizer.tokenize_all();
        
        assert_eq!(tokens.len(), 1);
        assert_eq!(tokens[0].kind, TokenKind::String);
    }

    #[test]
    fn test_tokenize_numbers() {
        let inputs = vec![
            "123",
            "-456",
            "12.34",
            "-78.90",
            "1e10",
            "1.5e-3",
        ];
        
        for input in inputs {
            let mut tokenizer = Tokenizer::new(input.to_string());
            let tokens = tokenizer.tokenize_all();
            assert_eq!(tokens.len(), 1, "Failed for input: {}", input);
            assert_eq!(tokens[0].kind, TokenKind::Number, "Failed for input: {}", input);
        }
    }
}
