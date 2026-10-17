use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BufferState {
    pub lines: Vec<String>,
    pub cursor_line: usize,
    pub cursor_col: usize,
    pub crlf: bool,
}

pub struct Buffer {
    pub lines: Vec<String>,
    pub cursor_line: usize,
    pub cursor_col: usize, // Grapheme cluster index within the line
    pub scroll_top: usize,
    pub scroll_left: usize,
    pub is_dirty: bool,
    pub crlf: bool,

    // Selection range (anchor_line, anchor_col) to (cursor_line, cursor_col)
    pub _selection_anchor: Option<(usize, usize)>,

    // Undo / Redo stacks
    undo_stack: Vec<BufferState>,
    redo_stack: Vec<BufferState>,
    pub last_was_insert: bool,
}

impl Default for Buffer {
    fn default() -> Self {
        Self::new()
    }
}

impl Buffer {
    pub fn new() -> Self {
        Self {
            lines: vec![String::new()],
            cursor_line: 0,
            cursor_col: 0,
            scroll_top: 0,
            scroll_left: 0,
            is_dirty: false,
            crlf: false,
            _selection_anchor: None,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            last_was_insert: false,
        }
    }

    pub fn from_string(content: &str) -> Self {
        let crlf = content.contains("\r\n");
        let norm = if crlf {
            content.replace("\r\n", "\n")
        } else {
            content.to_string()
        };

        let lines: Vec<String> = norm.split('\n').map(|s| s.to_string()).collect();
        let lines = if lines.is_empty() {
            vec![String::new()]
        } else {
            lines
        };

        Self {
            lines,
            cursor_line: 0,
            cursor_col: 0,
            scroll_top: 0,
            scroll_left: 0,
            is_dirty: false,
            crlf,
            _selection_anchor: None,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            last_was_insert: false,
        }
    }

    pub fn to_string_content(&self) -> String {
        self.lines
            .join(if self.crlf { "\r\n" } else { "\n" })
    }

    pub fn save_snapshot(&mut self) {
        let state = BufferState {
            lines: self.lines.clone(),
            cursor_line: self.cursor_line,
            cursor_col: self.cursor_col,
            crlf: self.crlf,
        };
        self.undo_stack.push(state);
        self.redo_stack.clear();
        if self.undo_stack.len() > 100 {
            self.undo_stack.remove(0);
        }
    }

    pub fn undo(&mut self) {
        if let Some(state) = self.undo_stack.pop() {
            let current = BufferState {
                lines: self.lines.clone(),
                cursor_line: self.cursor_line,
                cursor_col: self.cursor_col,
                crlf: self.crlf,
            };
            self.redo_stack.push(current);
            self.lines = state.lines;
            self.cursor_line = state.cursor_line;
            self.cursor_col = state.cursor_col;
            self.crlf = state.crlf;
            self.is_dirty = true;
            self.clamp_cursor();
        }
    }

    pub fn redo(&mut self) {
        if let Some(state) = self.redo_stack.pop() {
            let current = BufferState {
                lines: self.lines.clone(),
                cursor_line: self.cursor_line,
                cursor_col: self.cursor_col,
                crlf: self.crlf,
            };
            self.undo_stack.push(current);
            self.lines = state.lines;
            self.cursor_line = state.cursor_line;
            self.cursor_col = state.cursor_col;
            self.crlf = state.crlf;
            self.is_dirty = true;
            self.clamp_cursor();
        }
    }

    pub fn line_grapheme_count(&self, line_idx: usize) -> usize {
        self.lines
            .get(line_idx)
            .map(|l| l.graphemes(true).count())
            .unwrap_or(0)
    }

    pub fn clamp_cursor(&mut self) {
        if self.lines.is_empty() {
            self.lines.push(String::new());
        }
        if self.cursor_line >= self.lines.len() {
            self.cursor_line = self.lines.len() - 1;
        }
        let len = self.line_grapheme_count(self.cursor_line);
        if self.cursor_col > len {
            self.cursor_col = len;
        }
    }

pub fn byte_idx_for_grapheme(line_str: &str, grapheme_idx: usize) -> usize {
    line_str
        .grapheme_indices(true)
        .nth(grapheme_idx)
        .map(|(idx, _)| idx)
        .unwrap_or_else(|| line_str.len())
}

