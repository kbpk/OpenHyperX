use super::*;
use hyperx_core::{MacroEvent, MacroPlayback, UnresolvedButtonAssignment};
use std::sync::atomic::{AtomicUsize, Ordering};

struct Files(PathBuf);
impl Files {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "openhyperx-app-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
    fn path(&self, name: &str) -> PathBuf {
        self.0.join(name)
    }
}
impl Drop for Files {
    fn drop(&mut self) {
        for file in fs::read_dir(&self.0).unwrap() {
            fs::remove_file(file.unwrap().path()).unwrap();
        }
        fs::remove_dir(&self.0).unwrap();
    }
}

fn example() -> SoftwareProfile {
    parse_profile(include_str!(
        "../../../examples/profiles/pulsefire-raid.toml"
    ))
    .unwrap()
}

#[test]
fn typed_controls_preserve_unrelated_settings_and_provenance() {
    use ProfileValueEdit as E;
    let mut original = unresolved();
    original.dpi.as_mut().unwrap().source_active_stage = Some(99);
    let modified = edit_profile_value(&original, E::StageDpi { index: 1, dpi: 900 }).unwrap();
    let mut expected = original.clone();
    expected.dpi.as_mut().unwrap().stages[1].x = 900;
    expected.dpi.as_mut().unwrap().stages[1].y = 900;
    assert_eq!(modified, expected);
    let color = hyperx_core::RgbColor::new(11, 22, 33);
    let modified = edit_profile_value(&original, E::StageColor { index: 1, color }).unwrap();
    let mut expected = original.clone();
    expected.dpi.as_mut().unwrap().stages[1].color = color;
    assert_eq!(modified, expected);
    let modified = edit_profile_value(&original, E::ActiveStage(Some(2))).unwrap();
    let mut expected = original.clone();
    expected.dpi.as_mut().unwrap().active_stage = Some(2);
    assert_eq!(modified, expected);
    let modified = edit_profile_value(&original, E::Polling(Some(250))).unwrap();
    let mut expected = original.clone();
    expected.polling = Some(SoftwarePollingProfile { hz: 250 });
    assert_eq!(modified, expected);
    let modified = edit_profile_value(
        &original,
        E::SolidZone {
            zone: "wheel".into(),
            color,
        },
    )
    .unwrap();
    let mut expected = original.clone();
    expected
        .lighting
        .as_mut()
        .unwrap()
        .zones
        .insert("wheel".into(), color);
    assert_eq!(modified, expected);
    let modified = edit_profile_value(
        &original,
        E::PrimaryButtons(Some(PrimaryButtonLayout::Swapped)),
    )
    .unwrap();
    let mut expected = original.clone();
    expected.primary_buttons = Some(PrimaryButtonLayout::Swapped);
    assert_eq!(modified, expected);
}

#[test]
fn typed_controls_reject_invalid_fields_without_repairing_a_draft() {
    use ProfileValueEdit as E;
    let original = unresolved();
    for edit in [
        E::StageDpi { index: 0, dpi: 199 },
        E::StageDpi {
            index: 0,
            dpi: 16001,
        },
        E::StageDpi { index: 0, dpi: 801 },
        E::StageDpi {
            index: 100,
            dpi: 800,
        },
        E::StageColor {
            index: 100,
            color: hyperx_core::RgbColor::BLACK,
        },
        E::ActiveStage(Some(100)),
        E::Polling(Some(2000)),
        E::SolidZone {
            zone: "made-up".into(),
            color: hyperx_core::RgbColor::BLACK,
        },
    ] {
        assert!(edit_profile_value(&original, edit).is_err());
    }
    assert_eq!(original, unresolved());
    let mut full = original.clone();
    for _ in 0..2 {
        full = edit_profile_value(
            &full,
            E::AddStage {
                dpi: 16000,
                color: hyperx_core::RgbColor::BLACK,
            },
        )
        .unwrap();
    }
    assert_eq!(full.dpi.as_ref().unwrap().stages.len(), 5);
    assert!(edit_profile_value(
        &full,
        E::AddStage {
            dpi: 800,
            color: hyperx_core::RgbColor::BLACK
        }
    )
    .is_err());
    full = edit_profile_value(&full, E::ActiveStage(Some(4))).unwrap();
    assert!(edit_profile_value(&full, E::RemoveLastStage).is_err());
    full = edit_profile_value(&full, E::ActiveStage(Some(0))).unwrap();
    while full.dpi.as_ref().unwrap().stages.len() > 1 {
        full = edit_profile_value(&full, E::RemoveLastStage).unwrap();
    }
    assert!(edit_profile_value(&full, E::RemoveLastStage).is_err());
    assert_eq!(full.buttons, original.buttons);
}

