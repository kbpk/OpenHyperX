//! Explicit offline file browsing. Never opens HID or writes while navigating.
use std::{
    fs,
    path::{Component, Path, PathBuf},
};

use anyhow::{bail, Context, Result};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use hyperx_app::ProfileDocument;

use crate::{
    app::{App, EditAction, Modal},
    editor::Editor,
};

const MAX_ENTRIES: usize = 4096;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Open,
    SaveNew,
}

pub struct Entry {
    pub path: PathBuf,
    pub directory: bool,
}

pub enum Prompt {
    Directory(Editor),
    Filename(Editor),
}

pub enum Outcome {
    Stay,
    Close,
    Open(PathBuf),
    Save(PathBuf),
    Adopt(Box<ProfileDocument>),
}

pub struct FileBrowser {
    pub mode: Mode,
    pub directory: PathBuf,
    pub entries: Vec<Entry>,
    pub selected: usize,
    pub warning: Option<String>,
    pub error: Option<String>,
    pub prompt: Option<Prompt>,
    pub pending: Option<Box<ProfileDocument>>,
    filename: String,
}

fn list(path: &Path, limit: usize) -> Result<(Vec<Entry>, bool)> {
    let mut entries = Vec::new();
    let mut truncated = false;
    for (index, entry) in fs::read_dir(path)?.enumerate() {
        if index == limit {
            truncated = true;
            break;
        }
        let entry = entry?;
        let kind = entry.file_type()?;
        // Do not follow directory links/reparse points or open special files.
        // An explicitly typed directory can still be chosen and resolved.
        if kind.is_dir()
            || (kind.is_file()
                && entry
                    .path()
                    .extension()
                    .and_then(|value| value.to_str())
                    .is_some_and(|value| value.eq_ignore_ascii_case("toml")))
        {
            entries.push(Entry {
                path: entry.path(),
                directory: kind.is_dir(),
            });
        }
    }
    entries.sort_by(|a, b| {
        b.directory
            .cmp(&a.directory)
            .then_with(|| a.path.file_name().cmp(&b.path.file_name()))
    });
    Ok((entries, truncated))
}

impl FileBrowser {
    pub fn new(directory: &Path, mode: Mode, source: Option<&Path>) -> Result<Self> {
        let filename = source
            .and_then(Path::file_stem)
            .and_then(|value| value.to_str())
            .map_or_else(
                || "profile-copy.toml".into(),
                |stem| format!("{stem}-copy.toml"),
            );
        let mut value = Self {
            mode,
            directory: PathBuf::new(),
            entries: Vec::new(),
            selected: 0,
            warning: None,
            error: None,
            prompt: None,
            pending: None,
            filename,
        };
        value.change_directory(directory)?;
        Ok(value)
    }
    pub fn change_directory(&mut self, directory: &Path) -> Result<()> {
        let directory = fs::canonicalize(directory).context("cannot resolve profile directory")?;
        let (entries, truncated) =
            list(&directory, MAX_ENTRIES).context("cannot list profile directory")?;
        // Commit navigation only after the whole bounded directory scan succeeds.
        self.directory = directory;
        self.entries = entries;
        self.selected = 0;
        self.warning = truncated.then(|| "Scan limit reached (4096 entries); some entries may be missing. Choose a smaller directory.".into());
        self.error = None;
        Ok(())
    }
    pub fn select(&mut self, index: usize) {
        if self.prompt.is_none() && self.pending.is_none() && index < self.entries.len() {
            self.selected = index;
            self.error = None;
        }
    }
    pub fn move_selection(&mut self, delta: i32) {
        if self.prompt.is_none() && self.pending.is_none() {
            let previous = self.selected;
            self.selected = (self.selected as i64 + i64::from(delta))
                .clamp(0, self.entries.len().saturating_sub(1) as i64)
                as usize;
            if previous != self.selected {
                self.error = None;
            }
        }
    }
    pub fn editor_mut(&mut self) -> Option<&mut Editor> {
        match &mut self.prompt {
            Some(Prompt::Directory(editor) | Prompt::Filename(editor)) => Some(editor),
            None => None,
        }
    }
    pub fn paste(&mut self, text: &str) {
        if let Some(editor) = self.editor_mut() {
            editor.paste(text);
            self.error = None;
        }
    }
    fn filename_path(&self, name: &str) -> Result<PathBuf> {
        let mut components = Path::new(name).components();
        if name.trim().is_empty()
            || name.chars().any(char::is_control)
            || name.contains(['/', '\\', ':'])
            || !matches!(components.next(), Some(Component::Normal(_)))
            || components.next().is_some()
        {
            bail!("enter one new filename, not a path; choose its directory in the browser");
        }
        if !Path::new(name)
            .extension()
            .and_then(|value| value.to_str())
            .is_some_and(|extension| extension.eq_ignore_ascii_case("toml"))
        {
            bail!("use a .toml filename; the browser lists TOML profiles");
        }
        // No lossy display-name round trip: the explicit filename is joined once.
        Ok(self.directory.join(name))
    }
    pub fn key(&mut self, key: KeyEvent) -> Outcome {
        if self.pending.is_some() {
            return match key.code {
                KeyCode::Char('y') => Outcome::Adopt(self.pending.take().unwrap()),
                KeyCode::Esc | KeyCode::Char('n') => {
                    self.pending = None;
                    Outcome::Stay
                }
                _ => Outcome::Stay,
            };
        }
        let accept = key.code == KeyCode::Enter
            || (key.code == KeyCode::Char('s') && key.modifiers.contains(KeyModifiers::CONTROL));
        if let Some(mut prompt) = self.prompt.take() {
            if key.code == KeyCode::Esc {
                self.error = None;
                return Outcome::Stay;
            }
            if accept {
                match &prompt {
                    Prompt::Directory(editor) => {
                        let path = self.directory.join(editor.text());
                        match self.change_directory(&path) {
                            Ok(()) => return Outcome::Stay,
                            Err(error) => self.error = Some(format!("{error:#}")),
                        }
                    }
                    Prompt::Filename(editor) => match self.filename_path(&editor.text()) {
                        Ok(path) => {
                            self.filename = editor.text();
                            self.prompt = Some(prompt);
                            return Outcome::Save(path);
                        }
                        Err(error) => self.error = Some(format!("{error:#}")),
                    },
                }
            } else {
                match &mut prompt {
                    Prompt::Directory(editor) | Prompt::Filename(editor) => editor.key(key),
                }
                self.error = None;
            }
            self.prompt = Some(prompt);
            return Outcome::Stay;
        }
        let navigate = match key.code {
            KeyCode::Esc => return Outcome::Close,
            KeyCode::Up => {
                self.move_selection(-1);
                None
            }
            KeyCode::Down => {
                self.move_selection(1);
                None
            }
            KeyCode::PageUp => {
                self.move_selection(-8);
                None
            }
            KeyCode::PageDown => {
                self.move_selection(8);
                None
            }
            KeyCode::Home => {
                self.selected = 0;
                None
            }
            KeyCode::End => {
                self.selected = self.entries.len().saturating_sub(1);
                None
            }
            KeyCode::Left | KeyCode::Backspace => self.directory.parent().map(Path::to_path_buf),
            KeyCode::F(5) => Some(self.directory.clone()),
            KeyCode::Char('p') => {
                self.prompt = Some(Prompt::Directory(Editor::new(
                    &self.directory.to_string_lossy(),
                    true,
                )));
                None
            }
            KeyCode::Char('s') | KeyCode::F(2) if self.mode == Mode::SaveNew => {
                self.prompt = Some(Prompt::Filename(Editor::new(&self.filename, true)));
                None
            }
            KeyCode::Enter | KeyCode::Right => {
                if let Some(entry) = self.entries.get(self.selected) {
                    if entry.directory {
                        Some(entry.path.clone())
                    } else if key.code == KeyCode::Enter && self.mode == Mode::Open {
                        return Outcome::Open(entry.path.clone());
                    } else {
                        None
                    }
                } else {
                    None
                }
            }
            _ => None,
        };
        if let Some(path) = navigate {
            if let Err(error) = self.change_directory(&path) {
                self.error = Some(format!("{error:#}"));
            }
        }
        Outcome::Stay
    }
}

