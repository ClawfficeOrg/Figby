use crossterm::event::{KeyCode, MouseButton, MouseEventKind};
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph};
use ratatui::Frame;
use std::path::{Path, PathBuf};

use super::theme::Theme;
use crate::font::FIGfont;

#[derive(Debug, Clone)]
pub enum OpenTarget {
    File(PathBuf),
    ZipEntry {
        zip_path: PathBuf,
        entry_name: String,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileOpsMode {
    Idle,
    SaveAs,
    Open,
    ImportFont,
    ImportGif,
    OpenImage,
}

pub struct RecentFiles {
    files: Vec<PathBuf>,
    max: usize,
}

impl RecentFiles {
    const MAX: usize = 10;

    pub fn new() -> Self {
        Self {
            files: Vec::new(),
            max: Self::MAX,
        }
    }

    pub fn set_max(&mut self, max: usize) {
        self.max = max.max(1);
        self.files.truncate(self.max);
    }

    pub fn push(&mut self, path: PathBuf) {
        if let Some(pos) = self.files.iter().position(|p| p == &path) {
            self.files.remove(pos);
        }
        self.files.insert(0, path);
        self.files.truncate(self.max);
    }

    pub fn list(&self) -> &[PathBuf] {
        &self.files
    }

    pub fn len(&self) -> usize {
        self.files.len()
    }

    pub fn is_empty(&self) -> bool {
        self.files.is_empty()
    }

    pub fn get(&self, idx: usize) -> Option<&PathBuf> {
        self.files.get(idx)
    }

    pub fn remove_missing(&mut self) {
        self.files.retain(|p| p.exists());
    }

    pub fn remove(&mut self, idx: usize) {
        if idx < self.files.len() {
            self.files.remove(idx);
        }
    }

    pub fn load_from_disk() -> Self {
        let path = Self::storage_path();
        let path = match path {
            Some(p) => p,
            None => return Self::new(),
        };
        let content = match crate::bounded_io::read_bounded_string(
            &path,
            crate::bounded_io::MAX_TEXT_BYTES,
        ) {
            Ok(c) => c,
            Err(_) => return Self::new(),
        };
        let files: Vec<PathBuf> = content
            .lines()
            .filter(|l| !l.is_empty())
            .map(PathBuf::from)
            .collect();
        Self {
            files,
            max: Self::MAX,
        }
    }

    pub fn save_to_disk(&self) {
        let path = match Self::storage_path() {
            Some(p) => p,
            None => return,
        };
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let content = self
            .files
            .iter()
            .map(|p| p.to_string_lossy().to_string())
            .collect::<Vec<_>>()
            .join("\n");
        let _ = crate::atomic_io::atomic_write(&path, content.as_bytes());
    }

    fn storage_path() -> Option<PathBuf> {
        crate::config::config_dir().map(|d| d.join("recent_files.json"))
    }
}

impl Default for RecentFiles {
    fn default() -> Self {
        Self::new()
    }
}

/// Which field of the Save dialog Tab/click currently focuses. The
/// directory listing (Up/Down/Tab/Right/Left) always navigates
/// `path_buffer`'s directory regardless of focus; this only affects which
/// field typed characters/Backspace go to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SaveFocus {
    Directory,
    Filename,
}

pub struct FileOpsDialog {
    pub mode: FileOpsMode,
    pub path_buffer: String,
    /// Filename-only field for `SaveAs` (separate from `path_buffer`, which
    /// is the directory being browsed for that mode). Unused by every
    /// other mode.
    pub filename_buffer: String,
    pub save_focus: SaveFocus,
    pub directory_entries: Vec<String>,
    pub selected_entry: usize,
    pub error_message: String,
    pub hide_dotfiles: bool,
    pub browsing_zip: bool,
    pub current_zip_path: PathBuf,
    pub theme: Theme,
    recent_files_for_display: Vec<String>,
    /// Screen-space rect for each currently-rendered directory-entry row,
    /// paired with its index into `directory_entries`. Repopulated on every
    /// render call; used for mouse click/scroll hit-testing.
    entry_rects: Vec<(usize, Rect)>,
}

impl FileOpsDialog {
    pub fn new() -> Self {
        Self {
            mode: FileOpsMode::Idle,
            path_buffer: String::new(),
            filename_buffer: String::new(),
            save_focus: SaveFocus::Filename,
            directory_entries: Vec::new(),
            selected_entry: 0,
            error_message: String::new(),
            hide_dotfiles: true,
            browsing_zip: false,
            current_zip_path: PathBuf::new(),
            theme: Theme::default(),
            recent_files_for_display: Vec::new(),
            entry_rects: Vec::new(),
        }
    }

    pub fn enter_open(&mut self, recent: &[PathBuf]) {
        self.mode = FileOpsMode::Open;
        self.path_buffer.clear();
        self.selected_entry = 0;
        self.error_message.clear();
        self.recent_files_for_display = recent
            .iter()
            .map(|p| p.to_string_lossy().to_string())
            .collect();
        self.refresh_directory();
    }

    pub fn enter_import_font(&mut self) {
        self.mode = FileOpsMode::ImportFont;
        self.path_buffer.clear();
        self.selected_entry = 0;
        self.error_message.clear();
        self.recent_files_for_display.clear();
        self.browsing_zip = false;
        self.current_zip_path.clear();
        self.refresh_directory();
    }

    pub fn enter_import_gif(&mut self) {
        self.mode = FileOpsMode::ImportGif;
        self.path_buffer.clear();
        self.selected_entry = 0;
        self.error_message.clear();
        self.recent_files_for_display.clear();
        self.browsing_zip = false;
        self.current_zip_path.clear();
        self.refresh_directory();
    }

    pub fn enter_open_image(&mut self) {
        self.mode = FileOpsMode::OpenImage;
        self.path_buffer.clear();
        self.selected_entry = 0;
        self.error_message.clear();
        self.recent_files_for_display.clear();
        self.browsing_zip = false;
        self.current_zip_path.clear();
        self.refresh_directory();
    }

    /// Enter the Save dialog. `default_name` (without extension) seeds the
    /// filename field when `current` has no path of its own yet (e.g. a
    /// newly created, never-saved font) — typically the font's in-memory
    /// name, falling back to "untitled".
    pub fn enter_save_as(&mut self, current: Option<&Path>, default_name: &str) {
        self.mode = FileOpsMode::SaveAs;
        match current {
            Some(p) => {
                self.path_buffer = p
                    .parent()
                    .map(|d| d.to_string_lossy().to_string())
                    .unwrap_or_default();
                self.filename_buffer = p
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_else(|| format!("{default_name}.flf"));
            }
            None => {
                self.path_buffer.clear();
                self.filename_buffer = format!("{default_name}.flf");
            }
        }
        self.save_focus = SaveFocus::Filename;
        self.selected_entry = 0;
        self.error_message.clear();
        self.refresh_directory();
    }

    pub fn close(&mut self) {
        self.mode = FileOpsMode::Idle;
        self.path_buffer.clear();
        self.filename_buffer.clear();
        self.directory_entries.clear();
        self.selected_entry = 0;
        self.error_message.clear();
        self.recent_files_for_display.clear();
        self.browsing_zip = false;
        self.current_zip_path.clear();
    }

