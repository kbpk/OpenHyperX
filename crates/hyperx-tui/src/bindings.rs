//! Semantic file-binding selector. Choices and target validation belong to the
//! app layer; picking a row is only a preview until explicit acceptance.
use anyhow::{bail, Context, Result};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use hyperx_app::{profile_binding_choices, profile_controls, ProfileValueEdit};
use hyperx_core::{SoftwareButtonBinding, SoftwareProfile};
use ratatui::{
    layout::{Margin, Rect},
    style::{Color, Style},
    widgets::{Block, Borders, Paragraph, Wrap},
    Frame,
};

use crate::{
    app::{App, Modal},
    editor::Editor,
    render::safe_text,
    widgets::{sub, Action, Hit},
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Category {
    Omit,
    Mouse,
    Multimedia,
    Keyboard,
    Shortcut,
    Macro,
    Disabled,
}
impl Category {
    fn label(self) -> &'static str {
        match self {
            Self::Omit => "Not specified (preserve)",
            Self::Mouse => "Mouse function",
            Self::Multimedia => "Multimedia",
            Self::Keyboard => "Keyboard",
            Self::Shortcut => "Windows shortcut",
            Self::Macro => "Library macro",
            Self::Disabled => "Disabled",
        }
    }
    fn for_binding(binding: Option<&SoftwareButtonBinding>) -> Self {
        match binding {
            None => Self::Omit,
            Some(SoftwareButtonBinding::Mouse { .. }) => Self::Mouse,
            Some(SoftwareButtonBinding::Multimedia { .. }) => Self::Multimedia,
            Some(SoftwareButtonBinding::Keyboard { .. }) => Self::Keyboard,
            Some(SoftwareButtonBinding::WindowsShortcut { .. }) => Self::Shortcut,
            Some(SoftwareButtonBinding::Macro { .. }) => Self::Macro,
            Some(SoftwareButtonBinding::Disabled {}) => Self::Disabled,
        }
    }
}

struct Choice {
    label: String,
    binding: Option<SoftwareButtonBinding>,
    error: Option<String>,
}

