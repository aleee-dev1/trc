# trc

Terminal file explorer and editor. Opens text files in a built-in editor and CSV files in a dedicated spreadsheet-style editor.

## Usage

```
trc                  # Open current directory
trc <path>           # Open a directory or file directly in editor
trc -h               # Show help
```

## Features

- File explorer with search filtering and hidden file toggle
- Text editor with syntax highlighting, undo/redo, find, and goto-line
- CSV editor with cell editing, column paging, and mouse support
- Atomic file saves

## Keybindings

### Global

| Key | Action |
|-----|--------|
| `Ctrl+P` | Toggle path bar |
| `Ctrl+K` | Toggle search filter |
| `Ctrl+Q` | Quit (prompts if unsaved) |

### Explorer

| Key | Action |
|-----|--------|
| `↑ / ↓` / `j / k` | Navigate |
| `Enter / → / l` | Open folder / focus editor |
| `Backspace / ← / h` | Go to parent directory |
| `Tab` | Switch focus to editor panel |
| `r` | Refresh directory |
| `a` / `A` | New file / new folder |
| `d` | Delete selected (with confirmation) |
| `.` | Toggle hidden files |

### Text Editor

| Key | Action |
|-----|--------|
| `Esc` | Back to explorer (prompts if unsaved) |
| `Ctrl+S` | Save file |
| `Ctrl+F` | Find — `Enter/↓` next, `↑` prev |
| `Ctrl+G` | Goto line number |
| `Ctrl+Z / Ctrl+Y` | Undo / redo |
| `Ctrl+← / →` | Jump by word |
| `Tab` | Insert 4-space indent |
| `Home / End` | Start / end of line |
| `PageUp / PageDown` | Scroll viewport |

### CSV Editor

| Key | Action |
|-----|--------|
| `Esc / q` | Back to explorer (prompts if unsaved) |
| `Ctrl+S` | Save CSV |
| `Arrow keys` | Move cell selection |
| `Enter` / double-click | Edit selected cell |
| `Esc` (in edit mode) | Cancel edit |
| `Enter` (in edit mode) | Confirm edit |
| `Alt+← / →` | Previous / next column page |
| `Ctrl+D` | Delete focused cell |
