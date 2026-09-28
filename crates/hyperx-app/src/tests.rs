use super::*;
use hyperx_core::{MacroEvent, MacroPlayback, UnresolvedButtonAssignment};

#[test]
fn offline_validation_exposes_typed_file_locations_without_parsing_error_text() {
    let baseline = parse_profile(include_str!(
        "../../../examples/profiles/pulsefire-raid.toml"
    ))
    .unwrap();
    assert!(validate_profile(&baseline).field.is_none());
    let mut cases = Vec::new();

    let mut profile = baseline.clone();
    profile.device = "unknown-model".into();
    cases.push((profile, "device"));

    let mut profile = baseline.clone();
    profile
        .unresolved_button_assignments
        .push(UnresolvedButtonAssignment {
            source_id: "unknown-source".into(),
            macro_source_id: None,
        });
    cases.push((profile, "unresolved_button_assignments"));

    let mut profile = baseline.clone();
    profile.dpi.as_mut().unwrap().stages.clear();
    cases.push((profile, "dpi.stages"));

    let mut profile = baseline.clone();
    profile.dpi.as_mut().unwrap().active_stage = Some(99);
    cases.push((profile, "dpi.active_stage"));

    let mut profile = baseline.clone();
    profile.dpi.as_mut().unwrap().stages[0].x = 0;
    cases.push((profile, "dpi.stages[0].x"));

    let mut profile = baseline.clone();
    profile.dpi.as_mut().unwrap().stages[0].y = 0;
    cases.push((profile, "dpi.stages[0].y"));

    let mut profile = baseline.clone();
    profile.dpi.as_mut().unwrap().stages[0].x = 900;
    cases.push((profile, "dpi.stages[0]"));

    let mut profile = baseline.clone();
    profile.polling.as_mut().unwrap().hz = 2000;
    cases.push((profile, "polling.hz"));

    let mut profile = baseline.clone();
    profile.macros[0].source_id.clear();
    cases.push((profile, "macros[0].source_id"));

    let mut profile = baseline.clone();
    profile.macros[0].definition.events.truncate(1);
    cases.push((profile, "macros[0]"));

    let mut profile = baseline.clone();
    profile.buttons.insert(
        "unknown".into(),
        hyperx_core::SoftwareButtonBinding::Disabled {},
    );
    cases.push((profile, "buttons.unknown"));

    let mut profile = baseline.clone();
    profile.buttons.insert(
        "button4".into(),
        hyperx_core::SoftwareButtonBinding::Keyboard {
            key: "bad-key".into(),
        },
    );
    cases.push((profile, "buttons.button4.key"));

    let mut profile = baseline.clone();
    profile.buttons.insert(
        "button4".into(),
        hyperx_core::SoftwareButtonBinding::Macro {
            id: "missing".into(),
        },
    );
    cases.push((profile, "buttons.button4.id"));

    let mut profile = baseline.clone();
    profile.lighting.as_mut().unwrap().zones.remove("wheel");
    cases.push((profile, "lighting.zones"));

    let mut profile = baseline.clone();
    profile.dpi = None;
    profile.polling = None;
    profile.primary_buttons = None;
    profile.buttons.clear();
    profile.lighting = None;
    profile.macros.clear();
    cases.push((profile, "profile"));

    for (profile, expected_field) in cases {
        let result = validate_profile(&profile);
        assert!(result.error.is_some(), "{expected_field} was accepted");
        assert_eq!(
            result.field.as_deref(),
            Some(expected_field),
            "{:?}",
            result.error
        );
    }
}
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
        fs::remove_dir_all(&self.0).unwrap();
    }
}

fn example() -> SoftwareProfile {
    parse_profile(include_str!(
        "../../../examples/profiles/pulsefire-raid.toml"
    ))
    .unwrap()
}

