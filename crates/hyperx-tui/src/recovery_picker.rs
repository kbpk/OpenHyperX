//! Explicit management of offline TUI draft snapshots. This never opens HID.
use std::path::PathBuf;

use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent};
use hyperx_app::DraftRecoveryStore;

use crate::local_recovery::LocalRecoveryStore;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EntryKind {
    FileDraft,
    LocalEditor,
}

#[derive(Clone, Debug)]
pub struct Entry {
    pub kind: EntryKind,
    pub label: String,
    pub path: PathBuf,
    pub profile: Option<String>,
    pub original_file: Option<PathBuf>,
    pub error: Option<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Confirmation {
    Restore,
    Discard,
}

pub enum Outcome {
    Stay,
    Close,
    Refresh,
    Restore(EntryKind, PathBuf),
    Discard(EntryKind, PathBuf),
}

pub struct RecoveryPicker {
    pub entries: Vec<Entry>,
    pub selected: usize,
    pub confirmation: Option<Confirmation>,
    pub error: Option<String>,
}

impl RecoveryPicker {
    pub fn new(store: &DraftRecoveryStore, local: &LocalRecoveryStore) -> Result<Self> {
        let mut picker = Self {
            entries: Vec::new(),
            selected: 0,
            confirmation: None,
            error: None,
        };
        picker.refresh(store, local)?;
        Ok(picker)
    }

    pub fn refresh(
        &mut self,
        store: &DraftRecoveryStore,
        local: &LocalRecoveryStore,
    ) -> Result<()> {
        let selected_path = self
            .entries
            .get(self.selected)
            .map(|entry| entry.path.clone());
        let mut entries: Vec<Entry> = store
            .list()?
            .into_iter()
            .map(|path| match store.load(&path) {
                Ok(document) => Entry {
                    kind: EntryKind::FileDraft,
                    label: "FILE draft".into(),
                    path,
                    profile: Some(document.profile().name.clone()),
                    original_file: document.path().map(ToOwned::to_owned),
                    error: None,
                },
                Err(error) => Entry {
                    kind: EntryKind::FileDraft,
                    label: "FILE draft".into(),
                    path,
                    profile: None,
                    original_file: None,
                    error: Some(format!("{error:#}")),
                },
            })
            .collect();
        entries.extend(
            local
                .list()?
                .into_iter()
                .map(|path| match local.load(&path) {
                    Ok(record) => Entry {
                        kind: EntryKind::LocalEditor,
                        label: record.kind().into(),
                        profile: Some(record.profile_name().into()),
                        original_file: record.source_path().map(ToOwned::to_owned),
                        error: None,
                        path,
                    },
                    Err(error) => Entry {
                        kind: EntryKind::LocalEditor,
                        label: "local editor".into(),
                        profile: None,
                        original_file: None,
                        error: Some(format!("{error:#}")),
                        path,
                    },
                }),
        );
        entries.sort_by(|left, right| left.path.cmp(&right.path));
        self.entries = entries;
        self.selected = selected_path
            .and_then(|path| self.entries.iter().position(|entry| entry.path == path))
            .unwrap_or_else(|| self.selected.min(self.entries.len().saturating_sub(1)));
        self.error = None;
        self.confirmation = None;
        Ok(())
    }

    pub fn select(&mut self, index: usize) {
        if index < self.entries.len() && self.confirmation.is_none() {
            self.selected = index;
            self.error = None;
        }
    }

    pub fn move_selection(&mut self, delta: i32) {
        if self.confirmation.is_none() {
            self.selected = (self.selected as i64 + i64::from(delta))
                .clamp(0, self.entries.len().saturating_sub(1) as i64)
                as usize;
            self.error = None;
        }
    }

    pub fn key(&mut self, key: KeyEvent) -> Outcome {
        if let Some(reason) = self.confirmation {
            return match key.code {
                KeyCode::Char('y') => {
                    self.confirmation = None;
                    let Some(entry) = self.entries.get(self.selected) else {
                        return Outcome::Stay;
                    };
                    match reason {
                        Confirmation::Restore => Outcome::Restore(entry.kind, entry.path.clone()),
                        Confirmation::Discard => Outcome::Discard(entry.kind, entry.path.clone()),
                    }
                }
                KeyCode::Char('n') | KeyCode::Esc => {
                    self.confirmation = None;
                    Outcome::Stay
                }
                _ => Outcome::Stay,
            };
        }
        match key.code {
            KeyCode::Esc => Outcome::Close,
            KeyCode::F(5) => Outcome::Refresh,
            KeyCode::Up => {
                self.move_selection(-1);
                Outcome::Stay
            }
            KeyCode::Down => {
                self.move_selection(1);
                Outcome::Stay
            }
            KeyCode::PageUp => {
                self.move_selection(-8);
                Outcome::Stay
            }
            KeyCode::PageDown => {
                self.move_selection(8);
                Outcome::Stay
            }
            KeyCode::Home => {
                self.selected = 0;
                Outcome::Stay
            }
            KeyCode::End => {
                self.selected = self.entries.len().saturating_sub(1);
                Outcome::Stay
            }
            KeyCode::Char('r') | KeyCode::Enter => {
                if let Some(entry) = self.entries.get(self.selected) {
                    if entry.error.is_some() {
                        self.error = Some(
                            "Unreadable snapshot cannot be restored; inspect it before discarding."
                                .into(),
                        );
                    } else {
                        self.confirmation = Some(Confirmation::Restore);
                    }
                }
                Outcome::Stay
            }
            KeyCode::Char('d') => {
                if self.entries.get(self.selected).is_some() {
                    self.confirmation = Some(Confirmation::Discard);
                }
                Outcome::Stay
            }
            _ => Outcome::Stay,
        }
    }
}