    pub fn handle_paste(&mut self, text: &str) {
        if self.mode == FileOpsMode::Idle {
            return;
        }
        self.path_buffer.push_str(text);
        self.error_message.clear();
        self.selected_entry = 0;
        self.refresh_directory();
    }

    fn refresh_directory(&mut self) {
        self.directory_entries.clear();
        self.selected_entry = 0;

        if self.browsing_zip {
            match crate::font::list_zip_font_entries(&self.current_zip_path) {
                Ok(mut entries) => {
                    if entries.is_empty() {
                        self.error_message = "No .flf/.tlf fonts found in ZIP".to_string();
                    }
                    entries.insert(0, "..".to_string());
                    self.directory_entries = entries;
                }
                Err(e) => {
                    self.error_message = format!("ZIP error: {e}");
                }
            }
            return;
        }

        let parent = self.current_parent_dir();

        let read_dir = match std::fs::read_dir(&parent) {
            Ok(rd) => rd,
            Err(_) => {
                self.error_message = format!("Cannot read directory: {}", parent.display());
                return;
            }
        };

        let mut entries: Vec<String> = Vec::new();

        for entry in read_dir.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if self.hide_dotfiles && name.starts_with('.') {
                continue;
            }
            let lower = name.to_lowercase();
            let is_dir = entry.file_type().map(|t| t.is_dir()).unwrap_or(false);
            if self.allowed(&lower, is_dir) {
                entries.push(name);
            }
        }

        entries.sort();
        // Offer ".." to go up a directory whenever we're not already at the
        // filesystem root — previously this only existed while browsing a
        // zip archive, forcing users to delete typed characters to navigate
        // upward otherwise.
        if parent.parent().is_some() {
            entries.insert(0, "..".to_string());
        }
        self.directory_entries = entries;
    }

    /// Directory currently being browsed, derived from `path_buffer`
    /// (which may hold a bare directory or a file path within it).
    fn current_parent_dir(&self) -> PathBuf {
        if self.path_buffer.is_empty() {
            PathBuf::from(".")
        } else {
            let p = PathBuf::from(&self.path_buffer);
            if p.is_dir() {
                p
            } else {
                p.parent()
                    .map(|pp| pp.to_path_buf())
                    .unwrap_or_else(|| PathBuf::from("."))
            }
        }
    }

    /// Whether a filesystem entry (lowercased name) should be listed at all
    /// for the dialog's current mode — directories and zip archives are
    /// always navigable containers; the exact-match file types vary by mode.
    fn allowed(&self, lower: &str, is_dir: bool) -> bool {
        if is_dir {
            return true;
        }
        match self.mode {
            FileOpsMode::ImportGif => lower.ends_with(".gif"),
            FileOpsMode::OpenImage => Self::is_image_extension(lower),
            FileOpsMode::ImportFont => {
                lower.ends_with(".ttf") || lower.ends_with(".otf") || lower.ends_with(".zip")
            }
            FileOpsMode::Open | FileOpsMode::SaveAs => {
                lower.ends_with(".flf") || lower.ends_with(".tlf") || lower.ends_with(".zip")
            }
            FileOpsMode::Idle => false,
        }
    }

    /// Whether `lower` (a lowercased entry name) is a final, selectable file
    /// target for the dialog's current mode — i.e. what Enter/click should
    /// treat as "done", as opposed to a directory/zip to navigate into.
    fn mode_matches_extension(&self, lower: &str) -> bool {
        match self.mode {
            FileOpsMode::Open | FileOpsMode::SaveAs => {
                lower.ends_with(".flf") || lower.ends_with(".tlf")
            }
            FileOpsMode::ImportFont => lower.ends_with(".ttf") || lower.ends_with(".otf"),
            FileOpsMode::ImportGif => lower.ends_with(".gif"),
            FileOpsMode::OpenImage => Self::is_image_extension(lower),
            FileOpsMode::Idle => false,
        }
    }

