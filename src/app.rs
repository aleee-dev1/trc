use std::{
    cmp::Ordering,
    fs,
    io,
    path::{Path, PathBuf},
    time::SystemTime,
};

use ratatui::widgets::ListState;

use crate::buffer::Buffer;
use crate::csv::CsvEditor;

#[derive(Clone, Debug)]
pub struct FileEntry {
    pub path: PathBuf,
    pub name: String,
    pub is_dir: bool,
    pub is_symlink: bool,
    pub _size: u64,
}

#[derive(PartialEq, Eq, Debug, Clone, Copy)]
pub enum Focus {
    Explorer,
    Editor,
    CsvEditor,
    Path,
    Search,
    Find,
    GotoLine,
    NewFile,
}

#[derive(PartialEq, Eq, Debug, Clone)]
pub enum FileStatus {
    NoneSelected,
    IsDirectory,
    NotRegular,
    TooLarge,
    NotUtf8,
    CsvLoaded,
    Unreadable(String),
    Utf8Loaded,
}

#[derive(PartialEq, Eq, Debug, Clone)]
pub enum PromptMode {
    None,
    SaveOnTab,
    SaveOnQuit,
    CsvSaveOnEsc,
    ConfirmDelete(PathBuf),
}
use unicode_segmentation::UnicodeSegmentation;

#[derive(Default, Clone, Debug, PartialEq, Eq)]
pub struct TextInput {
    pub content: String,
    pub cursor_col: usize,
}

impl TextInput {
    pub fn new() -> Self {
        Self {
            content: String::new(),
            cursor_col: 0,
        }
    }

    pub fn set_content(&mut self, content: String) {
        let count = content.graphemes(true).count();
        self.content = content;
        self.cursor_col = count;
    }

    pub fn clear(&mut self) {
        self.content.clear();
        self.cursor_col = 0;
    }

    pub fn insert_char(&mut self, c: char) {
        let byte_idx = crate::buffer::Buffer::byte_idx_for_grapheme(&self.content, self.cursor_col);
        self.content.insert(byte_idx, c);
        self.cursor_col += 1;
    }

    pub fn backspace(&mut self) {
        if self.cursor_col > 0 {
            let prev_byte_idx = crate::buffer::Buffer::byte_idx_for_grapheme(&self.content, self.cursor_col - 1);
            let cur_byte_idx = crate::buffer::Buffer::byte_idx_for_grapheme(&self.content, self.cursor_col);
            self.content.drain(prev_byte_idx..cur_byte_idx);
            self.cursor_col -= 1;
        }
    }

    pub fn delete(&mut self) {
        let count = self.content.graphemes(true).count();
        if self.cursor_col < count {
            let cur_byte_idx = crate::buffer::Buffer::byte_idx_for_grapheme(&self.content, self.cursor_col);
            let next_byte_idx = crate::buffer::Buffer::byte_idx_for_grapheme(&self.content, self.cursor_col + 1);
            self.content.drain(cur_byte_idx..next_byte_idx);
        }
    }

    pub fn move_left(&mut self) {
        if self.cursor_col > 0 {
            self.cursor_col -= 1;
        }
    }

    pub fn move_right(&mut self) {
        let count = self.content.graphemes(true).count();
        if self.cursor_col < count {
            self.cursor_col += 1;
        }
    }

    pub fn move_home(&mut self) {
        self.cursor_col = 0;
    }

    pub fn move_end(&mut self) {
        self.cursor_col = self.content.graphemes(true).count();
    }
}

pub struct App {
    pub current_dir: PathBuf,
    pub entries: Vec<FileEntry>,
    pub selected: usize,
    pub explorer_state: ListState,
    pub show_hidden: bool,

    pub focus: Focus,
    pub path_input: TextInput,

    pub search_input: TextInput,
    pub show_search: bool,

    pub find_input: TextInput,
    pub goto_input: TextInput,
    pub new_file_input: TextInput,
    pub is_new_dir: bool,

