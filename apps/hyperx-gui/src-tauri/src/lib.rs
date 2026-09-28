//! Offline GUI session. This API intentionally has no transport/session handles.
use std::path::Path;

use anyhow::{bail, Result};
use hyperx_app::{
    device_descriptor, edit_profile_value, macro_keyboard_names, macro_mouse_button_names,
    macro_resolution_targets, omit_unresolved_assignment, parse_profile, profile_binding_choices,
    profile_controls, resolve_macro_assignment, validate_profile, ProfileBindingChoice,
    ProfileDocument, ProfileValueEdit,
};
use hyperx_core::{
    MacroPlayback, NamedMacro, PrimaryButtonLayout, RgbColor, SoftwareButtonBinding,
    SoftwareProfile,
};
use serde::{Deserialize, Serialize};

#[cfg(feature = "desktop")]
pub mod desktop;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Origin {
    Empty,
    Demo,
    File,
}

#[derive(Debug, Serialize)]
pub struct Snapshot {
    pub revision: u64,
    pub origin: Origin,
    pub path: Option<String>,
    /// Latest FILE-only recovery copy from a confirmed overwrite.
    pub recovery_path: Option<String>,
    pub dirty: bool,
    pub can_undo: bool,
    pub can_redo: bool,
    pub profile: SoftwareProfile,
    pub readiness: Readiness,
    pub changes: Changes,
    pub capabilities: Option<Capabilities>,
    pub binding_choices: Vec<ProfileBindingChoice>,
    pub macro_keys: Vec<String>,
    pub macro_mouse_buttons: Vec<String>,
    pub controls: Vec<Control>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub resolution_sources: Vec<ResolutionSource>,
}
#[derive(Debug, Serialize)]
pub struct ResolutionSource {
    pub source_id: String,
    pub error: Option<String>,
    pub targets: Vec<ResolutionTarget>,
}
#[derive(Debug, Serialize)]
pub struct ResolutionTarget {
    pub id: &'static str,
    pub name: &'static str,
    pub error: Option<String>,
}
#[derive(Debug, Serialize)]
pub struct Readiness {
    pub error: Option<String>,
    pub warnings: Vec<String>,
}
#[derive(Debug, Serialize)]
pub struct Changes {
    pub settings: Vec<Change>,
    pub metadata: Vec<Change>,
    pub error: Option<String>,
}
#[derive(Debug, Serialize)]
pub struct Change {
    pub field: String,
    pub before: Option<String>,
    pub after: Option<String>,
}
#[derive(Debug, Serialize)]
pub struct Capabilities {
    pub name: &'static str,
    pub button_count: u8,
    pub dpi: Option<DpiLimits>,
    pub polling_rates: Vec<u16>,
    pub zones: Vec<Zone>,
}
#[derive(Debug, Serialize)]
pub struct DpiLimits {
    pub minimum: u32,
    pub maximum: u32,
    pub step: u32,
    pub max_stages: u8,
}
#[derive(Debug, Serialize)]
pub struct Zone {
    pub id: &'static str,
    pub name: &'static str,
}
#[derive(Debug, Serialize)]
pub struct Control {
    pub id: &'static str,
    pub name: &'static str,
    pub primary: bool,
    /// Indices into Snapshot::binding_choices, never USB/HID slot numbers.
    pub bindings: Vec<usize>,
    pub macros: Option<MacroLimits>,
}
#[derive(Debug, Serialize)]
pub struct MacroLimits {
    pub runtime: Vec<MacroPlayback>,
    pub onboard: Vec<MacroPlayback>,
    pub max_events: usize,
    pub max_delay_ms: u16,
}

