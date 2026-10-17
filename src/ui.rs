use ratatui::{
    layout::{Alignment, Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph},
    Frame,
};
use unicode_width::UnicodeWidthStr;

use crate::{
    app::{App, FileStatus, Focus, PromptMode},
    highlight::highlight_line,
};

pub fn draw(frame: &mut Frame, app: &mut App) {
    let area = frame.area();

    // Main layout: Header (1), Content (Min 1), Status bar (1)
    let main_layout = Layout::vertical([
        Constraint::Length(1),
        Constraint::Min(1),
        Constraint::Length(1),
    ])
    .split(area);

    // 1. Path Bar Header
    let (title_text, title_style) = if matches!(app.focus, Focus::Path) {
        (
            format!("Path: {}", app.path_input.content),
            Style::default().fg(Color::Yellow).bold(),
        )
    } else {
        (
            app.current_dir.to_string_lossy().to_string(),
            Style::default().fg(Color::Cyan).bold(),
        )
    };

    let title = Paragraph::new(title_text).style(title_style);
    frame.render_widget(title, main_layout[0]);

    if matches!(app.focus, Focus::Path) {
        let vis_col = crate::buffer::Buffer::static_visual_column(&app.path_input.content, app.path_input.cursor_col, 4);
        let cursor_x = main_layout[0].x + 6 + vis_col as u16;
        let max_x = main_layout[0].x + main_layout[0].width.saturating_sub(1);
        frame.set_cursor_position((cursor_x.min(max_x), main_layout[0].y));
    }

    // 2. Content Layout (Explorer + Editor)
    let max_entry_len = app
        .entries
        .iter()
        .map(|e| UnicodeWidthStr::width(e.name.as_str()))
        .max()
        .unwrap_or(10);

    let explorer_width = (max_entry_len + 5).clamp(18, 35) as u16;

    let content_layout = Layout::horizontal([
        Constraint::Length(explorer_width),
        Constraint::Min(1),
    ])
    .split(main_layout[1]);

    render_explorer(frame, app, content_layout[0]);
    render_editor(frame, app, content_layout[1]);

    // 3. Status Bar
    render_status_bar(frame, app, main_layout[2]);

    // 4. Modal Overlays
    if app.prompt_mode != PromptMode::None {
        render_prompt_modal(frame, app, area);
    } else if matches!(app.focus, Focus::Find) {
        render_find_bar(frame, app, area);
    } else if matches!(app.focus, Focus::GotoLine) {
        render_goto_modal(frame, app, area);
    } else if matches!(app.focus, Focus::NewFile) {
        render_new_file_modal(frame, app, area);
    }
}

fn render_explorer(frame: &mut Frame, app: &mut App, area: Rect) {
    let bg_color = Color::Rgb(30, 30, 30);
    let bg_block = Block::default().style(Style::default().bg(bg_color));
    frame.render_widget(bg_block, area);

    let (search_area, list_area) = if app.show_search {
        let chunks = Layout::vertical([Constraint::Length(1), Constraint::Min(1)]).split(area);
        (Some(chunks[0]), chunks[1])
    } else {
        (None, area)
    };

    if let Some(s_area) = search_area {
        let search_style = if matches!(app.focus, Focus::Search) {
            Style::default().fg(Color::Yellow).bold().bg(bg_color)
        } else {
            Style::default().fg(Color::DarkGray).bg(bg_color)
        };

        let search_widget =
            Paragraph::new(format!("Search: {}", app.search_input.content)).style(search_style);
        frame.render_widget(search_widget, s_area);

        if matches!(app.focus, Focus::Search) {
            let vis_col = crate::buffer::Buffer::static_visual_column(&app.search_input.content, app.search_input.cursor_col, 4);
            let cursor_x = s_area.x + 8 + vis_col as u16;
            let max_x = s_area.x + s_area.width.saturating_sub(1);
            frame.set_cursor_position((cursor_x.min(max_x), s_area.y));
        }
    }

    let filtered = app.filtered_entries();

    let items: Vec<ListItem> = filtered
        .iter()
        .enumerate()
        .map(|(index, (_, entry))| {
            let symlink_flag = if entry.is_symlink { "@" } else { "" };
            let prefix = if entry.is_dir {
                "> "
            } else {
                "  "
            };

            let name_display = truncate_str(&entry.name, 30);
            let text = format!("{}{}{}", prefix, name_display, symlink_flag);

            let style = if index == app.selected && matches!(app.focus, Focus::Explorer) {
                Style::default().bg(Color::Blue).fg(Color::White).bold()
            } else if index == app.selected {
                Style::default().bg(Color::Rgb(65, 65, 65)).fg(Color::White)
            } else if entry.is_dir {
                Style::default().fg(Color::Cyan).bg(bg_color)
            } else {
                Style::default().fg(Color::Reset).bg(bg_color)
            };

            ListItem::new(Line::from(text)).style(style)
        })
        .collect();

    let list = List::new(items);
    frame.render_stateful_widget(list, list_area, &mut app.explorer_state);
}

