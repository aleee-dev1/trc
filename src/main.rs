mod app;
mod buffer;
mod csv;
mod highlight;
mod input;
mod ui;

use std::{io, path::PathBuf, time::Duration};

use ratatui::crossterm::event::{self, DisableBracketedPaste, DisableMouseCapture, EnableBracketedPaste, EnableMouseCapture};
use ratatui::crossterm::execute;

use app::App;

fn run(mut terminal: ratatui::DefaultTerminal, target_path: Option<PathBuf>) -> io::Result<()> {
    let mut app = App::new_with_path(target_path)?;
    execute!(io::stdout(), EnableBracketedPaste, EnableMouseCapture)?;

    while !app.should_quit {
        terminal.draw(|frame| {
            let area = frame.area();
            let view_height = area.height.saturating_sub(2) as usize;
            let max_entry_len = app
                .entries
                .iter()
                .map(|e| unicode_width::UnicodeWidthStr::width(e.name.as_str()))
                .max()
                .unwrap_or(10);
            let explorer_width = (max_entry_len + 5).clamp(18, 35);
            let total_lines = app.buffer.lines.len();
            let max_line_digits = format!("{}", total_lines).len().max(3);
            let gutter_width = max_line_digits + 3;

            let view_width = (area.width as usize).saturating_sub(explorer_width + gutter_width);
            app.buffer.ensure_cursor_visible(view_height, view_width);

            ui::draw(frame, &mut app);
        })?;

        if event::poll(Duration::from_millis(50))? {
            while event::poll(Duration::ZERO)? {
                let ev = event::read()?;
                input::handle_event(&mut app, ev)?;
            }
        }
    }

    let _ = execute!(io::stdout(), DisableBracketedPaste, DisableMouseCapture);
    Ok(())
}

fn print_help() {
    println!(
        r#"rfm - Terminal File Explorer & Editor

USAGE:
    rfm [PATH | FILE]
    rfm [FLAGS]

ARGS:
    <PATH>    Open the specified directory in the explorer panel (supports ~)
    <FILE>    Open the parent directory in explorer and load file directly in editor

FLAGS:
    -h, --help, help    Print usage help and keybindings

KEYBINDINGS:
    Global:
        Ctrl+P              Toggle Path bar input
        Ctrl+K              Toggle Search filter bar
        Ctrl+Q              Quit application (prompts if unsaved)

    Explorer Mode:
        Up / Down / j / k   Navigate files & folders
        Enter / Right / l   Open directory / Focus editor on file
        Backspace / Left / h Go to parent directory (reselects previous folder)
        Tab                 Switch focus to Editor (for UTF-8 files)
        r                   Refresh current directory
        a / A               Create new file / new folder
        d                   Delete selected file or folder (with confirmation)
        .                   Toggle hidden files (dotfiles)

    Editor Mode:
        Esc                 Switch focus back to Explorer (prompts if unsaved)
        Ctrl+S              Save active file (atomic save without prompt modal)
        Ctrl+F              Find text in active file (n/Down: next, Up: prev)
        Ctrl+G              Goto line number
        Ctrl+Z / Ctrl+Y     Undo / Redo changes
        Ctrl+Left / Right   Jump left / right by word
        Tab                 Insert 4-space indentation
        Arrow keys          Move cursor
        Home / End          Move to start / end of line
        PageUp / PageDown   Scroll viewport up / down
        Backspace / Delete  Delete characters / merge lines
        Enter               Insert newline with auto-indentation"#
    );
}

fn main() -> io::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    if args.len() > 1 {
        let arg = &args[1];
        if arg == "-h" || arg == "--help" || arg == "help" {
            print_help();
            return Ok(());
        }
    }

    let target_path = if args.len() > 1 {
        Some(PathBuf::from(&args[1]))
    } else {
        None
    };

    let terminal = ratatui::init();
    let result = run(terminal, target_path);
    ratatui::restore();

    result
}