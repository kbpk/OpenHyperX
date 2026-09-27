//! Offline GUI session. This API intentionally has no transport/session handles.
use std::path::Path;

use anyhow::{bail, Result};
use hyperx_app::{
    device_descriptor, edit_profile_value, parse_profile, profile_controls, validate_profile,
    ProfileDocument, ProfileValueEdit,
};
use hyperx_core::{MacroPlayback, PrimaryButtonLayout, RgbColor, SoftwareProfile};
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
    pub dirty: bool,
    pub profile: SoftwareProfile,
    pub readiness: Readiness,
    pub changes: Changes,
    pub capabilities: Option<Capabilities>,
    pub controls: Vec<Control>,
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
    StageDpi { index: usize, dpi: u32 },
    StageColor { index: usize, color: RgbColor },
    AddStage { dpi: u32, color: RgbColor },
    RemoveLastStage {},
    ActiveStage { index: Option<usize> },
    Polling { hz: Option<u16> },
    PrimaryButtons { layout: Option<PrimaryButtonLayout> },
    SolidZone { zone: String, color: RgbColor },
    Name { name: String },
}

pub struct Session {
    document: ProfileDocument,
    origin: Origin,
    revision: u64,
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
        }
    }
    pub fn demo() -> Result<Self> {
        Ok(Self {
            document: ProfileDocument::from_profile(parse_profile(include_str!(
                "../../../../examples/profiles/pulsefire-raid.toml"
            ))?),
            origin: Origin::Demo,
            revision: 0,
        })
    }
    pub fn dirty(&self) -> bool {
        self.document.dirty()
    }
    pub fn check_revision(&self, expected: u64) -> Result<()> {
        if self.revision != expected {
            bail!("the file draft changed while this action was pending; refresh and try again");
        }
        Ok(())
    }
    pub fn check_discard(&self, expected: u64, discard: bool) -> Result<()> {
        self.check_revision(expected)?;
        if self.dirty() && !discard {
            bail!("explicit confirmation is required to discard unsaved file edits");
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
        Snapshot {
            revision: self.revision,
            origin: self.origin,
            path: self
                .document
                .path()
                .map(|path| path.to_string_lossy().into_owned()),
            dirty: self.dirty(),
            profile: profile.clone(),
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
            controls: profile_controls(&profile.device)
                .into_iter()
                .map(|control| Control {
                    id: control.id,
                    name: control.name,
                    primary: control.primary,
                    macros: control.macros.map(|limits| MacroLimits {
                        runtime: limits.runtime_playback.to_vec(),
                        onboard: limits.onboard_playback.to_vec(),
                        max_events: limits.max_events,
                        max_delay_ms: limits.max_delay_ms,
                    }),
                })
                .collect(),
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
            Edit::SolidZone { zone, color } => ProfileValueEdit::SolidZone { zone, color },
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
        self.revision += 1;
        Ok(self.snapshot())
    }
    pub fn save_new(&mut self, expected: u64, path: &Path) -> Result<Snapshot> {
        self.check_revision(expected)?;
        self.document.save_as(path)?;
        self.origin = Origin::File;
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
    println!("OpenHyperX GUI offline smoke passed: typed edits, validation and diff; no WebView or HID initialized.");
    Ok(())
}

#[cfg(test)]
mod tests;
