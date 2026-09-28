use std::path::Path;

use anyhow::{bail, Result};
use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use hyperx_app::{edit_section, section_toml, ProfileDocument, ProfileSection};
use serde::Deserialize;

use crate::editor::Editor;

#[derive(Clone, Copy, Debug)]
pub enum EditAction {
    Section(ProfileSection),
    Open,
    SaveAs,
    Resolve,
    Omit,
    ImportMacro,
    ProfileName,
    StageDpi(usize),
    StageColor(usize),
    AddStage,
    ZoneColor(&'static str),
}

pub enum Modal {
    Resolution(Box<crate::resolution::ResolutionPicker>),
    Files(Box<crate::files::FileBrowser>),
    Macro(Box<crate::macros::MacroEditor>),
    MacroDelete {
        original: hyperx_core::NamedMacro,
        error: Option<String>,
    },
    Binding(crate::bindings::BindingPicker),
    Editor {
        title: String,
        action: EditAction,
        editor: Editor,
        error: Option<String>,
    },
    Viewer {
        title: String,
        text: String,
        scroll: u16,
        jump: Option<usize>,
    },
    Confirm {
        open: bool,
    },
    Overwrite {
        path: std::path::PathBuf,
        error: Option<String>,
    },
}

pub struct App {
    pub document: ProfileDocument,
    pub demo: bool,
    pub tab: usize,
    pub scroll: u16,
    pub status: String,
    pub modal: Option<Modal>,
    pub quit: bool,
    pub selected_stage: usize,
    pub selected_control: usize,
    pub selected_macro: usize,
    pub hits: Vec<crate::widgets::Hit>,
    pub drag: Option<(usize, ratatui::layout::Rect)>,
}

impl App {
    pub fn new(document: ProfileDocument, demo: bool) -> Self {
        Self {
            document,
            demo,
            tab: 0,
            scroll: 0,
            modal: None,
            quit: false,
            selected_stage: 0,
            selected_control: 0,
            selected_macro: 0,
            hits: Vec::new(),
            drag: None,
            status: "OFFLINE: file edits only; no HID access. Save to mouse is unavailable.".into(),
        }
    }
    pub(crate) fn editor(
        &mut self,
        title: &str,
        action: EditAction,
        text: String,
        single_line: bool,
    ) {
        self.drag = None;
        self.modal = Some(Modal::Editor {
            title: title.into(),
            action,
            editor: Editor::new(&text, single_line),
            error: None,
        });
    }
    fn edit(&mut self, section: ProfileSection) {
        match section_toml(self.document.profile(), section) {
            Ok(text) => self.editor(
                "Edit section TOML - Ctrl+S accept draft, Esc cancel",
                EditAction::Section(section),
                text,
                false,
            ),
            Err(error) => self.status = error.to_string(),
        }
    }
    pub fn handle_paste(&mut self, text: &str) {
        if let Some(Modal::Editor { editor, .. }) = &mut self.modal {
            editor.paste(text);
        } else if let Some(Modal::Binding(picker)) = &mut self.modal {
            picker.search.paste(text);
            picker.filter_changed();
        } else if let Some(Modal::Macro(editor)) = &mut self.modal {
            editor.paste(text);
        } else if let Some(Modal::Files(browser)) = &mut self.modal {
            browser.paste(text);
        } else if let Some(Modal::Resolution(picker)) = &mut self.modal {
            picker.paste(text);
        }
    }
    pub fn handle_key(&mut self, key: KeyEvent) {
        // Windows reports release events too. Do not execute actions twice.
        if key.kind == KeyEventKind::Release {
            return;
        }
        // A keyboard action ends a mouse drag (including changing tab or
        // opening a modal); a later drag event must not target the old view.
        self.drag = None;
        let key = if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL)
        {
            KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE)
        } else {
            key
        };
        if let Some(mut modal) = self.modal.take() {
            match &mut modal {
                Modal::Resolution(picker) => match picker.key(key, self.document.profile()) {
                    crate::resolution::Outcome::Close => return,
                    crate::resolution::Outcome::Commit => {
                        match picker.edited(self.document.profile()) {
                            Ok(edited) => {
                                self.document.replace(edited);
                                self.status = "Explicit source selection accepted into FILE draft only. Other settings/library unchanged; no HID access.".into();
                                return;
                            }
                            Err(error) => picker.error = Some(format!("{error:#}")),
                        }
                    }
                    crate::resolution::Outcome::Stay => {}
                },
                Modal::Files(browser) => match browser.key(key) {
                    crate::files::Outcome::Close => return,
                    crate::files::Outcome::Adopt(document) => {
                        self.adopt_document(*document);
                        return;
                    }
                    crate::files::Outcome::Open(path) => match ProfileDocument::open(&path) {
                        Ok(document) if self.document.dirty() => {
                            browser.pending = Some(Box::new(document))
                        }
                        Ok(document) => {
                            self.adopt_document(document);
                            return;
                        }
                        Err(error) => browser.error = Some(format!("{error:#}")),
                    },
                    crate::files::Outcome::Save(path) => match self.document.save_as(&path) {
                        Ok(()) => {
                            self.status = "Saved a NEW profile copy; this file is now the baseline. Original untouched; NOT Save to mouse.".into();
                            return;
                        }
                        Err(error) => browser.error = Some(format!("{error:#}")),
                    },
                    crate::files::Outcome::Stay => {}
                },
                Modal::Macro(editor) => match editor.key(key, self.document.profile()) {
                    crate::macros::Outcome::Close => return,
                    crate::macros::Outcome::Commit => {
                        match editor
                            .edit(self.document.profile())
                            .and_then(|edit| self.edit_value(edit))
                        {
                            Ok(()) => {
                                self.selected_macro = self
                                    .document
                                    .profile()
                                    .macros
                                    .iter()
                                    .position(|entry| entry.source_id == editor.draft.source_id)
                                    .unwrap_or(0);
                                return;
                            }
                            Err(error) => editor.error = Some(format!("{error:#}")),
                        }
                    }
                    crate::macros::Outcome::Stay => {}
                },
                Modal::MacroDelete { original, error } => match key.code {
                    KeyCode::Esc | KeyCode::Char('n') => return,
                    KeyCode::Char('y') => match self.remove_macro(original) {
                        Ok(()) => return,
                        Err(failure) => *error = Some(format!("{failure:#}")),
                    },
                    _ => {}
                },
                Modal::Binding(picker) => {
                    if key.code == KeyCode::Esc {
                        return;
                    }
                    let accept = key.code == KeyCode::Enter
                        || (key.code == KeyCode::Char('s')
                            && key.modifiers.contains(KeyModifiers::CONTROL));
                    if accept {
                        match picker.edit().and_then(|edit| self.edit_value(edit)) {
                            Ok(()) => return,
                            Err(error) => picker.error = Some(format!("{error:#}")),
                        }
                    } else {
                        picker.key(key);
                    }
                }
                Modal::Confirm { open } => match key.code {
                    KeyCode::Char('y') => {
                        if *open {
                            self.editor(
                                "Open profile path - Enter to load, Esc cancel",
                                EditAction::Open,
                                String::new(),
                                true,
                            );
                        } else {
                            self.quit = true;
                        }
                        return;
                    }
                    KeyCode::Char('n') | KeyCode::Esc => return,
                    _ => {}
                },
                Modal::Overwrite { path, error } => match key.code {
                    KeyCode::Esc | KeyCode::Char('n') => return,
                    KeyCode::Char('y') => {
                        if self.document.path() != Some(path.as_path()) {
                            *error = Some(
                                "File path changed; reopen the overwrite confirmation.".into(),
                            );
                        } else {
                            match self.document.save_overwrite_with_backup() {
                                Ok(backup) => {
                                    self.status = format!(
                                        "FILE overwritten with recovery copy at {}. No mouse settings changed.",
                                        backup.display()
                                    );
                                    return;
                                }
                                Err(failure) => *error = Some(format!("{failure:#}")),
                            }
                        }
                    }
                    _ => {}
                },
                Modal::Viewer { scroll, jump, .. } => match key.code {
                    KeyCode::Esc | KeyCode::Char('q') => return,
                    KeyCode::Char('g') if jump.is_some() => {
                        self.tab = jump.unwrap_or(self.tab);
                        self.scroll = 0;
                        return;
                    }
                    KeyCode::Down | KeyCode::PageDown => {
                        *scroll = scroll.saturating_add(if key.code == KeyCode::PageDown {
                            10
                        } else {
                            1
                        })
                    }
                    KeyCode::Up | KeyCode::PageUp => {
                        *scroll =
                            scroll.saturating_sub(if key.code == KeyCode::PageUp { 10 } else { 1 })
                    }
                    KeyCode::Home => *scroll = 0,
                    _ => {}
                },
                Modal::Editor {
                    action,
                    editor,
                    error,
                    ..
                } => {
                    if key.code == KeyCode::Esc {
                        return;
                    }
                    if crate::color_palette::is_color_action(*action)
                        && crate::color_palette::preview_key(key.code, editor)
                    {
                        *error = None;
                        self.modal = Some(modal);
                        return;
                    }
                    let accept = (key.code == KeyCode::Char('s')
                        && key.modifiers.contains(KeyModifiers::CONTROL))
                        || (key.code == KeyCode::Enter && editor.single_line);
                    if accept {
                        match self.accept(*action, &editor.text()) {
                            Ok(()) => return,
                            Err(failure) => *error = Some(format!("{failure:#}")),
                        }
                    } else {
                        editor.key(key);
                    }
                }
            }
            self.modal = Some(modal);
            return;
        }
        if self.profiles_key(key)
            || self.macros_key(key)
            || self.bindings_key(key)
            || self.performance_key(key)
        {
            return;
        }
        match key.code {
            KeyCode::Tab | KeyCode::Right => {
                self.tab = (self.tab + 1) % 5;
                self.scroll = 0;
            }
            KeyCode::BackTab | KeyCode::Left => {
                self.tab = (self.tab + 4) % 5;
                self.scroll = 0;
            }
            KeyCode::Char(value @ '1'..='5') => {
                self.tab = value as usize - '1' as usize;
                self.scroll = 0;
            }
            KeyCode::Down => self.scroll = self.scroll.saturating_add(1),
            KeyCode::Up => self.scroll = self.scroll.saturating_sub(1),
            KeyCode::PageDown => self.scroll = self.scroll.saturating_add(10),
            KeyCode::PageUp => self.scroll = self.scroll.saturating_sub(10),
            KeyCode::Home => self.scroll = 0,
            KeyCode::Char('e') => self.edit(
                [
                    ProfileSection::Performance,
                    ProfileSection::Buttons,
                    ProfileSection::Macros,
                    ProfileSection::Lighting,
                    ProfileSection::All,
                ][self.tab],
            ),
            KeyCode::Char('a') => self.edit(ProfileSection::All),
            KeyCode::Char('s') => self.editor(
                "Save NEW profile path - Enter; never overwrite",
                EditAction::SaveAs,
                String::new(),
                true,
            ),
            KeyCode::Char('o') => {
                if self.document.dirty() {
                    self.modal = Some(Modal::Confirm { open: true });
                } else {
                    self.editor(
                        "Open profile path - Enter",
                        EditAction::Open,
                        String::new(),
                        true,
                    );
                }
            }
            KeyCode::Char('q') | KeyCode::Esc => {
                if self.document.dirty() {
                    self.modal = Some(Modal::Confirm { open: false });
                } else {
                    self.quit = true;
                }
            }
            KeyCode::Char('v') => {
                let readiness = hyperx_app::validate_profile(self.document.profile());
                let (text, jump) = crate::diagnostics::validation_view(&readiness);
                self.modal = Some(Modal::Viewer {
                    title: "Offline validation (Esc close)".into(),
                    text,
                    scroll: 0,
                    jump,
                });
            }
            KeyCode::Char('d') => match self.document.diff() {
                Ok(diff) => {
                    self.modal = Some(Modal::Viewer {
                        title: "Diff against opened/saved file (Esc close)".into(),
                        text: crate::diagnostics::diff_view(&diff),
                        scroll: 0,
                        jump: None,
                    });
                }
                Err(error) => self.status = error.to_string(),
            },
            KeyCode::Char('r') => self.editor(
                "Resolve macro - explicit source/target/definition; Ctrl+S",
                EditAction::Resolve,
                "source_id = \"\"\ncontrol = \"\"\nmacro_id = \"\"\n".into(),
                false,
            ),
            KeyCode::Char('x') => self.editor(
                "Explicitly omit ONE unresolved source ID - Enter",
                EditAction::Omit,
                String::new(),
                true,
            ),
            KeyCode::Char('m') => self.editor(
                "Import macro timeline under a NEW ID - Ctrl+S",
                EditAction::ImportMacro,
                "path = \"\"\nid = \"\"\n".into(),
                false,
            ),
            _ => {}
        }
    }
    fn accept(&mut self, action: EditAction, text: &str) -> Result<()> {
        match action {
            EditAction::Section(section) => {
                self.document
                    .replace(edit_section(self.document.profile(), section, text)?);
                self.status = "Draft updated; use v to validate and d to inspect file changes. No device writes.".into();
            }
            EditAction::Open => {
                if text.trim().is_empty() {
                    bail!("enter a profile path");
                }
                // Replace only AFTER reading/parsing succeeds. Failed opens keep
                // the current document, including unsaved edits.
                self.adopt_document(ProfileDocument::open(Path::new(text.trim()))?);
            }
            EditAction::ProfileName => {
                self.document
                    .replace(hyperx_app::rename_profile(self.document.profile(), text)?);
                self.status =
                    "Renamed FILE draft only; path, settings and provenance are unchanged.".into();
            }
            EditAction::SaveAs => {
                if text.trim().is_empty() {
                    bail!("enter a NEW destination path");
                }
                self.document.save_as(Path::new(text.trim()))?;
                self.status = "Saved new TOML file, including any unresolved draft fields. NOT Save to mouse.".into();
            }
            EditAction::StageDpi(index) => {
                self.edit_value(hyperx_app::ProfileValueEdit::StageDpi {
                    index,
                    dpi: text.trim().parse()?,
                })?
            }
            EditAction::StageColor(index) => {
                self.edit_value(hyperx_app::ProfileValueEdit::StageColor {
                    index,
                    color: text.trim().parse()?,
                })?
            }
            EditAction::AddStage => {
                self.edit_value(hyperx_app::ProfileValueEdit::AddStage {
                    dpi: text.trim().parse()?,
                    color: hyperx_core::RgbColor::new(255, 255, 255),
                })?;
                self.selected_stage =
                    self.document.profile().dpi.as_ref().unwrap().stages.len() - 1;
            }
            EditAction::ZoneColor(zone) => {
                self.edit_value(hyperx_app::ProfileValueEdit::SolidZone {
                    zone: zone.into(),
                    color: text.trim().parse()?,
                })?
            }
            EditAction::Resolve => {
                #[derive(Deserialize)]
                #[serde(deny_unknown_fields)]
                struct Request {
                    source_id: String,
                    control: String,
                    macro_id: String,
                }
                let request: Request = toml::from_str(text)?;
                let edited = hyperx_app::resolve_macro_assignment(
                    self.document.profile(),
                    &request.source_id,
                    &request.control,
                    &request.macro_id,
                    None,
                )?;
                self.document.replace(edited);
                self.status = "Explicit assignment resolved in draft; no upload performed.".into();
            }
            EditAction::Omit => {
                self.document
                    .replace(hyperx_app::omit_unresolved_assignment(
                        self.document.profile(),
                        text.trim(),
                    )?);
                self.status = "Explicitly omitted one unresolved entry; absent bindings preserve device state.".into();
            }
            EditAction::ImportMacro => {
                #[derive(Deserialize)]
                #[serde(deny_unknown_fields)]
                struct Request {
                    path: String,
                    id: String,
                }
                let request: Request = toml::from_str(text)?;
                let definition = hyperx_app::load_macro(Path::new(&request.path))?;
                self.document.replace(hyperx_app::import_macro(
                    self.document.profile(),
                    &request.id,
                    definition,
                )?);
                self.status = "Imported macro library definition; validate before choosing a target. No device upload.".into();
            }
        }
        Ok(())
    }
}