fn render_editor(frame: &mut Frame, app: &mut App, area: Rect) {
    let inner_area = area;

    match &app.file_status {
        FileStatus::NoneSelected | FileStatus::IsDirectory => {
            let msg = Paragraph::new("Select a file to edit")
                .style(Style::default().fg(Color::DarkGray).add_modifier(Modifier::ITALIC))
                .alignment(Alignment::Center);

            let center_layout = Layout::vertical([
                Constraint::Percentage(45),
                Constraint::Length(1),
                Constraint::Percentage(45),
            ])
            .split(inner_area);

            frame.render_widget(msg, center_layout[1]);
        }
        FileStatus::NotRegular => {
            let msg = Paragraph::new("Special / Non-regular file (FIFO/Device)")
                .style(Style::default().fg(Color::Yellow).bold())
                .alignment(Alignment::Center);

            let center_layout = Layout::vertical([
                Constraint::Percentage(45),
                Constraint::Length(1),
                Constraint::Percentage(45),
            ])
            .split(inner_area);

            frame.render_widget(msg, center_layout[1]);
        }
        FileStatus::TooLarge => {
            let msg = Paragraph::new("File too large (> 2 MiB)")
                .style(Style::default().fg(Color::Yellow).bold())
                .alignment(Alignment::Center);

            let center_layout = Layout::vertical([
                Constraint::Percentage(45),
                Constraint::Length(1),
                Constraint::Percentage(45),
            ])
            .split(inner_area);

            frame.render_widget(msg, center_layout[1]);
        }
        FileStatus::CsvLoaded => {
            if let Some(ref mut csv) = app.csv_editor {
                crate::csv::csv_ui(frame, csv, inner_area);
            }
        }
        FileStatus::NotUtf8 => {
            let msg = Paragraph::new("Not a UTF-8 encoded text file")
                .style(Style::default().fg(Color::Red).bold())
                .alignment(Alignment::Center);

            let center_layout = Layout::vertical([
                Constraint::Percentage(45),
                Constraint::Length(1),
                Constraint::Percentage(45),
            ])
            .split(inner_area);

            frame.render_widget(msg, center_layout[1]);
        }
        FileStatus::Unreadable(err) => {
            let msg = Paragraph::new(format!("Cannot read file: {err}"))
                .style(Style::default().fg(Color::Red).bold())
                .alignment(Alignment::Center);

            let center_layout = Layout::vertical([
                Constraint::Percentage(45),
                Constraint::Length(1),
                Constraint::Percentage(45),
            ])
            .split(inner_area);

            frame.render_widget(msg, center_layout[1]);
        }
        FileStatus::Utf8Loaded => {
            let total_lines = app.buffer.lines.len();
            let max_line_digits = format!("{}", total_lines).len().max(3);
            let gutter_width = max_line_digits + 3;

            let view_height = inner_area.height as usize;
            let view_width = (inner_area.width as usize).saturating_sub(gutter_width);

            let ext = app
                .active_path
                .as_ref()
                .and_then(|p| p.extension())
                .and_then(|e| e.to_str())
                .unwrap_or("")
                .to_lowercase();

            let visible_lines: Vec<Line> = app
                .buffer
                .lines
                .iter()
                .enumerate()
                .skip(app.buffer.scroll_top)
                .take(view_height)
                .map(|(line_idx, line_str)| {
                    let gutter = Span::styled(
                        format!("{:>width$} │ ", line_idx + 1, width = max_line_digits),
                        Style::default().fg(Color::DarkGray),
                    );

                    // Highlight full line first
                    let full_spans = highlight_line(line_str, &ext);

                    // Expand tabs and horizontal slice based on visual scroll
                    let sliced_spans = slice_and_expand_spans(
                        full_spans,
                        app.buffer.scroll_left,
                        view_width,
                        4,
                    );

                    let mut line_spans = vec![gutter];
                    line_spans.extend(sliced_spans);

                    Line::from(line_spans)
                })
                .collect();

            let editor_paragraph = Paragraph::new(visible_lines);
            frame.render_widget(editor_paragraph, inner_area);

            // Render cursor if in Editor mode
            if matches!(app.focus, Focus::Editor) {
                let screen_y =
                    inner_area.y + (app.buffer.cursor_line.saturating_sub(app.buffer.scroll_top)) as u16;
                let current_line = app
                    .buffer
                    .lines
                    .get(app.buffer.cursor_line)
                    .map(|s| s.as_str())
                    .unwrap_or("");

                let vis_col = app
                    .buffer
                    .visual_column(current_line, app.buffer.cursor_col, 4);
                let visible_vis_col = vis_col.saturating_sub(app.buffer.scroll_left);

                let screen_x = inner_area.x + gutter_width as u16 + visible_vis_col as u16;

                if screen_y < inner_area.y + inner_area.height
                    && screen_x < inner_area.x + inner_area.width
                {
                    frame.set_cursor_position((screen_x, screen_y));
                }
            }
        }
    }
}

