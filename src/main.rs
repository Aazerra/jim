use anyhow::Result;
use crossterm::{
    cursor::SetCursorStyle,
    event::{self, Event, KeyCode, KeyEvent, KeyModifiers},
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    ExecutableCommand,
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Terminal,
};
use std::io::{stdout, Stdout};
use std::time::{Duration, Instant};

mod buffer;
mod edit;
mod mode;
mod navigation;
mod parser;
mod ui;

use buffer::{Buffer, Cursor};
use ui::viewport::Viewport;
use parser::{Tokenizer, StructuralIndex};
use mode::{Mode, ModeHandler, EditorContext, InputResult, normal::NormalMode, insert::InsertMode, RegisterMap, PendingOperator, StructuralNavAction};
use edit::undo::UndoStack;
use std::time::Instant as StdInstant;

struct App {
    should_quit: bool,
    buffer: Buffer,
    viewport: Viewport,
    cursor: Cursor,
    frame_count: u64,
    last_fps_update: Instant,
    fps: f64,
    structural_index: Option<StructuralIndex>,
    index_build_time: f64,
    current_node_id: Option<usize>, // Current node we're on
    indexed_up_to_line: usize, // Last line that's been indexed
    max_index_size_mb: usize, // Max memory for index (default 500MB)
    show_performance: bool, // Toggle performance overlay with F12
    frame_times: Vec<Duration>, // Track last 60 frame times
    // Phase 1 additions
    mode: Mode,
    normal_mode_handler: NormalMode,
    insert_mode_handler: InsertMode,
    visual_mode_handler: Option<mode::visual::VisualMode>,
    command_mode_handler: mode::command::CommandMode,
    undo_stack: UndoStack,
    register_map: RegisterMap,
    pending_operator: Option<PendingOperator>,
    // Message display
    message: Option<String>,
    message_time: Option<Instant>,
}

impl App {
    fn new() -> Self {
        Self {
            should_quit: false,
            buffer: Buffer::new(),
            viewport: Viewport::new(0, 40), // Start at line 0, 40 lines visible
            cursor: Cursor::new(),
            frame_count: 0,
            last_fps_update: Instant::now(),
            fps: 0.0,
            structural_index: None,
            index_build_time: 0.0,
            current_node_id: None,
            indexed_up_to_line: 0,
            max_index_size_mb: 500,
            show_performance: false,
            frame_times: Vec::with_capacity(60),
            // Phase 1 initialization
            mode: Mode::Normal,
            normal_mode_handler: NormalMode::new(),
            insert_mode_handler: InsertMode::new(),
            visual_mode_handler: None,
            command_mode_handler: mode::command::CommandMode::new(),
            undo_stack: UndoStack::new(),
            register_map: RegisterMap::new(),
            pending_operator: None,
            message: None,
            message_time: None,
        }
    }

    fn load_file(&mut self, path: &str) -> Result<()> {
        let start = StdInstant::now();
        self.buffer.load_file(path)?;
        let load_time = start.elapsed();
        
        eprintln!("File loaded in {:.2}s (indexed {} lines)", 
                  load_time.as_secs_f64(), 
                  self.buffer.line_count());
        
        // Build structural index incrementally (start with first 10000 lines)
        self.expand_structural_index(10000)?;
        
        Ok(())
    }
    
    fn expand_structural_index(&mut self, target_line: usize) -> Result<()> {
        let total_lines = self.buffer.line_count();
        
        // Already indexed enough
        if self.indexed_up_to_line >= target_line.min(total_lines) {
            return Ok(());
        }
        
        let index_start = StdInstant::now();
        
        // Index in chunks to avoid loading entire file at once
        let chunk_size = 5000;
        let start_line = self.indexed_up_to_line;
        let end_line = (target_line + chunk_size).min(total_lines);
        
        // Get the text for this chunk
        let chunk_text = self.buffer.get_visible_lines(start_line, end_line - start_line);
        
        // Tokenize the chunk
        let mut tokenizer = Tokenizer::new(chunk_text);
        let tokens = tokenizer.tokenize_all();
        
        // If this is the first chunk, create new index
        if self.structural_index.is_none() {
            self.structural_index = Some(StructuralIndex::from_tokens(&tokens));
        } else {
            // Rebuild from scratch with extended range
            // This is acceptable for now as incremental parsing is a Phase 3+ feature
            let extended_text = self.buffer.get_visible_lines(0, end_line);
            let mut tokenizer = Tokenizer::new(extended_text);
            let tokens = tokenizer.tokenize_all();
            self.structural_index = Some(StructuralIndex::from_tokens(&tokens));
        }
        
        self.indexed_up_to_line = end_line;
        self.index_build_time = index_start.elapsed().as_secs_f64();
        
        eprintln!("Indexed up to line {} ({} nodes, {:.3}s)",
                  end_line,
                  self.structural_index.as_ref().map(|i| i.len()).unwrap_or(0),
                  self.index_build_time);
        
        Ok(())
    }

