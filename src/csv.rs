use std::{
    fs,
    io,
    time::{Duration, Instant},
};

use ratatui::{
    crossterm::event::{KeyCode, KeyModifiers, MouseButton, MouseEventKind},
    buffer::Buffer,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    widgets::{Block, Borders},
    Frame,
};

#[derive(Clone, Copy, PartialEq)]
pub enum Confirm {
    Save,
    Delete,
}

pub enum CsvAction {
    Continue,
    Quit,
}

pub struct CsvEditor {
    pub path: String,
    pub rows: Vec<Vec<String>>,
    widths: Vec<u16>,
    pub sel_row: usize,
    pub sel_col: usize,
    scroll_row: usize,
    col_page: usize,
    last_width: u16,
    pub editing: bool,
    pub edit_buf: String,
    pub dirty: bool,
    pub confirm: Option<Confirm>,
    pub msg: String,
    last_click: Option<(Instant, usize, usize)>,
    cell_rects: Vec<(Rect, usize, usize)>,
}

impl CsvEditor {
    pub fn load(path: &str) -> io::Result<Self> {
        let mut rows: Vec<Vec<String>> = Vec::new();
        if fs::metadata(path).is_ok() {
            let mut rdr = csv::ReaderBuilder::new()
                .has_headers(false)
                .flexible(true)
                .from_path(path)
                .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
            for rec in rdr.records() {
                let rec = rec.map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
                rows.push(rec.iter().map(|s| s.to_string()).collect());
            }
        }
        if rows.is_empty() {
            rows.push(vec![String::new()]);
        }
        let mut editor = CsvEditor {
            path: path.to_string(),
            rows,
            widths: Vec::new(),
            sel_row: 0,
            sel_col: 0,
            scroll_row: 0,
            col_page: 0,
            last_width: 80,
            editing: false,
            edit_buf: String::new(),
            dirty: false,
            confirm: None,
            msg: String::new(),
            last_click: None,
            cell_rects: Vec::new(),
        };
        editor.ensure_trailing_empty_row_and_col();
        Ok(editor)
    }

    fn recalc_widths(&mut self) {
        let ncols = self.ncols();
        let mut widths = vec![3u16; ncols];
        for r in &self.rows {
            for (i, c) in r.iter().enumerate() {
                let w = (c.chars().count() as u16 + 2).max(3);
                if w > widths[i] {
                    widths[i] = w;
                }
            }
        }
        self.widths = widths;
    }

    fn ensure_trailing_empty_row_and_col(&mut self) {
        let max_cols = self.rows.iter().map(|r| r.len()).max().unwrap_or(1);
        for r in self.rows.iter_mut() {
            while r.len() < max_cols {
                r.push(String::new());
            }
        }

        let last_col_has_data = self
            .rows
            .iter()
            .any(|r| !r.last().map_or(true, |s| s.is_empty()));
        if last_col_has_data {
            for r in self.rows.iter_mut() {
                r.push(String::new());
            }
        }

        let ncols = self.ncols();
        let last_row_has_data = self
            .rows
            .last()
            .map_or(true, |r| r.iter().any(|s| !s.is_empty()));
        if last_row_has_data {
            self.rows.push(vec![String::new(); ncols]);
        }

        self.recalc_widths();
    }

    fn ncols(&self) -> usize {
        self.rows.first().map(|r| r.len()).unwrap_or(1)
    }