#[test]
fn typed_controls_never_fill_missing_fields_with_default_values() {
    use ProfileValueEdit as E;
    let empty = SoftwareProfile {
        name: "Empty".into(),
        device: "pulsefire-raid".into(),
        partial: true,
        ..Default::default()
    };
    let modified = edit_profile_value(
        &empty,
        E::SolidZone {
            zone: "logo".into(),
            color: hyperx_core::RgbColor::BLACK,
        },
    )
    .unwrap();
    assert_eq!(modified.lighting.as_ref().unwrap().zones.len(), 1);
    assert!(modified.dpi.is_none() && modified.polling.is_none());
    assert!(validate_profile(&modified).error.is_some());
    let color = hyperx_core::RgbColor::new(255, 255, 255);
    let modified = edit_profile_value(&empty, E::AddStage { dpi: 800, color }).unwrap();
    assert_eq!(
        modified.dpi.as_ref().unwrap().stages[0],
        hyperx_core::DpiStage::new(800, 800, color)
    );
    assert!(modified.dpi.as_ref().unwrap().active_stage.is_none());
    assert!(modified.polling.is_none() && modified.lighting.is_none());
    let omitted = edit_profile_value(&example(), E::Polling(None)).unwrap();
    assert!(omitted.polling.is_none());
    let unknown = SoftwareProfile {
        device: "future-device".into(),
        ..empty
    };
    assert!(edit_profile_value(&unknown, E::Polling(Some(1000))).is_err());
}
fn unresolved() -> SoftwareProfile {
    let mut profile = example();
    profile.partial = true;
    profile.buttons.remove("button5");
    profile.unresolved_button_assignments = vec![
        UnresolvedButtonAssignment {
            source_id: "runtime:button5".into(),
            macro_source_id: None,
        },
        UnresolvedButtonAssignment {
            source_id: "opaque-legacy".into(),
            macro_source_id: Some("ab".into()),
        },
    ];
    profile
}

#[test]
fn every_section_round_trips_without_modifying_unrelated_fields() {
    let original = example();
    for section in [
        ProfileSection::Performance,
        ProfileSection::Buttons,
        ProfileSection::Macros,
        ProfileSection::Lighting,
        ProfileSection::All,
    ] {
        assert_eq!(
            edit_section(
                &original,
                section,
                &section_toml(&original, section).unwrap()
            )
            .unwrap(),
            original
        );
    }
    let edited = edit_section(
        &original,
        ProfileSection::Performance,
        "[polling]\nhz = 500\n",
    )
    .unwrap();
    assert_eq!(edited.polling.unwrap().hz, 500);
    assert!(edited.dpi.is_none() && edited.primary_buttons.is_none());
    assert_eq!(edited.buttons, original.buttons);
    assert_eq!(edited.macros, original.macros);
    assert_eq!(edited.lighting, original.lighting);
    let edited = edit_section(&original, ProfileSection::Buttons, "").unwrap();
    assert!(edited.buttons.is_empty());
    assert_eq!(edited.dpi, original.dpi);
    assert_eq!(edited.macros, original.macros);
}

#[test]
fn malformed_or_cross_section_edits_are_rejected_without_mutation() {
    let profile = example();
    for (section, text) in [
        (ProfileSection::Performance, "[polling]\nhz = 'oops'"),
        (
            ProfileSection::Performance,
            "[buttons.button4]\ntype = 'disabled'",
        ),
        (
            ProfileSection::Buttons,
            "[buttons.button4]\ntype = 'disabled'\nextra = 1",
        ),
        (
            ProfileSection::Macros,
            "[[macros]]\nid = 'a'\nname = 'A'\nplayback = 'once'\nevents = []\nextra = 1",
        ),
        (
            ProfileSection::Lighting,
            "[lighting]\nmode = 'guessed-effect'\n",
        ),
        (ProfileSection::All, "format_version = 1"),
    ] {
        assert!(edit_section(&profile, section, text).is_err(), "{text}");
    }
    assert_eq!(profile, example());
}