#[test]
fn profile_naming_is_a_bounded_metadata_edit_not_a_target_capability() {
    let mut profile = example();
    profile.device = "future-mouse".into();
    profile.partial = true;
    profile
        .unresolved_button_assignments
        .push(UnresolvedButtonAssignment {
            source_id: "legacy-source".into(),
            macro_source_id: Some("missing-definition".into()),
        });
    let mut expected = profile.clone();
    expected.name = "Zażółć gęślą jaźń 🖱".into();
    assert_eq!(rename_profile(&profile, &expected.name).unwrap(), expected);
    assert_eq!(rename_profile(&profile, &profile.name).unwrap(), profile);
    for name in [
        "",
        "   ",
        "a\nb",
        "x\u{1b}y",
        &"x".repeat(129),
        &"ą".repeat(65),
    ] {
        assert!(rename_profile(&profile, name).is_err());
    }
    assert!(rename_profile(&profile, &"ą".repeat(64)).is_ok());
    assert_eq!(profile.name, "Raid example");
}

fn library_macro(id: &str) -> NamedMacro {
    NamedMacro {
        source_id: id.into(),
        name: "Shift A with individual delays".into(),
        definition: MacroDefinition {
            playback: MacroPlayback::Once,
            events: vec![
                MacroEvent::KeyDown {
                    key: "left_shift".into(),
                    delay_ms: 0,
                },
                MacroEvent::KeyDown {
                    key: "A".into(),
                    delay_ms: 37,
                },
                MacroEvent::KeyUp {
                    key: "A".into(),
                    delay_ms: 12,
                },
                MacroEvent::KeyUp {
                    key: "left_shift".into(),
                    delay_ms: 9999,
                },
            ],
        },
    }
}

#[test]
fn macro_library_edits_preserve_chords_delays_and_every_other_profile_field() {
    use ProfileValueEdit as E;
    let mut original = unresolved();
    original.polling = Some(SoftwarePollingProfile { hz: 123 });
    let definition = library_macro("new-chord");
    let edited = edit_profile_value(
        &original,
        E::MacroCreate {
            definition: definition.clone(),
        },
    )
    .unwrap();
    let mut expected = original.clone();
    expected.macros.push(definition.clone());
    assert_eq!(edited, expected);
    assert_eq!(
        parse_profile(&encode_profile(&edited).unwrap()).unwrap(),
        edited
    );
    validate_button_binding(
        &edited,
        "button4",
        &SoftwareButtonBinding::Macro {
            id: "new-chord".into(),
        },
    )
    .unwrap();
    let mut changed = definition;
    changed.name = "Renamed chord".into();
    changed.definition.playback = MacroPlayback::ToggleRepeat;
    changed.definition.events.swap(1, 2);
    let replaced = edit_profile_value(
        &edited,
        E::MacroReplace {
            source_id: "new-chord".into(),
            definition: changed.clone(),
            confirm_references: false,
        },
    )
    .unwrap();
    expected.macros.last_mut().unwrap().clone_from(&changed);
    assert_eq!(replaced, expected);
    let removed = edit_profile_value(
        &replaced,
        E::MacroRemove {
            source_id: "new-chord".into(),
        },
    )
    .unwrap();
    assert_eq!(removed, original);
}