pub struct BindingPicker {
    pub control: String,
    current: String,
    limits: String,
    categories: Vec<Category>,
    category: usize,
    choices: Vec<Choice>,
    pub selected: Option<usize>,
    pub search: Editor,
    pub error: Option<String>,
}
impl BindingPicker {
    fn new(profile: &SoftwareProfile, control: &str) -> Self {
        let current = profile.buttons.get(control);
        let mut choices = vec![Choice {
            label: "Omit file assignment; preserve device state".into(),
            binding: None,
            error: None,
        }];
        choices.extend(
            profile_binding_choices(profile, control)
                .into_iter()
                .map(|choice| Choice {
                    label: choice.label,
                    binding: Some(choice.binding),
                    error: choice.error,
                }),
        );
        let categories: Vec<_> = [
            Category::Omit,
            Category::Mouse,
            Category::Multimedia,
            Category::Keyboard,
            Category::Shortcut,
            Category::Macro,
            Category::Disabled,
        ]
        .into_iter()
        .filter(|category| {
            choices
                .iter()
                .any(|choice| Category::for_binding(choice.binding.as_ref()) == *category)
        })
        .collect();
        let category = categories
            .iter()
            .position(|value| *value == Category::for_binding(current))
            .unwrap_or(0);
        let selected = choices
            .iter()
            .position(|choice| choice.binding.as_ref() == current);
        Self {
            control: control.into(),
            current: binding_label(current),
            limits: profile_controls(&profile.device)
                .into_iter()
                .find(|target| target.id == control)
                .and_then(|target| target.macros)
                .map_or_else(
                    || "No implemented macro encoding for this target.".into(),
                    |caps| {
                        let modes = |values: &[hyperx_core::MacroPlayback]| {
                            values
                                .iter()
                                .map(ToString::to_string)
                                .collect::<Vec<_>>()
                                .join(", ")
                        };
                        format!(
                            "Runtime: {}; onboard: {}; {} events / {} ms delay (encoding limits)",
                            modes(caps.runtime_playback),
                            modes(caps.onboard_playback),
                            caps.max_events,
                            caps.max_delay_ms
                        )
                    },
                ),
            categories,
            category,
            choices,
            selected,
            search: Editor::new("", true),
            error: None,
        }
    }
    pub fn filtered(&self) -> Vec<usize> {
        let query = self.search.text().to_lowercase();
        self.choices
            .iter()
            .enumerate()
            .filter_map(|(index, choice)| {
                (Category::for_binding(choice.binding.as_ref()) == self.categories[self.category]
                    && (choice.label.to_lowercase().contains(&query)
                        || match &choice.binding {
                            Some(SoftwareButtonBinding::Keyboard { key }) => {
                                key.to_lowercase().contains(&query)
                            }
                            Some(SoftwareButtonBinding::Macro { id }) => {
                                id.to_lowercase().contains(&query)
                            }
                            _ => false,
                        }))
                .then_some(index)
            })
            .collect()
    }
    pub fn filter_changed(&mut self) {
        if self
            .selected
            .is_some_and(|selected| !self.filtered().contains(&selected))
        {
            self.selected = None;
        }
        self.error = None;
    }
    pub fn category_move(&mut self, delta: i8) {
        self.category = (self.category as isize + isize::from(delta))
            .rem_euclid(self.categories.len() as isize) as usize;
        self.search = Editor::new("", true);
        // A category switch never implicitly chooses its first option.
        self.selected = None;
        self.error = None;
    }
    pub fn move_selection(&mut self, delta: i32) {
        let filtered = self.filtered();
        if filtered.is_empty() {
            self.selected = None;
            return;
        }
        let next = self
            .selected
            .and_then(|index| filtered.iter().position(|value| *value == index))
            .map_or(if delta < 0 { filtered.len() - 1 } else { 0 }, |index| {
                (index as i64 + i64::from(delta)).clamp(0, filtered.len() as i64 - 1) as usize
            });
        self.select(filtered[next]);
    }
    pub fn select(&mut self, index: usize) {
        if self.filtered().contains(&index) {
            self.selected = Some(index);
            self.error = None;
        }
    }
    pub fn key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Left | KeyCode::BackTab => self.category_move(-1),
            KeyCode::Tab if key.modifiers.contains(KeyModifiers::SHIFT) => self.category_move(-1),
            KeyCode::Right | KeyCode::Tab => self.category_move(1),
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
    }
    pub fn edit(&self) -> Result<ProfileValueEdit> {
        let choice = self
            .selected
            .and_then(|index| self.choices.get(index))
            .context("choose an explicit binding first; arrows/click preview, Enter accepts")?;
        if let Some(error) = &choice.error {
            bail!("{error}");
        }
        Ok(ProfileValueEdit::ButtonBinding {
            control: self.control.clone(),
            binding: choice.binding.clone(),
        })
    }
}

fn binding_label(binding: Option<&SoftwareButtonBinding>) -> String {
    match binding {
        None => "<not specified / not read>".into(),
        Some(SoftwareButtonBinding::Mouse { action }) => {
            format!("Mouse: {}", friendly(&format!("{action:?}")))
        }
        Some(SoftwareButtonBinding::Multimedia { action }) => {
            format!("Media: {}", friendly(&format!("{action:?}")))
        }
        Some(SoftwareButtonBinding::Keyboard { key }) => format!("Key: {key}"),
        Some(SoftwareButtonBinding::WindowsShortcut { action }) => {
            format!("Shortcut: {}", friendly(&format!("{action:?}")))
        }
        Some(SoftwareButtonBinding::Macro { id }) => format!("Macro: {id}"),
        Some(SoftwareButtonBinding::Disabled {}) => "Disabled".into(),
    }
}