    fn col_pages(&self, max_width: u16) -> Vec<std::ops::Range<usize>> {
        let ncols = self.ncols();
        if ncols == 0 || max_width == 0 {
            return vec![0..1];
        }
        let mut pages = Vec::new();
        let mut start = 0;
        while start < ncols {
            let mut end = start;
            let mut current_w = 0u16;
            while end < ncols {
                let mut col_w = self.widths.get(end).copied().unwrap_or(10);
                if self.editing && end == self.sel_col {
                    let edit_w = (self.edit_buf.chars().count() as u16 + 3).max(3);
                    if edit_w > col_w {
                        col_w = edit_w;
                    }
                }
                let sep_w = if end > start { 1 } else { 0 };
                if current_w + sep_w + col_w > max_width && end > start {
                    break;
                }
                current_w += sep_w + col_w;
                end += 1;
            }
            if end == start {
                end = start + 1;
            }
            pages.push(start..end);
            start = end;
        }
        if pages.is_empty() {
            pages.push(0..ncols);
        }
        pages
    }

    fn prev_page(&mut self) {
        let pages = self.col_pages(self.last_width);
        if pages.is_empty() {
            return;
        }
        let current_p = pages
            .iter()
            .position(|r| r.contains(&self.sel_col))
            .unwrap_or(0);
        if current_p > 0 {
            let new_p = current_p - 1;
            self.sel_col = pages[new_p].start;
            self.col_page = new_p;
        }
    }

    fn next_page(&mut self) {
        let pages = self.col_pages(self.last_width);
        if pages.is_empty() {
            return;
        }
        let current_p = pages
            .iter()
            .position(|r| r.contains(&self.sel_col))
            .unwrap_or(0);
        if current_p + 1 < pages.len() {
            let new_p = current_p + 1;
            self.sel_col = pages[new_p].start;
            self.col_page = new_p;
        }
    }

    pub fn save(&mut self) -> io::Result<()> {
        let mut last_non_empty_row = 0;
        let mut last_non_empty_col = 0;
        let mut has_data = false;
        for (r, row) in self.rows.iter().enumerate() {
            for (c, val) in row.iter().enumerate() {
                if !val.is_empty() {
                    has_data = true;
                    if r > last_non_empty_row {
                        last_non_empty_row = r;
                    }
                    if c > last_non_empty_col {
                        last_non_empty_col = c;
                    }
                }
            }
        }
        let mut wtr = csv::WriterBuilder::new()
            .from_path(&self.path)
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
        if has_data {
            for r in 0..=last_non_empty_row {
                if r < self.rows.len() {
                    let slice = &self.rows[r]
                        [..=last_non_empty_col.min(self.rows[r].len().saturating_sub(1))];
                    wtr.write_record(slice)
                        .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
                }
            }
        } else {
            wtr.write_record(&[""])
                .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
        }
        wtr.flush()?;
        self.dirty = false;
        self.msg = format!("Saved to {}", self.path);
        Ok(())
    }

    fn delete_cell(&mut self) {
        let col = self.sel_col;
        for r in self.sel_row..self.rows.len().saturating_sub(1) {
            let next = self.rows[r + 1][col].clone();
            self.rows[r][col] = next;
        }
        if let Some(last) = self.rows.last_mut() {
            last[col] = String::new();
        }
        self.dirty = true;
        self.ensure_trailing_empty_row_and_col();
        self.msg = "Cell deleted, column shifted up".into();
    }

    fn move_sel(&mut self, dr: isize, dc: isize) {
        let nr = self.rows.len() as isize;
        let nc = self.ncols() as isize;
        let mut r = self.sel_row as isize + dr;
        let mut c = self.sel_col as isize + dc;
        r = r.clamp(0, nr - 1);
        c = c.clamp(0, nc - 1);
        self.sel_row = r as usize;
        self.sel_col = c as usize;
    }
}