    fn mode_extension_error(&self) -> &'static str {
        match self.mode {
            FileOpsMode::ImportFont => "Select a .ttf or .otf file",
            FileOpsMode::ImportGif => "Select a .gif file",
            FileOpsMode::OpenImage => "Select a .png/.jpg/.bmp/.webp/.gif file",
            FileOpsMode::Open | FileOpsMode::SaveAs | FileOpsMode::Idle => "",
        }
    }

    /// Whether a listed entry is a valid target for Enter/click to act on:
    /// ".." and navigable containers (directories, zip archives, or any
    /// entry while already browsing inside a zip) are always selectable;
    /// a plain file is only selectable if it matches the mode's file type.
    fn entry_is_selectable(&self, entry: &str) -> bool {
        if entry == ".." {
            return true;
        }
        if self.browsing_zip {
            return true;
        }
        let lower = entry.to_lowercase();
        if lower.ends_with(".zip") {
            return true;
        }
        if self.current_parent_dir().join(entry).is_dir() {
            return true;
        }
        self.mode_matches_extension(&lower)
    }

    pub fn is_browsing_zip(&self) -> bool {
        self.browsing_zip
    }

    pub fn resolve_open_target(&self) -> OpenTarget {
        if self.browsing_zip {
            let entry_name = self.directory_entries[self.selected_entry].clone();
            OpenTarget::ZipEntry {
                zip_path: self.current_zip_path.clone(),
                entry_name,
            }
        } else {
            OpenTarget::File(self.selected_path())
        }
    }

    pub fn selected_path(&self) -> PathBuf {
        if self.mode == FileOpsMode::SaveAs {
            return self.save_target_path();
        }
        let trimmed = self.path_buffer.trim().to_string();
        if trimmed.is_empty() {
            return PathBuf::from("untitled.flf");
        }
        let p = PathBuf::from(&trimmed);
        if p.extension().is_none() {
            let mut with_ext = p;
            with_ext.set_extension("flf");
            with_ext
        } else {
            p
        }
    }

    /// Combine the Save dialog's directory (`path_buffer`) and filename
    /// (`filename_buffer`) fields into the final save path, auto-appending
    /// `.flf` when the typed filename has no extension. Unlike
    /// `current_parent_dir()`, this trusts `path_buffer` directly as the
    /// directory rather than checking it exists on disk — in `SaveAs` mode
    /// `path_buffer` is only ever written by `enter_save_as`/`select_entry`/
    /// `go_to_parent`, all of which already guarantee it's a directory
    /// (possibly one that doesn't exist yet, which is fine for a save
    /// target).
    fn save_target_path(&self) -> PathBuf {
        let dir = if self.path_buffer.is_empty() {
            PathBuf::from(".")
        } else {
            PathBuf::from(&self.path_buffer)
        };
        let name = self.filename_buffer.trim();
        let name = if name.is_empty() { "untitled" } else { name };
        let mut p = dir.join(name);
        if p.extension().is_none() {
            p.set_extension("flf");
        }
        p
    }

    fn select_entry(&mut self) {
        let entry = self.directory_entries[self.selected_entry].clone();

        if self.browsing_zip {
            if entry == ".." {
                self.go_to_parent();
            }
            return;
        }

        if entry == ".." {
            self.go_to_parent();
            return;
        }

        if entry.to_lowercase().ends_with(".zip") {
            let parent = self.current_parent_dir();
            self.current_zip_path = parent.join(&entry);
            self.browsing_zip = true;
            self.selected_entry = 0;
            self.error_message.clear();
            self.refresh_directory();
            return;
        }

        let parent = self.current_parent_dir();
        if self.mode == FileOpsMode::SaveAs && !parent.join(&entry).is_dir() {
            // Selecting an existing file in the Save dialog populates the
            // filename field (so the user can overwrite it) rather than
            // folding it into path_buffer, which must stay a pure
            // directory for the split path/filename fields.
            self.filename_buffer = entry;
            self.error_message.clear();
            return;
        }
        let abs = parent.join(&entry);
        self.path_buffer = abs.to_string_lossy().to_string();
        self.selected_entry = 0;
        self.error_message.clear();
        self.refresh_directory();
    }

    /// Navigate up one level: out of a zip archive back to its containing
    /// directory, or up to the parent filesystem directory. Bound to the
    /// Left arrow key and to selecting a ".." entry.
    fn go_to_parent(&mut self) {
        if self.browsing_zip {
            self.browsing_zip = false;
            self.path_buffer = self
                .current_zip_path
                .parent()
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_default();
            self.current_zip_path.clear();
            self.selected_entry = 0;
            self.error_message.clear();
            self.refresh_directory();
            return;
        }
        if let Some(parent) = self.current_parent_dir().parent() {
            self.path_buffer = parent.to_string_lossy().to_string();
            self.selected_entry = 0;
            self.error_message.clear();
            self.refresh_directory();
        }
    }

    /// Activate a listed entry the way Enter/click do: navigate into
    /// directories and zips, or finalize the dialog (close it) when the
    /// entry is a final selectable file for this mode. No-ops on entries
    /// that aren't selectable for the current mode (item 3.2).
    fn select_and_maybe_finalize(&mut self, entry: &str) {
        if !self.entry_is_selectable(entry) {
            return;
        }
        if self.browsing_zip {
            if entry == ".." {
                self.go_to_parent();
            } else {
                self.mode = FileOpsMode::Idle;
            }
            return;
        }
        if entry == ".." {
            self.go_to_parent();
            return;
        }
        let lower = entry.to_lowercase();
        let is_navigable =
            lower.ends_with(".zip") || self.current_parent_dir().join(entry).is_dir();
        self.select_entry();
        if !is_navigable {
            // entry_is_selectable() already confirmed this matches the
            // mode's file type.
            self.mode = FileOpsMode::Idle;
        }
    }

    pub fn handle_key(&mut self, code: KeyCode) -> bool {
        match self.mode {
            FileOpsMode::SaveAs => self.handle_key_save_as(code),
            FileOpsMode::Open
            | FileOpsMode::ImportFont
            | FileOpsMode::ImportGif
            | FileOpsMode::OpenImage => self.handle_key_browse(code),
            FileOpsMode::Idle => false,
        }
    }

    /// Shared key handling for the four "browse for an existing file" modes
    /// (Open/ImportFont/ImportGif/OpenImage) — these differed only in which
    /// file extensions they accept, now centralized in `allowed()` /
    /// `mode_matches_extension()` / `mode_extension_error()`.
    fn handle_key_browse(&mut self, code: KeyCode) -> bool {
        match code {
            // Recent-file shortcuts (1-9) are an Open-mode-only feature;
            // other modes fall through and type the digit normally.
            KeyCode::Char(c)
                if self.mode == FileOpsMode::Open && c.is_ascii_digit() && c != '0' =>
            {
                if self.browsing_zip {
                    self.browsing_zip = false;
                    self.current_zip_path.clear();
                }
                let idx = (c as u8 - b'1') as usize;
                if idx < self.recent_files_for_display.len() {
                    self.path_buffer = self.recent_files_for_display[idx].clone();
                    self.selected_entry = 0;
                    self.error_message.clear();
                    self.refresh_directory();
                }
                true
            }
            KeyCode::Char(c) if !c.is_control() => {
                if self.browsing_zip {
                    return true;
                }
                self.path_buffer.push(c);
                self.error_message.clear();
                self.selected_entry = 0;
                self.refresh_directory();
                true
            }
            KeyCode::Backspace => {
                if self.browsing_zip {
                    self.go_to_parent();
                    return true;
                }
                self.path_buffer.pop();
                self.error_message.clear();
                self.selected_entry = 0;
                self.refresh_directory();
                true
            }
            KeyCode::Up => {
                if !self.directory_entries.is_empty() && self.selected_entry > 0 {
                    self.selected_entry -= 1;
                }
                true
            }
            KeyCode::Down => {
                if !self.directory_entries.is_empty()
                    && self.selected_entry < self.directory_entries.len() - 1
                {
                    self.selected_entry += 1;
                }
                true
            }
            // Right descends into a directory/zip, same as Tab — distinct
            // from Enter, which additionally finalizes on a selectable file.
            KeyCode::Tab | KeyCode::Right => {
                if !self.directory_entries.is_empty() {
                    self.select_entry();
                }
                true
            }
            KeyCode::Left => {
                self.go_to_parent();
                true
            }
            KeyCode::Enter => {
                if !self.browsing_zip && !self.path_buffer.trim().is_empty() {
                    let p = PathBuf::from(self.path_buffer.trim());
                    if p.is_file() {
                        let lower = self.path_buffer.to_lowercase();
                        if lower.ends_with(".zip")
                            && matches!(self.mode, FileOpsMode::Open | FileOpsMode::ImportFont)
                        {
                            self.current_zip_path = p;
                            self.browsing_zip = true;
                            self.selected_entry = 0;
                            self.error_message.clear();
                            self.refresh_directory();
                        } else if self.mode == FileOpsMode::Open {
                            // Open has always accepted any existing file path
                            // outright — a bad FIGfont surfaces as an async
                            // parse error from the actual load attempt.
                            self.mode = FileOpsMode::Idle;
                        } else if self.mode_matches_extension(&lower) {
                            self.mode = FileOpsMode::Idle;
                        } else {
                            self.error_message = self.mode_extension_error().to_string();
                        }
                        return true;
                    }
                }
                if self.directory_entries.is_empty() {
                    return true;
                }
                let entry = self.directory_entries[self.selected_entry].clone();
                self.select_and_maybe_finalize(&entry);
                true
            }
            KeyCode::Esc => {
                self.close();
                true
            }
            _ => false,
        }
    }

    fn is_image_extension(lower: &str) -> bool {
        lower.ends_with(".png")
            || lower.ends_with(".jpg")
            || lower.ends_with(".jpeg")
            || lower.ends_with(".bmp")
            || lower.ends_with(".webp")
            || lower.ends_with(".gif")
    }

    fn handle_key_save_as(&mut self, code: KeyCode) -> bool {
        match code {
            KeyCode::Tab => {
                self.save_focus = match self.save_focus {
                    SaveFocus::Directory => SaveFocus::Filename,
                    SaveFocus::Filename => SaveFocus::Directory,
                };
                true
            }
            KeyCode::Char(c) if self.save_focus == SaveFocus::Filename => {
                self.filename_buffer.push(c);
                self.error_message.clear();
                true
            }
            KeyCode::Backspace if self.save_focus == SaveFocus::Filename => {
                self.filename_buffer.pop();
                self.error_message.clear();
                true
            }
            KeyCode::Up => {
                if !self.directory_entries.is_empty() && self.selected_entry > 0 {
                    self.selected_entry -= 1;
                }
                true
            }
            KeyCode::Down => {
                if !self.directory_entries.is_empty()
                    && self.selected_entry < self.directory_entries.len() - 1
                {
                    self.selected_entry += 1;
                }
                true
            }
            KeyCode::Right => {
                if !self.directory_entries.is_empty() {
                    self.select_entry();
                }
                true
            }
            KeyCode::Left => {
                self.go_to_parent();
                true
            }
            KeyCode::Enter => {
                // Finalize save — path/filename are in path_buffer/
                // filename_buffer, combined by selected_path() for the
                // caller.
                self.mode = FileOpsMode::Idle;
                true
            }
            KeyCode::Esc => {
                self.close();
                true
            }
            _ => false,
        }
    }

    /// Mouse handling shared by all dialog modes: click a row to
    /// select/activate it (single click, matching what Enter does for
    /// browse modes — Save mode only navigates, never auto-finalizes,
    /// since there's no "correct" file to click to complete a save), wheel
    /// scrolls the selection.
    pub fn handle_mouse(&mut self, col: u16, row: u16, kind: MouseEventKind) -> bool {
        if self.mode == FileOpsMode::Idle {
            return false;
        }
        match kind {
            MouseEventKind::Down(MouseButton::Left) => {
                let hit = self
                    .entry_rects
                    .iter()
                    .find(|(_, r)| r.contains((col, row).into()))
                    .map(|(i, _)| *i);
                let Some(idx) = hit else {
                    return false;
                };
                self.selected_entry = idx;
                let entry = self.directory_entries[idx].clone();
                if self.mode == FileOpsMode::SaveAs {
                    if self.entry_is_selectable(&entry) {
                        self.select_entry();
                    }
                } else {
                    self.select_and_maybe_finalize(&entry);
                }
                true
            }
            MouseEventKind::ScrollUp => {
                if self.selected_entry > 0 {
                    self.selected_entry -= 1;
                }
                true
            }
            MouseEventKind::ScrollDown => {
                if !self.directory_entries.is_empty()
                    && self.selected_entry < self.directory_entries.len() - 1
                {
                    self.selected_entry += 1;
                }
                true
            }
            _ => false,
        }
    }

    pub fn render(&mut self, frame: &mut Frame, area: Rect) {
        match self.mode {
            FileOpsMode::SaveAs => self.render_save_as(frame, area),
            FileOpsMode::Open => self.render_open(frame, area),
            FileOpsMode::ImportFont => self.render_import_font(frame, area),
            FileOpsMode::ImportGif => self.render_import_gif(frame, area),
            FileOpsMode::OpenImage => self.render_open_image(frame, area),
            FileOpsMode::Idle => {}
        }
    }

    /// Render the directory-entry list into `lines`, recording each row's
    /// screen rect into `entry_rects` for mouse hit-testing. Shared by every
    /// mode's render function — they previously duplicated this loop with
    /// only cosmetic differences (flat zip entries vs dir-suffixed entries).
    fn push_entry_lines(
        &mut self,
        lines: &mut Vec<Line<'static>>,
        inner: Rect,
        max_visible: usize,
    ) {
        self.entry_rects.clear();
        if self.directory_entries.is_empty() {
            return;
        }
        let start = self.selected_entry.saturating_sub(max_visible / 2);
        let end = (start + max_visible).min(self.directory_entries.len());
        let parent = self.current_parent_dir();
        for i in start..end {
            let entry = self.directory_entries[i].clone();
            let is_selected = i == self.selected_entry;
            let prefix = if is_selected { " >" } else { "  " };
            // Untrusted label (filesystem / ZIP entry name): sanitized
            // for display only; selection still uses the raw entry (F-24).
            let text = if self.browsing_zip {
                format!("{prefix}{}", crate::sanitize::sanitize_display(&entry))
            } else {
                let is_dir = entry == ".." || parent.join(&entry).is_dir();
                let suffix = if is_dir { "/" } else { "" };
                format!(
                    "{prefix}{}{suffix}",
                    crate::sanitize::sanitize_display(&entry)
                )
            };
            let style = if is_selected {
                Style::default().add_modifier(Modifier::REVERSED)
            } else {
                Style::default()
            };
            let row_y = inner.y + lines.len() as u16;
            if row_y < inner.y.saturating_add(inner.height) {
                self.entry_rects.push((
                    i,
                    Rect {
                        x: inner.x,
                        y: row_y,
                        width: inner.width,
                        height: 1,
                    },
                ));
            }
            lines.push(Line::from(Span::styled(text, style)));
        }
    }

    fn render_open(&mut self, frame: &mut Frame, area: Rect) {
        frame.render_widget(Clear, area);
        let block = Block::default()
            .title(" Open Font ")
            .borders(Borders::ALL)
            .style(Style::default().fg(self.theme.dialog.border_path));
        let inner = block.inner(area);
        frame.render_widget(block, area);

        if inner.width < 24 || inner.height < 8 {
            return;
        }

        let mut lines: Vec<Line> = Vec::new();

        lines.push(Line::from(Span::styled(
            " Path:",
            Style::default().add_modifier(Modifier::BOLD),
        )));

        let path_display = if self.browsing_zip {
            format!("[ZIP] {}", self.current_zip_path.display())
        } else if self.path_buffer.is_empty() {
            " (type path, browse with arrows, or paste)".to_string()
        } else {
            // Typed paths are untrusted input: display sanitized (F-24).
            crate::sanitize::sanitize_display(&self.path_buffer)
        };
        lines.push(Line::from(Span::styled(
            format!(" {}", path_display),
            Style::default().fg(self.theme.dialog.border_path),
        )));

        if !self.error_message.is_empty() {
            lines.push(Line::from(Span::styled(
                format!(" Error: {}", self.error_message),
                Style::default().fg(self.theme.dialog.error),
            )));
        }

        lines.push(Line::from(""));

        if self.directory_entries.is_empty() {
            let msg = if self.browsing_zip {
                " (no .flf/.tlf files in this ZIP archive)"
            } else {
                " (no .flf/.tlf files in directory)"
            };
            lines.push(Line::from(Span::styled(
                msg,
                Style::default().fg(self.theme.dialog.meta),
            )));
        } else {
            lines.push(Line::from(Span::styled(
                " Directory:",
                Style::default().add_modifier(Modifier::BOLD),
            )));

            let max_visible = (inner.height as usize).saturating_sub(6).min(8);
            self.push_entry_lines(&mut lines, inner, max_visible);
        }

        if !self.recent_files_for_display.is_empty() {
            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                " Recent files:",
                Style::default().add_modifier(Modifier::BOLD),
            )));
            let max_recent = 9.min(self.recent_files_for_display.len());
            let recent_start = if self.recent_files_for_display.len() > 9 {
                self.recent_files_for_display.len() - 9
            } else {
                0
            };
            for i in recent_start..recent_start + max_recent {
                let display = &self.recent_files_for_display[i];
                let num = i + 1;
                let text = format!("  {num}. {display}");
                lines.push(Line::from(Span::styled(
                    text,
                    Style::default().fg(self.theme.dialog.meta),
                )));
            }
        }

        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            " \u{2190}\u{2192}/Tab: navigate  Enter/click: open  Esc: cancel  1-9: recent  \u{2191}\u{2193}: select",
            Style::default().fg(self.theme.dialog.meta),
        )));

        let paragraph = Paragraph::new(lines);
        frame.render_widget(paragraph, inner);
    }

    fn render_import_font(&mut self, frame: &mut Frame, area: Rect) {
        frame.render_widget(Clear, area);
        let block = Block::default()
            .title(" Import Font (.ttf/.otf) ")
            .borders(Borders::ALL)
            .style(Style::default().fg(self.theme.dialog.border_path));
        let inner = block.inner(area);
        frame.render_widget(block, area);

        if inner.width < 24 || inner.height < 8 {
            return;
        }

        let mut lines: Vec<Line> = Vec::new();

        lines.push(Line::from(Span::styled(
            " Path:",
            Style::default().add_modifier(Modifier::BOLD),
        )));

        let path_display = if self.browsing_zip {
            format!("[ZIP] {}", self.current_zip_path.display())
        } else if self.path_buffer.is_empty() {
            " (type path or browse with arrows)".to_string()
        } else {
            self.path_buffer.clone()
        };
        lines.push(Line::from(Span::styled(
            format!(" {}", path_display),
            Style::default().fg(self.theme.dialog.border_path),
        )));

        if !self.error_message.is_empty() {
            lines.push(Line::from(Span::styled(
                format!(" Error: {}", self.error_message),
                Style::default().fg(self.theme.dialog.error),
            )));
        }

        lines.push(Line::from(""));

        if self.directory_entries.is_empty() {
            let msg = if self.browsing_zip {
                " (no .flf/.tlf files in this ZIP archive)"
            } else {
                " (no .ttf/.otf/.zip files in directory)"
            };
            lines.push(Line::from(Span::styled(
                msg,
                Style::default().fg(self.theme.dialog.meta),
            )));
        } else {
            lines.push(Line::from(Span::styled(
                " Directory:",
                Style::default().add_modifier(Modifier::BOLD),
            )));

            let max_visible = (inner.height as usize).saturating_sub(6).min(8);
            self.push_entry_lines(&mut lines, inner, max_visible);
        }

        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            " \u{2190}\u{2192}/Tab: navigate  Enter/click: import  Esc: cancel  \u{2191}\u{2193}: select",
            Style::default().fg(self.theme.dialog.meta),
        )));

        let paragraph = Paragraph::new(lines);
        frame.render_widget(paragraph, inner);
    }

    fn render_import_gif(&mut self, frame: &mut Frame, area: Rect) {
        frame.render_widget(Clear, area);
        let block = Block::default()
            .title(" Import GIF (.gif) ")
            .borders(Borders::ALL)
            .style(Style::default().fg(self.theme.dialog.border_path));
        let inner = block.inner(area);
        frame.render_widget(block, area);

        if inner.width < 24 || inner.height < 8 {
            return;
        }

        let mut lines: Vec<Line> = Vec::new();

        lines.push(Line::from(Span::styled(
            " Path:",
            Style::default().add_modifier(Modifier::BOLD),
        )));

        let path_display = if self.path_buffer.is_empty() {
            " (type path or browse with arrows)".to_string()
        } else {
            self.path_buffer.clone()
        };
        lines.push(Line::from(Span::styled(
            format!(" {}", path_display),
            Style::default().fg(self.theme.dialog.border_path),
        )));

        if !self.error_message.is_empty() {
            lines.push(Line::from(Span::styled(
                format!(" Error: {}", self.error_message),
                Style::default().fg(self.theme.dialog.error),
            )));
        }

        lines.push(Line::from(""));

        if self.directory_entries.is_empty() {
            lines.push(Line::from(Span::styled(
                " (no .gif files in directory)",
                Style::default().fg(self.theme.dialog.meta),
            )));
        } else {
            lines.push(Line::from(Span::styled(
                " Directory:",
                Style::default().add_modifier(Modifier::BOLD),
            )));

            let max_visible = (inner.height as usize).saturating_sub(6).min(8);
            self.push_entry_lines(&mut lines, inner, max_visible);
        }

        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            " \u{2190}\u{2192}/Tab: navigate  Enter/click: import  Esc: cancel  \u{2191}\u{2193}: select",
            Style::default().fg(self.theme.dialog.meta),
        )));

        let paragraph = Paragraph::new(lines);
        frame.render_widget(paragraph, inner);
    }

    fn render_open_image(&mut self, frame: &mut Frame, area: Rect) {
        frame.render_widget(Clear, area);
        let block = Block::default()
            .title(" Open Image (.png/.jpg/.bmp/.webp/.gif) ")
            .borders(Borders::ALL)
            .style(Style::default().fg(self.theme.dialog.border_path));
        let inner = block.inner(area);
        frame.render_widget(block, area);

        if inner.width < 24 || inner.height < 8 {
            return;
        }

        let mut lines: Vec<Line> = Vec::new();

        lines.push(Line::from(Span::styled(
            " Path:",
            Style::default().add_modifier(Modifier::BOLD),
        )));

        let path_display = if self.path_buffer.is_empty() {
            " (type path or browse with arrows)".to_string()
        } else {
            self.path_buffer.clone()
        };
        lines.push(Line::from(Span::styled(
            format!(" {}", path_display),
            Style::default().fg(self.theme.dialog.border_path),
        )));

        if !self.error_message.is_empty() {
            lines.push(Line::from(Span::styled(
                format!(" Error: {}", self.error_message),
                Style::default().fg(self.theme.dialog.error),
            )));
        }

        lines.push(Line::from(""));

        if self.directory_entries.is_empty() {
            lines.push(Line::from(Span::styled(
                " (no image files in directory)",
                Style::default().fg(self.theme.dialog.meta),
            )));
        } else {
            lines.push(Line::from(Span::styled(
                " Directory:",
                Style::default().add_modifier(Modifier::BOLD),
            )));

            let max_visible = (inner.height as usize).saturating_sub(6).min(8);
            self.push_entry_lines(&mut lines, inner, max_visible);
        }

        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            " \u{2190}\u{2192}/Tab: navigate  Enter/click: open  Esc: cancel  \u{2191}\u{2193}: select",
            Style::default().fg(self.theme.dialog.meta),
        )));

        let paragraph = Paragraph::new(lines);
        frame.render_widget(paragraph, inner);
    }

    fn render_save_as(&mut self, frame: &mut Frame, area: Rect) {
        frame.render_widget(Clear, area);
        let block = Block::default()
            .title(" Save Font As ")
            .borders(Borders::ALL)
            .style(Style::default().fg(self.theme.dialog.highlight));
        let inner = block.inner(area);
        frame.render_widget(block, area);

        if inner.width < 20 || inner.height < 6 {
            return;
        }

        let mut lines: Vec<Line> = Vec::new();

        let dir_style = if self.save_focus == SaveFocus::Directory {
            Style::default().add_modifier(Modifier::REVERSED)
        } else {
            Style::default().fg(self.theme.dialog.border_path)
        };
        lines.push(Line::from(Span::styled(
            " Folder:",
            Style::default().add_modifier(Modifier::BOLD),
        )));
        let dir_display = if self.path_buffer.is_empty() {
            ".".to_string()
        } else {
            self.path_buffer.clone()
        };
        lines.push(Line::from(Span::styled(
            format!(" {}", dir_display),
            dir_style,
        )));

        let name_style = if self.save_focus == SaveFocus::Filename {
            Style::default().add_modifier(Modifier::REVERSED)
        } else {
            Style::default().fg(self.theme.dialog.border_path)
        };
        lines.push(Line::from(vec![
            Span::styled(" Filename: ", Style::default().add_modifier(Modifier::BOLD)),
            Span::styled(self.filename_buffer.clone(), name_style),
        ]));

        if !self.error_message.is_empty() {
            lines.push(Line::from(Span::styled(
                format!(" Error: {}", self.error_message),
                Style::default().fg(self.theme.dialog.error),
            )));
        }

        lines.push(Line::from(""));

        if self.directory_entries.is_empty() {
            lines.push(Line::from(Span::styled(
                " (no .flf/.tlf files in directory)",
                Style::default().fg(self.theme.dialog.meta),
            )));
        } else {
            lines.push(Line::from(Span::styled(
                " Directory contents:",
                Style::default().add_modifier(Modifier::BOLD),
            )));

            let max_visible = (inner.height as usize).saturating_sub(7).min(10);
            self.push_entry_lines(&mut lines, inner, max_visible);
        }

        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            " Tab: switch field  \u{2190}\u{2192}: browse  Enter: save  Esc: cancel  \u{2191}\u{2193}: select",
            Style::default().fg(self.theme.dialog.meta),
        )));

        let paragraph = Paragraph::new(lines);
        frame.render_widget(paragraph, inner);
    }
}