fn friendly(value: &str) -> String {
    let mut text = String::new();
    for character in value.chars() {
        if character.is_uppercase() && !text.is_empty() {
            text.push(' ');
        }
        text.push(character);
    }
    text
}

impl App {
    pub(crate) fn open_binding_picker(&mut self) {
        let controls = profile_controls(&self.document.profile().device);
        let Some(control) = controls.get(self.selected_control) else {
            return;
        };
        if control.primary {
            self.status = "Primary clicks are a coupled pair. Use Standard/Swapped in Performance; no separate binding edit.".into();
            return;
        }
        self.drag = None;
        self.modal = Some(Modal::Binding(BindingPicker::new(
            self.document.profile(),
            control.id,
        )));
    }
    pub(crate) fn bindings_key(&mut self, key: KeyEvent) -> bool {
        if self.tab != 1 || self.modal.is_some() {
            return false;
        }
        let count = profile_controls(&self.document.profile().device).len();
        match key.code {
            KeyCode::Up => self.selected_control = self.selected_control.saturating_sub(1),
            KeyCode::Down => {
                self.selected_control = (self.selected_control + 1).min(count.saturating_sub(1))
            }
            KeyCode::Home => self.selected_control = 0,
            KeyCode::End => self.selected_control = count.saturating_sub(1),
            KeyCode::Enter | KeyCode::F(2) => self.open_binding_picker(),
            _ => return false,
        }
        // Keep keyboard selection visible even in compact terminal windows.
        self.scroll = self
            .selected_control
            .saturating_sub(1)
            .min(u16::MAX as usize) as u16;
        true
    }
    pub(crate) fn render_buttons(&mut self, frame: &mut Frame, area: Rect) {
        let title = if area.width < 70 {
            "Buttons: ↑/↓ select · Enter edit"
        } else {
            "Buttons - click / Up, Down; Enter or F2 edits"
        };
        frame.render_widget(Block::default().borders(Borders::ALL).title(title), area);
        let inner = area.inner(Margin {
            horizontal: 1,
            vertical: 1,
        });
        let controls = profile_controls(&self.document.profile().device);
        self.selected_control = self.selected_control.min(controls.len().saturating_sub(1));
        let lines = controls.len() + 3;
        self.scroll = self
            .scroll
            .min(lines.saturating_sub(inner.height as usize) as u16);
        let profile = self.document.profile();
        let rows: Vec<_> = controls
            .iter()
            .map(|control| {
                let binding = if control.primary {
                    format!(
                        "Coupled primary pair: {}",
                        match profile.primary_buttons {
                            Some(hyperx_core::PrimaryButtonLayout::Standard) => "Standard",
                            Some(hyperx_core::PrimaryButtonLayout::Swapped) => "Swapped",
                            None => "not specified / not read",
                        }
                    )
                } else {
                    binding_label(profile.buttons.get(control.id))
                };
                safe_text(&format!("{} ({}) -> {binding}", control.id, control.name))
            })
            .collect();
        for (index, label) in rows.iter().enumerate() {
            let Some(y) = (index as u16).checked_sub(self.scroll) else {
                continue;
            };
            if y >= inner.height {
                continue;
            }
            self.button(
                frame,
                Rect::new(inner.x, inner.y + y, inner.width, 1),
                label,
                Action::Control(index),
                index == self.selected_control,
            );
        }
        let unknown = profile_controls(&self.document.profile().device);
        let unknown_count = self
            .document
            .profile()
            .buttons
            .keys()
            .filter(|id| !unknown.iter().any(|control| control.id == id.as_str()))
            .count();
        for (offset, text) in [
            "File edits only. Omit != Disabled; primary layout stays coupled.".into(),
            format!(
                "Unresolved: {}; undeclared supplied controls: {} (preserved; inspect TOML).",
                self.document.profile().unresolved_button_assignments.len(),
                unknown_count
            ),
            "Macros use target runtime preflight, not proof of onboard support.".into(),
        ]
        .iter()
        .enumerate()
        {
            if let Some(y) = ((controls.len() + offset) as u16).checked_sub(self.scroll) {
                if y < inner.height {
                    frame.render_widget(
                        Paragraph::new(text.as_str()),
                        Rect::new(inner.x, inner.y + y, inner.width, 1),
                    );
                }
            }
        }
    }
}