    pub file_status: FileStatus,
    pub active_path: Option<PathBuf>,
    pub active_mtime: Option<SystemTime>,
    pub buffer: Buffer,

    pub csv_editor: Option<CsvEditor>,

    pub prompt_mode: PromptMode,
    pub status_message: Option<String>,
    pub should_quit: bool,
}

impl App {
    pub fn new_with_path(target: Option<PathBuf>) -> io::Result<Self> {
        let (initial_dir, target_file) = match target {
            Some(path) => {
                let expanded = expand_path(&path.to_string_lossy());
                let canonical = expanded.canonicalize().unwrap_or_else(|_| expanded.clone());
                if canonical.is_file() {
                    let parent = canonical
                        .parent()
                        .map(|p| p.to_path_buf())
                        .unwrap_or_else(|| std::env::current_dir().unwrap_or_default());
                    (parent, Some(canonical))
                } else if canonical.is_dir() {
                    (canonical, None)
                } else {
                    (std::env::current_dir()?, None)
                }
            }
            None => (std::env::current_dir()?, None),
        };

        let mut app = Self {
            current_dir: initial_dir,
            entries: Vec::new(),
            selected: 0,
            explorer_state: ListState::default(),
            show_hidden: false,

            focus: Focus::Explorer,
            path_input: TextInput::new(),

            search_input: TextInput::new(),
            show_search: false,

            find_input: TextInput::new(),
            goto_input: TextInput::new(),
            new_file_input: TextInput::new(),
            is_new_dir: false,

            file_status: FileStatus::NoneSelected,
            active_path: None,
            active_mtime: None,
            buffer: Buffer::new(),

            csv_editor: None,

            prompt_mode: PromptMode::None,
            status_message: None,
            should_quit: false,
        };

        app.refresh()?;

        if let Some(file_path) = target_file {
            let filtered = app.filtered_entries();
            if let Some(pos) = filtered.iter().position(|(_, e)| e.path == file_path) {
                app.selected = pos;
                app.explorer_state.select(Some(pos));
                app.load_selected_file();
                if matches!(app.file_status, FileStatus::Utf8Loaded) {
                    app.focus = Focus::Editor;
                }
            }
        }

        Ok(app)
    }

    pub fn refresh(&mut self) -> io::Result<()> {
        let mut entries = Vec::new();
        let mut error_msg = None;

        match fs::read_dir(&self.current_dir) {
            Ok(read_dir) => {
                for entry in read_dir {
                    let entry = match entry {
                        Ok(e) => e,
                        Err(_) => continue,
                    };

                    let path = entry.path();
                    let name = entry.file_name().to_string_lossy().to_string();

                    if !self.show_hidden && name.starts_with('.') {
                        continue;
                    }

                    let symlink_meta = fs::symlink_metadata(&path);
                    let is_symlink = symlink_meta.as_ref().map(|m| m.file_type().is_symlink()).unwrap_or(false);

                    let (is_dir, size) = match fs::metadata(&path) {
                        Ok(m) => (m.is_dir(), m.len()),
                        Err(_) => (false, 0),
                    };

                    entries.push(FileEntry {
                        path,
                        name,
                        is_dir,
                        is_symlink,
                        _size: size,
                    });
                }
            }
            Err(e) => {
                error_msg = Some(format!("Error reading directory: {e}"));
            }
        }

        entries.sort_by(|a, b| match (a.is_dir, b.is_dir) {
            (true, false) => Ordering::Less,
            (false, true) => Ordering::Greater,
            _ => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
        });

        self.entries = entries;
        if self.selected >= self.entries.len() {
            self.selected = self.entries.len().saturating_sub(1);
        }
        self.explorer_state.select(if self.entries.is_empty() { None } else { Some(self.selected) });

        if let Some(msg) = error_msg {
            self.status_message = Some(msg);
        }

        self.load_selected_file();
        Ok(())
    }