fn render_status_bar(frame: &mut Frame, app: &App, area: Rect) {
    let bg_style = Style::default().bg(Color::Rgb(40, 40, 40)).fg(Color::White);
    let block = Block::default().style(bg_style);
    frame.render_widget(block, area);

    let active_filename = app
        .active_path
        .as_ref()
        .and_then(|p| p.file_name())
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "[No File]".to_string());

    let dirty_marker = if app.buffer.is_dirty { " [+]" } else { "" };
    let crlf_marker = if app.buffer.crlf { " CRLF" } else { " LF" };

    let left_status = if let Some(ref msg) = app.status_message {
        format!(" {} ", msg)
    } else {
        format!(" {}{}", active_filename, dirty_marker)
    };

    let right_status = format!(
        "Ln {}, Col {}{}  ",
        app.buffer.cursor_line + 1,
        app.buffer.cursor_col + 1,
        crlf_marker
    );

    let layout = Layout::horizontal([
        Constraint::Min(1),
        Constraint::Length(right_status.len() as u16),
    ])
    .split(area);

    let p_left = Paragraph::new(left_status).style(bg_style.bold());
    let p_right = Paragraph::new(right_status).style(bg_style.fg(Color::DarkGray));

    frame.render_widget(p_left, layout[0]);
    frame.render_widget(p_right, layout[1]);
}

fn render_prompt_modal(frame: &mut Frame, app: &App, area: Rect) {
    let dialog_area = centered_rect(55, 7, area);
    frame.render_widget(Clear, dialog_area);

    if let PromptMode::ConfirmDelete(ref path) = app.prompt_mode {
        return render_delete_modal(frame, path, dialog_area);
    }

    let (title, prompt_text) = match &app.prompt_mode {
        PromptMode::SaveOnTab => (
            " Unsaved Changes ",
            "Unsaved changes detected. Save before leaving? (y/n)",
        ),
        PromptMode::SaveOnQuit => (
            " Confirm Quit ",
            "Unsaved changes detected. Save before quitting? (y/n)",
        ),
        PromptMode::CsvSaveOnEsc => (
            " Unsaved CSV Changes ",
            "CSV has unsaved changes. Save before leaving? (y/n)",
        ),
        _ => return,
    };

    let popup = Paragraph::new(format!(
        "\n{}\n\n[y] Yes    [n] No / Discard    [Esc] Cancel",
        prompt_text
    ))
    .block(
        Block::default()
            .title(title)
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Yellow).bold()),
    )
    .alignment(Alignment::Center);

    frame.render_widget(popup, dialog_area);
}