    fn handle_event(&mut self, event: Event) -> Result<()> {
        if let Event::Key(key) = event {
            self.handle_key(key)?;
        }
        Ok(())
    }

    fn handle_key(&mut self, key: KeyEvent) -> Result<()> {
        // Global shortcuts (work in all modes)
        match key.code {
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.should_quit = true;
                return Ok(());
            }
            KeyCode::F(12) => {
                self.show_performance = !self.show_performance;
                return Ok(());
            }
            _ => {}
        }
        
        // Read current mode before borrowing
        let current_mode = self.mode;
        
        // Capture cursor offset before borrowing (needed for visual mode initialization)
        let cursor_offset = self.cursor.byte_offset;
        
        // Create editor context for mode handlers
        let ctx = EditorContext {
            buffer: &mut self.buffer,
            cursor: &mut self.cursor,
            mode: &mut self.mode,
            undo_stack: &mut self.undo_stack,
            register_map: &mut self.register_map,
            pending_operator: &mut self.pending_operator,
            structural_index: self.structural_index.as_ref(),
        };
        
        // Route to appropriate mode handler based on saved mode value
        let result = match current_mode {
            Mode::Normal => self.normal_mode_handler.handle_key(key, ctx)?,
            Mode::Insert => self.insert_mode_handler.handle_key(key, ctx)?,
            Mode::Visual { line_wise } => {
                // Initialize visual mode if not already active
                if self.visual_mode_handler.is_none() {
                    self.visual_mode_handler = Some(mode::visual::VisualMode::new(
                        cursor_offset,
                        line_wise,
                    ));
                }
                
                if let Some(ref mut handler) = self.visual_mode_handler {
                    handler.handle_key(key, ctx)?
                } else {
                    InputResult::NotHandled
                }
            }
            Mode::Command => {
                self.command_mode_handler.handle_key(key, ctx)?
            }
        };
        
        // Handle mode handler results
        match result {
            InputResult::Handled => {
                // Update viewport to follow cursor
                self.update_viewport_for_cursor();
            }
            InputResult::ModeSwitch(new_mode) => {
                // Clear visual mode handler when leaving visual mode
                if !matches!(new_mode, Mode::Visual { .. }) {
                    self.visual_mode_handler = None;
                }
                
                // Change cursor style based on mode
                let mut out = stdout();
                let _ = match new_mode {
                    Mode::Normal => out.execute(SetCursorStyle::SteadyBlock),
                    Mode::Insert => out.execute(SetCursorStyle::SteadyBar),
                    Mode::Visual { .. } => out.execute(SetCursorStyle::SteadyBlock),
                    Mode::Command => out.execute(SetCursorStyle::SteadyUnderScore),
                };
                
                self.mode = new_mode;
                // Update viewport to follow cursor
                self.update_viewport_for_cursor();
            }
            InputResult::Quit => {
                self.should_quit = true;
            }
            InputResult::StructuralNav(action) => {
                match action {
                    mode::StructuralNavAction::NextSibling => self.navigate_next_sibling(),
                    mode::StructuralNavAction::PrevSibling => self.navigate_prev_sibling(),
                    mode::StructuralNavAction::Parent => self.navigate_parent(),
                    mode::StructuralNavAction::FirstChild => self.navigate_first_child(),
                    mode::StructuralNavAction::NextKey => self.navigate_next_key(),
                    mode::StructuralNavAction::PrevKey => self.navigate_prev_key(),
                    mode::StructuralNavAction::NextValue => self.navigate_next_value(),
                    mode::StructuralNavAction::PrevValue => self.navigate_prev_value(),
                }
                self.update_viewport_for_cursor();
            }
            InputResult::ClearNodeTracking => {
                // Cursor moved manually, invalidate cached node position
                self.current_node_id = None;
                self.update_viewport_for_cursor();
            }
            InputResult::NotHandled => {
                // Fallback to structural navigation (kept from Phase 0)
                match key.code {
                    KeyCode::Char(']') if self.mode == Mode::Normal => {
                        // Next sibling navigation - wait for 'j'
                        if let Ok(true) = event::poll(Duration::from_millis(100)) {
                            if let Ok(Event::Key(next_key)) = event::read() {
                                if next_key.code == KeyCode::Char('j') {
                                    self.navigate_next_sibling();
                                    self.update_viewport_for_cursor();
                                }
                            }
                        }
                    }
                    KeyCode::Char('[') if self.mode == Mode::Normal => {
                        // Previous sibling navigation - wait for 'j'
                        if let Ok(true) = event::poll(Duration::from_millis(100)) {
                            if let Ok(Event::Key(next_key)) = event::read() {
                                if next_key.code == KeyCode::Char('j') {
                                    self.navigate_prev_sibling();
                                    self.update_viewport_for_cursor();
                                }
                            }
                        }
                    }
                    _ => {}
                }
            }
            InputResult::Message(msg) => {
                // Display message for 3 seconds
                self.message = Some(msg);
                self.message_time = Some(Instant::now());
                
                // Return to normal mode after showing message
                if matches!(self.mode, Mode::Command) {
                    self.command_mode_handler.command_line.clear();
                    self.mode = Mode::Normal;
                    let mut out = stdout();
                    let _ = out.execute(SetCursorStyle::SteadyBlock);
                }
            }
        }
        