#[test]
fn macro_library_requires_reference_consent_and_never_silently_resolves_imports() {
    use ProfileValueEdit as E;
    let mut original = example();
    original.buttons.insert(
        "button4".into(),
        SoftwareButtonBinding::Macro { id: "ab".into() },
    );
    original
        .unresolved_button_assignments
        .push(UnresolvedButtonAssignment {
            source_id: "opaque-import".into(),
            macro_source_id: Some("ab".into()),
        });
    let references = macro_references(&original, "ab");
    assert_eq!(
        references,
        [
            "control: button4",
            "control: button5",
            "unresolved source: opaque-import"
        ]
    );
    let replacement = library_macro("ab");
    assert!(edit_profile_value(
        &original,
        E::MacroReplace {
            source_id: "ab".into(),
            definition: replacement.clone(),
            confirm_references: false,
        }
    )
    .is_err());
    assert!(edit_profile_value(
        &original,
        E::MacroRemove {
            source_id: "ab".into()
        }
    )
    .is_err());
    let edited = edit_profile_value(
        &original,
        E::MacroReplace {
            source_id: "ab".into(),
            definition: replacement.clone(),
            confirm_references: true,
        },
    )
    .unwrap();
    let mut expected = original.clone();
    *expected
        .macros
        .iter_mut()
        .find(|entry| entry.source_id == "ab")
        .unwrap() = replacement;
    assert_eq!(edited, expected);
    let same = edited
        .macros
        .iter()
        .find(|entry| entry.source_id == "ab")
        .unwrap()
        .clone();
    assert_eq!(
        edit_profile_value(
            &edited,
            E::MacroReplace {
                source_id: "ab".into(),
                definition: same,
                confirm_references: false,
            }
        )
        .unwrap(),
        edited
    );
    // Even a currently missing definition's ID is reserved by its references.
    original
        .unresolved_button_assignments
        .push(UnresolvedButtonAssignment {
            source_id: "second-import".into(),
            macro_source_id: Some("missing".into()),
        });
    assert!(edit_profile_value(
        &original,
        E::MacroCreate {
            definition: library_macro("missing")
        }
    )
    .is_err());
    original.buttons.insert(
        "button6".into(),
        SoftwareButtonBinding::Macro {
            id: "missing-button".into(),
        },
    );
    assert!(edit_profile_value(
        &original,
        E::MacroCreate {
            definition: library_macro("missing-button")
        }
    )
    .is_err());
}

#[test]
fn incomplete_macro_drafts_remain_files_not_executable_assignments() {
    use ProfileValueEdit as E;
    let original = example();
    let mut definition = library_macro("unfinished");
    definition.definition.events.truncate(1);
    let edited = edit_profile_value(
        &original,
        E::MacroCreate {
            definition: definition.clone(),
        },
    )
    .unwrap();
    assert!(validate_button_binding(
        &edited,
        "button4",
        &SoftwareButtonBinding::Macro {
            id: "unfinished".into()
        }
    )
    .is_err());
    // The file data model is wider than the capture-backed encoder. Do not
    // truncate imported timelines or conflate a file save with hardware support.
    definition.definition.events = vec![
        MacroEvent::KeyDown {
            key: "unknown-imported-key".into(),
            delay_ms: u16::MAX
        };
        15
    ];
    let edited = edit_profile_value(
        &edited,
        E::MacroReplace {
            source_id: "unfinished".into(),
            definition: definition.clone(),
            confirm_references: false,
        },
    )
    .unwrap();
    assert_eq!(edited.macros.last().unwrap(), &definition);
    let choices = profile_binding_choices(&edited, "button4");
    assert!(choices.iter().any(
        |entry| matches!(&entry.binding, SoftwareButtonBinding::Macro { id } if id == "unfinished")
            && entry.error.is_some()
    ));
    let files = Files::new();
    let path = files.path("unfinished.toml");
    save_profile_new(&path, &edited, &[]).unwrap();
    assert_eq!(load_profile(&path).unwrap(), edited);
    assert_eq!(edited.buttons, original.buttons);
}

