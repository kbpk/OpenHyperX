use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, fmt};

use crate::{
    DpiStage, MacroDefinition, MouseFunction, MultimediaFunction, PrimaryButtonLayout, RgbColor,
    WindowsShortcut,
};

/// A portable application profile.
///
/// A profile may be partial when it was imported from a format whose fields
/// are not completely understood. Loading a profile never implies permission
/// to write it to a device or to onboard memory.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SoftwareProfile {
    pub name: String,
    pub device: String,
    #[serde(default)]
    pub partial: bool,
    pub source: Option<SoftwareProfileSource>,
    pub dpi: Option<SoftwareDpiProfile>,
    pub polling: Option<SoftwarePollingProfile>,
    pub primary_buttons: Option<PrimaryButtonLayout>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub buttons: BTreeMap<String, SoftwareButtonBinding>,
    pub lighting: Option<SoftwareLightingProfile>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub macros: Vec<NamedMacro>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub unresolved_button_assignments: Vec<UnresolvedButtonAssignment>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SoftwarePollingProfile {
    pub hz: u16,
}

/// Portable actions use semantic names; no usage IDs or vendor records appear
/// in a software profile. Macro references resolve within this same file.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "kebab-case", deny_unknown_fields)]
pub enum SoftwareButtonBinding {
    Mouse { action: MouseFunction },
    Keyboard { key: String },
    Multimedia { action: MultimediaFunction },
    WindowsShortcut { action: WindowsShortcut },
    // Empty struct, not a unit variant: serde's internally tagged unit variant
    // can ignore extra fields despite deny_unknown_fields. Reject typos here.
    Disabled {},
    Macro { id: String },
}

impl fmt::Display for SoftwareButtonBinding {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Mouse { action } => write!(formatter, "Mouse: {action}"),
            Self::Keyboard { key } => write!(formatter, "Keyboard: {}", key.escape_debug()),
            Self::Multimedia { action } => write!(formatter, "Multimedia: {action}"),
            Self::WindowsShortcut { action } => write!(formatter, "Windows shortcut: {action}"),
            Self::Disabled {} => formatter.write_str("Disabled"),
            Self::Macro { id } => write!(formatter, "Macro reference: {}", id.escape_debug()),
        }
    }
}

/// Zone IDs belong to the device's public capabilities, not USB offsets.
/// Drivers must reject partial zone updates when current colors cannot be read.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SoftwareLightingProfile {
    pub mode: SoftwareLightingMode,
    pub zones: BTreeMap<String, RgbColor>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum SoftwareLightingMode {
    Solid,
}

impl fmt::Display for SoftwareLightingMode {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Solid => formatter.write_str("Solid"),
        }
    }
}

/// DPI data in a portable software profile.
///
/// `source_active_stage` preserves an imported value when its indexing
/// semantics are not confirmed. Such a value must not be applied as an active
/// stage until a format-specific importer can populate `active_stage`.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SoftwareDpiProfile {
    pub stages: Vec<DpiStage>,
    pub active_stage: Option<usize>,
    pub source_active_stage: Option<u32>,
}

/// Provenance retained when a portable profile was imported.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SoftwareProfileSource {
    pub format: String,
    pub format_version: u32,
}

/// A named macro stored in a software profile.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct NamedMacro {
    /// `id` is convenient for authored profiles; Legacy imports retain their
    /// opaque source identifier. Neither spelling implies a physical button.
    #[serde(alias = "id")]
    pub source_id: String,
    pub name: String,
    #[serde(flatten)]
    pub definition: MacroDefinition,
}

/// Source assignment retained until its physical control or definition is known.
/// Legacy imports retain opaque source IDs; capture exports use explicit
/// diagnostic labels such as `runtime:button5` for an unreadable macro timeline.
/// These labels are provenance, never vendor slot IDs or executable bindings.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct UnresolvedButtonAssignment {
    pub source_id: String,
    pub macro_source_id: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{DpiStage, MacroEvent, MacroPlayback, RgbColor};

    #[test]
    fn portable_profile_round_trips_through_toml_shape() {
        let profile = SoftwareProfile {
            name: "Imported preset".to_owned(),
            device: "pulsefire-raid".to_owned(),
            partial: true,
            source: Some(SoftwareProfileSource {
                format: "ngenuity-legacy-hxp".to_owned(),
                format_version: 40,
            }),
            dpi: Some(SoftwareDpiProfile {
                stages: vec![DpiStage::new(800, 800, RgbColor::new(1, 2, 3))],
                active_stage: Some(0),
                source_active_stage: None,
            }),
            macros: vec![NamedMacro {
                source_id: "00112233".to_owned(),
                name: "A".to_owned(),
                definition: MacroDefinition {
                    playback: MacroPlayback::Once,
                    events: vec![MacroEvent::KeyDown {
                        key: "a".to_owned(),
                        delay_ms: 20,
                    }],
                },
            }],
            unresolved_button_assignments: vec![UnresolvedButtonAssignment {
                source_id: "44556677".to_owned(),
                macro_source_id: Some("00112233".to_owned()),
            }],
            ..SoftwareProfile::default()
        };

        let encoded = toml::to_string_pretty(&profile).unwrap();
        let decoded: SoftwareProfile = toml::from_str(&encoded).unwrap();
        assert_eq!(decoded, profile);
    }

    #[test]
    fn authored_profile_sections_round_trip_and_defaults_do_not_reset_omissions() {
        let profile: SoftwareProfile = toml::from_str(include_str!(
            "../../../examples/profiles/pulsefire-raid.toml"
        ))
        .unwrap();
        assert!(!profile.partial);
        assert_eq!(profile.macros[0].source_id, "ab");
        let encoded = toml::to_string_pretty(&profile).unwrap();
        assert_eq!(
            toml::from_str::<SoftwareProfile>(&encoded).unwrap(),
            profile
        );
        let minimal: SoftwareProfile =
            toml::from_str("name = 'Minimal'\ndevice = 'pulsefire-raid'\n[polling]\nhz = 1000\n")
                .unwrap();
        assert!(minimal.dpi.is_none());
        assert!(minimal.buttons.is_empty());
        assert!(minimal.primary_buttons.is_none());
        assert!(minimal.lighting.is_none());
    }
}