        // Clear message after 3 seconds
        if let Some(message_time) = self.message_time {
            if message_time.elapsed().as_secs() >= 3 {
                self.message = None;
                self.message_time = None;
            }
        }
        
        Ok(())
    }
    
    fn update_viewport_for_cursor(&mut self) {
        let viewport_height = self.viewport.height;
        let cursor_line = self.cursor.line;
        let start_line = self.viewport.start_line;
        
        // Keep cursor in view with some padding
        if cursor_line < start_line {
            // Cursor above viewport - scroll up
            self.viewport.start_line = cursor_line;
        } else if cursor_line >= start_line + viewport_height {
            // Cursor below viewport - scroll down
            self.viewport.start_line = cursor_line.saturating_sub(viewport_height - 1);
        }
    }
    
    fn navigate_next_sibling(&mut self) {
        // Ensure we've indexed enough of the file
        let target_line = self.cursor.line + 1000; // Look ahead
        let _ = self.expand_structural_index(target_line);
        
        if let Some(ref index) = self.structural_index {
            // Find current node or node at current byte offset
            let current_node = if let Some(node_id) = self.current_node_id {
                node_id
            } else {
                // Find node at current byte offset
                if let Some(node) = index.node_at(self.cursor.byte_offset) {
                    // node_at returns &NodeInfo, we need to find its index
                    // Use the node's byte offset to find its ID in the index
                    let node_id = index.nodes().iter().position(|n| n.start == node.start).unwrap_or(0);
                    self.current_node_id = Some(node_id);
                    node_id
                } else {
                    return;
                }
            };
            
            // Get next sibling - returns NodeId (usize)
            if let Some(next_sibling_id) = index.next_sibling(current_node) {
                self.current_node_id = Some(next_sibling_id);
                
                // Get the actual node info
                if let Some(next_node) = index.nodes().get(next_sibling_id) {
                    // Update cursor byte offset
                    self.cursor.byte_offset = next_node.start;
                    
                    // Convert byte offset to line number
                    let target_line = self.buffer.byte_offset_to_line(next_node.start);
                    self.cursor.line = target_line;
                    
                    // Calculate column within the line
                    let line_start_offset = self.buffer.line_to_byte_offset(target_line);
                    let offset_in_line = next_node.start - line_start_offset;
                    let line_text = self.buffer.get_line(target_line);
                    let col = line_text.chars().take_while(|c| {
                        let len = c.len_utf8();
                        let current_bytes: usize = line_text.chars()
                            .take_while(|ch| ch != c)
                            .map(|ch| ch.len_utf8())
                            .sum();
                        current_bytes < offset_in_line
                    }).count();
                    self.cursor.col = col;
                }
            }
        }
    }
    
    fn navigate_prev_sibling(&mut self) {
        // Ensure we've indexed enough of the file
        let target_line = self.cursor.line + 1000; // Current region
        let _ = self.expand_structural_index(target_line);
        
        if let Some(ref index) = self.structural_index {
            // Find current node or node at current byte offset
            let current_node = if let Some(node_id) = self.current_node_id {
                node_id
            } else {
                // Find node at current byte offset
                if let Some(node) = index.node_at(self.cursor.byte_offset) {
                    // node_at returns &NodeInfo, we need to find its index
                    let node_id = index.nodes().iter().position(|n| n.start == node.start).unwrap_or(0);
                    self.current_node_id = Some(node_id);
                    node_id
                } else {
                    return;
                }
            };
            
            // Get previous sibling - returns NodeId (usize)
            if let Some(prev_sibling_id) = index.prev_sibling(current_node) {
                self.current_node_id = Some(prev_sibling_id);
                
                // Get the actual node info
                if let Some(prev_node) = index.nodes().get(prev_sibling_id) {
                    // Update cursor byte offset
                    self.cursor.byte_offset = prev_node.start;
                    
                    // Convert byte offset to line number
                    let target_line = self.buffer.byte_offset_to_line(prev_node.start);
                    self.cursor.line = target_line;
                    
                    // Calculate column within the line
                    let line_start_offset = self.buffer.line_to_byte_offset(target_line);
                    let offset_in_line = prev_node.start - line_start_offset;
                    let line_text = self.buffer.get_line(target_line);
                    let col = line_text.chars().take_while(|c| {
                        let len = c.len_utf8();
                        let current_bytes: usize = line_text.chars()
                            .take_while(|ch| ch != c)
                            .map(|ch| ch.len_utf8())
                            .sum();
                        current_bytes < offset_in_line
                    }).count();
                    self.cursor.col = col;
                }
            }
        }
    }
    
    fn navigate_parent(&mut self) {
        // Ensure we've indexed enough of the file
        let target_line = self.cursor.line + 1000;
        let _ = self.expand_structural_index(target_line);
        
        if let Some(ref index) = self.structural_index {
            // Find current node
            let current_node = if let Some(node_id) = self.current_node_id {
                node_id
            } else {
                if let Some(node) = index.node_at(self.cursor.byte_offset) {
                    let node_id = index.nodes().iter().position(|n| n.start == node.start).unwrap_or(0);
                    self.current_node_id = Some(node_id);
                    node_id
                } else {
                    return;
                }
            };
            
            // Get parent node
            if let Some(parent_id) = index.parent(current_node) {
                self.current_node_id = Some(parent_id);
                
                if let Some(parent_node) = index.nodes().get(parent_id) {
                    self.cursor.byte_offset = parent_node.start;
                    let target_line = self.buffer.byte_offset_to_line(parent_node.start);
                    self.cursor.line = target_line;
                    
                    let line_start_offset = self.buffer.line_to_byte_offset(target_line);
                    let offset_in_line = parent_node.start - line_start_offset;
                    let line_text = self.buffer.get_line(target_line);
                    let col = line_text.chars().take_while(|c| {
                        let current_bytes: usize = line_text.chars()
                            .take_while(|ch| ch != c)
                            .map(|ch| ch.len_utf8())
                            .sum();
                        current_bytes < offset_in_line
                    }).count();
                    self.cursor.col = col;
                }
            }
        }
    }
    
    fn navigate_first_child(&mut self) {
        // Ensure we've indexed enough of the file
        let target_line = self.cursor.line + 1000;
        let _ = self.expand_structural_index(target_line);
        
        if let Some(ref index) = self.structural_index {
            // Find current node
            let current_node = if let Some(node_id) = self.current_node_id {
                node_id
            } else {
                if let Some(node) = index.node_at(self.cursor.byte_offset) {
                    let node_id = index.nodes().iter().position(|n| n.start == node.start).unwrap_or(0);
                    self.current_node_id = Some(node_id);
                    node_id
                } else {
                    return;
                }
            };
            
            // Get first child
            if let Some(child_id) = index.first_child(current_node) {
                self.current_node_id = Some(child_id);
                
                if let Some(child_node) = index.nodes().get(child_id) {
                    self.cursor.byte_offset = child_node.start;
                    let target_line = self.buffer.byte_offset_to_line(child_node.start);
                    self.cursor.line = target_line;
                    
                    let line_start_offset = self.buffer.line_to_byte_offset(target_line);
                    let offset_in_line = child_node.start - line_start_offset;
                    let line_text = self.buffer.get_line(target_line);
                    let col = line_text.chars().take_while(|c| {
                        let current_bytes: usize = line_text.chars()
                            .take_while(|ch| ch != c)
                            .map(|ch| ch.len_utf8())
                            .sum();
                        current_bytes < offset_in_line
                    }).count();
                    self.cursor.col = col;
                }
            }
        }
    }
    
    fn navigate_next_key(&mut self) {
        // Ensure we've indexed enough of the file
        let target_line = self.cursor.line + 1000;
        let _ = self.expand_structural_index(target_line);
        
        if let Some(ref index) = self.structural_index {
            if let Some(next_key_id) = index.next_key(self.cursor.byte_offset) {
                self.current_node_id = Some(next_key_id);
                
                if let Some(key_node) = index.nodes().get(next_key_id) {
                    self.cursor.byte_offset = key_node.start;
                    let target_line = self.buffer.byte_offset_to_line(key_node.start);
                    self.cursor.line = target_line;
                    
                    let line_start_offset = self.buffer.line_to_byte_offset(target_line);
                    let offset_in_line = key_node.start - line_start_offset;
                    let line_text = self.buffer.get_line(target_line);
                    let col = line_text.chars().take_while(|c| {
                        let current_bytes: usize = line_text.chars()
                            .take_while(|ch| ch != c)
                            .map(|ch| ch.len_utf8())
                            .sum();
                        current_bytes < offset_in_line
                    }).count();
                    self.cursor.col = col;
                }
            }
        }
    }
    
    fn navigate_prev_key(&mut self) {
        // Ensure we've indexed enough of the file
        let target_line = self.cursor.line + 1000;
        let _ = self.expand_structural_index(target_line);
        
        if let Some(ref index) = self.structural_index {
            if let Some(prev_key_id) = index.prev_key(self.cursor.byte_offset) {
                self.current_node_id = Some(prev_key_id);
                
                if let Some(key_node) = index.nodes().get(prev_key_id) {
                    self.cursor.byte_offset = key_node.start;
                    let target_line = self.buffer.byte_offset_to_line(key_node.start);
                    self.cursor.line = target_line;
                    
                    let line_start_offset = self.buffer.line_to_byte_offset(target_line);
                    let offset_in_line = key_node.start - line_start_offset;
                    let line_text = self.buffer.get_line(target_line);
                    let col = line_text.chars().take_while(|c| {
                        let current_bytes: usize = line_text.chars()
                            .take_while(|ch| ch != c)
                            .map(|ch| ch.len_utf8())
                            .sum();
                        current_bytes < offset_in_line
                    }).count();
                    self.cursor.col = col;
                }
            }
        }
    }
    
    fn navigate_next_value(&mut self) {
        // Ensure we've indexed enough of the file
        let target_line = self.cursor.line + 1000;
        let _ = self.expand_structural_index(target_line);
        
        if let Some(ref index) = self.structural_index {
            if let Some(next_value_id) = index.next_value(self.cursor.byte_offset) {
                self.current_node_id = Some(next_value_id);
                
                if let Some(value_node) = index.nodes().get(next_value_id) {
                    self.cursor.byte_offset = value_node.start;
                    let target_line = self.buffer.byte_offset_to_line(value_node.start);
                    self.cursor.line = target_line;
                    
                    let line_start_offset = self.buffer.line_to_byte_offset(target_line);
                    let offset_in_line = value_node.start - line_start_offset;
                    let line_text = self.buffer.get_line(target_line);
                    let col = line_text.chars().take_while(|c| {
                        let current_bytes: usize = line_text.chars()
                            .take_while(|ch| ch != c)
                            .map(|ch| ch.len_utf8())
                            .sum();
                        current_bytes < offset_in_line
                    }).count();
                    self.cursor.col = col;
                }
            }
        }
    }
    
    fn navigate_prev_value(&mut self) {
        // Ensure we've indexed enough of the file
        let target_line = self.cursor.line + 1000;
        let _ = self.expand_structural_index(target_line);
        
        if let Some(ref index) = self.structural_index {
            if let Some(prev_value_id) = index.prev_value(self.cursor.byte_offset) {
                self.current_node_id = Some(prev_value_id);
                
                if let Some(value_node) = index.nodes().get(prev_value_id) {
                    self.cursor.byte_offset = value_node.start;
                    let target_line = self.buffer.byte_offset_to_line(value_node.start);
                    self.cursor.line = target_line;
                    
                    let line_start_offset = self.buffer.line_to_byte_offset(target_line);
                    let offset_in_line = value_node.start - line_start_offset;
                    let line_text = self.buffer.get_line(target_line);
                    let col = line_text.chars().take_while(|c| {
                        let current_bytes: usize = line_text.chars()
                            .take_while(|ch| ch != c)
                            .map(|ch| ch.len_utf8())
                            .sum();
                        current_bytes < offset_in_line
                    }).count();
                    self.cursor.col = col;
                }
            }
        }
    }

    fn update_fps(&mut self) {
        self.frame_count += 1;
        let elapsed = self.last_fps_update.elapsed();
        if elapsed >= Duration::from_secs(1) {
            self.fps = self.frame_count as f64 / elapsed.as_secs_f64();
            self.frame_count = 0;
            self.last_fps_update = Instant::now();
        }
    }
}