#[test]
fn macro_library_rejects_ambiguous_identity_invalid_names_and_oversized_storage() {
    use ProfileValueEdit as E;
    let original = example();
    for id in ["ab", "", " ", "bad\u{1b}id", &"x".repeat(257)] {
        assert!(edit_profile_value(
            &original,
            E::MacroCreate {
                definition: library_macro(id)
            }
        )
        .is_err());
    }
    for name in ["", "\t", "bad\nname", &"a".repeat(129)] {
        let mut definition = library_macro("new");
        definition.name = name.into();
        assert!(edit_profile_value(&original, E::MacroCreate { definition }).is_err());
    }
    assert!(edit_profile_value(
        &original,
        E::MacroReplace {
            source_id: "ab".into(),
            definition: library_macro("new-id"),
            confirm_references: true,
        }
    )
    .is_err());
    assert!(edit_profile_value(
        &original,
        E::MacroRemove {
            source_id: "missing".into()
        }
    )
    .is_err());
    let mut ambiguous = original.clone();
    ambiguous.macros.push(ambiguous.macros[0].clone());
    for edit in [
        E::MacroRemove {
            source_id: "ab".into(),
        },
        E::MacroReplace {
            source_id: "ab".into(),
            definition: library_macro("ab"),
            confirm_references: true,
        },
    ] {
        assert!(edit_profile_value(&ambiguous, edit).is_err());
    }
    let mut definition = library_macro("large");
    definition.definition.events = vec![MacroEvent::KeyDown {
        key: "x".repeat(MAX_PROFILE_BYTES),
        delay_ms: 20,
    }];
    assert!(edit_profile_value(&original, E::MacroCreate { definition }).is_err());
    assert_eq!(original, example());
}