/// Render the CSV editor into the given area.
pub fn csv_ui(f: &mut Frame, editor: &mut CsvEditor, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(3), Constraint::Length(1)])
        .split(area);
    let table_area = chunks[0];
    let status_area = chunks[1];

    let block = Block::default().borders(Borders::ALL);
    let inner = block.inner(table_area);

    let pages = editor.col_pages(inner.width);
    editor.last_width = inner.width;
    if let Some(p_idx) = pages.iter().position(|r| r.contains(&editor.sel_col)) {
        editor.col_page = p_idx;
    } else if editor.col_page >= pages.len() {
        editor.col_page = pages.len().saturating_sub(1);
    }

    let page_str = if pages.len() > 1 {
        format!(" [Page {}/{}]", editor.col_page + 1, pages.len())
    } else {
        String::new()
    };

    let title = format!(
        " csv: {}{} {}",
        editor.path,
        page_str,
        if editor.dirty { "[modified]" } else { "" }
    );
    let block = block.title(title);
    f.render_widget(block, table_area);

    editor.cell_rects.clear();

    if editor.sel_row < editor.scroll_row {
        editor.scroll_row = editor.sel_row;
    }
    let visible_rows = inner.height as usize;
    if visible_rows > 0 && editor.sel_row >= editor.scroll_row + visible_rows {
        editor.scroll_row = editor.sel_row + 1 - visible_rows;
    }

    let current_page = pages.get(editor.col_page).cloned().unwrap_or(0..editor.ncols());
    let buf: &mut Buffer = f.buffer_mut();

    for (ridx, row) in editor.rows.iter().enumerate().skip(editor.scroll_row) {
        let y = inner.y + (ridx - editor.scroll_row) as u16;
        if y >= inner.y + inner.height {
            break;
        }
        let mut x = inner.x;
        for cidx in current_page.clone() {
            if cidx >= row.len() {
                break;
            }
            let mut width = editor.widths.get(cidx).copied().unwrap_or(10);
            if editor.editing && cidx == editor.sel_col {
                let edit_w = (editor.edit_buf.chars().count() as u16 + 3).max(3);
                if edit_w > width {
                    width = edit_w;
                }
            }
            if x >= inner.x + inner.width {
                break;
            }
            let avail = (inner.x + inner.width).saturating_sub(x);
            let w = width.min(avail);
            let is_sel = ridx == editor.sel_row && cidx == editor.sel_col;
            let content = if is_sel && editor.editing {
                format!("{}_", editor.edit_buf)
            } else {
                row[cidx].clone()
            };
            let mut text = content;
            if text.chars().count() as u16 > w.saturating_sub(1) && w < width {
                let take = w.saturating_sub(2).max(1) as usize;
                text = text.chars().take(take).collect::<String>() + "~";
            }
            let padded = format!("{:<width$}", text, width = w as usize);
            let mut style = Style::default();
            if ridx == 0 {
                style = style.add_modifier(Modifier::BOLD);
            }
            if is_sel {
                style = if editor.editing {
                    Style::default().bg(Color::Yellow).fg(Color::Black)
                } else {
                    Style::default().bg(Color::Cyan).fg(Color::Black)
                };
            }
            buf.set_string(x, y, &padded, style);
            editor.cell_rects.push((
                Rect {
                    x,
                    y,
                    width: w,
                    height: 1,
                },
                ridx,
                cidx,
            ));
            x += w;
            if x < inner.x + inner.width && cidx + 1 < current_page.end {
                buf.set_string(x, y, "|", Style::default().fg(Color::DarkGray));
                x += 1;
            }
        }
    }

    let status = if let Some(c) = editor.confirm {
        match c {
            Confirm::Save => "Save changes? (y/n)".to_string(),
            Confirm::Delete => "Delete focused cell? (y/n)".to_string(),
        }
    } else if editor.editing {
        "Editing: Enter=confirm  Esc=cancel".to_string()
    } else if !editor.msg.is_empty() {
        editor.msg.clone()
    } else {
        "Arrows=move Alt+Left/Right=page Enter/DblClick=edit Ctrl+S=save Ctrl+D=delete Esc=back"
            .to_string()
    };
    buf.set_string(status_area.x, status_area.y, &status, Style::default());
}

fn hit_test(editor: &CsvEditor, x: u16, y: u16) -> Option<(usize, usize)> {
    for (rect, r, c) in &editor.cell_rects {
        if x >= rect.x && x < rect.x + rect.width && y == rect.y {
            return Some((*r, *c));
        }
    }
    None
}

