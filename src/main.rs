use anyhow::Result;
use crossterm::{
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
mod navigation;
mod parser;
mod ui;

use buffer::{Buffer, Cursor};
use ui::viewport::Viewport;
use parser::{Tokenizer, StructuralIndex};
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
    show_performance: bool, // Toggle performance overlay with F12
    frame_times: Vec<Duration>, // Track last 60 frame times
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
            show_performance: false,
            frame_times: Vec::with_capacity(60),
        }
    }

    fn load_file(&mut self, path: &str) -> Result<()> {
        let start = StdInstant::now();
        self.buffer.load_file(path)?;
        let load_time = start.elapsed();
        
        eprintln!("File loaded in {:.2}s (indexed {} lines)", 
                  load_time.as_secs_f64(), 
                  self.buffer.line_count());
        
        // Build structural index from visible portion for syntax highlighting
        let index_start = StdInstant::now();
        let visible_lines = self.buffer.get_visible_lines(0, 1000.min(self.buffer.line_count()));
        let mut tokenizer = Tokenizer::new(visible_lines);
        let tokens = tokenizer.tokenize_all();
        self.structural_index = Some(StructuralIndex::from_tokens(&tokens));
        self.index_build_time = index_start.elapsed().as_secs_f64();
        
        eprintln!("Structural index built in {:.3}s ({} nodes)",
                  self.index_build_time,
                  self.structural_index.as_ref().map(|i| i.len()).unwrap_or(0));
        
        Ok(())
    }

    fn handle_event(&mut self, event: Event) -> Result<()> {
        if let Event::Key(key) = event {
            self.handle_key(key)?;
        }
        Ok(())
    }

    fn handle_key(&mut self, key: KeyEvent) -> Result<()> {
        match key.code {
            KeyCode::Char('q') => self.should_quit = true,
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.should_quit = true
            }
            KeyCode::Char('j') | KeyCode::Down => self.viewport.scroll_down(),
            KeyCode::Char('k') | KeyCode::Up => self.viewport.scroll_up(),
            KeyCode::Char('d') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.viewport.scroll_down_page()
            }
            KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.viewport.scroll_up_page()
            }
            KeyCode::Char(']') => {
                // Next sibling navigation - wait for 'j'
                if let Ok(true) = event::poll(Duration::from_millis(100)) {
                    if let Ok(Event::Key(next_key)) = event::read() {
                        if next_key.code == KeyCode::Char('j') {
                            self.navigate_next_sibling();
                        }
                    }
                }
            }
            KeyCode::Char('[') => {
                // Previous sibling navigation - wait for 'j'
                if let Ok(true) = event::poll(Duration::from_millis(100)) {
                    if let Ok(Event::Key(next_key)) = event::read() {
                        if next_key.code == KeyCode::Char('j') {
                            self.navigate_prev_sibling();
                        }
                    }
                }
            }
            KeyCode::F(12) => {
                self.show_performance = !self.show_performance;
            }
            _ => {}
        }
        Ok(())
    }
    
    fn navigate_next_sibling(&mut self) {
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
                    self.cursor.set_byte_offset(next_node.start);
                    
                    // Convert byte offset to line number and scroll viewport
                    let target_line = self.buffer.byte_offset_to_line(next_node.start);
                    self.cursor.line = target_line;
                    self.viewport.start_line = target_line.saturating_sub(5); // Center with some context
                }
            }
        }
    }
    
    fn navigate_prev_sibling(&mut self) {
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
                    self.cursor.set_byte_offset(prev_node.start);
                    
                    // Convert byte offset to line number and scroll viewport
                    let target_line = self.buffer.byte_offset_to_line(prev_node.start);
                    self.cursor.line = target_line;
                    self.viewport.start_line = target_line.saturating_sub(5); // Center with some context
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
            .title("JSON Tool v0.1.0 - Phase 0");
        
        let inner_area = main_block.inner(chunks[0]);
        frame.render_widget(main_block, chunks[0]);

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
            let file_size = format_size(app.buffer.file_size());
            
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
            
            format!(
                " {} ({}) | {}:{} | {}{} | FPS: {:.1} | F12: perf",
                file_name,
                file_size,
                app.viewport.start_line + 1,
                app.buffer.line_count(),
                cursor_pos,
                node_info,
                app.fps
            )
        };
        
        let status = Paragraph::new(status_text)
            .style(Style::default().bg(Color::DarkGray).fg(Color::White));
        frame.render_widget(status, chunks[1]);
        
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
        let _ = stdout().execute(LeaveAlternateScreen);
        default_panic(info);
    }));

    let mut app = App::new();
    
    // Load file if provided as argument
    let args: Vec<String> = std::env::args().collect();
    if args.len() > 1 {
        app.load_file(&args[1])?;
    }

    let terminal = setup_terminal()?;
    let result = run(app, terminal);
    
    // Restore terminal
    let terminal = setup_terminal()?;
    restore_terminal(terminal)?;

    result
}