    pub fn insert_char(&mut self, c: char) {
        if !self.last_was_insert {
            self.save_snapshot();
            self.last_was_insert = true;
        }

        self.clamp_cursor();
        let line_str = &mut self.lines[self.cursor_line];
        let byte_idx = Self::byte_idx_for_grapheme(line_str, self.cursor_col);
        line_str.insert(byte_idx, c);
        self.cursor_col += 1;
        self.is_dirty = true;
    }

    pub fn insert_str(&mut self, text: &str) {
        self.save_snapshot();
        self.last_was_insert = false;

        self.clamp_cursor();
        let normalized = text.replace("\r\n", "\n");
        let pasted_lines: Vec<&str> = normalized.split('\n').collect();

        if pasted_lines.is_empty() {
            return;
        }

        let cur_line = &mut self.lines[self.cursor_line];
        let byte_idx = Self::byte_idx_for_grapheme(cur_line, self.cursor_col);
        let remainder = cur_line[byte_idx..].to_string();
        cur_line.truncate(byte_idx);

        if pasted_lines.len() == 1 {
            cur_line.push_str(pasted_lines[0]);
            let inserted_graphemes = pasted_lines[0].graphemes(true).count();
            cur_line.push_str(&remainder);
            self.cursor_col += inserted_graphemes;
        } else {
            cur_line.push_str(pasted_lines[0]);
            let mut insert_idx = self.cursor_line + 1;
            for i in 1..pasted_lines.len() - 1 {
                self.lines.insert(insert_idx, pasted_lines[i].to_string());
                insert_idx += 1;
            }
            let last_line = format!("{}{}", pasted_lines.last().unwrap(), remainder);
            self.cursor_col = pasted_lines.last().unwrap().graphemes(true).count();
            self.lines.insert(insert_idx, last_line);
            self.cursor_line = insert_idx;
        }

        self.is_dirty = true;
    }

    pub fn insert_tab(&mut self) {
        self.save_snapshot();
        self.last_was_insert = false;
        // Insert 4 spaces as indent
        self.insert_str("    ");
    }

    pub fn insert_newline(&mut self) {
        self.save_snapshot();
        self.last_was_insert = false;

        self.clamp_cursor();
        let current_line = &self.lines[self.cursor_line];
        let byte_idx = Self::byte_idx_for_grapheme(current_line, self.cursor_col);

        let leading_spaces: String = current_line
            .chars()
            .take_while(|c| c.is_whitespace() && *c != '\n' && *c != '\r')
            .collect();

        let remainder = current_line[byte_idx..].to_string();
        self.lines[self.cursor_line].truncate(byte_idx);

        let new_line = format!("{}{}", leading_spaces, remainder);
        self.cursor_line += 1;
        self.lines.insert(self.cursor_line, new_line);
        self.cursor_col = leading_spaces.graphemes(true).count();
        self.is_dirty = true;
    }

    pub fn backspace(&mut self) {
        self.save_snapshot();
        self.last_was_insert = false;

        self.clamp_cursor();
        if self.cursor_col > 0 {
            let line = &mut self.lines[self.cursor_line];
            let prev_byte_idx = Self::byte_idx_for_grapheme(line, self.cursor_col - 1);
            let cur_byte_idx = Self::byte_idx_for_grapheme(line, self.cursor_col);
            line.drain(prev_byte_idx..cur_byte_idx);
            self.cursor_col -= 1;
            self.is_dirty = true;
        } else if self.cursor_line > 0 {
            let current_line = self.lines.remove(self.cursor_line);
            self.cursor_line -= 1;
            let prev_line = &mut self.lines[self.cursor_line];
            self.cursor_col = prev_line.graphemes(true).count();
            prev_line.push_str(&current_line);
            self.is_dirty = true;
        }
    }

    pub fn delete(&mut self) {
        self.save_snapshot();
        self.last_was_insert = false;

        self.clamp_cursor();
        let grapheme_count = self.line_grapheme_count(self.cursor_line);
        if self.cursor_col < grapheme_count {
            let line = &mut self.lines[self.cursor_line];
            let cur_byte_idx = Self::byte_idx_for_grapheme(line, self.cursor_col);
            let next_byte_idx = Self::byte_idx_for_grapheme(line, self.cursor_col + 1);
            line.drain(cur_byte_idx..next_byte_idx);
            self.is_dirty = true;
        } else if self.cursor_line + 1 < self.lines.len() {
            let next_line = self.lines.remove(self.cursor_line + 1);
            self.lines[self.cursor_line].push_str(&next_line);
            self.is_dirty = true;
        }
    }