fn setup_terminal() -> Result<Terminal<CrosstermBackend<Stdout>>> {
    enable_raw_mode()?;
    stdout().execute(EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout());
    let terminal = Terminal::new(backend)?;
    Ok(terminal)
}

fn format_size(bytes: usize) -> String {
    const KB: usize = 1024;
    const MB: usize = KB * 1024;
    const GB: usize = MB * 1024;
    
    if bytes >= GB {
        format!("{:.2} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.2} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.2} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}

fn restore_terminal(mut terminal: Terminal<CrosstermBackend<Stdout>>) -> Result<()> {
    disable_raw_mode()?;
    terminal.show_cursor()?;
    stdout().execute(LeaveAlternateScreen)?;
    Ok(())
}

fn colorize_json_line(line: &str) -> Line {
    let mut spans = Vec::new();
    let chars: Vec<char> = line.chars().collect();
    let mut pos = 0;
    
    while pos < chars.len() {
        let ch = chars[pos];
        match ch {
            '{' | '}' | '[' | ']' => {
                spans.push(Span::styled(ch.to_string(), Style::default().fg(Color::Blue)));
                pos += 1;
            }
            '"' => {
                // Find matching quote
                let start = pos;
                pos += 1;
                while pos < chars.len() && chars[pos] != '"' {
                    if chars[pos] == '\\' {
                        pos += 1; // Skip escaped char
                    }
                    pos += 1;
                }
                if pos < chars.len() {
                    pos += 1; // Include closing quote
                }
                let string: String = chars[start..pos].iter().collect();
                spans.push(Span::styled(string, Style::default().fg(Color::Green)));
            }
            '0'..='9' | '-' => {
                // Number
                let start = pos;
                while pos < chars.len() && 
                      (chars[pos].is_ascii_digit() || 
                       chars[pos] == '.' || 
                       chars[pos] == '-' ||
                       chars[pos] == 'e' ||
                       chars[pos] == 'E' ||
                       chars[pos] == '+') {
                    pos += 1;
                }
                let number: String = chars[start..pos].iter().collect();
                spans.push(Span::styled(number, Style::default().fg(Color::Yellow)));
            }
            't' if pos + 4 <= chars.len() => {
                let word: String = chars[pos..pos+4].iter().collect();
                if word == "true" {
                    spans.push(Span::styled(word, Style::default().fg(Color::Cyan)));
                    pos += 4;
                } else {
                    spans.push(Span::styled(ch.to_string(), Style::default().fg(Color::White)));
                    pos += 1;
                }
            }
            'f' if pos + 5 <= chars.len() => {
                let word: String = chars[pos..pos+5].iter().collect();
                if word == "false" {
                    spans.push(Span::styled(word, Style::default().fg(Color::Cyan)));
                    pos += 5;
                } else {
                    spans.push(Span::styled(ch.to_string(), Style::default().fg(Color::White)));
                    pos += 1;
                }
            }
            'n' if pos + 4 <= chars.len() => {
                let word: String = chars[pos..pos+4].iter().collect();
                if word == "null" {
                    spans.push(Span::styled(word, Style::default().fg(Color::Gray)));
                    pos += 4;
                } else {
                    spans.push(Span::styled(ch.to_string(), Style::default().fg(Color::White)));
                    pos += 1;
                }
            }
            ':' => {
                spans.push(Span::styled(ch.to_string(), Style::default().fg(Color::Magenta)));
                pos += 1;
            }
            _ => {
                spans.push(Span::styled(ch.to_string(), Style::default().fg(Color::White)));
                pos += 1;
            }
        }
    }
    
    Line::from(spans)
}

fn render_ui(
    terminal: &mut Terminal<CrosstermBackend<Stdout>>,
    app: &mut App,
) -> Result<()> {
    terminal.draw(|frame| {
        let size = frame.area();
        
        // Split into main area and status bar
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Min(1),
                Constraint::Length(1),
            ])
            .split(size);

        // Main content area with border
        let main_block = Block::default()
            .borders(Borders::ALL)
            .title("Jim - JSON Interactive Manager v0.1.0");
        
        let inner_area = main_block.inner(chunks[0]);
        frame.render_widget(main_block, chunks[0]);
        
        // Update viewport height to match actual terminal size
        let old_height = app.viewport.height;
        app.viewport.height = inner_area.height as usize;
        
        // If height changed or cursor out of view, update viewport
        if old_height != app.viewport.height || 
           app.cursor.line < app.viewport.start_line ||
           app.cursor.line >= app.viewport.start_line + app.viewport.height {
            let cursor_line = app.cursor.line;
            let viewport_height = app.viewport.height;
            if cursor_line < app.viewport.start_line {
                app.viewport.start_line = cursor_line;
            } else if cursor_line >= app.viewport.start_line + viewport_height {
                app.viewport.start_line = cursor_line.saturating_sub(viewport_height - 1);
            }
        }

        // Render buffer content with syntax highlighting
        let content = app.buffer.get_visible_lines(
            app.viewport.start_line,
            inner_area.height as usize,
        );
        
        // Apply syntax highlighting if we have content
        let lines: Vec<Line> = if app.structural_index.is_some() {
            content.lines().map(|line| {
                colorize_json_line(line)
            }).collect()
        } else {
            content.lines().map(|line| Line::from(line.to_string())).collect()
        };
        
        let paragraph = Paragraph::new(lines);
        frame.render_widget(paragraph, inner_area);
        
        // Set cursor position for visibility
        // Calculate cursor position relative to viewport
        let cursor_screen_line = app.cursor.line.saturating_sub(app.viewport.start_line);
        if cursor_screen_line < inner_area.height as usize {
            // Cursor is visible in viewport
            let cursor_x = inner_area.x + app.cursor.col as u16;
            let cursor_y = inner_area.y + cursor_screen_line as u16;
            frame.set_cursor_position((cursor_x, cursor_y));
        }

        // Status bar
        let status_text = if app.buffer.is_empty() {
            format!(
                " No file loaded | Press 'q' to quit | F12: perf | FPS: {:.1}",
                app.fps
            )
        } else {
            // Get file info
            let file_name = app.buffer.path()
                .and_then(|p| p.file_name())
                .and_then(|n| n.to_str())
                .unwrap_or("<unknown>");
            let file_size = format_size(app.buffer.get_file_size());
            
            // Get node type if available
            let node_info = if let (Some(node_id), Some(ref index)) = (app.current_node_id, &app.structural_index) {
                if let Some(node) = index.nodes().get(node_id) {
                    format!(" | {:?}", node.kind)
                } else {
                    String::new()
                }
            } else {
                String::new()
            };
            
            // Cursor position
            let cursor_pos = format!("{}:{}", app.cursor.line + 1, app.cursor.col + 1);
            
            // Mode indicator (Phase 1)
            let mode_indicator = app.mode.display();
            let mode_str = if !mode_indicator.is_empty() {
                format!(" {} |", mode_indicator)
            } else {
                String::new()
            };
            
            // Modified indicator
            let modified = if app.buffer.is_modified() { " [+]" } else { "" };
            // If a background save is in progress, show a small progress bar
            let mut save_suffix = String::new();
            if app.buffer.is_saving() {
                let pct = app.buffer.save_progress_percent();
                let bar_len = 10usize;
                let filled = ((pct as usize * bar_len) / 100).min(bar_len);
                let mut bar = String::new();
                for i in 0..bar_len {
                    if i < filled { bar.push('#'); } else { bar.push('-'); }
                }
                save_suffix = format!(" | Saving: [{}] {}%", bar, pct);
            }

            format!(
                " {}{} ({}) | {}:{} | {}{} |{} FPS: {:.1}{} | F12: perf",
                file_name,
                modified,
                file_size,
                app.viewport.start_line + 1,
                app.buffer.line_count(),
                cursor_pos,
                node_info,
                mode_str,
                app.fps,
                save_suffix
            )
        };
        
        // Override status with command line or message if present
        let (final_status_text, cursor_in_status) = if matches!(app.mode, Mode::Command) {
            let cmd_text = format!(":{}", app.command_mode_handler.command_line);
            let cursor_pos = cmd_text.len();
            (cmd_text, Some(cursor_pos))
        } else if let Some(ref msg) = app.message {
            (msg.clone(), None)
        } else {
            (status_text, None)
        };
        
        let status = Paragraph::new(final_status_text)
            .style(Style::default().bg(Color::DarkGray).fg(Color::White));
        frame.render_widget(status, chunks[1]);
        
        // Set cursor in status bar if in command mode
        if let Some(cursor_pos) = cursor_in_status {
            let cursor_x = chunks[1].x + cursor_pos as u16;
            let cursor_y = chunks[1].y;
            frame.set_cursor_position((cursor_x, cursor_y));
        }
        
        // Performance overlay (toggle with F12)
        if app.show_performance {
            let perf_area = ratatui::layout::Rect {
                x: size.width.saturating_sub(35),
                y: 2,
                width: 33,
                height: 8,
            };
            
            let avg_frame_time = if !app.frame_times.is_empty() {
                let sum: Duration = app.frame_times.iter().sum();
                sum.as_micros() as f64 / app.frame_times.len() as f64 / 1000.0
            } else {
                0.0
            };
            
            let p99_frame_time = if !app.frame_times.is_empty() {
                let mut sorted = app.frame_times.clone();
                sorted.sort();
                let idx = (sorted.len() as f64 * 0.99) as usize;
                sorted.get(idx).unwrap_or(&Duration::ZERO).as_micros() as f64 / 1000.0
            } else {
                0.0
            };
            
            let perf_text = vec![
                Line::from(vec![Span::styled(" Performance ", Style::default().fg(Color::Yellow))]),
                Line::from(""),
                Line::from(format!(" FPS: {:.1}", app.fps)),
                Line::from(format!(" Frame: {:.2}ms avg", avg_frame_time)),
                Line::from(format!(" Frame: {:.2}ms p99", p99_frame_time)),
                Line::from(format!(" Nodes: {}", app.structural_index.as_ref().map(|i| i.len()).unwrap_or(0))),
                Line::from(format!(" Index: {:.3}s", app.index_build_time)),
            ];
            
            let perf_block = Block::default()
                .borders(Borders::ALL)
                .style(Style::default().bg(Color::Black).fg(Color::White));
            let perf_paragraph = Paragraph::new(perf_text).block(perf_block);
            frame.render_widget(perf_paragraph, perf_area);
        }
    })?;
    
    Ok(())
}

fn run(mut app: App, mut terminal: Terminal<CrosstermBackend<Stdout>>) -> Result<()> {
    loop {
        let frame_start = Instant::now();
        
        // If a background save just finished, finalize (reload mmap)
        if let Err(e) = app.buffer.finalize_save() {
            // show message (non-fatal)
            eprintln!("Failed to finalize save: {:?}", e);
        }

        app.update_fps();
        render_ui(&mut terminal, &mut app)?;
        
        let frame_time = frame_start.elapsed();
        app.frame_times.push(frame_time);
        if app.frame_times.len() > 60 {
            app.frame_times.remove(0);
        }

        if app.should_quit {
            break;
        }

        // Poll for events with timeout to maintain ~60fps
        if event::poll(Duration::from_millis(16))? {
            let event = event::read()?;
            app.handle_event(event)?;
        }
    }

    Ok(())
}

fn main() -> Result<()> {
    // Set up panic hook to restore terminal
    let default_panic = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let _ = disable_raw_mode();
        let _ = stdout().execute(SetCursorStyle::DefaultUserShape);
        let _ = stdout().execute(LeaveAlternateScreen);
        default_panic(info);
    }));

    let mut app = App::new();
    
    // Load file if provided as argument
    let args: Vec<String> = std::env::args().collect();
    if args.len() > 1 {
        app.load_file(&args[1])?;
    }
    
    // Set initial cursor style (Normal mode = block)
    stdout().execute(SetCursorStyle::SteadyBlock)?;

    let terminal = setup_terminal()?;
    let result = run(app, terminal);
    
    // Restore terminal and cursor
    let terminal = setup_terminal()?;
    stdout().execute(SetCursorStyle::DefaultUserShape)?;
    restore_terminal(terminal)?;

    result
}