fn render_delete_modal(frame: &mut Frame, path: &std::path::Path, dialog_area: Rect) {
    let popup = Paragraph::new(format!(
        "\nAre you sure you want to delete:\n{}\n\n[y] Yes, delete    [n/Esc] Cancel",
        path.display()
    ))
    .block(
        Block::default()
            .title(" Confirm Delete ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Red).bold()),
    )
    .alignment(Alignment::Center);

    frame.render_widget(popup, dialog_area);
}

fn render_find_bar(frame: &mut Frame, app: &App, area: Rect) {
    let dialog_area = centered_rect(60, 7, area);
    frame.render_widget(Clear, dialog_area);

    let popup = Paragraph::new(format!(
        "\n Find: {}\n\n [Enter/Down] Next    [Up] Prev    [Esc] Close",
        app.find_input.content
    ))
    .block(
        Block::default()
            .title(" Find Text (Ctrl+F) ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan).bold()),
    );

    frame.render_widget(popup, dialog_area);

    let vis_col = crate::buffer::Buffer::static_visual_column(&app.find_input.content, app.find_input.cursor_col, 4);
    let cursor_x = dialog_area.x + 1 + 7 + vis_col as u16;
    let max_x = dialog_area.x + dialog_area.width.saturating_sub(2);
    let cursor_y = dialog_area.y + 2;

    frame.set_cursor_position((cursor_x.min(max_x), cursor_y));
}

fn render_goto_modal(frame: &mut Frame, app: &App, area: Rect) {
    let dialog_area = centered_rect(45, 7, area);
    frame.render_widget(Clear, dialog_area);

    let popup = Paragraph::new(format!(
        "\n Go to Line: {}\n\n [Enter] Jump    [Esc] Cancel",
        app.goto_input.content
    ))
    .block(
        Block::default()
            .title(" Goto Line (Ctrl+G) ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Green).bold()),
    );

    frame.render_widget(popup, dialog_area);

    let vis_col = crate::buffer::Buffer::static_visual_column(&app.goto_input.content, app.goto_input.cursor_col, 4);
    let cursor_x = dialog_area.x + 1 + 13 + vis_col as u16;
    let max_x = dialog_area.x + dialog_area.width.saturating_sub(2);
    let cursor_y = dialog_area.y + 2;

    frame.set_cursor_position((cursor_x.min(max_x), cursor_y));
}

fn render_new_file_modal(frame: &mut Frame, app: &App, area: Rect) {
    let dialog_area = centered_rect(55, 7, area);
    frame.render_widget(Clear, dialog_area);

    let kind = if app.is_new_dir { "Directory" } else { "File" };
    let popup = Paragraph::new(format!(
        "\n New {} Name: {}\n\n [Enter] Create    [Esc] Cancel",
        kind, app.new_file_input.content
    ))
    .block(
        Block::default()
            .title(format!(" Create New {} ", kind))
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Magenta).bold()),
    );

    frame.render_widget(popup, dialog_area);

    let label_len = format!(" New {} Name: ", kind).chars().count();
    let vis_col = crate::buffer::Buffer::static_visual_column(&app.new_file_input.content, app.new_file_input.cursor_col, 4);
    let cursor_x = dialog_area.x + 1 + label_len as u16 + vis_col as u16;
    let max_x = dialog_area.x + dialog_area.width.saturating_sub(2);
    let cursor_y = dialog_area.y + 2;

    frame.set_cursor_position((cursor_x.min(max_x), cursor_y));
}

fn centered_rect(width: u16, height: u16, r: Rect) -> Rect {
    let popup_width = width.min(r.width.saturating_sub(2)).max(15);
    let popup_height = height.min(r.height.saturating_sub(2)).max(5);

    let x = r.x + (r.width.saturating_sub(popup_width)) / 2;
    let y = r.y + (r.height.saturating_sub(popup_height)) / 2;

    Rect::new(x, y, popup_width, popup_height)
}

fn slice_and_expand_spans(
    spans: Vec<Span<'static>>,
    scroll_left: usize,
    view_width: usize,
    _tab_width: usize,
) -> Vec<Span<'static>> {
    let mut expanded_spans = Vec::new();

    for span in spans {
        let style = span.style;
        let text = span.content;
        let mut expanded_text = String::new();

        for c in text.chars() {
            if c == '\t' {
                expanded_text.push_str("    ");
            } else {
                expanded_text.push(c);
            }
        }
        expanded_spans.push(Span::styled(expanded_text, style));
    }

    // Horizontally slice across expanded spans
    let mut result = Vec::new();
    let mut current_vis_col = 0;
    let end_vis_col = scroll_left + view_width;

    for span in expanded_spans {
        let span_width = UnicodeWidthStr::width(&span.content[..]);
        let span_end = current_vis_col + span_width;

        if span_end <= scroll_left || current_vis_col >= end_vis_col {
            current_vis_col = span_end;
            continue;
        }

        let slice_start = scroll_left.saturating_sub(current_vis_col);
        let slice_end = end_vis_col.saturating_sub(current_vis_col).min(span_width);

        let sliced_text: String = span
            .content
            .chars()
            .skip(slice_start)
            .take(slice_end - slice_start)
            .collect();

        result.push(Span::styled(sliced_text, span.style));
        current_vis_col = span_end;
    }

    result
}

fn truncate_str(s: &str, max_chars: usize) -> String {
    let count = s.chars().count();
    if count <= max_chars {
        s.to_string()
    } else {
        let truncated: String = s.chars().take(max_chars.saturating_sub(3)).collect();
        format!("{}...", truncated)
    }
}