    pub fn move_left(&mut self) {
        self.last_was_insert = false;
        if self.cursor_col > 0 {
            self.cursor_col -= 1;
        } else if self.cursor_line > 0 {
            self.cursor_line -= 1;
            self.cursor_col = self.line_grapheme_count(self.cursor_line);
        }
    }

    pub fn move_right(&mut self) {
        self.last_was_insert = false;
        let count = self.line_grapheme_count(self.cursor_line);
        if self.cursor_col < count {
            self.cursor_col += 1;
        } else if self.cursor_line + 1 < self.lines.len() {
            self.cursor_line += 1;
            self.cursor_col = 0;
        }
    }

    pub fn move_up(&mut self) {
        self.last_was_insert = false;
        if self.cursor_line > 0 {
            self.cursor_line -= 1;
            let count = self.line_grapheme_count(self.cursor_line);
            if self.cursor_col > count {
                self.cursor_col = count;
            }
        }
    }

    pub fn move_down(&mut self) {
        self.last_was_insert = false;
        if self.cursor_line + 1 < self.lines.len() {
            self.cursor_line += 1;
            let count = self.line_grapheme_count(self.cursor_line);
            if self.cursor_col > count {
                self.cursor_col = count;
            }
        }
    }

    pub fn move_home(&mut self) {
        self.last_was_insert = false;
        self.cursor_col = 0;
    }

    pub fn move_end(&mut self) {
        self.last_was_insert = false;
        self.cursor_col = self.line_grapheme_count(self.cursor_line);
    }

    pub fn move_page_up(&mut self, height: usize) {
        self.last_was_insert = false;
        self.cursor_line = self.cursor_line.saturating_sub(height.max(1));
        let count = self.line_grapheme_count(self.cursor_line);
        if self.cursor_col > count {
            self.cursor_col = count;
        }
    }

    pub fn move_page_down(&mut self, height: usize) {
        self.last_was_insert = false;
        if !self.lines.is_empty() {
            self.cursor_line = (self.cursor_line + height.max(1)).min(self.lines.len() - 1);
            let count = self.line_grapheme_count(self.cursor_line);
            if self.cursor_col > count {
                self.cursor_col = count;
            }
        }
    }

    pub fn move_word_left(&mut self) {
        self.last_was_insert = false;
        if self.cursor_col == 0 {
            if self.cursor_line > 0 {
                self.cursor_line -= 1;
                self.cursor_col = self.line_grapheme_count(self.cursor_line);
            }
            return;
        }

        let line = &self.lines[self.cursor_line];
        let graphemes: Vec<&str> = line.graphemes(true).collect();
        let mut idx = self.cursor_col;

        // Skip whitespace
        while idx > 0 && graphemes.get(idx - 1).map_or(false, |g| g.chars().all(|c| c.is_whitespace())) {
            idx -= 1;
        }
        // Skip word chars
        while idx > 0 && graphemes.get(idx - 1).map_or(false, |g| g.chars().all(|c| c.is_alphanumeric() || c == '_')) {
            idx -= 1;
        }
        self.cursor_col = idx;
    }

    pub fn move_word_right(&mut self) {
        self.last_was_insert = false;
        let count = self.line_grapheme_count(self.cursor_line);
        if self.cursor_col >= count {
            if self.cursor_line + 1 < self.lines.len() {
                self.cursor_line += 1;
                self.cursor_col = 0;
            }
            return;
        }

        let line = &self.lines[self.cursor_line];
        let graphemes: Vec<&str> = line.graphemes(true).collect();
        let mut idx = self.cursor_col;

        // Skip current word chars
        while idx < count && graphemes.get(idx).map_or(false, |g| g.chars().all(|c| c.is_alphanumeric() || c == '_')) {
            idx += 1;
        }
        // Skip whitespace
        while idx < count && graphemes.get(idx).map_or(false, |g| g.chars().all(|c| c.is_whitespace())) {
            idx += 1;
        }
        self.cursor_col = idx;
    }

    pub fn find_next(&mut self, query: &str) -> bool {
        if query.is_empty() {
            return false;
        }
        let query_lower = query.to_lowercase();
        let start_line = self.cursor_line;
        let num_lines = self.lines.len();

        for i in 0..num_lines {
            let line_idx = (start_line + i) % num_lines;
            let line_str = &self.lines[line_idx];
            let line_lower = line_str.to_lowercase();

            let search_from_byte = if line_idx == start_line && i == 0 {
                Self::byte_idx_for_grapheme(line_str, self.cursor_col + 1)
            } else {
                0
            };

            if search_from_byte < line_lower.len() {
                if let Some(byte_pos) = line_lower[search_from_byte..].find(&query_lower) {
                    let total_byte_pos = search_from_byte + byte_pos;
                    self.cursor_line = line_idx;
                    self.cursor_col = line_str[..total_byte_pos].graphemes(true).count();
                    return true;
                }
            }
        }
        false
    }

