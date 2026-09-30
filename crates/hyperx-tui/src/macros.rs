//! Offline timeline draft state. No recording, transport or simulated inputs.
use anyhow::{bail, Context, Result};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use hyperx_app::{
    macro_keyboard_names, macro_mouse_button_names, macro_references, profile_controls,
    validate_button_binding, ProfileValueEdit,
};
use hyperx_core::{
    MacroDefinition, MacroEvent, MacroPlayback, NamedMacro, SoftwareButtonBinding, SoftwareProfile,
};
use serde::{Deserialize, Serialize};

use crate::{
    app::{App, Modal},
    editor::{Editor, EditorRecovery},
};

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub(crate) enum MacroPromptRecovery {
    Name {
        editor: EditorRecovery,
    },
    Delay {
        index: usize,
        editor: EditorRecovery,
    },
    Input {
        index: usize,
        search: EditorRecovery,
        choices: Vec<String>,
        selected: Option<usize>,
    },
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct MacroRecovery {
    pub draft: NamedMacro,
    pub original: Option<NamedMacro>,
    pub selected: usize,
    pub is_new: bool,
    pub prompt: Option<MacroPromptRecovery>,
}

#[derive(Clone, Copy)]
pub enum TextField {
    Name,
    Delay(usize),
}
#[derive(Clone, Copy)]
pub enum Confirmation {
    Discard,
    Replace,
}
pub enum Prompt {
    Info {
        text: String,
        scroll: u16,
    },
    Text {
        field: TextField,
        editor: Editor,
    },
    Input {
        index: usize,
        search: Editor,
        choices: Vec<String>,
        selected: Option<usize>,
    },
    Confirm(Confirmation),
}
pub enum Outcome {
    Stay,
    Close,
    Commit,
}

pub struct MacroEditor {
    pub draft: NamedMacro,
    pub original: Option<NamedMacro>,
    pub selected: usize,
    pub prompt: Option<Prompt>,
    pub error: Option<String>,
    confirmed: bool,
    is_new: bool,
}

pub fn event_parts(event: &MacroEvent) -> (&'static str, &str, u16) {
    match event {
        MacroEvent::KeyDown { key, delay_ms } => ("Key down", key, *delay_ms),
        MacroEvent::KeyUp { key, delay_ms } => ("Key up", key, *delay_ms),
        MacroEvent::MouseButtonDown { button, delay_ms } => ("Mouse down", button, *delay_ms),
        MacroEvent::MouseButtonUp { button, delay_ms } => ("Mouse up", button, *delay_ms),
    }
}
fn set_input(event: &mut MacroEvent, value: String) {
    match event {
        MacroEvent::KeyDown { key, .. } | MacroEvent::KeyUp { key, .. } => *key = value,
        MacroEvent::MouseButtonDown { button, .. } | MacroEvent::MouseButtonUp { button, .. } => {
            *button = value
        }
    }
}
fn set_delay(event: &mut MacroEvent, value: u16) {
    match event {
        MacroEvent::KeyDown { delay_ms, .. }
        | MacroEvent::KeyUp { delay_ms, .. }
        | MacroEvent::MouseButtonDown { delay_ms, .. }
        | MacroEvent::MouseButtonUp { delay_ms, .. } => *delay_ms = value,
    }
}

impl MacroEditor {
    pub(crate) fn recovery(&self) -> MacroRecovery {
        let prompt = match &self.prompt {
            Some(Prompt::Text {
                field: TextField::Name,
                editor,
            }) => Some(MacroPromptRecovery::Name {
                editor: editor.recovery(),
            }),
            Some(Prompt::Text {
                field: TextField::Delay(index),
                editor,
            }) => Some(MacroPromptRecovery::Delay {
                index: *index,
                editor: editor.recovery(),
            }),
            Some(Prompt::Input {
                index,
                search,
                choices,
                selected,
            }) => Some(MacroPromptRecovery::Input {
                index: *index,
                search: search.recovery(),
                choices: choices.clone(),
                selected: *selected,
            }),
            Some(Prompt::Info { .. } | Prompt::Confirm(_)) | None => None,
        };
        MacroRecovery {
            draft: self.draft.clone(),
            original: self.original.clone(),
            selected: self.selected,
            is_new: self.is_new,
            prompt,
        }
    }

    pub(crate) fn from_recovery(saved: MacroRecovery) -> Result<Self> {
        if saved.is_new != saved.original.is_none() {
            bail!("local macro snapshot has inconsistent new/existing identity");
        }
        if saved
            .original
            .as_ref()
            .is_some_and(|original| original.source_id != saved.draft.source_id)
        {
            bail!("local macro snapshot changed its source identifier");
        }
        if saved.selected > saved.draft.definition.events.len().saturating_sub(1) {
            bail!("local macro snapshot has an invalid selected event");
        }
        let prompt = match saved.prompt {
            Some(MacroPromptRecovery::Name { editor }) => Some(Prompt::Text {
                field: TextField::Name,
                editor: Editor::from_recovery(editor)?,
            }),
            Some(MacroPromptRecovery::Delay { index, editor }) => {
                if index >= saved.draft.definition.events.len() {
                    bail!("local macro snapshot names a missing delay row");
                }
                Some(Prompt::Text {
                    field: TextField::Delay(index),
                    editor: Editor::from_recovery(editor)?,
                })
            }
            Some(MacroPromptRecovery::Input {
                index,
                search,
                choices,
                selected,
            }) => {
                if index >= saved.draft.definition.events.len()
                    || selected.is_some_and(|value| value >= choices.len())
                    || choices.len() > 512
                {
                    bail!("local macro snapshot has invalid input choices");
                }
                Some(Prompt::Input {
                    index,
                    search: Editor::from_recovery(search)?,
                    choices,
                    selected,
                })
            }
            None => None,
        };
        Ok(Self {
            draft: saved.draft,
            original: saved.original,
            selected: saved.selected,
            prompt,
            error: None,
            confirmed: false,
            is_new: saved.is_new,
        })
    }

    fn existing(definition: NamedMacro) -> Self {
        Self {
            draft: definition.clone(),
            original: Some(definition),
            selected: 0,
            prompt: None,
            error: None,
            confirmed: false,
            is_new: false,
        }
    }
    fn new(profile: &SoftwareProfile) -> Self {
        // Reserve broken/imported references as well as existing definitions.
        // ID generation is file-local, deterministic, and never a vendor slot.
        let mut number = 1u64;
        let id = loop {
            let id = format!("macro-{number}");
            if !profile.macros.iter().any(|entry| entry.source_id == id)
                && macro_references(profile, &id).is_empty()
            {
                break id;
            }
            number += 1;
        };
        Self {
            draft: NamedMacro {
                source_id: id,
                name: "New macro".into(),
                definition: MacroDefinition {
                    playback: MacroPlayback::Once,
                    events: Vec::new(),
                },
            },
            original: None,
            selected: 0,
            prompt: None,
            error: None,
            confirmed: false,
            is_new: true,
        }
    }
    pub fn dirty(&self) -> bool {
        self.is_new || self.original.as_ref() != Some(&self.draft)
    }
    fn changed(&mut self) {
        self.confirmed = false;
        self.error = None;
    }
    fn fresh(&self, profile: &SoftwareProfile) -> Result<()> {
        if let Some(original) = &self.original {
            let definitions: Vec<_> = profile
                .macros
                .iter()
                .filter(|entry| entry.source_id == original.source_id)
                .collect();
            if definitions.len() != 1 || definitions[0] != original {
                bail!("definition changed while editing; local draft retained, discard and reopen");
            }
        }
        Ok(())
    }
    pub fn edit(&self, profile: &SoftwareProfile) -> Result<ProfileValueEdit> {
        self.fresh(profile)?;
        Ok(if self.is_new {
            ProfileValueEdit::MacroCreate {
                definition: self.draft.clone(),
            }
        } else {
            ProfileValueEdit::MacroReplace {
                source_id: self.draft.source_id.clone(),
                definition: self.draft.clone(),
                confirm_references: self.confirmed,
            }
        })
    }
    pub fn text_editor_mut(&mut self) -> Option<&mut Editor> {
        match &mut self.prompt {
            Some(Prompt::Text { editor, .. }) => Some(editor),
            Some(Prompt::Input { search, .. }) => Some(search),
            _ => None,
        }
    }
    pub fn paste(&mut self, text: &str) {
        if let Some(editor) = self.text_editor_mut() {
            editor.paste(text);
            self.filter_changed();
        }
    }
    fn filter_changed(&mut self) {
        if let Some(Prompt::Input {
            search,
            choices,
            selected,
            ..
        }) = &mut self.prompt
        {
            let query = search.text().to_lowercase();
            if selected.is_some_and(|index| !choices[index].to_lowercase().contains(&query)) {
                *selected = None;
            }
        }
        self.error = None;
    }
    pub fn input_indices(&self) -> Vec<usize> {
        let Some(Prompt::Input {
            search, choices, ..
        }) = &self.prompt
        else {
            return Vec::new();
        };
        let query = search.text().to_lowercase();
        choices
            .iter()
            .enumerate()
            .filter_map(|(index, choice)| choice.to_lowercase().contains(&query).then_some(index))
            .collect()
    }
    pub fn select_input(&mut self, index: usize) {
        if self.input_indices().contains(&index) {
            if let Some(Prompt::Input { selected, .. }) = &mut self.prompt {
                *selected = Some(index);
                self.error = None;
            }
        }
    }
    pub fn move_selection(&mut self, delta: i32) {
        if let Some(Prompt::Info { scroll, .. }) = &mut self.prompt {
            *scroll = (i64::from(*scroll) + i64::from(delta)).clamp(0, i64::from(u16::MAX)) as u16;
        } else if let Some(Prompt::Input { selected, .. }) = &self.prompt {
            let indices = self.input_indices();
            if indices.is_empty() {
                return;
            }
            let next = selected
                .and_then(|index| indices.iter().position(|value| *value == index))
                .map_or(if delta < 0 { indices.len() - 1 } else { 0 }, |position| {
                    (position as i64 + i64::from(delta)).clamp(0, indices.len() as i64 - 1) as usize
                });
            self.select_input(indices[next]);
        } else if self.prompt.is_none() {
            self.selected = (self.selected as i64 + i64::from(delta)).clamp(
                0,
                self.draft.definition.events.len().saturating_sub(1) as i64,
            ) as usize;
        }
    }
    fn apply_prompt(&mut self, prompt: &Prompt) -> Result<()> {
        match prompt {
            Prompt::Text {
                field: TextField::Name,
                editor,
            } => self.draft.name = editor.text(),
            Prompt::Text {
                field: TextField::Delay(index),
                editor,
            } => {
                let delay = editor.text().trim().parse::<u16>().context(
                    "file delay must be an integer 0..65535 ms; device limits are separate",
                )?;
                set_delay(
                    self.draft
                        .definition
                        .events
                        .get_mut(*index)
                        .context("select an existing event")?,
                    delay,
                );
            }
            Prompt::Input {
                index,
                choices,
                selected,
                ..
            } => {
                let choice = selected
                    .and_then(|index| choices.get(index))
                    .context("choose an explicit named input first")?;
                set_input(
                    self.draft
                        .definition
                        .events
                        .get_mut(*index)
                        .context("select an existing event")?,
                    choice.clone(),
                );
            }
            Prompt::Confirm(_) | Prompt::Info { .. } => {
                unreachable!("information/confirmation has a separate explicit transition")
            }
        }
        self.changed();
        Ok(())
    }
    fn reorder(&mut self, direction: i32) {
        let destination = self.selected as i64 + i64::from(direction);
        if destination >= 0 && (destination as usize) < self.draft.definition.events.len() {
            self.draft
                .definition
                .events
                .swap(self.selected, destination as usize);
            self.selected = destination as usize;
            self.changed();
        }
    }
    pub fn key(&mut self, key: KeyEvent, profile: &SoftwareProfile) -> Outcome {
        let accept = key.code == KeyCode::Enter
            || (key.code == KeyCode::Char('s') && key.modifiers.contains(KeyModifiers::CONTROL));
        if let Some(mut prompt) = self.prompt.take() {
            if let Prompt::Info { scroll, .. } = &mut prompt {
                match key.code {
                    KeyCode::Esc => return Outcome::Stay,
                    KeyCode::Home => *scroll = 0,
                    KeyCode::Up => *scroll = scroll.saturating_sub(1),
                    KeyCode::Down => *scroll = scroll.saturating_add(1),
                    KeyCode::PageUp => *scroll = scroll.saturating_sub(8),
                    KeyCode::PageDown => *scroll = scroll.saturating_add(8),
                    _ => {}
                }
                self.prompt = Some(prompt);
                return Outcome::Stay;
            }
            if let Prompt::Confirm(reason) = prompt {
                if key.code == KeyCode::Char('y') {
                    return match reason {
                        Confirmation::Discard => Outcome::Close,
                        Confirmation::Replace => {
                            self.confirmed = true;
                            Outcome::Commit
                        }
                    };
                }
                if matches!(key.code, KeyCode::Esc | KeyCode::Char('n')) {
                    return Outcome::Stay;
                }
                self.prompt = Some(Prompt::Confirm(reason));
                return Outcome::Stay;
            }
            if key.code == KeyCode::Esc {
                self.error = None;
                return Outcome::Stay;
            }
            if accept {
                match self.apply_prompt(&prompt) {
                    Ok(()) => return Outcome::Stay,
                    Err(error) => self.error = Some(format!("{error:#}")),
                }
            } else {
                match &mut prompt {
                    Prompt::Text { editor, .. } => editor.key(key),
                    Prompt::Input { search, .. } => {
                        if matches!(
                            key.code,
                            KeyCode::Up
                                | KeyCode::Down
                                | KeyCode::PageUp
                                | KeyCode::PageDown
                                | KeyCode::Home
                                | KeyCode::End
                        ) {
                            self.prompt = Some(prompt);
                            match key.code {
                                KeyCode::Home => {
                                    if let Some(index) = self.input_indices().first() {
                                        self.select_input(*index);
                                    }
                                }
                                KeyCode::End => {
                                    if let Some(index) = self.input_indices().last() {
                                        self.select_input(*index);
                                    }
                                }
                                _ => self.move_selection(match key.code {
                                    KeyCode::Up => -1,
                                    KeyCode::Down => 1,
                                    KeyCode::PageUp => -8,
                                    _ => 8,
                                }),
                            }
                            return Outcome::Stay;
                        }
                        search.key(key);
                    }
                    Prompt::Confirm(_) | Prompt::Info { .. } => unreachable!(),
                }
                self.error = None;
            }
            self.prompt = Some(prompt);
            if !accept {
                self.filter_changed();
            }
            return Outcome::Stay;
        }
        if key.code == KeyCode::Char('s') && key.modifiers.contains(KeyModifiers::CONTROL) {
            if let Err(error) = self.fresh(profile) {
                self.error = Some(error.to_string());
                return Outcome::Stay;
            }
            if !self.is_new
                && self.dirty()
                && !self.confirmed
                && !macro_references(profile, &self.draft.source_id).is_empty()
            {
                self.prompt = Some(Prompt::Confirm(Confirmation::Replace));
                return Outcome::Stay;
            }
            return Outcome::Commit;
        }
        match key.code {
            KeyCode::F(9) => {
                self.prompt = Some(Prompt::Info {
                    text: self.preflight(profile),
                    scroll: 0,
                })
            }
            KeyCode::Esc => {
                if self.dirty() {
                    self.prompt = Some(Prompt::Confirm(Confirmation::Discard));
                } else {
                    return Outcome::Close;
                }
            }
            KeyCode::Up | KeyCode::Down if key.modifiers.contains(KeyModifiers::SHIFT) => {
                self.reorder(if key.code == KeyCode::Up { -1 } else { 1 })
            }
            KeyCode::F(7) => self.reorder(-1),
            KeyCode::F(8) => self.reorder(1),
            KeyCode::Up => self.move_selection(-1),
            KeyCode::Down => self.move_selection(1),
            KeyCode::PageUp => self.move_selection(-8),
            KeyCode::PageDown => self.move_selection(8),
            KeyCode::Home => self.selected = 0,
            KeyCode::End => self.selected = self.draft.definition.events.len().saturating_sub(1),
            KeyCode::F(2) => {
                self.prompt = Some(Prompt::Text {
                    field: TextField::Name,
                    editor: Editor::new(&self.draft.name, true),
                })
            }
            KeyCode::F(3) => {
                self.draft.definition.playback = match self.draft.definition.playback {
                    MacroPlayback::Once => MacroPlayback::ToggleRepeat,
                    MacroPlayback::ToggleRepeat => MacroPlayback::RepeatWhileHeld,
                    MacroPlayback::RepeatWhileHeld => MacroPlayback::Once,
                };
                self.changed();
            }
            KeyCode::Insert => {
                // New rows require explicit input selection; do not invent 'A'.
                self.draft.definition.events.push(MacroEvent::KeyDown {
                    key: String::new(),
                    delay_ms: 0,
                });
                self.selected = self.draft.definition.events.len() - 1;
                self.changed();
            }
            KeyCode::Delete if !self.draft.definition.events.is_empty() => {
                self.draft.definition.events.remove(self.selected);
                self.selected = self
                    .selected
                    .min(self.draft.definition.events.len().saturating_sub(1));
                self.changed();
            }
            KeyCode::F(4) => {
                if let Some(event) = self.draft.definition.events.get_mut(self.selected) {
                    let (_, value, delay_ms) = event_parts(event);
                    let value = value.to_owned();
                    *event = match event {
                        MacroEvent::KeyDown { .. } => MacroEvent::KeyUp {
                            key: value,
                            delay_ms,
                        },
                        MacroEvent::KeyUp { .. } => MacroEvent::MouseButtonDown {
                            button: String::new(),
                            delay_ms,
                        },
                        MacroEvent::MouseButtonDown { .. } => MacroEvent::MouseButtonUp {
                            button: value,
                            delay_ms,
                        },
                        MacroEvent::MouseButtonUp { .. } => MacroEvent::KeyDown {
                            key: String::new(),
                            delay_ms,
                        },
                    };
                    self.changed();
                }
            }
            KeyCode::F(5) | KeyCode::Enter => {
                if let Some(event) = self.draft.definition.events.get(self.selected) {
                    let keyboard =
                        matches!(event, MacroEvent::KeyDown { .. } | MacroEvent::KeyUp { .. });
                    let mut choices = if keyboard {
                        macro_keyboard_names(&profile.device)
                    } else {
                        macro_mouse_button_names(&profile.device)
                    };
                    let (_, value, _) = event_parts(event);
                    if !value.is_empty() && !choices.iter().any(|choice| choice == value) {
                        choices.insert(0, value.into());
                    }
                    let selected = choices.iter().position(|choice| choice == value);
                    self.prompt = Some(Prompt::Input {
                        index: self.selected,
                        search: Editor::new("", true),
                        choices,
                        selected,
                    });
                }
            }
            KeyCode::F(6) => {
                if let Some(event) = self.draft.definition.events.get(self.selected) {
                    self.prompt = Some(Prompt::Text {
                        field: TextField::Delay(self.selected),
                        editor: Editor::new(&event_parts(event).2.to_string(), true),
                    });
                }
            }
            _ => {}
        }
        Outcome::Stay
    }
    pub fn preflight(&self, profile: &SoftwareProfile) -> String {
        let mut text = "LOCAL timeline: offline encoder preflight, not hardware verification.\nFile delays are 0..65535 ms; capture-backed device limits are separate.\n\n".to_owned();
        let mut isolated = profile.clone();
        isolated
            .macros
            .retain(|value| value.source_id != self.draft.source_id);
        isolated.macros.push(self.draft.clone());
        for control in profile_controls(&profile.device) {
            if let Some(caps) = control.macros {
                let modes = |values: &[MacroPlayback]| {
                    values
                        .iter()
                        .map(ToString::to_string)
                        .collect::<Vec<_>>()
                        .join(", ")
                };
                let result = validate_button_binding(
                    &isolated,
                    control.id,
                    &SoftwareButtonBinding::Macro {
                        id: self.draft.source_id.clone(),
                    },
                )
                .map_or_else(
                    |error| format!("Rejected: {error}"),
                    |()| {
                        "Runtime encoder accepts this timeline; no device was opened or tested."
                            .into()
                    },
                );
                text.push_str(&format!("{}\nRuntime modes: {}\nOnboard modes: {}\nCapture-backed encoder: {} events; 0..{} ms per event.\n{result}\n\n", control.name, modes(caps.runtime_playback), modes(caps.onboard_playback), caps.max_events, caps.max_delay_ms));
            }
        }
        text.push_str("Empty/unbalanced/unsupported timelines may remain file drafts. Saving does not grant permission to assign/apply them. No firmware/USB operations exist here.");
        text
    }
}

impl App {
    pub(crate) fn macros_key(&mut self, key: KeyEvent) -> bool {
        if self.tab != 2 || self.modal.is_some() {
            return false;
        }
        let count = self.document.profile().macros.len();
        match key.code {
            KeyCode::Up => self.selected_macro = self.selected_macro.saturating_sub(1),
            KeyCode::Down => {
                self.selected_macro = (self.selected_macro + 1).min(count.saturating_sub(1))
            }
            KeyCode::Home => self.selected_macro = 0,
            KeyCode::End => self.selected_macro = count.saturating_sub(1),
            KeyCode::Enter | KeyCode::F(2) => self.open_macro(false),
            KeyCode::Insert | KeyCode::Char('n') => self.open_macro(true),
            KeyCode::Delete => {
                if let Some(original) = self.document.profile().macros.get(self.selected_macro) {
                    if !macro_references(self.document.profile(), &original.source_id).is_empty() {
                        self.status = "Cannot delete a referenced macro; remove physical/unresolved assignments explicitly first.".into();
                    } else {
                        self.modal = Some(Modal::MacroDelete {
                            original: original.clone(),
                            error: None,
                        });
                    }
                }
            }
            _ => return false,
        }
        true
    }
    pub(crate) fn open_macro(&mut self, create: bool) {
        let profile = self.document.profile();
        let editor = if create {
            MacroEditor::new(profile)
        } else {
            let Some(definition) = profile.macros.get(self.selected_macro) else {
                return;
            };
            if profile
                .macros
                .iter()
                .filter(|entry| entry.source_id == definition.source_id)
                .count()
                != 1
            {
                self.status =
                    "Duplicate macro ID: inspect advanced TOML; no implicit replacement.".into();
                return;
            }
            MacroEditor::existing(definition.clone())
        };
        self.modal = Some(Modal::Macro(Box::new(editor)));
        self.drag = None;
    }
    pub(crate) fn remove_macro(&mut self, original: &NamedMacro) -> Result<()> {
        let definitions: Vec<_> = self
            .document
            .profile()
            .macros
            .iter()
            .filter(|entry| entry.source_id == original.source_id)
            .collect();
        if definitions.len() != 1 || definitions[0] != original {
            bail!("definition changed while confirmation was pending; cancel and reopen");
        }
        self.edit_value(ProfileValueEdit::MacroRemove {
            source_id: original.source_id.clone(),
        })?;
        self.selected_macro = self
            .selected_macro
            .min(self.document.profile().macros.len().saturating_sub(1));
        Ok(())
    }
}