#[test]
fn macro_input_metadata_is_semantic_and_model_scoped() {
    let keys = macro_keyboard_names("pulsefire-raid");
    assert_eq!(keys.len(), 120);
    assert!(keys
        .iter()
        .all(|key| key.parse::<hyperx_core::KeyboardUsage>().is_ok()));
    assert_eq!(
        macro_mouse_button_names("pulsefire-raid"),
        ["left", "right", "middle"]
    );
    assert!(macro_keyboard_names("unknown").is_empty());
    assert!(macro_mouse_button_names("unknown").is_empty());
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
fn resolution_target_metadata_and_commit_share_gates_without_guessing_a_timeline() {
    let original = unresolved();
    let targets = macro_resolution_targets(&original, "runtime:button5").unwrap();
    assert_eq!(targets.len(), profile_controls(&original.device).len());
    for target in targets {
        assert_eq!(target.error.is_none(), target.control.id == "button5");
        assert_eq!(
            target.error.is_none(),
            resolve_macro_assignment(&original, "runtime:button5", target.control.id, "ab", None)
                .is_ok()
        );
    }
    let mut profile = original.clone();
    profile.buttons.remove("button4");
    profile.macros[0].definition.playback = MacroPlayback::ToggleRepeat;
    let targets = macro_resolution_targets(&profile, "opaque-legacy").unwrap();
    assert!(targets
        .iter()
        .find(|target| target.control.id == "button5")
        .unwrap()
        .error
        .is_none());
    // Target legality is not playback legality; the real definition is required.
    assert!(resolve_macro_assignment(&profile, "opaque-legacy", "button5", "ab", None).is_err());
    assert!(resolve_macro_assignment(&profile, "opaque-legacy", "button4", "ab", None).is_ok());
    profile.device = "unknown-mouse".into();
    assert!(macro_resolution_targets(&profile, "opaque-legacy")
        .unwrap()
        .is_empty());
    assert!(macro_resolution_targets(&profile, "missing").is_err());
    profile
        .unresolved_button_assignments
        .push(profile.unresolved_button_assignments[0].clone());
    assert!(macro_resolution_targets(&profile, "runtime:button5").is_err());
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
fn overwrite_preserves_exact_old_file_and_requires_a_fresh_file_baseline() {
    let files = Files::new();
    let path = files.path("custom.toml");
    let original = format!(
        "# user comment and formatting\n{}",
        encode_profile(&example()).unwrap()
    );
    fs::write(&path, &original).unwrap();
    let mut document = ProfileDocument::open(&path).unwrap();
    let mut edited = document.profile().clone();
    edited.name = "Edited file".into();
    document.replace(edited.clone());
    let backup = document.save_overwrite_with_backup().unwrap();
    assert_eq!(fs::read(&backup).unwrap(), original.as_bytes());
    assert_eq!(load_profile(&path).unwrap(), edited);
    assert!(!document.dirty());
    assert_eq!(document.path(), Some(path.as_path()));
    assert!(backup.parent().unwrap().join("replacement.toml").exists());
    assert!(document.save_overwrite_with_backup().is_err());

    let mut next = document.profile().clone();
    next.name = "Second edit".into();
    document.replace(next.clone());
    let second_backup = document.save_overwrite_with_backup().unwrap();
    assert_ne!(backup, second_backup);
    assert_eq!(load_profile(&second_backup).unwrap(), edited);
    assert_eq!(load_profile(&path).unwrap(), next);
}

#[test]
fn overwrite_refuses_external_changes_and_keeps_document_dirty() {
    let files = Files::new();
    let path = files.path("stale.toml");
    save_profile_new(&path, &example(), &[]).unwrap();
    let mut document = ProfileDocument::open(&path).unwrap();
    let mut edited = document.profile().clone();
    edited.name = "My draft".into();
    document.replace(edited);
    fs::write(
        &path,
        format!("# external edit\n{}", encode_profile(&example()).unwrap()),
    )
    .unwrap();
    let current = fs::read(&path).unwrap();
    let error = document
        .save_overwrite_with_backup()
        .unwrap_err()
        .to_string();
    assert!(error.contains("changed on disk"), "{error}");
    assert_eq!(fs::read(&path).unwrap(), current);
    assert!(document.dirty());
    assert_eq!(fs::read_dir(&files.0).unwrap().count(), 1);
}

#[test]
fn overwrite_requires_an_existing_regular_file_and_never_overwrites_a_symlink() {
    let files = Files::new();
    let mut unnamed = ProfileDocument::from_profile(example());
    let mut edited = unnamed.profile().clone();
    edited.name = "Edited".into();
    unnamed.replace(edited);
    assert!(unnamed.save_overwrite_with_backup().is_err());

    let path = files.path("missing.toml");
    save_profile_new(&path, &example(), &[]).unwrap();
    let mut document = ProfileDocument::open(&path).unwrap();
    let mut edited = document.profile().clone();
    edited.name = "Edited".into();
    document.replace(edited);
    fs::remove_file(&path).unwrap();
    assert!(document.save_overwrite_with_backup().is_err());
    assert!(!path.exists());

    #[cfg(unix)]
    {
        use std::os::unix::fs::symlink;
        let target = files.path("target.toml");
        save_profile_new(&target, &example(), &[]).unwrap();
        let link = files.path("link.toml");
        symlink(&target, &link).unwrap();
        let mut linked = ProfileDocument::open(&link).unwrap();
        let mut edited = linked.profile().clone();
        edited.name = "Edited".into();
        linked.replace(edited);
        assert!(linked.save_overwrite_with_backup().is_err());
        assert_eq!(load_profile(&target).unwrap(), example());
    }
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

#[test]
fn binding_choices_cover_named_actions_and_all_canonical_keyboard_usages() {
    use hyperx_core::KeyboardUsage;
    use std::collections::BTreeSet;
    let profile = example();
    for control in profile_controls(&profile.device) {
        let choices = profile_binding_choices(&profile, control.id);
        if control.primary {
            assert!(choices.is_empty());
            continue;
        }
        let mut keys = BTreeSet::new();
        let mut ordinary = 0;
        for choice in &choices {
            if !matches!(choice.binding, SoftwareButtonBinding::Macro { .. }) {
                ordinary += 1;
                assert!(choice.error.is_none(), "{}: {:?}", control.id, choice);
                validate_button_binding(&profile, control.id, &choice.binding).unwrap();
            }
            if let SoftwareButtonBinding::Keyboard { key } = &choice.binding {
                assert!(
                    keys.insert(key.parse::<KeyboardUsage>().unwrap().0),
                    "duplicate key {key}"
                );
            }
        }
        assert_eq!(ordinary, 144); // 10 mouse + 7 media + 6 shortcuts + disabled + 120 keys
        assert_eq!(keys, (0x04..=0x73).chain(0xE0..=0xE7).collect());
    }
    assert!(profile_binding_choices(&profile, "unknown").is_empty());
}

#[test]
fn typed_binding_edits_preserve_other_fields_and_distinguish_omission_from_disabled() {
    use ProfileValueEdit::ButtonBinding as E;
    let mut original = unresolved();
    original.polling = Some(SoftwarePollingProfile { hz: 123 }); // unrelated invalid draft
    let modified = edit_profile_value(
        &original,
        E {
            control: "button7".into(),
            binding: Some(SoftwareButtonBinding::Keyboard {
                key: "RETURN".into(),
            }),
        },
    )
    .unwrap();
    let mut expected = original.clone();
    expected.buttons.insert(
        "button7".into(),
        SoftwareButtonBinding::Keyboard {
            key: "RETURN".into(),
        },
    );
    assert_eq!(modified, expected); // importing/picking never normalizes aliases
    assert!(validate_profile(&modified).error.is_some());
    let disabled = edit_profile_value(
        &modified,
        E {
            control: "button7".into(),
            binding: Some(SoftwareButtonBinding::Disabled {}),
        },
    )
    .unwrap();
    assert!(matches!(
        disabled.buttons["button7"],
        SoftwareButtonBinding::Disabled {}
    ));
    let omitted = edit_profile_value(
        &disabled,
        E {
            control: "button7".into(),
            binding: None,
        },
    )
    .unwrap();
    assert_eq!(omitted, original);
    assert_eq!(
        encode_profile(&omitted).unwrap(),
        encode_profile(&original).unwrap()
    );
}

#[test]
fn binding_validation_rejects_primary_targets_bad_keys_and_ambiguous_macros_atomically() {
    use ProfileValueEdit::ButtonBinding as E;
    let original = example();
    for (control, binding) in [
        ("left-click", Some(SoftwareButtonBinding::Disabled {})),
        ("right-click", None),
        ("unknown", None),
        (
            "button4",
            Some(SoftwareButtonBinding::Keyboard { key: "a+b".into() }),
        ),
        (
            "button4",
            Some(SoftwareButtonBinding::Macro {
                id: "missing".into(),
            }),
        ),
        (
            "dpi",
            Some(SoftwareButtonBinding::Macro { id: "ab".into() }),
        ),
    ] {
        assert!(edit_profile_value(
            &original,
            E {
                control: control.into(),
                binding
            }
        )
        .is_err());
    }
    let mut duplicate = original.clone();
    duplicate.macros.push(duplicate.macros[0].clone());
    assert!(edit_profile_value(
        &duplicate,
        E {
            control: "button4".into(),
            binding: Some(SoftwareButtonBinding::Macro { id: "ab".into() })
        }
    )
    .is_err());
    assert!(profile_binding_choices(&duplicate, "button4")
        .iter()
        .filter(|choice| matches!(choice.binding, SoftwareButtonBinding::Macro { .. }))
        .all(|choice| choice.error.is_some()));
}

#[test]
fn macro_binding_choices_gate_playback_and_timelines_per_target() {
    use ProfileValueEdit::ButtonBinding as E;
    let mut profile = example();
    profile.macros[0].definition.playback = MacroPlayback::ToggleRepeat;
    let binding = SoftwareButtonBinding::Macro { id: "ab".into() };
    for (control, legal) in [("button4", true), ("button5", false), ("dpi", false)] {
        let choices = profile_binding_choices(&profile, control);
        let choice = choices
            .iter()
            .find(|choice| choice.binding == binding)
            .unwrap();
        assert_eq!(choice.error.is_none(), legal);
        assert_eq!(
            edit_profile_value(
                &profile,
                E {
                    control: control.into(),
                    binding: Some(binding.clone())
                }
            )
            .is_ok(),
            legal
        );
    }
    profile.macros[0].definition.events.pop(); // unbalanced down/up must not become assignable
    assert!(validate_button_binding(&profile, "button4", &binding).is_err());
    let unknown = SoftwareProfile {
        device: "unknown".into(),
        ..Default::default()
    };
    assert!(profile_binding_choices(&unknown, "button4").is_empty());
}
