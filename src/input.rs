use std::io;
use ratatui::crossterm::event::{Event, KeyCode, KeyModifiers};

use crate::app::{App, FileStatus, Focus, PromptMode, expand_path};
use crate::csv::{CsvAction, csv_handle_key, csv_handle_mouse};

pub fn handle_event(app: &mut App, event: Event) -> io::Result<()> {
    match event {
        Event::Paste(pasted_text) => {
            if matches!(app.focus, Focus::Editor) && matches!(app.file_status, FileStatus::Utf8Loaded) {
                app.buffer.insert_str(&pasted_text);
            }
        }
        Event::Key(key) if key.kind == ratatui::crossterm::event::KeyEventKind::Press => {
            handle_key(app, key.code, key.modifiers)?;
        }
        Event::Mouse(mouse) => {
            if matches!(app.focus, Focus::CsvEditor) {
                if let Some(ref mut csv) = app.csv_editor {
                    csv_handle_mouse(csv, mouse.kind, mouse.column, mouse.row);
                }
            }
        }
        _ => {}
    }
    Ok(())
}

fn handle_key(app: &mut App, code: KeyCode, modifiers: KeyModifiers) -> io::Result<()> {
    // 1. Handle Prompt Modal input first
    if app.prompt_mode != PromptMode::None {
        match app.prompt_mode.clone() {
            PromptMode::SaveOnTab => match code {
                KeyCode::Char('y') | KeyCode::Char('Y') => {
                    app.save_active_file()?;
                    app.prompt_mode = PromptMode::None;
                    app.focus = Focus::Explorer;
                }
                KeyCode::Char('n') | KeyCode::Char('N') => {
                    app.prompt_mode = PromptMode::None;
                    app.load_selected_file();
                    app.focus = Focus::Explorer;
                }
                KeyCode::Esc => {
                    app.prompt_mode = PromptMode::None;
                }
                _ => {}
            },
            PromptMode::SaveOnQuit => match code {
                KeyCode::Char('y') | KeyCode::Char('Y') => {
                    app.save_active_file()?;
                    app.prompt_mode = PromptMode::None;
                    app.should_quit = true;
                }
                KeyCode::Char('n') | KeyCode::Char('N') => {
                    app.prompt_mode = PromptMode::None;
                    app.should_quit = true;
                }
                KeyCode::Esc => {
                    app.prompt_mode = PromptMode::None;
                }
                _ => {}
            },
            PromptMode::CsvSaveOnEsc => match code {
                KeyCode::Char('y') | KeyCode::Char('Y') => {
                    if let Some(ref mut csv) = app.csv_editor {
                        let _ = csv.save();
                    }
                    app.prompt_mode = PromptMode::None;
                    app.focus = Focus::Explorer;
                }
                KeyCode::Char('n') | KeyCode::Char('N') => {
                    // Discard changes and go back to explorer
                    if let Some(ref mut csv) = app.csv_editor {
                        csv.dirty = false;
                    }
                    app.prompt_mode = PromptMode::None;
                    app.focus = Focus::Explorer;
                }
                KeyCode::Esc => {
                    // Cancel — stay in CSV editor
                    app.prompt_mode = PromptMode::None;
                }
                _ => {}
            },
            PromptMode::ConfirmDelete(path) => match code {
                KeyCode::Char('y') | KeyCode::Char('Y') => {
                    app.prompt_mode = PromptMode::None;
                    app.delete_selected(path)?;
                }
                KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
                    app.prompt_mode = PromptMode::None;
                }
                _ => {}
            },
            PromptMode::None => {}
        }
        return Ok(());
    }

    // 2. Global Control shortcuts
    if modifiers.contains(KeyModifiers::CONTROL) {
        match code {
            KeyCode::Char('p') | KeyCode::Char('P') => {
                if matches!(app.focus, Focus::Path) {
                    app.focus = Focus::Explorer;
                } else {
                    app.focus = Focus::Path;
                    app.path_input.set_content(app.current_dir.to_string_lossy().to_string());
                }
                return Ok(());
            }
            KeyCode::Char('k') | KeyCode::Char('K') => {
                if matches!(app.focus, Focus::Search) {
                    app.focus = Focus::Explorer;
                    if !app.search_input.content.is_empty() {
                        app.search_input.clear();
                        app.show_search = false;
                        app.selected = 0;
                        app.explorer_state.select(Some(0));
                        app.load_selected_file();
                    }
                } else {
                    app.focus = Focus::Search;
                    app.show_search = true;
                }
                return Ok(());
            }
            KeyCode::Char('s') | KeyCode::Char('S') => {
                if matches!(app.focus, Focus::Editor) && matches!(app.file_status, FileStatus::Utf8Loaded) {
                    app.save_active_file()?;
                } else if matches!(app.focus, Focus::CsvEditor) {
                    if let Some(ref mut csv) = app.csv_editor {
                        let _ = csv.save();
                        app.status_message = Some("CSV saved.".to_string());
                    }
                }
                return Ok(());
            }
            KeyCode::Char('q') | KeyCode::Char('Q') => {
                if app.buffer.is_dirty {
                    app.prompt_mode = PromptMode::SaveOnQuit;
                } else {
                    app.should_quit = true;
                }
                return Ok(());
            }
            KeyCode::Char('f') | KeyCode::Char('F') => {
                if matches!(app.focus, Focus::Editor) {
                    app.focus = Focus::Find;
                }
                return Ok(());
            }
            KeyCode::Char('g') | KeyCode::Char('G') => {
                if matches!(app.focus, Focus::Editor) {
                    app.focus = Focus::GotoLine;
                    app.goto_input.clear();
                }
                return Ok(());
            }
            KeyCode::Char('z') | KeyCode::Char('Z') => {
                if matches!(app.focus, Focus::Editor) {
                    app.buffer.undo();
                }
                return Ok(());
            }
            KeyCode::Char('y') | KeyCode::Char('Y') => {
                if matches!(app.focus, Focus::Editor) {
                    app.buffer.redo();
                }
                return Ok(());
            }
            KeyCode::Left => {
                if matches!(app.focus, Focus::Editor) {
                    app.buffer.move_word_left();
                }
                return Ok(());
            }
            KeyCode::Right => {
                if matches!(app.focus, Focus::Editor) {
                    app.buffer.move_word_right();
                }
                return Ok(());
            }
            _ => {
                if !modifiers.contains(KeyModifiers::ALT) {
                    return Ok(());
                }
            }
        }
    }

    // 3. Focus mode handling
    match app.focus {
        Focus::Explorer => match code {
            KeyCode::Tab => {
                if matches!(app.file_status, FileStatus::Utf8Loaded) {
                    app.focus = Focus::Editor;
                } else if matches!(app.file_status, FileStatus::CsvLoaded) {
                    app.focus = Focus::CsvEditor;
                } else {
                    app.status_message = Some("Select a valid UTF-8 file to edit.".to_string());
                }
            }
            KeyCode::Char('q') => {
                if app.buffer.is_dirty {
                    app.prompt_mode = PromptMode::SaveOnQuit;
                } else {
                    app.should_quit = true;
                }
            }
            KeyCode::Down | KeyCode::Char('j') => app.move_down(),
            KeyCode::Up | KeyCode::Char('k') => app.move_up(),
            KeyCode::Enter | KeyCode::Right | KeyCode::Char('l') => app.enter_directory()?,
            KeyCode::Backspace | KeyCode::Left | KeyCode::Char('h') => app.go_up()?,
            KeyCode::Char('r') => app.refresh()?,
            KeyCode::Char('a') => {
                app.focus = Focus::NewFile;
                app.is_new_dir = false;
                app.new_file_input.clear();
            }
            KeyCode::Char('A') => {
                app.focus = Focus::NewFile;
                app.is_new_dir = true;
                app.new_file_input.clear();
            }
            KeyCode::Char('d') => {
                if let Some(entry) = app.selected_entry().cloned() {
                    app.prompt_mode = PromptMode::ConfirmDelete(entry.path);
                }
            }
            KeyCode::Char('.') => {
                app.show_hidden = !app.show_hidden;
                app.refresh()?;
            }
            KeyCode::Esc => {
                if !app.search_input.content.is_empty() {
                    app.search_input.clear();
                    app.show_search = false;
                    app.selected = 0;
                    app.explorer_state.select(Some(0));
                    app.load_selected_file();
                }
            }
            _ => {}
        },

        Focus::Editor => match code {
            KeyCode::Esc => {
                if app.buffer.is_dirty {
                    app.prompt_mode = PromptMode::SaveOnTab;
                } else {
                    app.focus = Focus::Explorer;
                }
            }
            KeyCode::Tab => app.buffer.insert_tab(),
            KeyCode::Up => app.buffer.move_up(),
            KeyCode::Down => app.buffer.move_down(),
            KeyCode::Left => app.buffer.move_left(),
            KeyCode::Right => app.buffer.move_right(),
            KeyCode::Home => app.buffer.move_home(),
            KeyCode::End => app.buffer.move_end(),
            KeyCode::PageUp => app.buffer.move_page_up(15),
            KeyCode::PageDown => app.buffer.move_page_down(15),
            KeyCode::Char(c) => app.buffer.insert_char(c),
            KeyCode::Enter => app.buffer.insert_newline(),
            KeyCode::Backspace => app.buffer.backspace(),
            KeyCode::Delete => app.buffer.delete(),
            _ => {}
        },

        Focus::CsvEditor => {
            if let Some(ref mut csv) = app.csv_editor {
                let action = csv_handle_key(csv, code, modifiers);
                if matches!(action, CsvAction::Quit) {
                    if csv.dirty {
                        app.prompt_mode = PromptMode::CsvSaveOnEsc;
                    } else {
                        app.focus = Focus::Explorer;
                    }
                }
            } else {
                app.focus = Focus::Explorer;
            }
        }

        Focus::Path => match code {
            KeyCode::Enter => {
                let expanded = expand_path(&app.path_input.content);
                if expanded.is_dir() {
                    app.current_dir = expanded;
                    let _ = app.refresh();
                } else if expanded.is_file() {
                    if let Some(parent) = expanded.parent() {
                        app.current_dir = parent.to_path_buf();
                        let _ = app.refresh();
                        if let Some(pos) = app.entries.iter().position(|e| e.path == expanded) {
                            app.selected = pos;
                            app.explorer_state.select(Some(pos));
                            app.load_selected_file();
                        }
                    }
                } else {
                    app.status_message = Some(format!("Invalid path: {}", app.path_input.content));
                }
                app.focus = Focus::Explorer;
            }
            KeyCode::Esc => app.focus = Focus::Explorer,
            KeyCode::Left => app.path_input.move_left(),
            KeyCode::Right => app.path_input.move_right(),
            KeyCode::Home => app.path_input.move_home(),
            KeyCode::End => app.path_input.move_end(),
            KeyCode::Backspace => app.path_input.backspace(),
            KeyCode::Delete => app.path_input.delete(),
            KeyCode::Char(c) => app.path_input.insert_char(c),
            _ => {}
        },

        Focus::Search => match code {
            KeyCode::Esc => {
                app.focus = Focus::Explorer;
                app.search_input.clear();
                app.show_search = false;
                app.selected = 0;
                app.explorer_state.select(Some(0));
                app.load_selected_file();
            }
            KeyCode::Enter => app.focus = Focus::Explorer,
            KeyCode::Down => app.move_down(),
            KeyCode::Up => app.move_up(),
            KeyCode::Left => app.search_input.move_left(),
            KeyCode::Right => app.search_input.move_right(),
            KeyCode::Home => app.search_input.move_home(),
            KeyCode::End => app.search_input.move_end(),
            KeyCode::Backspace => {
                app.search_input.backspace();
                app.selected = 0;
                app.explorer_state.select(Some(0));
                app.load_selected_file();
            }
            KeyCode::Delete => {
                app.search_input.delete();
                app.selected = 0;
                app.explorer_state.select(Some(0));
                app.load_selected_file();
            }
            KeyCode::Char(c) => {
                app.search_input.insert_char(c);
                app.selected = 0;
                app.explorer_state.select(Some(0));
                app.load_selected_file();
            }
            _ => {}
        },

        Focus::Find => match code {
            KeyCode::Esc => app.focus = Focus::Editor,
            KeyCode::Enter => {
                app.buffer.find_next(&app.find_input.content);
            }
            KeyCode::Left => app.find_input.move_left(),
            KeyCode::Right => app.find_input.move_right(),
            KeyCode::Home => app.find_input.move_home(),
            KeyCode::End => app.find_input.move_end(),
            KeyCode::Char(c) => {
                app.find_input.insert_char(c);
                app.buffer.find_next(&app.find_input.content);
            }
            KeyCode::Backspace => {
                app.find_input.backspace();
                app.buffer.find_next(&app.find_input.content);
            }
            KeyCode::Delete => {
                app.find_input.delete();
                app.buffer.find_next(&app.find_input.content);
            }
            KeyCode::Down => {
                app.buffer.find_next(&app.find_input.content);
            }
            KeyCode::Up => {
                app.buffer.find_prev(&app.find_input.content);
            }
            _ => {}
        },

        Focus::GotoLine => match code {
            KeyCode::Esc => app.focus = Focus::Editor,
            KeyCode::Enter => {
                if let Ok(line_num) = app.goto_input.content.parse::<usize>() {
                    app.buffer.goto_line(line_num);
                }
                app.focus = Focus::Editor;
            }
            KeyCode::Left => app.goto_input.move_left(),
            KeyCode::Right => app.goto_input.move_right(),
            KeyCode::Home => app.goto_input.move_home(),
            KeyCode::End => app.goto_input.move_end(),
            KeyCode::Char(c) if c.is_ascii_digit() => {
                app.goto_input.insert_char(c);
            }
            KeyCode::Backspace => {
                app.goto_input.backspace();
            }
            KeyCode::Delete => {
                app.goto_input.delete();
            }
            _ => {}
        },

        Focus::NewFile => match code {
            KeyCode::Esc => app.focus = Focus::Explorer,
            KeyCode::Enter => {
                app.create_file_or_dir()?;
                app.focus = Focus::Explorer;
            }
            KeyCode::Left => app.new_file_input.move_left(),
            KeyCode::Right => app.new_file_input.move_right(),
            KeyCode::Home => app.new_file_input.move_home(),
            KeyCode::End => app.new_file_input.move_end(),
            KeyCode::Backspace => app.new_file_input.backspace(),
            KeyCode::Delete => app.new_file_input.delete(),
            KeyCode::Char(c) => app.new_file_input.insert_char(c),
            _ => {}
        },
    }

    Ok(())
}