    pub fn find_prev(&mut self, query: &str) -> bool {
        if query.is_empty() {
            return false;
        }
        let query_lower = query.to_lowercase();
        let start_line = self.cursor_line;
        let num_lines = self.lines.len();

        for i in 0..num_lines {
            let line_idx = (start_line + num_lines - i) % num_lines;
            let line_str = &self.lines[line_idx];
            let line_lower = line_str.to_lowercase();

            let search_up_to_byte = if line_idx == start_line && i == 0 {
                Self::byte_idx_for_grapheme(line_str, self.cursor_col)
            } else {
                line_lower.len()
            };

            if search_up_to_byte > 0 {
                if let Some(byte_pos) = line_lower[..search_up_to_byte].rfind(&query_lower) {
                    self.cursor_line = line_idx;
                    self.cursor_col = line_str[..byte_pos].graphemes(true).count();
                    return true;
                }
            }
        }
        false
    }

    pub fn goto_line(&mut self, target_line: usize) {
        if target_line == 0 {
            self.cursor_line = 0;
        } else {
            self.cursor_line = (target_line - 1).min(self.lines.len().saturating_sub(1));
        }
        self.cursor_col = 0;
    }

    pub fn visual_column(&self, line_str: &str, grapheme_idx: usize, tab_width: usize) -> usize {
        Self::static_visual_column(line_str, grapheme_idx, tab_width)
    }

    pub fn static_visual_column(line_str: &str, grapheme_idx: usize, tab_width: usize) -> usize {
        let mut visual_col = 0;
        for (idx, g) in line_str.graphemes(true).enumerate() {
            if idx >= grapheme_idx {
                break;
            }
            if g == "\t" {
                visual_col += tab_width - (visual_col % tab_width);
            } else {
                visual_col += UnicodeWidthStr::width(g);
            }
        }
        visual_col
    }

    pub fn ensure_cursor_visible(&mut self, height: usize, width: usize) {
        if height == 0 || width == 0 {
            return;
        }
        if self.cursor_line < self.scroll_top {
            self.scroll_top = self.cursor_line;
        } else if self.cursor_line >= self.scroll_top + height {
            self.scroll_top = self.cursor_line.saturating_sub(height - 1);
        }

        let current_line = self.lines.get(self.cursor_line).map(|s| s.as_str()).unwrap_or("");
        let vis_col = self.visual_column(current_line, self.cursor_col, 4);

        if vis_col < self.scroll_left {
            self.scroll_left = vis_col;
        } else if vis_col >= self.scroll_left + width {
            self.scroll_left = vis_col.saturating_sub(width - 1);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lossless_crlf_roundtrip() {
        let text = "line1\r\nline2\r\n";
        let buf = Buffer::from_string(text);
        assert!(buf.crlf);
        assert_eq!(buf.lines, vec!["line1", "line2", ""]);
        assert_eq!(buf.to_string_content(), text);
    }

    #[test]
    fn test_lossless_lf_roundtrip() {
        let text = "a\n\n";
        let buf = Buffer::from_string(text);
        assert!(!buf.crlf);
        assert_eq!(buf.lines, vec!["a", "", ""]);
        assert_eq!(buf.to_string_content(), text);
    }

    #[test]
    fn test_grapheme_cluster_navigation() {
        let mut buf = Buffer::from_string("e\u{0301}s"); // é (e + combining acute accent)
        assert_eq!(buf.line_grapheme_count(0), 2);
        buf.move_right();
        assert_eq!(buf.cursor_col, 1);
        buf.backspace();
        assert_eq!(buf.cursor_col, 0);
        assert_eq!(buf.lines[0], "s");
    }

    #[test]
    fn test_undo_redo() {
        let mut buf = Buffer::from_string("hello");
        buf.move_end();
        buf.insert_str(" world");
        assert_eq!(buf.lines[0], "hello world");
        buf.undo();
        assert_eq!(buf.lines[0], "hello");
        buf.redo();
        assert_eq!(buf.lines[0], "hello world");
    }
}