#[test]
fn unsupported_drafts_can_be_edited_and_inspected_without_granting_readiness() {
    let profile = example();
    let invalid = edit_section(
        &profile,
        ProfileSection::Performance,
        "[polling]\nhz = 2000\n",
    )
    .unwrap();
    assert!(validate_profile(&invalid).error.is_some());
    assert!(validate_profile(&profile).error.is_none());
    assert!(validate_profile(&unresolved())
        .error
        .unwrap()
        .contains("unresolved"));
    let mut unknown = profile;
    unknown.device = "unconfirmed-model".into();
    assert!(validate_profile(&unknown)
        .error
        .unwrap()
        .contains("unsupported device"));
    assert!(profile_controls(&unknown.device).is_empty());
}

#[test]
fn explicit_resolution_retains_other_unknowns_and_does_not_change_the_library() {
    let original = unresolved();
    let resolved =
        resolve_macro_assignment(&original, "runtime:button5", "button5", "ab", None).unwrap();
    assert_eq!(resolved.unresolved_button_assignments.len(), 1);
    assert_eq!(
        resolved.buttons["button5"],
        SoftwareButtonBinding::Macro { id: "ab".into() }
    );
    assert_eq!(resolved.macros, original.macros);
    assert_eq!(resolved.dpi, original.dpi);
    assert!(validate_profile(&resolved).error.is_some());
    let omitted = omit_unresolved_assignment(&resolved, "opaque-legacy").unwrap();
    assert!(validate_profile(&omitted).error.is_none());
    assert_eq!(original, unresolved());
}

#[test]
fn resolution_rejects_target_gates_ambiguous_ids_and_implicit_overwrites() {
    let original = unresolved();
    for (source, control, id) in [
        ("missing", "button5", "ab"),
        ("runtime:button5", "button4", "ab"),
        ("runtime:button5", "button5", "missing"),
        ("opaque-legacy", "dpi", "ab"),
        ("opaque-legacy", "left-click", "ab"),
        ("opaque-legacy", "unknown", "ab"),
        ("opaque-legacy", "button4", "ab"), // Already supplied Back binding.
    ] {
        assert!(
            resolve_macro_assignment(&original, source, control, id, None).is_err(),
            "{source} {control} {id}"
        );
    }
    let mut repeated = original.clone();
    repeated.macros[0].definition.playback = MacroPlayback::ToggleRepeat;
    assert!(resolve_macro_assignment(&repeated, "runtime:button5", "button5", "ab", None).is_err());
    let mut duplicate = original.clone();
    duplicate.macros.push(duplicate.macros[0].clone());
    assert!(
        resolve_macro_assignment(&duplicate, "runtime:button5", "button5", "ab", None).is_err()
    );
    let mut duplicate = original.clone();
    duplicate
        .unresolved_button_assignments
        .push(duplicate.unresolved_button_assignments[0].clone());
    assert!(
        resolve_macro_assignment(&duplicate, "runtime:button5", "button5", "ab", None).is_err()
    );
    assert!(omit_unresolved_assignment(&duplicate, "runtime:button5").is_err());
    assert_eq!(original, unresolved());
}

#[test]
fn imported_macro_preserves_chords_and_timings_and_never_replaces_an_id() {
    let original = unresolved();
    let definition = MacroDefinition {
        playback: MacroPlayback::Once,
        events: vec![
            MacroEvent::KeyDown {
                key: "left-shift".into(),
                delay_ms: 0,
            },
            MacroEvent::KeyDown {
                key: "a".into(),
                delay_ms: 37,
            },
            MacroEvent::KeyUp {
                key: "a".into(),
                delay_ms: 11,
            },
            MacroEvent::KeyUp {
                key: "left-shift".into(),
                delay_ms: 0,
            },
        ],
    };
    let resolved = resolve_macro_assignment(
        &original,
        "runtime:button5",
        "button5",
        "chord",
        Some(definition.clone()),
    )
    .unwrap();
    assert_eq!(resolved.macros.last().unwrap().definition, definition);
    assert!(resolve_macro_assignment(
        &original,
        "runtime:button5",
        "button5",
        "ab",
        Some(definition.clone())
    )
    .is_err());
    assert!(import_macro(&original, " ", definition).is_err());
}