impl Default for FileOpsDialog {
    fn default() -> Self {
        Self::new()
    }
}

pub fn save_font(font: &FIGfont, path: &Path) -> std::io::Result<()> {
    let content = crate::font_gen::generate_figfont(font);
    // Atomic + symlink-safe replacement (GPT review F-22); the old code
    // wrote a predictable `.stem.tmp` and followed any symlink there.
    crate::atomic_io::atomic_write(path, content.as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::font::{parse_tlf_font, FIGcharacter, FIGfont};
    use std::collections::HashMap;

    fn make_test_font() -> FIGfont {
        FIGfont {
            charheight: 3,
            maxlength: 5,
            chars: HashMap::from([
                (
                    0,
                    FIGcharacter::from(vec![
                        "     ".to_string(),
                        "     ".to_string(),
                        "     ".to_string(),
                    ]),
                ),
                (
                    65,
                    FIGcharacter::from(vec![
                        " AA  ".to_string(),
                        "A  A ".to_string(),
                        "AAAA ".to_string(),
                    ]),
                ),
            ]),
            ..Default::default()
        }
    }

    #[test]
    fn test_save_and_reload_roundtrip() {
        let font = make_test_font();
        let dir = std::env::temp_dir();
        let path = dir.join("test_save_roundtrip.flf");

        save_font(&font, &path).expect("save should succeed");
        assert!(path.exists(), "file should exist after save");

        let content = std::fs::read_to_string(&path).expect("should read saved file");
        let parsed = parse_tlf_font(&content).expect("saved content should parse as FIGfont");

        assert_eq!(parsed.charheight, font.charheight);
        assert_eq!(parsed.maxlength, font.maxlength);

        let a_parsed = parsed.chars.get(&65).expect("char 65 should exist");
        assert_eq!(a_parsed.rows(), &[" AA  ", "A  A ", "AAAA "]);

        let bytes_orig = content.as_bytes();
        let bytes_reload = std::fs::read(&path).expect("should read raw bytes");
        assert_eq!(
            bytes_orig, bytes_reload,
            "reloaded bytes should match saved content"
        );

        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn test_save_creates_valid_flf() {
        let font = make_test_font();
        let dir = std::env::temp_dir();
        let path = dir.join("test_save_valid.flf");

        save_font(&font, &path).expect("save should succeed");

        let content = std::fs::read_to_string(&path).expect("should read saved file");
        let parsed = parse_tlf_font(&content).expect("saved content should parse");

        assert_eq!(parsed.charheight, 3);
        assert!(parsed.chars.contains_key(&32), "space char should exist");
        assert!(parsed.chars.contains_key(&126), "tilde char should exist");

        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn test_save_as_error_handling() {
        let font = make_test_font();
        let result = save_font(&font, Path::new("/nonexistent_dir/out.flf"));
        assert!(result.is_err(), "save to invalid dir should error");
    }

    #[test]
    fn test_file_ops_dialog_new() {
        let dialog = FileOpsDialog::new();
        assert_eq!(dialog.mode, FileOpsMode::Idle);
        assert!(dialog.path_buffer.is_empty());
        assert!(dialog.error_message.is_empty());
    }

    #[test]
    fn test_file_ops_enter_save_as() {
        let mut dialog = FileOpsDialog::new();
        dialog.enter_save_as(None, "untitled");
        assert_eq!(dialog.mode, FileOpsMode::SaveAs);
        assert!(dialog.path_buffer.is_empty());
        assert_eq!(dialog.filename_buffer, "untitled.flf");

        dialog.enter_save_as(Some(Path::new("/tmp/test.flf")), "untitled");
        assert_eq!(dialog.path_buffer, "/tmp");
        assert_eq!(dialog.filename_buffer, "test.flf");
    }

    #[test]
    fn test_file_ops_close() {
        let mut dialog = FileOpsDialog::new();
        dialog.enter_save_as(Some(Path::new("test.flf")), "untitled");
        dialog.close();
        assert_eq!(dialog.mode, FileOpsMode::Idle);
    }

    #[test]
    fn test_selected_path_adds_extension() {
        let dialog = FileOpsDialog {
            path_buffer: "myfont".to_string(),
            ..FileOpsDialog::new()
        };
        let path = dialog.selected_path();
        assert_eq!(
            path.extension().map(|e| e.to_string_lossy().to_string()),
            Some("flf".to_string())
        );
    }

    #[test]
    fn test_selected_path_keeps_extension() {
        let dialog = FileOpsDialog {
            path_buffer: "myfont.flf".to_string(),
            ..FileOpsDialog::new()
        };
        let path = dialog.selected_path();
        assert_eq!(
            path.file_name().map(|n| n.to_string_lossy().to_string()),
            Some("myfont.flf".to_string())
        );
    }

    #[test]
    fn test_save_target_path_combines_directory_and_filename() {
        let dialog = FileOpsDialog {
            mode: FileOpsMode::SaveAs,
            path_buffer: "/tmp/fonts".to_string(),
            filename_buffer: "myfont".to_string(),
            ..FileOpsDialog::new()
        };
        let path = dialog.selected_path();
        assert_eq!(path, PathBuf::from("/tmp/fonts/myfont.flf"));
    }

    #[test]
    fn test_save_target_path_keeps_existing_extension() {
        let dialog = FileOpsDialog {
            mode: FileOpsMode::SaveAs,
            path_buffer: "/tmp/fonts".to_string(),
            filename_buffer: "myfont.tlf".to_string(),
            ..FileOpsDialog::new()
        };
        let path = dialog.selected_path();
        assert_eq!(path, PathBuf::from("/tmp/fonts/myfont.tlf"));
    }

    #[test]
    fn test_save_target_path_falls_back_to_untitled_when_filename_empty() {
        let dialog = FileOpsDialog {
            mode: FileOpsMode::SaveAs,
            path_buffer: "/tmp/fonts".to_string(),
            filename_buffer: String::new(),
            ..FileOpsDialog::new()
        };
        let path = dialog.selected_path();
        assert_eq!(path, PathBuf::from("/tmp/fonts/untitled.flf"));
    }

    #[test]
    fn test_tab_switches_save_focus_between_directory_and_filename() {
        let mut dialog = FileOpsDialog::new();
        dialog.enter_save_as(None, "untitled");
        assert_eq!(dialog.save_focus, SaveFocus::Filename);
        dialog.handle_key(KeyCode::Tab);
        assert_eq!(dialog.save_focus, SaveFocus::Directory);
        dialog.handle_key(KeyCode::Tab);
        assert_eq!(dialog.save_focus, SaveFocus::Filename);
    }

    #[test]
    fn test_typing_only_affects_filename_when_directory_focused() {
        let mut dialog = FileOpsDialog::new();
        dialog.enter_save_as(None, "untitled");
        dialog.save_focus = SaveFocus::Directory;
        let before = dialog.filename_buffer.clone();
        dialog.handle_key(KeyCode::Char('x'));
        assert_eq!(
            dialog.filename_buffer, before,
            "typing while Directory is focused should not touch the filename field"
        );
    }

    // --- Open dialog tests ---

    #[test]
    fn test_open_dialog_enter() {
        let mut dialog = FileOpsDialog::new();
        let recent = Vec::new();
        dialog.enter_open(&recent);
        assert_eq!(dialog.mode, FileOpsMode::Open);
        assert!(dialog.path_buffer.is_empty());
        assert!(dialog.error_message.is_empty());
    }

    #[test]
    fn test_open_dialog_enter_exit() {
        let mut dialog = FileOpsDialog::new();
        let recent = Vec::new();
        dialog.enter_open(&recent);
        assert_eq!(dialog.mode, FileOpsMode::Open);
        dialog.handle_key(KeyCode::Esc);
        assert_eq!(dialog.mode, FileOpsMode::Idle);
    }

    #[test]
    fn test_open_dialog_type_path() {
        let mut dialog = FileOpsDialog::new();
        let recent = Vec::new();
        dialog.enter_open(&recent);
        dialog.handle_key(KeyCode::Char('m'));
        dialog.handle_key(KeyCode::Char('y'));
        dialog.handle_key(KeyCode::Char('.'));
        dialog.handle_key(KeyCode::Char('f'));
        dialog.handle_key(KeyCode::Char('l'));
        dialog.handle_key(KeyCode::Char('f'));
        assert_eq!(dialog.path_buffer, "my.flf");
    }

    #[test]
    fn test_open_dialog_paste_path() {
        let mut dialog = FileOpsDialog::new();
        let recent = Vec::new();
        dialog.enter_open(&recent);
        dialog.handle_paste("/path/to/font.flf");
        assert_eq!(dialog.path_buffer, "/path/to/font.flf");
    }

    #[test]
    fn test_open_dialog_paste_idle_noop() {
        let mut dialog = FileOpsDialog::new();
        assert_eq!(dialog.mode, FileOpsMode::Idle);
        dialog.handle_paste("should not set");
        assert!(dialog.path_buffer.is_empty());
    }

    #[test]
    fn test_open_dialog_enter_finalizes() {
        let mut dialog = FileOpsDialog::new();
        let recent = Vec::new();
        dialog.enter_open(&recent);
        // Enter with empty path stays Open (no file selected)
        dialog.handle_key(KeyCode::Enter);
        assert_eq!(dialog.mode, FileOpsMode::Open);
        // Enter with a non-existent path also stays Open (file must exist on disk)
        dialog.path_buffer = "/tmp/figby_test_nonexistent.flf".to_string();
        dialog.handle_key(KeyCode::Enter);
        assert_eq!(dialog.mode, FileOpsMode::Open);
        // Esc closes the dialog
        dialog.handle_key(KeyCode::Esc);
        assert_eq!(dialog.mode, FileOpsMode::Idle);
    }

    #[test]
    fn test_open_dialog_backspace_empty() {
        let mut dialog = FileOpsDialog::new();
        let recent = Vec::new();
        dialog.enter_open(&recent);
        // Backspace on empty buffer is a no-op
        dialog.handle_key(KeyCode::Backspace);
        assert!(dialog.path_buffer.is_empty());
    }

    // --- Recent files tests ---

    #[test]
    fn test_recent_files_new_empty() {
        let recent = RecentFiles::new();
        assert!(recent.is_empty());
        assert_eq!(recent.len(), 0);
    }

    #[test]
    fn test_recent_files_push() {
        let mut recent = RecentFiles::new();
        recent.push(PathBuf::from("/a.flf"));
        recent.push(PathBuf::from("/b.flf"));
        recent.push(PathBuf::from("/c.flf"));
        assert_eq!(recent.len(), 3);
        assert_eq!(recent.get(0), Some(&PathBuf::from("/c.flf")));
        assert_eq!(recent.get(2), Some(&PathBuf::from("/a.flf")));
    }

    #[test]
    fn test_recent_files_max() {
        let mut recent = RecentFiles::new();
        for i in 0..15 {
            recent.push(PathBuf::from(format!("/font_{i}.flf")));
        }
        assert_eq!(recent.len(), 10);
        assert_eq!(recent.get(0), Some(&PathBuf::from("/font_14.flf")));
        assert_eq!(recent.get(9), Some(&PathBuf::from("/font_5.flf")));
    }

    #[test]
    fn test_recent_files_dedup() {
        let mut recent = RecentFiles::new();
        recent.push(PathBuf::from("/a.flf"));
        recent.push(PathBuf::from("/b.flf"));
        recent.push(PathBuf::from("/a.flf"));
        assert_eq!(recent.len(), 2);
        assert_eq!(recent.get(0), Some(&PathBuf::from("/a.flf")));
        assert_eq!(recent.get(1), Some(&PathBuf::from("/b.flf")));
    }

    #[test]
    fn test_recent_files_roundtrip() {
        let dir = std::env::temp_dir();
        let subdir = dir.join("figby-test-recent");
        let _ = std::fs::create_dir_all(&subdir);
        // Override storage path via XDG_CONFIG_HOME (config_dir derives from it)
        std::env::set_var("XDG_CONFIG_HOME", dir.to_str().unwrap());

        let mut recent = RecentFiles::new();
        recent.push(PathBuf::from("/font_a.flf"));
        recent.push(PathBuf::from("/font_b.flf"));
        recent.save_to_disk();

        let loaded = RecentFiles::load_from_disk();
        assert_eq!(loaded.len(), 2);
        assert_eq!(loaded.get(0), Some(&PathBuf::from("/font_b.flf")));
        assert_eq!(loaded.get(1), Some(&PathBuf::from("/font_a.flf")));

        let saved_path = dir.join("figby/recent_files.json");
        let _ = std::fs::remove_file(&saved_path);
        let _ = std::fs::remove_dir(&subdir);
        std::env::remove_var("XDG_CONFIG_HOME");
    }

    #[test]
    fn test_recent_files_load_empty_on_missing() {
        // Loading from a non-existent file should give empty list
        let recent = RecentFiles::new();
        assert!(recent.is_empty());
    }

    #[test]
    fn test_recent_files_remove_missing() {
        let mut recent = RecentFiles::new();
        recent.push(PathBuf::from("/nonexistent_path_12345.flf"));
        recent.remove_missing();
        assert!(recent.is_empty());
    }

    #[test]
    fn test_recent_files_remove_idx() {
        let mut recent = RecentFiles::new();
        recent.push(PathBuf::from("/a.flf"));
        recent.push(PathBuf::from("/b.flf"));
        recent.remove(0);
        assert_eq!(recent.len(), 1);
        assert_eq!(recent.get(0), Some(&PathBuf::from("/a.flf")));
        recent.remove(5); // out of bounds, no panic
        assert_eq!(recent.len(), 1);
    }

    // --- Integration test: open known font, verify all glyphs ---

    #[test]
    fn test_open_known_font_all_glyphs_loaded() {
        let font_dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../fonts");
        let path = std::path::Path::new(font_dir).join("standard.flf");
        let content = std::fs::read_to_string(&path).expect("standard.flf should exist");
        let font = parse_tlf_font(&content).expect("standard.flf should parse");

        // Verify 95 ASCII chars (32-126)
        for code in 32..=126u32 {
            assert!(
                font.chars.contains_key(&code),
                "ASCII char U+{code:04X} should be present"
            );
        }

        // Verify 7 Deutsch chars
        let deutsch_codes = [196u32, 214, 220, 228, 246, 252, 223];
        for &code in &deutsch_codes {
            assert!(
                font.chars.contains_key(&code),
                "Deutsch char U+{code:04X} should be present"
            );
        }

        assert_eq!(font.charheight, 6, "standard.flf should have height 6");
    }

    #[test]
    fn test_open_dialog_recent_file_by_digit() {
        let mut dialog = FileOpsDialog::new();
        let recent = vec![
            PathBuf::from("/first.flf"),
            PathBuf::from("/second.flf"),
            PathBuf::from("/third.flf"),
        ];
        dialog.enter_open(&recent);

        // Press '2' to select second recent file
        dialog.handle_key(KeyCode::Char('2'));
        assert_eq!(dialog.path_buffer, "/second.flf");
    }

    #[test]
    fn test_open_dialog_recent_digit_out_of_range() {
        let mut dialog = FileOpsDialog::new();
        let recent = vec![PathBuf::from("/first.flf")];
        dialog.enter_open(&recent);

        // Press '9' - only 1 recent file, so should be no-op
        dialog.handle_key(KeyCode::Char('9'));
        assert!(dialog.path_buffer.is_empty());
    }

    // --- Part Twah 8.1: navigation / mouse / zip-in-ImportFont tests ---

    fn make_test_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("figby-fileops-test-{name}"));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("child")).unwrap();
        std::fs::write(dir.join("font.flf"), "test").unwrap();
        dir
    }

    #[test]
    fn test_dotdot_entry_in_normal_directory_listing() {
        let dir = make_test_dir("dotdot");
        let mut dialog = FileOpsDialog::new();
        dialog.mode = FileOpsMode::Open;
        dialog.handle_paste(dir.to_str().unwrap());
        assert_eq!(
            dialog.directory_entries.first().map(String::as_str),
            Some(".."),
            ".. should be offered when not at the filesystem root"
        );
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn test_left_key_navigates_to_parent_directory() {
        let dir = make_test_dir("left-nav");
        let child = dir.join("child");
        let mut dialog = FileOpsDialog::new();
        dialog.mode = FileOpsMode::Open;
        dialog.handle_paste(child.to_str().unwrap());
        dialog.handle_key(KeyCode::Left);
        assert_eq!(PathBuf::from(&dialog.path_buffer), dir);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn test_right_key_descends_into_directory() {
        let dir = make_test_dir("right-nav");
        let mut dialog = FileOpsDialog::new();
        dialog.mode = FileOpsMode::Open;
        dialog.handle_paste(dir.to_str().unwrap());
        // Entries are sorted with ".." first, then "child", then "font.flf".
        let child_idx = dialog
            .directory_entries
            .iter()
            .position(|e| e == "child")
            .expect("child dir should be listed");
        dialog.selected_entry = child_idx;
        dialog.handle_key(KeyCode::Right);
        assert_eq!(PathBuf::from(&dialog.path_buffer), dir.join("child"));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn test_import_font_mode_allows_zip_visibility() {
        let dialog = FileOpsDialog {
            mode: FileOpsMode::ImportFont,
            ..FileOpsDialog::new()
        };
        assert!(dialog.allowed("bundle.zip", false));
        assert!(dialog.allowed("myfont.ttf", false));
        assert!(!dialog.allowed("readme.txt", false));
    }

    #[test]
    fn test_entry_is_selectable_gates_on_mode_extension() {
        let dialog = FileOpsDialog {
            mode: FileOpsMode::ImportGif,
            ..FileOpsDialog::new()
        };
        assert!(dialog.entry_is_selectable(".."));
        assert!(dialog.entry_is_selectable("clip.gif"));
        assert!(dialog.entry_is_selectable("bundle.zip"), "zip is navigable");
        assert!(
            !dialog.entry_is_selectable("picture.png"),
            "wrong extension for ImportGif should not be selectable"
        );
    }

    #[test]
    fn test_mouse_click_on_directory_entry_navigates() {
        let dir = make_test_dir("mouse-nav");
        let mut dialog = FileOpsDialog {
            mode: FileOpsMode::Open,
            path_buffer: dir.to_str().unwrap().to_string(),
            directory_entries: vec!["child".to_string()],
            entry_rects: vec![(
                0,
                Rect {
                    x: 0,
                    y: 5,
                    width: 20,
                    height: 1,
                },
            )],
            ..FileOpsDialog::new()
        };
        let consumed = dialog.handle_mouse(3, 5, MouseEventKind::Down(MouseButton::Left));
        assert!(consumed);
        assert_eq!(
            dialog.mode,
            FileOpsMode::Open,
            "directory click shouldn't finalize"
        );
        assert_eq!(PathBuf::from(&dialog.path_buffer), dir.join("child"));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn test_mouse_scroll_moves_selection() {
        let mut dialog = FileOpsDialog {
            mode: FileOpsMode::Open,
            directory_entries: vec!["a".to_string(), "b".to_string(), "c".to_string()],
            selected_entry: 0,
            ..FileOpsDialog::new()
        };
        dialog.handle_mouse(0, 0, MouseEventKind::ScrollDown);
        assert_eq!(dialog.selected_entry, 1);
        dialog.handle_mouse(0, 0, MouseEventKind::ScrollUp);
        assert_eq!(dialog.selected_entry, 0);
    }

    #[test]
    fn test_mouse_ignored_when_dialog_idle() {
        let mut dialog = FileOpsDialog::new();
        assert_eq!(dialog.mode, FileOpsMode::Idle);
        let consumed = dialog.handle_mouse(0, 0, MouseEventKind::Down(MouseButton::Left));
        assert!(!consumed);
    }
}
