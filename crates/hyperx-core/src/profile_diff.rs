//! Pure comparison of profile files, not a plan to change a device.
use std::{collections::BTreeMap, fmt};

use crate::{KeyboardUsage, MacroEvent, SoftwareButtonBinding, SoftwareProfile};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SoftwareProfileFieldChange {
    pub field: String,
    /// None means absent in this file, not a request to reset the device.
    pub before: Option<String>,
    pub after: Option<String>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct SoftwareProfileDiff {
    pub settings: Vec<SoftwareProfileFieldChange>,
    pub metadata: Vec<SoftwareProfileFieldChange>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SoftwareProfileDiffError(pub String);

impl fmt::Display for SoftwareProfileDiffError {
    fn fmt(&self, output: &mut fmt::Formatter<'_>) -> fmt::Result {
        output.write_str(&self.0)
    }
}

impl std::error::Error for SoftwareProfileDiffError {}

type Fields = BTreeMap<String, String>;

/// Match macros by stable ID, ignoring library/table order and known keyboard
/// spelling aliases. Event order and every delay are significant. Inspection
/// accepts partial/unresolved profiles; it does NOT grant hardware support.
pub fn diff_software_profiles(
    before: &SoftwareProfile,
    after: &SoftwareProfile,
) -> Result<SoftwareProfileDiff, SoftwareProfileDiffError> {
    let (before_settings, before_metadata) = fields(before)?;
    let (after_settings, after_metadata) = fields(after)?;
    Ok(SoftwareProfileDiff {
        settings: compare(before_settings, after_settings),
        metadata: compare(before_metadata, after_metadata),
    })
}

fn compare(before: Fields, after: Fields) -> Vec<SoftwareProfileFieldChange> {
    let mut remaining = after;
    let mut changes = Vec::new();
    for (field, left) in before {
        let right = remaining.remove(&field);
        if right.as_ref() != Some(&left) {
            changes.push(SoftwareProfileFieldChange {
                field,
                before: Some(left),
                after: right,
            });
        }
    }
    changes.extend(
        remaining
            .into_iter()
            .map(|(field, value)| SoftwareProfileFieldChange {
                field,
                before: None,
                after: Some(value),
            }),
    );
    changes.sort_by(|left, right| left.field.cmp(&right.field));
    changes
}

fn fields(profile: &SoftwareProfile) -> Result<(Fields, Fields), SoftwareProfileDiffError> {
    let mut settings = Fields::new();
    let mut metadata = Fields::from([
        ("name".into(), format!("{:?}", profile.name)),
        ("device".into(), profile.device.clone()),
        ("partial".into(), profile.partial.to_string()),
    ]);
    if let Some(source) = &profile.source {
        metadata.insert("source.format".into(), source.format.clone());
        metadata.insert(
            "source.format_version".into(),
            source.format_version.to_string(),
        );
    }
    if let Some(dpi) = &profile.dpi {
        settings.insert("dpi.stages.count".into(), dpi.stages.len().to_string());
        if let Some(active) = dpi.active_stage {
            settings.insert("dpi.active_stage (zero-based)".into(), active.to_string());
        }
        if let Some(source_active) = dpi.source_active_stage {
            metadata.insert("dpi.source_active_stage".into(), source_active.to_string());
        }
        for (index, stage) in dpi.stages.iter().enumerate() {
            let prefix = format!("dpi.stages[{}]", index + 1);
            settings.insert(format!("{prefix}.x"), stage.x.to_string());
            settings.insert(format!("{prefix}.y"), stage.y.to_string());
            settings.insert(format!("{prefix}.color"), stage.color.to_string());
        }
    }
    if let Some(polling) = profile.polling {
        settings.insert("polling.hz".into(), polling.hz.to_string());
    }
    if let Some(primary) = profile.primary_buttons {
        settings.insert("primary_buttons".into(), format!("{primary:?}"));
    }
    for (control, binding) in &profile.buttons {
        let value = match binding {
            SoftwareButtonBinding::Keyboard { key } => format!("keyboard {}", key_value(key)),
            SoftwareButtonBinding::Macro { id } => format!("macro {id:?}"),
            other => format!("{other:?}"),
        };
        settings.insert(format!("buttons[{control:?}]"), value);
    }
    if let Some(lighting) = &profile.lighting {
        settings.insert("lighting.mode".into(), format!("{:?}", lighting.mode));
        for (zone, color) in &lighting.zones {
            settings.insert(format!("lighting.zones[{zone:?}]"), color.to_string());
        }
    }
    let mut macro_ids = std::collections::BTreeSet::new();
    for named in &profile.macros {
        if named.source_id.trim().is_empty() || !macro_ids.insert(&named.source_id) {
            return Err(SoftwareProfileDiffError(format!(
                "profile {:?}: macro IDs must be nonempty and unique; ambiguous ID {:?}",
                profile.name, named.source_id
            )));
        }
        let prefix = format!("macros[{:?}]", named.source_id);
        metadata.insert(format!("{prefix}.name"), format!("{:?}", named.name));
        settings.insert(
            format!("{prefix}.playback"),
            named.definition.playback.to_string(),
        );
        settings.insert(
            format!("{prefix}.events.count"),
            named.definition.events.len().to_string(),
        );
        for (index, event) in named.definition.events.iter().enumerate() {
            let (action, input, delay) = match event {
                MacroEvent::KeyDown { key, delay_ms } => ("key-down", key_value(key), delay_ms),
                MacroEvent::KeyUp { key, delay_ms } => ("key-up", key_value(key), delay_ms),
                MacroEvent::MouseButtonDown { button, delay_ms } => {
                    ("mouse-button-down", mouse_value(button), delay_ms)
                }
                MacroEvent::MouseButtonUp { button, delay_ms } => {
                    ("mouse-button-up", mouse_value(button), delay_ms)
                }
            };
            let event_prefix = format!("{prefix}.events[{}]", index + 1);
            settings.insert(format!("{event_prefix}.action"), action.into());
            settings.insert(format!("{event_prefix}.input"), input);
            settings.insert(format!("{event_prefix}.delay_ms"), delay.to_string());
        }
    }
    // These are unresolved provenance, never interpreted as physical bindings.
    // Keep each source record (including duplicates) visible rather than losing
    // one through an ID-keyed map; the source list order is significant here.
    for (index, assignment) in profile.unresolved_button_assignments.iter().enumerate() {
        let prefix = format!("unresolved_button_assignments[{}]", index + 1);
        metadata.insert(
            format!("{prefix}.source_id"),
            format!("{:?}", assignment.source_id),
        );
        if let Some(reference) = &assignment.macro_source_id {
            metadata.insert(
                format!("{prefix}.macro_source_id"),
                format!("{reference:?}"),
            );
        }
    }
    Ok((settings, metadata))
}

fn key_value(key: &str) -> String {
    key.parse::<KeyboardUsage>().map_or_else(
        |_| format!("unrecognized key {key:?}"),
        |usage| format!("HID 0x{:04X}", usage.0),
    )
}

fn mouse_value(button: &str) -> String {
    let normalized = button.trim().to_ascii_lowercase().replace('_', "-");
    match normalized.as_str() {
        "left" | "right" | "middle" => normalized,
        "wheel" => "middle".into(),
        _ => format!("unrecognized mouse button {button:?}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{MacroDefinition, MacroPlayback, NamedMacro, SoftwarePollingProfile};

    fn profile() -> SoftwareProfile {
        SoftwareProfile {
            name: "Test".into(),
            device: "pulsefire-raid".into(),
            ..SoftwareProfile::default()
        }
    }

    fn named(id: &str) -> NamedMacro {
        NamedMacro {
            source_id: id.into(),
            name: id.into(),
            definition: MacroDefinition {
                playback: MacroPlayback::Once,
                events: vec![
                    MacroEvent::KeyDown {
                        key: "left-ctrl".into(),
                        delay_ms: 0,
                    },
                    MacroEvent::KeyUp {
                        key: "left-ctrl".into(),
                        delay_ms: 20,
                    },
                ],
            },
        }
    }

    #[test]
    fn compares_omissions_and_metadata_without_claiming_a_device_reset() {
        let mut before = profile();
        before.polling = Some(SoftwarePollingProfile { hz: 1000 });
        let mut after = profile();
        after.name = "Renamed".into();
        let diff = diff_software_profiles(&before, &after).unwrap();
        assert_eq!(
            diff.settings,
            [SoftwareProfileFieldChange {
                field: "polling.hz".into(),
                before: Some("1000".into()),
                after: None
            }]
        );
        assert_eq!(diff.metadata[0].field, "name");
    }

    #[test]
    fn matches_macros_by_id_and_normalizes_only_known_key_aliases() {
        let mut before = profile();
        before.macros = vec![named("a"), named("b")];
        before.buttons.insert(
            "button4".into(),
            SoftwareButtonBinding::Keyboard {
                key: "left-ctrl".into(),
            },
        );
        let mut after = before.clone();
        after.macros.reverse();
        after.macros[0].definition.events[0] = MacroEvent::KeyDown {
            key: "left-control".into(),
            delay_ms: 0,
        };
        after.buttons.insert(
            "button4".into(),
            SoftwareButtonBinding::Keyboard {
                key: "LEFT_CONTROL".into(),
            },
        );
        assert_eq!(
            diff_software_profiles(&before, &after).unwrap(),
            SoftwareProfileDiff::default()
        );
        after.macros[0].definition.events[1] = MacroEvent::KeyUp {
            key: "a".into(),
            delay_ms: 37,
        };
        let changes = diff_software_profiles(&before, &after).unwrap().settings;
        assert_eq!(changes.len(), 2);
        assert!(changes
            .iter()
            .any(|change| change.field == "macros[\"b\"].events[2].delay_ms"
                && change.after.as_deref() == Some("37")));
    }

    #[test]
    fn preserves_event_order_playback_chords_and_unknown_names_in_the_comparison() {
        let mut before = profile();
        before.macros.push(named("a"));
        let mut after = before.clone();
        after.macros[0].definition.playback = MacroPlayback::ToggleRepeat;
        after.macros[0].definition.events.reverse();
        let changes = diff_software_profiles(&before, &after).unwrap().settings;
        assert!(changes
            .iter()
            .any(|change| change.field.ends_with(".playback")));
        assert!(changes
            .iter()
            .any(|change| change.field.ends_with(".action")));
        before.buttons.insert(
            "button4".into(),
            SoftwareButtonBinding::Keyboard {
                key: "future-key-A".into(),
            },
        );
        after.buttons.insert(
            "button4".into(),
            SoftwareButtonBinding::Keyboard {
                key: "future-key-B".into(),
            },
        );
        assert!(diff_software_profiles(&before, &after)
            .unwrap()
            .settings
            .iter()
            .any(|change| change.field == "buttons[\"button4\"]"));
    }

    #[test]
    fn rejects_ambiguous_macro_ids_instead_of_dropping_definitions() {
        let mut invalid = profile();
        invalid.macros = vec![named("a"), named("a")];
        assert!(diff_software_profiles(&profile(), &invalid).is_err());
        invalid.macros = vec![named("")];
        assert!(diff_software_profiles(&invalid, &profile()).is_err());
    }
}