/// Handle a key event for the CSV editor. Returns CsvAction::Quit when the
/// user wants to leave the CSV editor (Esc/q in normal mode).
pub fn csv_handle_key(editor: &mut CsvEditor, code: KeyCode, modifiers: KeyModifiers) -> CsvAction {
    // Internal confirmation prompts (Save / Delete)
    if let Some(c) = editor.confirm {
        match code {
            KeyCode::Char('y') | KeyCode::Char('Y') => {
                match c {
                    Confirm::Save => {
                        let _ = editor.save();
                    }
                    Confirm::Delete => editor.delete_cell(),
                }
                editor.confirm = None;
            }
            KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
                editor.confirm = None;
                editor.msg = "Cancelled".into();
            }
            _ => {}
        }
        return CsvAction::Continue;
    }

    // Cell edit mode
    if editor.editing {
        match code {
            KeyCode::Enter => {
                editor.rows[editor.sel_row][editor.sel_col] = editor.edit_buf.clone();
                editor.editing = false;
                editor.dirty = true;
                editor.ensure_trailing_empty_row_and_col();
            }
            KeyCode::Esc => {
                editor.editing = false;
            }
            KeyCode::Backspace => {
                editor.edit_buf.pop();
                editor.rows[editor.sel_row][editor.sel_col] = editor.edit_buf.clone();
                editor.dirty = true;
                editor.ensure_trailing_empty_row_and_col();
            }
            KeyCode::Char(c) => {
                editor.edit_buf.push(c);
                editor.rows[editor.sel_row][editor.sel_col] = editor.edit_buf.clone();
                editor.dirty = true;
                editor.ensure_trailing_empty_row_and_col();
            }
            _ => {}
        }
        return CsvAction::Continue;
    }

    // Normal navigation mode
    match (code, modifiers) {
        (KeyCode::Char('s'), KeyModifiers::CONTROL) => {
            editor.confirm = Some(Confirm::Save);
        }
        (KeyCode::Char('d'), KeyModifiers::CONTROL) => {
            editor.confirm = Some(Confirm::Delete);
        }
        (KeyCode::Esc, _) | (KeyCode::Char('q'), KeyModifiers::NONE) => {
            return CsvAction::Quit;
        }
        (KeyCode::Left, m) if m.contains(KeyModifiers::ALT) => {
            editor.prev_page();
        }
        (KeyCode::Right, m) if m.contains(KeyModifiers::ALT) => {
            editor.next_page();
        }
        (KeyCode::Up, _) => editor.move_sel(-1, 0),
        (KeyCode::Down, _) => editor.move_sel(1, 0),
        (KeyCode::Left, _) => editor.move_sel(0, -1),
        (KeyCode::Right, _) => editor.move_sel(0, 1),
        (KeyCode::Enter, _) => {
            editor.editing = true;
            editor.edit_buf = editor.rows[editor.sel_row][editor.sel_col].clone();
        }
        _ => {}
    }
    CsvAction::Continue
}

/// Handle a mouse event for the CSV editor.
pub fn csv_handle_mouse(editor: &mut CsvEditor, kind: MouseEventKind, col: u16, row: u16) {
    if let MouseEventKind::Down(MouseButton::Left) = kind {
        if let Some((r, c)) = hit_test(editor, col, row) {
            let now = Instant::now();
            let is_double = matches!(
                editor.last_click,
                Some((t, lr, lc)) if lr == r && lc == c && now.duration_since(t) < Duration::from_millis(400)
            );
            editor.sel_row = r;
            editor.sel_col = c;
            if is_double && editor.confirm.is_none() {
                editor.editing = true;
                editor.edit_buf = editor.rows[r][c].clone();
                editor.last_click = None;
            } else {
                editor.last_click = Some((now, r, c));
            }
        }
    }
}