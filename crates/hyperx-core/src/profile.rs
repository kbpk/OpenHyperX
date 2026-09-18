use serde::{Deserialize, Serialize};

use crate::{DpiStage, MacroDefinition};

/// A portable application profile.
///
/// A profile may be partial when it was imported from a format whose fields
/// are not completely understood. Loading a profile never implies permission
/// to write it to a device or to onboard memory.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SoftwareProfile {
    pub name: String,
    pub device: String,
    pub partial: bool,
    pub source: Option<SoftwareProfileSource>,
    pub dpi: Option<SoftwareDpiProfile>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub macros: Vec<NamedMacro>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub unresolved_button_assignments: Vec<UnresolvedButtonAssignment>,
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
pub struct NamedMacro {
    pub source_id: String,
    pub name: String,
    #[serde(flatten)]
    pub definition: MacroDefinition,
}

/// Source assignment retained until its physical control is decoded.
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
                format: "ngenuity-hxp".to_owned(),
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
        };

        let encoded = toml::to_string_pretty(&profile).unwrap();
        let decoded: SoftwareProfile = toml::from_str(&encoded).unwrap();
        assert_eq!(decoded, profile);
    }
}
