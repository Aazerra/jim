use crossbeam::channel::{bounded, Sender, Receiver};
use std::thread;
use crate::parser::{Tokenizer, Token};

#[derive(Debug, Clone)]
pub enum ParserMessage {
    Parse(String),  // JSON content to parse
    Shutdown,
}

#[derive(Debug, Clone)]
pub enum ParserResponse {
    Tokens(Vec<Token>),
    Progress(f32),  // Progress percentage (0.0 to 1.0)
    Complete,
    Error(String),
}

pub struct ParserThread {
    cmd_tx: Sender<ParserMessage>,
    resp_rx: Receiver<ParserResponse>,
    handle: Option<thread::JoinHandle<()>>,
}

impl ParserThread {
    pub fn new() -> Self {
        let (cmd_tx, cmd_rx) = bounded::<ParserMessage>(100);
        let (resp_tx, resp_rx) = bounded::<ParserResponse>(100);

        let handle = thread::spawn(move || {
            Self::parser_worker(cmd_rx, resp_tx);
        });

        Self {
            cmd_tx,
            resp_rx,
            handle: Some(handle),
        }
    }

    fn parser_worker(
        cmd_rx: Receiver<ParserMessage>,
        resp_tx: Sender<ParserResponse>,
    ) {
        loop {
            match cmd_rx.recv() {
                Ok(ParserMessage::Parse(content)) => {
                    // Send progress updates
                    let _ = resp_tx.send(ParserResponse::Progress(0.0));
                    
                    // Tokenize the content
                    let mut tokenizer = Tokenizer::new(content.clone());
                    
                    // For large content, tokenize in chunks and send progress
                    let total_size = content.len();
                    let mut tokens = Vec::new();
                    
                    while let Some(token) = tokenizer.next_token() {
                        tokens.push(token);
                        
                        // Send progress update every 1000 tokens
                        if tokens.len() % 1000 == 0 {
                            let progress = token.end as f32 / total_size as f32;
                            let _ = resp_tx.send(ParserResponse::Progress(progress));
                        }
                    }
                    
                    // Send final tokens
                    let _ = resp_tx.send(ParserResponse::Tokens(tokens));
                    let _ = resp_tx.send(ParserResponse::Complete);
                }
                Ok(ParserMessage::Shutdown) => {
                    break;
                }
                Err(_) => {
                    break;
                }
            }
        }
    }

    pub fn parse(&self, content: String) -> Result<(), String> {
        self.cmd_tx
            .send(ParserMessage::Parse(content))
            .map_err(|e| format!("Failed to send parse message: {}", e))
    }

    pub fn try_recv_response(&self) -> Option<ParserResponse> {
        self.resp_rx.try_recv().ok()
    }

    pub fn shutdown(mut self) {
        let _ = self.cmd_tx.send(ParserMessage::Shutdown);
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

impl Default for ParserThread {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for ParserThread {
    fn drop(&mut self) {
        let _ = self.cmd_tx.send(ParserMessage::Shutdown);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn test_parser_thread() {
        let parser = ParserThread::new();
        let json = r#"{"key": "value"}"#.to_string();
        
        parser.parse(json).unwrap();
        
        // Wait a bit for parsing to complete
        std::thread::sleep(Duration::from_millis(100));
        
        let mut got_tokens = false;
        let mut got_complete = false;
        
        while let Some(response) = parser.try_recv_response() {
            match response {
                ParserResponse::Tokens(_) => got_tokens = true,
                ParserResponse::Complete => got_complete = true,
                _ => {}
            }
        }
        
        assert!(got_tokens, "Should receive tokens");
        assert!(got_complete, "Should receive complete signal");
        
        parser.shutdown();
    }
}