impl App {
    pub(crate) fn open_browser(&mut self, mode: Mode) {
        let directory = self
            .document
            .path()
            .and_then(Path::parent)
            .filter(|path| !path.as_os_str().is_empty())
            .map(Path::to_path_buf)
            .or_else(|| std::env::current_dir().ok());
        let result = directory
            .context("current directory unavailable")
            .and_then(|directory| FileBrowser::new(&directory, mode, self.document.path()));
        match result {
            Ok(browser) => {
                self.modal = Some(Modal::Files(Box::new(browser)));
                self.drag = None;
            }
            Err(error) => self.status = format!("{error:#}"),
        }
    }
    pub(crate) fn adopt_document(&mut self, document: ProfileDocument) {
        self.document = document;
        self.demo = false;
        self.scroll = 0;
        self.selected_stage = 0;
        self.selected_control = 0;
        self.selected_macro = 0;
        self.status = "Opened offline file; current device state remains unknown.".into();
    }
    pub(crate) fn profiles_key(&mut self, key: KeyEvent) -> bool {
        if self.tab != 4 || self.modal.is_some() {
            return false;
        }
        match key.code {
            KeyCode::F(5) => {
                self.modal = Some(Modal::Resolution(Box::new(
                    crate::resolution::ResolutionPicker::new(self.document.profile()),
                )));
                self.drag = None;
            }
            KeyCode::F(2) => self.open_browser(Mode::Open),
            KeyCode::F(3) => self.editor(
                "Profile name - offline draft only",
                EditAction::ProfileName,
                self.document.profile().name.clone(),
                true,
            ),
            KeyCode::F(4) => self.open_browser(Mode::SaveNew),
            KeyCode::F(7) => {
                if let (Some(store), Some(local)) = (&self.recovery, &self.local_recovery) {
                    match crate::recovery_picker::RecoveryPicker::new(store, local) {
                        Ok(picker) => {
                            self.modal = Some(Modal::Recovery(Box::new(picker)));
                            self.drag = None;
                        }
                        Err(error) => {
                            self.status =
                                format!("Cannot list offline recovery snapshots: {error:#}")
                        }
                    }
                } else {
                    self.status =
                        "Interactive recovery requires an attached offline snapshot store.".into();
                }
            }
            KeyCode::F(6) => {
                if !self.document.dirty() {
                    self.status = "No unsaved FILE changes to overwrite.".into();
                } else if let Some(path) = self.document.path() {
                    self.modal = Some(Modal::Overwrite {
                        path: path.to_path_buf(),
                        error: None,
                    });
                    self.drag = None;
                } else {
                    self.status = "No opened/saved file path; use Copy NEW first.".into();
                }
            }
            _ => return false,
        }
        true
    }
}

#[cfg(test)]
pub(crate) fn limited_listing(path: &Path, limit: usize) -> Result<(Vec<Entry>, bool)> {
    list(path, limit)
}