#[test]
fn explicit_omission_removes_only_one_provenance_entry_not_a_supplied_binding() {
    let original = unresolved();
    let edited = omit_unresolved_assignment(&original, "runtime:button5").unwrap();
    assert!(!edited.buttons.contains_key("button5"));
    assert_eq!(edited.buttons, original.buttons);
    assert_eq!(edited.macros, original.macros);
    assert!(omit_unresolved_assignment(&original, "unknown").is_err());
}

#[test]
fn bounded_readers_reject_unknown_fields_large_and_non_utf8_files() {
    let files = Files::new();
    let input = files.path("input.toml");
    for source in [
        "format_version = 1",
        "name = 'Bad'\ndevice = 'pulsefire-raid'\nunknown = 1",
        &" ".repeat(MAX_PROFILE_BYTES + 1),
    ] {
        fs::write(&input, source).unwrap();
        assert!(load_profile(&input).is_err());
    }
    fs::write(&input, [0xFF, 0xFE, 0]).unwrap();
    assert!(load_profile(&input).is_err());
    fs::write(&input, "\u{feff}name = 'BOM'\ndevice = 'pulsefire-raid'\n").unwrap();
    assert_eq!(load_profile(&input).unwrap().name, "BOM");
    fs::write(&input, "playback = 'once'\nevents = []\nextra = 1").unwrap();
    assert!(load_macro(&input).is_err());
    fs::write(
        &input,
        include_str!("../../../examples/macros/ab-20ms.toml"),
    )
    .unwrap();
    assert_eq!(load_macro(&input).unwrap().events.len(), 4);
}

#[test]
fn saves_never_overwrite_and_comment_lines_cannot_inject_toml_fields() {
    let files = Files::new();
    let output = files.path("saved.toml");
    let original = example();
    save_profile_new(
        &output,
        &original,
        &["warning\npolling = 'injected'".into()],
    )
    .unwrap();
    assert_eq!(load_profile(&output).unwrap(), original);
    let saved = fs::read_to_string(&output).unwrap();
    assert!(save_profile_new(&output, &unresolved(), &[]).is_err());
    assert_eq!(fs::read_to_string(&output).unwrap(), saved);
    let mut huge = original;
    huge.name = "x".repeat(MAX_PROFILE_BYTES);
    let absent = files.path("oversized.toml");
    assert!(save_profile_new(&absent, &huge, &[]).is_err());
    assert!(!absent.exists());
}

#[test]
fn document_diff_and_save_baseline_track_file_changes_not_hardware() {
    let files = Files::new();
    let output = files.path("new.toml");
    let mut document = ProfileDocument::from_profile(example());
    assert!(!document.dirty());
    let mut edited = document.profile().clone();
    edited.polling = Some(SoftwarePollingProfile { hz: 500 });
    document.replace(edited);
    assert!(document.dirty());
    assert_eq!(document.diff().unwrap().settings[0].field, "polling.hz");
    fs::write(&output, "existing").unwrap();
    assert!(document.save_as(&output).is_err());
    assert!(document.dirty());
    assert!(document.path().is_none());
    let destination = files.path("actual.toml");
    document.save_as(&destination).unwrap();
    assert!(!document.dirty());
    assert!(document.diff().unwrap().settings.is_empty());
    assert_eq!(
        ProfileDocument::open(&destination).unwrap().profile(),
        document.profile()
    );
}

#[test]
fn model_metadata_and_encoding_capabilities_are_available_without_discovery() {
    let controls = profile_controls("pulsefire-raid");
    assert_eq!(controls.len(), 11);
    assert_eq!(controls.iter().filter(|control| control.primary).count(), 2);
    assert_eq!(
        controls
            .iter()
            .filter(|control| control.macros.is_some())
            .count(),
        2
    );
    assert_eq!(
        controls
            .iter()
            .find(|control| control.id == "button5")
            .unwrap()
            .macros
            .unwrap()
            .runtime_playback,
        &[MacroPlayback::Once]
    );
    assert!(device_descriptor("pulsefire-raid").is_some());
    assert!(device_descriptor("unknown").is_none());
}
