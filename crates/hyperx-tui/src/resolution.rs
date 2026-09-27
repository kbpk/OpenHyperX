//! Explicit provenance-to-library selection; all changes remain offline files.
use anyhow::{bail, Context, Result};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use hyperx_app::{macro_resolution_targets, omit_unresolved_assignment, resolve_macro_assignment};
use hyperx_core::{NamedMacro, SoftwareProfile, UnresolvedButtonAssignment};

use crate::editor::Editor;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Stage {
    Source,
    Target,
    Macro,
    Review,
    Omit,
}
pub enum Value {
    Source(UnresolvedButtonAssignment),
    Target(String),
    Macro(NamedMacro),
}
pub struct Choice {
    pub label: String,
    pub value: Value,
    pub error: Option<String>,
}
pub enum Outcome {
    Stay,
    Close,
    Commit,
}

pub struct ResolutionPicker {
    pub stage: Stage,
    pub choices: Vec<Choice>,
    pub selected: Option<usize>,
    pub search: Editor,
    pub source: Option<UnresolvedButtonAssignment>,
    pub target: Option<String>,
    pub definition: Option<NamedMacro>,
    pub error: Option<String>,
    pub scroll: u16,
    pub info: Option<String>,
}

impl ResolutionPicker {
    pub fn new(profile: &SoftwareProfile) -> Self {
        let mut picker = Self {
            stage: Stage::Source,
            choices: Vec::new(),
            selected: None,
            search: Editor::new("", true),
            source: None,
            target: None,
            definition: None,
            error: None,
            scroll: 0,
            info: None,
        };
        picker.sources(profile);
        picker
    }
    pub fn reviewing(&self) -> bool {
        matches!(self.stage, Stage::Review | Stage::Omit)
    }
    pub fn viewing(&self) -> bool {
        self.reviewing() || self.info.is_some()
    }
    fn reset_choices(&mut self, stage: Stage, choices: Vec<Choice>) {
        self.stage = stage;
        self.choices = choices;
        self.selected = None;
        self.search = Editor::new("", true);
        self.error = None;
        self.scroll = 0;
        self.info = None;
    }
    fn sources(&mut self, profile: &SoftwareProfile) {
        let mut counts = std::collections::HashMap::new();
        for source in &profile.unresolved_button_assignments {
            *counts.entry(source.source_id.as_str()).or_insert(0usize) += 1;
        }
        let choices = profile.unresolved_button_assignments.iter().map(|source| {
            let duplicate = counts[source.source_id.as_str()] != 1;
            Choice { label: format!("{} | macro source: {}", source.source_id,
                source.macro_source_id.as_deref().unwrap_or("<unknown>")),
                value: Value::Source(source.clone()),
                error: duplicate.then(|| "Duplicate source ID: inspect advanced TOML; no implicit resolution/removal.".into()) }
        }).collect();
        self.source = None;
        self.target = None;
        self.definition = None;
        self.reset_choices(Stage::Source, choices);
    }
    fn fresh_source(&self, profile: &SoftwareProfile) -> Result<&UnresolvedButtonAssignment> {
        let source = self
            .source
            .as_ref()
            .context("choose an explicit unresolved source first")?;
        let entries: Vec<_> = profile
            .unresolved_button_assignments
            .iter()
            .filter(|entry| entry.source_id == source.source_id)
            .collect();
        if entries.len() != 1 || entries[0] != source {
            bail!(
                "source changed or became ambiguous; file retained, cancel and reopen the selector"
            );
        }
        Ok(source)
    }
    fn targets(&mut self, profile: &SoftwareProfile) -> Result<()> {
        let source = self.fresh_source(profile)?;
        let choices = macro_resolution_targets(profile, &source.source_id)?
            .into_iter()
            .map(|entry| Choice {
                label: format!("{} ({})", entry.control.name, entry.control.id),
                value: Value::Target(entry.control.id.into()),
                error: entry.error,
            })
            .collect();
        self.target = None;
        self.definition = None;
        self.reset_choices(Stage::Target, choices);
        Ok(())
    }
    fn macros(&mut self, profile: &SoftwareProfile) -> Result<()> {
        let source = self.fresh_source(profile)?;
        let target = self
            .target
            .as_deref()
            .context("choose an explicit target first")?;
        let choices = profile
            .macros
            .iter()
            .map(|definition| {
                let error = resolve_macro_assignment(
                    profile,
                    &source.source_id,
                    target,
                    &definition.source_id,
                    None,
                )
                .err()
                .map(|error| format!("{error:#}"));
                Choice {
                    label: format!(
                        "{} | ID {} | {} | {} events",
                        definition.name,
                        definition.source_id,
                        definition.definition.playback,
                        definition.definition.events.len()
                    ),
                    value: Value::Macro(definition.clone()),
                    error,
                }
            })
            .collect();
        self.definition = None;
        self.reset_choices(Stage::Macro, choices);
        Ok(())
    }
    pub fn filtered(&self) -> Vec<usize> {
        let query = self.search.text().to_lowercase();
        self.choices
            .iter()
            .enumerate()
            .filter_map(|(index, choice)| {
                choice
                    .label
                    .to_lowercase()
                    .contains(&query)
                    .then_some(index)
            })
            .collect()
    }
    pub fn filter_changed(&mut self) {
        if self
            .selected
            .is_some_and(|index| !self.filtered().contains(&index))
        {
            self.selected = None;
        }
        self.error = None;
    }
    pub fn select(&mut self, index: usize) {
        if !self.viewing() && self.filtered().contains(&index) {
            self.selected = Some(index);
            self.error = None;
        }
    }
    pub fn move_selection(&mut self, delta: i32) {
        if self.viewing() {
            self.scroll =
                (i64::from(self.scroll) + i64::from(delta)).clamp(0, i64::from(u16::MAX)) as u16;
            return;
        }
        let indices = self.filtered();
        if indices.is_empty() {
            return;
        }
        let position = self
            .selected
            .and_then(|index| indices.iter().position(|entry| *entry == index))
            .map_or(if delta < 0 { indices.len() - 1 } else { 0 }, |position| {
                (position as i64 + i64::from(delta)).clamp(0, indices.len() as i64 - 1) as usize
            });
        self.select(indices[position]);
    }
    pub fn editor_mut(&mut self) -> Option<&mut Editor> {
        (!self.viewing()).then_some(&mut self.search)
    }
    pub fn paste(&mut self, text: &str) {
        if let Some(editor) = self.editor_mut() {
            editor.paste(text);
            self.filter_changed();
        }
    }
    pub fn edited(&self, profile: &SoftwareProfile) -> Result<SoftwareProfile> {
        let source = self.fresh_source(profile)?;
        if self.stage == Stage::Omit {
            return omit_unresolved_assignment(profile, &source.source_id);
        }
        let definition = self
            .definition
            .as_ref()
            .context("choose a real library definition first")?;
        let definitions: Vec<_> = profile
            .macros
            .iter()
            .filter(|entry| entry.source_id == definition.source_id)
            .collect();
        if definitions.len() != 1 || definitions[0] != definition {
            bail!("selected macro changed or became ambiguous; file retained, go back and select it again");
        }
        resolve_macro_assignment(
            profile,
            &source.source_id,
            self.target.as_deref().context("choose a target first")?,
            &definition.source_id,
            None,
        )
    }
    fn next(&mut self, profile: &SoftwareProfile) -> Result<()> {
        let choice = self
            .selected
            .and_then(|index| self.choices.get(index))
            .context("select an explicit preview first; typing a search never selects a result")?;
        if let Some(error) = &choice.error {
            bail!("{error}");
        }
        match &choice.value {
            Value::Source(source) => {
                self.source = Some(source.clone());
                self.targets(profile)?;
            }
            Value::Target(target) => {
                self.target = Some(target.clone());
                self.macros(profile)?;
            }
            Value::Macro(definition) => {
                self.definition = Some(definition.clone());
                self.edited(profile)?;
                self.stage = Stage::Review;
                self.scroll = 0;
                self.error = None;
            }
        }
        Ok(())
    }
    fn omit(&mut self, profile: &SoftwareProfile) -> Result<()> {
        let choice = self
            .selected
            .and_then(|index| self.choices.get(index))
            .context("select the exact source to omit first")?;
        if let Some(error) = &choice.error {
            bail!("{error}");
        }
        let Value::Source(source) = &choice.value else {
            bail!("omission starts from the source list");
        };
        self.source = Some(source.clone());
        self.fresh_source(profile)?;
        self.stage = Stage::Omit;
        self.scroll = 0;
        self.error = None;
        Ok(())
    }
    pub fn key(&mut self, key: KeyEvent, profile: &SoftwareProfile) -> Outcome {
        if self.info.is_some() {
            match key.code {
                KeyCode::Esc => {
                    self.info = None;
                    self.scroll = 0;
                }
                KeyCode::Home => self.scroll = 0,
                KeyCode::Up => self.move_selection(-1),
                KeyCode::Down => self.move_selection(1),
                KeyCode::PageUp => self.move_selection(-8),
                KeyCode::PageDown => self.move_selection(8),
                _ => {}
            }
            return Outcome::Stay;
        }
        if key.code == KeyCode::Esc || (self.reviewing() && key.code == KeyCode::Char('n')) {
            let result = match self.stage {
                Stage::Source => return Outcome::Close,
                Stage::Target | Stage::Omit => {
                    self.sources(profile);
                    Ok(())
                }
                Stage::Macro => self.targets(profile),
                Stage::Review => self.macros(profile),
            };
            if let Err(error) = result {
                self.sources(profile);
                self.error = Some(format!("{error:#}"));
            }
            return Outcome::Stay;
        }
        if self.reviewing() {
            if key.code == KeyCode::Char('y')
                || (key.code == KeyCode::Char('s') && key.modifiers.contains(KeyModifiers::CONTROL))
            {
                return Outcome::Commit;
            }
            match key.code {
                KeyCode::Home => self.scroll = 0,
                KeyCode::Up => self.move_selection(-1),
                KeyCode::Down => self.move_selection(1),
                KeyCode::PageUp => self.move_selection(-8),
                KeyCode::PageDown => self.move_selection(8),
                _ => {}
            }
            return Outcome::Stay;
        }
        match key.code {
            KeyCode::F(1) => {
                self.info = Some(self.selected.and_then(|index| self.choices.get(index))
                    .map_or_else(|| self.error.clone().unwrap_or_else(|| "Choose a preview to inspect its complete reason. No profile edits or HID access.".into()),
                        |choice| format!("{}\n\n{}\n\nSelection metadata/encoder checks only; no hardware verification. Esc returns to unchanged choices.", choice.label,
                            self.error.as_deref().or(choice.error.as_deref()).unwrap_or("This choice is available; final resolution still validates the complete source/target/timeline."))));
                self.scroll = 0;
            }
            KeyCode::Enter => {
                if let Err(error) = self.next(profile) {
                    self.error = Some(format!("{error:#}"));
                }
            }
            KeyCode::Delete if self.stage == Stage::Source => {
                if let Err(error) = self.omit(profile) {
                    self.error = Some(format!("{error:#}"));
                }
            }
            KeyCode::Up => self.move_selection(-1),
            KeyCode::Down => self.move_selection(1),
            KeyCode::PageUp => self.move_selection(-8),
            KeyCode::PageDown => self.move_selection(8),
            KeyCode::Home => {
                if let Some(index) = self.filtered().first() {
                    self.select(*index);
                }
            }
            KeyCode::End => {
                if let Some(index) = self.filtered().last() {
                    self.select(*index);
                }
            }
            _ => {
                self.search.key(key);
                self.filter_changed();
            }
        }
        Outcome::Stay
    }
}