    pub fn filtered_entries(&self) -> Vec<(usize, &FileEntry)> {
        if self.search_input.content.is_empty() {
            self.entries.iter().enumerate().collect()
        } else {
            let query = self.search_input.content.to_lowercase();
            self.entries
                .iter()
                .enumerate()
                .filter(|(_, entry)| entry.name.to_lowercase().contains(&query))
                .collect()
        }
    }

    pub fn selected_entry(&self) -> Option<&FileEntry> {
        let filtered = self.filtered_entries();
        filtered.get(self.selected).map(|(_, entry)| *entry)
    }

    pub fn move_down(&mut self) {
        let filtered = self.filtered_entries();
        if filtered.is_empty() {
            return;
        }

        self.selected = (self.selected + 1) % filtered.len();
        self.explorer_state.select(Some(self.selected));
        self.load_selected_file();
    }

    pub fn move_up(&mut self) {
        let filtered = self.filtered_entries();
        if filtered.is_empty() {
            return;
        }

        if self.selected == 0 {
            self.selected = filtered.len() - 1;
        } else {
            self.selected -= 1;
        }

        self.explorer_state.select(Some(self.selected));
        self.load_selected_file();
    }

    pub fn enter_directory(&mut self) -> io::Result<()> {
        let selected = self.selected_entry().cloned();
        if let Some(entry) = selected {
            if entry.is_dir {
                self.current_dir = entry.path;
                self.selected = 0;
                self.explorer_state.select(Some(0));
                self.search_input.clear();
                self.show_search = false;
                self.refresh()?;
            } else {
                // Pressing Enter/l on a file switches focus to Editor
                self.load_selected_file();
                if matches!(self.file_status, FileStatus::Utf8Loaded) {
                    self.focus = Focus::Editor;
                } else if matches!(self.file_status, FileStatus::CsvLoaded) {
                    self.focus = Focus::CsvEditor;
                }
            }
        }
        Ok(())
    }

    pub fn go_up(&mut self) -> io::Result<()> {
        if let Some(parent) = self.current_dir.parent() {
            let previous_dir = self.current_dir.clone();
            self.current_dir = parent.to_path_buf();
            self.search_input.clear();
            self.show_search = false;
            self.refresh()?;

            // Reselect the directory we just came up from
            if let Some(pos) = self.entries.iter().position(|e| e.path == previous_dir) {
                self.selected = pos;
                self.explorer_state.select(Some(pos));
            } else {
                self.selected = 0;
                self.explorer_state.select(Some(0));
            }
            self.load_selected_file();
        }
        Ok(())
    }

    pub fn load_selected_file(&mut self) {
        self.status_message = None;

        let entry = match self.selected_entry() {
            Some(entry) => entry.clone(),
            None => {
                self.file_status = FileStatus::NoneSelected;
                self.active_path = None;
                self.buffer = Buffer::new();
                return;
            }
        };

        if entry.is_dir {
            self.file_status = FileStatus::IsDirectory;
            self.active_path = None;
            self.buffer = Buffer::new();
            return;
        }

        let path = entry.path;
        let metadata = match fs::metadata(&path) {
            Ok(m) => m,
            Err(e) => {
                self.file_status = FileStatus::Unreadable(e.to_string());
                self.active_path = Some(path);
                self.buffer = Buffer::new();
                return;
            }
        };

        if !metadata.file_type().is_file() {
            self.file_status = FileStatus::NotRegular;
            self.active_path = Some(path);
            self.buffer = Buffer::new();
            return;
        }

        const MAX_FILE_SIZE: u64 = 2 * 1024 * 1024; // 2 MiB
        if metadata.len() > MAX_FILE_SIZE {
            self.file_status = FileStatus::TooLarge;
            self.active_path = Some(path);
            self.buffer = Buffer::new();
            self.csv_editor = None;
            return;
        }

        // Detect CSV files by extension
        let is_csv = path
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.eq_ignore_ascii_case("csv"))
            .unwrap_or(false);