/// A narrow JSON surface, not arbitrary profile replacement or report sending.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub enum Edit {
    StageDpi {
        index: usize,
        dpi: u32,
    },
    StageColor {
        index: usize,
        color: RgbColor,
    },
    AddStage {
        dpi: u32,
        color: RgbColor,
    },
    RemoveLastStage {},
    ActiveStage {
        index: Option<usize>,
    },
    Polling {
        hz: Option<u16>,
    },
    PrimaryButtons {
        layout: Option<PrimaryButtonLayout>,
    },
    ButtonBinding {
        control: String,
        #[serde(deserialize_with = "required_binding")]
        binding: Option<SoftwareButtonBinding>,
    },
    MacroCreate {
        #[serde(rename = "macro")]
        definition: NamedMacro,
    },
    MacroReplace {
        source_id: String,
        #[serde(rename = "macro")]
        definition: NamedMacro,
        confirm_references: bool,
    },
    MacroRemove {
        source_id: String,
    },
    SolidZone {
        zone: String,
        color: RgbColor,
    },
    Name {
        name: String,
    },
    ResolveUnresolved {
        source_id: String,
        control: String,
        macro_id: String,
        confirm: bool,
    },
    OmitUnresolved {
        source_id: String,
        confirm: bool,
    },
}

// A missing binding field is not consent to remove an assignment. Only an
// explicitly supplied JSON null denotes omission in a file.
fn required_binding<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> std::result::Result<Option<SoftwareButtonBinding>, D::Error> {
    Option::deserialize(deserializer)
}

pub struct Session {
    document: ProfileDocument,
    origin: Origin,
    revision: u64,
    local_draft: bool,
    local_draft_generation: u64,
    recovery_path: Option<String>,
}