pub(crate) fn render_picker(
    frame: &mut Frame,
    area: Rect,
    picker: &BindingPicker,
    hits: &mut Vec<Hit>,
) {
    frame.render_widget(
        Block::default().borders(Borders::ALL).title(format!(
            "File binding: {} - Enter accepts; Esc cancels",
            safe_text(&picker.control)
        )),
        area,
    );
    let inner = area.inner(Margin {
        horizontal: 1,
        vertical: 1,
    });
    let row = |offset: u16| Rect::new(inner.x, inner.y + offset, inner.width, 1);
    // At the minimum usable size show a safe resize notice; never create hit
    // regions outside the modal or leak the underlying table's mouse targets.
    if inner.height < 9 {
        frame.render_widget(
            Paragraph::new("Enlarge terminal to edit bindings.\nEsc cancels; no file changes."),
            inner,
        );
        return;
    }
    frame.render_widget(
        Paragraph::new(safe_text(&format!("Current file: {}", picker.current))),
        row(0),
    );
    frame.render_widget(
        Paragraph::new(format!(
            "[<] {} [>]",
            picker.categories[picker.category].label()
        ))
        .style(Style::default().fg(Color::Cyan)),
        row(1),
    );
    hits.push(Hit {
        area: sub(row(1), 0, 3),
        action: Action::BindingCategory(-1),
    });
    let width = picker.categories[picker.category].label().len() as u16;
    hits.push(Hit {
        area: sub(row(1), width + 5, 3),
        action: Action::BindingCategory(1),
    });
    frame.render_widget(
        Paragraph::new(safe_text(&format!("Search: {}", picker.search.text()))),
        row(2),
    );
    frame.render_widget(
        Paragraph::new("Left/Right: category; type: search; Up/Down: preview"),
        row(3),
    );
    let list_height = inner.height.saturating_sub(8) as usize;
    let filtered = picker.filtered();
    let selected_position = picker
        .selected
        .and_then(|index| filtered.iter().position(|value| *value == index));
    let start = selected_position
        .unwrap_or(0)
        .saturating_sub(list_height.saturating_sub(1));
    for (offset, index) in filtered.iter().skip(start).take(list_height).enumerate() {
        let choice = &picker.choices[*index];
        let text = format!(
            "{} {}{}",
            if picker.selected == Some(*index) {
                ">"
            } else {
                " "
            },
            choice.label,
            if choice.error.is_some() {
                " [unavailable]"
            } else {
                ""
            }
        );
        let area = row(4 + offset as u16);
        frame.render_widget(
            Paragraph::new(safe_text(&text)).style(Style::default().fg(
                if choice.error.is_some() {
                    Color::DarkGray
                } else if picker.selected == Some(*index) {
                    Color::Yellow
                } else {
                    Color::White
                },
            )),
            area,
        );
        hits.push(Hit {
            area,
            action: Action::BindingChoice(*index),
        });
    }
    if filtered.is_empty() {
        frame.render_widget(Paragraph::new("No matching choices."), row(4));
    }
    let selected = picker.selected.and_then(|index| picker.choices.get(index));
    let detail = picker.error.as_deref().or_else(|| selected.and_then(|choice| choice.error.as_deref()))
        .unwrap_or("Preview only. Enter/Accept updates file draft; never writes to mouse. Omit preserves device state; Disabled disables explicitly.");
    let bottom = Rect::new(inner.x, inner.y + inner.height - 4, inner.width, 3);
    frame.render_widget(
        Paragraph::new(safe_text(detail)).wrap(Wrap { trim: false }),
        bottom,
    );
    frame.render_widget(
        Paragraph::new(safe_text(&picker.limits)),
        row(inner.height - 1),
    );
}