        if is_csv {
            match CsvEditor::load(&path.to_string_lossy()) {
                Ok(editor) => {
                    self.file_status = FileStatus::CsvLoaded;
                    self.active_path = Some(path);
                    self.csv_editor = Some(editor);
                    self.buffer = Buffer::new();
                }
                Err(e) => {
                    self.file_status = FileStatus::Unreadable(e.to_string());
                    self.active_path = Some(path);
                    self.csv_editor = None;
                    self.buffer = Buffer::new();
                }
            }
            return;
        }

        match fs::read(&path) {
            Ok(bytes) => match String::from_utf8(bytes) {
                Ok(content) => {
                    self.file_status = FileStatus::Utf8Loaded;
                    self.active_path = Some(path);
                    self.active_mtime = metadata.modified().ok();
                    self.buffer = Buffer::from_string(&content);
                    self.csv_editor = None;
                }
                Err(_) => {
                    self.file_status = FileStatus::NotUtf8;
                    self.active_path = Some(path);
                    self.buffer = Buffer::new();
                }
            },
            Err(e) => {
                self.file_status = FileStatus::Unreadable(e.to_string());
                self.active_path = Some(path);
                self.buffer = Buffer::new();
            }
        }
    }

    pub fn save_active_file(&mut self) -> io::Result<()> {
        if let Some(ref path) = self.active_path {
            let canonical_path = path.canonicalize().unwrap_or_else(|_| path.clone());
            let parent_dir = canonical_path
                .parent()
                .unwrap_or_else(|| Path::new("."));

            let content = self.buffer.to_string_content();
            let temp_filename = format!(".rfm_tmp_{}", std::process::id());
            let temp_path = parent_dir.join(temp_filename);

            let write_res = (|| -> io::Result<()> {
                fs::write(&temp_path, content.as_bytes())?;
                let file = fs::File::open(&temp_path)?;
                file.sync_all()?;
                if let Ok(meta) = fs::metadata(&canonical_path) {
                    let _ = fs::set_permissions(&temp_path, meta.permissions());
                }
                fs::rename(&temp_path, &canonical_path)?;
                Ok(())
            })();

            match write_res {
                Ok(_) => {
                    self.buffer.is_dirty = false;
                    self.active_mtime = fs::metadata(&canonical_path).and_then(|m| m.modified()).ok();
                    self.status_message = Some("File saved successfully.".to_string());
                }
                Err(e) => {
                    let _ = fs::remove_file(&temp_path);
                    self.status_message = Some(format!("Save failed: {e}"));
                }
            }
        }
        Ok(())
    }

    pub fn create_file_or_dir(&mut self) -> io::Result<()> {
        if self.new_file_input.content.is_empty() {
            return Ok(());
        }

        let target_path = self.current_dir.join(&self.new_file_input.content);
        if self.is_new_dir {
            fs::create_dir_all(&target_path)?;
            self.status_message = Some(format!("Directory created: {}", self.new_file_input.content));
        } else {
            if let Some(parent) = target_path.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::write(&target_path, "")?;
            self.status_message = Some(format!("File created: {}", self.new_file_input.content));
        }

        self.new_file_input.clear();
        self.refresh()?;

        if let Some(pos) = self.entries.iter().position(|e| e.path == target_path) {
            self.selected = pos;
            self.explorer_state.select(Some(pos));
            self.load_selected_file();
        }

        Ok(())
    }

    pub fn delete_selected(&mut self, path: PathBuf) -> io::Result<()> {
        if path.is_dir() {
            fs::remove_dir_all(&path)?;
        } else if path.is_file() || path.is_symlink() {
            fs::remove_file(&path)?;
        }

        self.status_message = Some(format!("Deleted {}", path.display()));
        self.refresh()?;
        Ok(())
    }
}

pub fn expand_path(input: &str) -> PathBuf {
    if input.starts_with("~/") || input == "~" {
        if let Some(home) = std::env::var_os("HOME").map(PathBuf::from) {
            if input == "~" {
                return home;
            }
            return home.join(&input[2..]);
        }
    }
    PathBuf::from(input)
}