impl Default for Session {
    fn default() -> Self {
        Self::empty()
    }
}
impl Session {
    pub fn empty() -> Self {
        Self {
            document: ProfileDocument::from_profile(SoftwareProfile {
                name: "Untitled profile".into(),
                device: "pulsefire-raid".into(),
                partial: true,
                ..Default::default()
            }),
            origin: Origin::Empty,
            revision: 0,
            local_draft: false,
            local_draft_generation: 0,
            recovery_path: None,
        }
    }
    pub fn demo() -> Result<Self> {
        Ok(Self {
            document: ProfileDocument::from_profile(parse_profile(include_str!(
                "../../../../examples/profiles/pulsefire-raid.toml"
            ))?),
            origin: Origin::Demo,
            revision: 0,
            local_draft: false,
            local_draft_generation: 0,
            recovery_path: None,
        })
    }
    pub fn dirty(&self) -> bool {
        self.document.dirty()
    }
    /// Frontend-only timeline edits are not yet in the file document. Protect
    /// native close without fabricating a document diff or changing its revision.
    pub fn set_local_draft(&mut self, pending: bool) {
        self.local_draft = pending;
        self.local_draft_generation += 1;
    }
    pub fn close_guard(&self) -> ((u64, u64), bool) {
        (
            (self.revision, self.local_draft_generation),
            self.dirty() || self.local_draft,
        )
    }
    pub fn check_revision(&self, expected: u64) -> Result<()> {
        if self.revision != expected {
            bail!("the file draft changed while this action was pending; refresh and try again");
        }
        Ok(())
    }
    pub fn check_discard(&self, expected: u64, discard: bool) -> Result<()> {
        self.check_revision(expected)?;
        if self.local_draft {
            bail!("update or discard the local macro timeline before replacing the file");
        }
        if self.dirty() && !discard {
            bail!("explicit confirmation is required to discard unsaved file edits");
        }
        Ok(())
    }
    pub fn check_save(&self, expected: u64) -> Result<()> {
        self.check_revision(expected)?;
        if self.local_draft {
            bail!("update or discard the local macro timeline before saving the file");
        }
        Ok(())
    }
    pub fn snapshot(&self) -> Snapshot {
        let profile = self.document.profile();
        let readiness = validate_profile(profile);
        let changes = match self.document.diff() {
            Ok(diff) => {
                fn map(changes: Vec<hyperx_core::SoftwareProfileFieldChange>) -> Vec<Change> {
                    changes
                        .into_iter()
                        .map(|change| Change {
                            field: change.field,
                            before: change.before,
                            after: change.after,
                        })
                        .collect()
                }
                Changes {
                    settings: map(diff.settings),
                    metadata: map(diff.metadata),
                    error: None,
                }
            }
            Err(error) => Changes {
                settings: Vec::new(),
                metadata: Vec::new(),
                error: Some(error.to_string()),
            },
        };
        let mut binding_choices = Vec::new();
        let controls = profile_controls(&profile.device)
            .into_iter()
            .map(|control| Control {
                id: control.id,
                name: control.name,
                primary: control.primary,
                bindings: profile_binding_choices(profile, control.id)
                    .into_iter()
                    .map(|choice| {
                        if let Some(index) =
                            binding_choices.iter().position(|entry| entry == &choice)
                        {
                            index
                        } else {
                            binding_choices.push(choice);
                            binding_choices.len() - 1
                        }
                    })
                    .collect(),
                macros: control.macros.map(|limits| MacroLimits {
                    runtime: limits.runtime_playback.to_vec(),
                    onboard: limits.onboard_playback.to_vec(),
                    max_events: limits.max_events,
                    max_delay_ms: limits.max_delay_ms,
                }),
            })
            .collect();
        let resolution_sources = profile
            .unresolved_button_assignments
            .iter()
            .map(
                |source| match macro_resolution_targets(profile, &source.source_id) {
                    Ok(targets) => ResolutionSource {
                        source_id: source.source_id.clone(),
                        error: None,
                        targets: targets
                            .into_iter()
                            .map(|target| ResolutionTarget {
                                id: target.control.id,
                                name: target.control.name,
                                error: target.error,
                            })
                            .collect(),
                    },
                    Err(error) => ResolutionSource {
                        source_id: source.source_id.clone(),
                        error: Some(error.to_string()),
                        targets: Vec::new(),
                    },
                },
            )
            .collect();
        Snapshot {
            revision: self.revision,
            origin: self.origin,
            path: self
                .document
                .path()
                .map(|path| path.to_string_lossy().into_owned()),
            recovery_path: self.recovery_path.clone(),
            dirty: self.dirty(),
            can_undo: self.document.can_undo(),
            can_redo: self.document.can_redo(),
            profile: profile.clone(),
            macro_keys: macro_keyboard_names(&profile.device),
            macro_mouse_buttons: macro_mouse_button_names(&profile.device),
            readiness: Readiness {
                error: readiness.error,
                warnings: readiness.warnings,
            },
            changes,
            capabilities: device_descriptor(&profile.device).map(|device| Capabilities {
                name: device.name,
                button_count: device.capabilities.button_count,
                dpi: device.capabilities.dpi.map(|limits| DpiLimits {
                    minimum: limits.minimum,
                    maximum: limits.maximum,
                    step: limits.step,
                    max_stages: limits.max_stages,
                }),
                polling_rates: device
                    .capabilities
                    .polling_rates
                    .iter()
                    .map(|rate| rate.hz())
                    .collect(),
                zones: device
                    .capabilities
                    .lighting_zones
                    .iter()
                    .map(|zone| Zone {
                        id: zone.id,
                        name: zone.name,
                    })
                    .collect(),
            }),
            controls,
            binding_choices,
            resolution_sources,
        }
    }
    pub fn edit(&mut self, expected: u64, edit: Edit) -> Result<Snapshot> {
        self.check_revision(expected)?;
        let edit = match edit {
            Edit::StageDpi { index, dpi } => ProfileValueEdit::StageDpi { index, dpi },
            Edit::StageColor { index, color } => ProfileValueEdit::StageColor { index, color },
            Edit::AddStage { dpi, color } => ProfileValueEdit::AddStage { dpi, color },
            Edit::RemoveLastStage {} => ProfileValueEdit::RemoveLastStage,
            Edit::ActiveStage { index } => ProfileValueEdit::ActiveStage(index),
            Edit::Polling { hz } => ProfileValueEdit::Polling(hz),
            Edit::PrimaryButtons { layout } => ProfileValueEdit::PrimaryButtons(layout),
            Edit::ButtonBinding { control, binding } => {
                ProfileValueEdit::ButtonBinding { control, binding }
            }
            Edit::MacroCreate { definition } => ProfileValueEdit::MacroCreate { definition },
            Edit::MacroReplace {
                source_id,
                definition,
                confirm_references,
            } => ProfileValueEdit::MacroReplace {
                source_id,
                definition,
                confirm_references,
            },
            Edit::MacroRemove { source_id } => ProfileValueEdit::MacroRemove { source_id },
            Edit::SolidZone { zone, color } => ProfileValueEdit::SolidZone { zone, color },
            Edit::ResolveUnresolved {
                source_id,
                control,
                macro_id,
                confirm,
            } => {
                if !confirm {
                    bail!("explicit confirmation is required to resolve an imported assignment");
                }
                let profile = resolve_macro_assignment(
                    self.document.profile(),
                    &source_id,
                    &control,
                    &macro_id,
                    None,
                )?;
                self.document.replace(profile);
                self.revision += 1;
                return Ok(self.snapshot());
            }
            Edit::OmitUnresolved { source_id, confirm } => {
                if !confirm {
                    bail!("explicit confirmation is required to omit imported provenance");
                }
                let profile = omit_unresolved_assignment(self.document.profile(), &source_id)?;
                self.document.replace(profile);
                self.revision += 1;
                return Ok(self.snapshot());
            }
            Edit::Name { name } => {
                let name = name.trim();
                if name.is_empty() || name.len() > 128 || name.chars().any(char::is_control) {
                    bail!("profile name must be 1–128 UTF-8 bytes without control characters");
                }
                let mut profile = self.document.profile().clone();
                profile.name = name.into();
                hyperx_app::encode_profile(&profile)?;
                if &profile != self.document.profile() {
                    self.document.replace(profile);
                    self.revision += 1;
                }
                return Ok(self.snapshot());
            }
        };
        let profile = edit_profile_value(self.document.profile(), edit)?;
        if &profile != self.document.profile() {
            self.document.replace(profile);
            self.revision += 1;
        }
        Ok(self.snapshot())
    }
    pub fn reset(&mut self, expected: u64, discard: bool, demo: bool) -> Result<Snapshot> {
        self.check_discard(expected, discard)?;
        let next = if demo { Self::demo()? } else { Self::empty() };
        self.document = next.document;
        self.origin = next.origin;
        self.recovery_path = None;
        self.revision += 1;
        Ok(self.snapshot())
    }
    /// The desktop adapter supplies a path selected in a native dialog, never
    /// an arbitrary frontend-supplied filesystem path. Read/parse first.
    pub fn open(&mut self, expected: u64, discard: bool, path: &Path) -> Result<Snapshot> {
        self.check_discard(expected, discard)?;
        let document = ProfileDocument::open(path)?;
        self.document = document;
        self.origin = Origin::File;
        self.recovery_path = None;
        self.revision += 1;
        Ok(self.snapshot())
    }
    pub fn save_new(&mut self, expected: u64, path: &Path) -> Result<Snapshot> {
        self.check_save(expected)?;
        self.document.save_as(path)?;
        self.origin = Origin::File;
        self.recovery_path = None;
        self.revision += 1;
        Ok(self.snapshot())
    }
    /// Explicit FILE overwrite, never an onboard/hardware operation. The
    /// desktop adapter passes no path from the WebView; only this session's
    /// already-opened/saved file can be replaced.
    pub fn overwrite_file(&mut self, expected: u64, confirmed: bool) -> Result<Snapshot> {
        self.check_save(expected)?;
        if !confirmed {
            bail!("explicit confirmation is required to overwrite the FILE profile");
        }
        let backup = self.document.save_overwrite_with_backup()?;
        self.recovery_path = Some(backup.to_string_lossy().into_owned());
        self.revision += 1;
        Ok(self.snapshot())
    }
    pub fn undo_file_edit(&mut self, expected: u64) -> Result<Snapshot> {
        self.check_revision(expected)?;
        if self.local_draft {
            bail!("update or discard the local macro timeline before undoing a file edit");
        }
        if !self.document.undo() {
            bail!("no file draft edit to undo");
        }
        self.revision += 1;
        Ok(self.snapshot())
    }
    pub fn redo_file_edit(&mut self, expected: u64) -> Result<Snapshot> {
        self.check_revision(expected)?;
        if self.local_draft {
            bail!("update or discard the local macro timeline before redoing a file edit");
        }
        if !self.document.redo() {
            bail!("no file draft edit to redo");
        }
        self.revision += 1;
        Ok(self.snapshot())
    }
}

