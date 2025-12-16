pub struct Viewport {
    pub start_line: usize,
    pub height: usize,
}

impl Viewport {
    pub fn new(start_line: usize, height: usize) -> Self {
        Self { start_line, height }
    }

    pub fn scroll_down(&mut self) {
        self.start_line = self.start_line.saturating_add(1);
    }

    pub fn scroll_up(&mut self) {
        self.start_line = self.start_line.saturating_sub(1);
    }

    pub fn scroll_down_page(&mut self) {
        self.start_line = self.start_line.saturating_add(self.height / 2);
    }

    pub fn scroll_up_page(&mut self) {
        self.start_line = self.start_line.saturating_sub(self.height / 2);
    }
}