/// Executable smoke path: exercises app logic without initializing WebView/HID.
pub fn smoke_test() -> Result<()> {
    let mut session = Session::demo()?;
    let snapshot = session.edit(0, Edit::StageDpi { index: 0, dpi: 900 })?;
    assert!(snapshot.dirty && snapshot.profile.dpi.as_ref().unwrap().stages[0].x == 900);
    let snapshot = session.edit(snapshot.revision, Edit::StageDpi { index: 0, dpi: 800 })?;
    assert!(!snapshot.dirty && snapshot.readiness.error.is_none());
    let original = snapshot.profile.buttons.get("button4").cloned();
    let snapshot = session.edit(
        snapshot.revision,
        Edit::ButtonBinding {
            control: "button4".into(),
            binding: Some(SoftwareButtonBinding::Disabled {}),
        },
    )?;
    assert!(
        snapshot.dirty
            && matches!(
                snapshot.profile.buttons["button4"],
                SoftwareButtonBinding::Disabled {}
            )
    );
    let snapshot = session.edit(
        snapshot.revision,
        Edit::ButtonBinding {
            control: "button4".into(),
            binding: original,
        },
    )?;
    assert!(!snapshot.dirty && snapshot.readiness.error.is_none());
    let mut definition = snapshot.profile.macros[0].clone();
    definition.source_id = "offline-smoke-macro".into();
    let snapshot = session.edit(snapshot.revision, Edit::MacroCreate { definition })?;
    assert!(snapshot.dirty && snapshot.profile.macros.len() == 2);
    let snapshot = session.edit(
        snapshot.revision,
        Edit::MacroRemove {
            source_id: "offline-smoke-macro".into(),
        },
    )?;
    assert!(!snapshot.dirty && snapshot.readiness.error.is_none());
    let mut imported = snapshot.profile;
    imported.buttons.remove("button4");
    imported
        .unresolved_button_assignments
        .push(hyperx_core::UnresolvedButtonAssignment {
            source_id: "runtime:button4".into(),
            macro_source_id: Some("ab".into()),
        });
    session.document.replace(imported);
    let snapshot = session.snapshot();
    assert_eq!(snapshot.resolution_sources.len(), 1);
    let snapshot = session.edit(
        snapshot.revision,
        Edit::ResolveUnresolved {
            source_id: "runtime:button4".into(),
            control: "button4".into(),
            macro_id: "ab".into(),
            confirm: true,
        },
    )?;
    assert!(snapshot.resolution_sources.is_empty());
    assert_eq!(
        snapshot.profile.buttons.get("button4"),
        Some(&SoftwareButtonBinding::Macro { id: "ab".into() })
    );
    println!("OpenHyperX GUI offline smoke passed: typed edits, imported-source resolution, validation and diff; no WebView or HID initialized.");
    Ok(())
}

#[cfg(test)]
mod tests;
