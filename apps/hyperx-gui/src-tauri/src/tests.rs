use super::*;
use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicUsize, Ordering},
};

struct Files(PathBuf);
impl Files {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let dir = std::env::temp_dir().join(format!(
            "openhyperx-gui-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&dir).unwrap();
        Self(dir)
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

#[test]
fn empty_session_never_invents_mouse_defaults_or_connected_state() {
    let snapshot = Session::empty().snapshot();
    assert_eq!(snapshot.origin, Origin::Empty);
    assert!(!snapshot.dirty && snapshot.profile.partial);
    assert!(
        snapshot.profile.dpi.is_none()
            && snapshot.profile.polling.is_none()
            && snapshot.profile.lighting.is_none()
    );
    assert!(snapshot.readiness.error.is_some());
    assert_eq!(snapshot.controls.len(), 11);
}
#[test]
fn browser_demo_contract_is_the_actual_rust_snapshot_not_a_second_driver() {
    let expected: serde_json::Value =
        serde_json::from_str(include_str!("../../src/demo.json")).unwrap();
    assert_eq!(
        serde_json::to_value(Session::demo().unwrap().snapshot()).unwrap(),
        expected
    );
}
#[test]
fn typed_gui_edits_use_the_shared_validator_and_preserve_other_fields() {
    let mut session = Session::demo().unwrap();
    let original = session.snapshot().profile;
    let changed = session
        .edit(0, Edit::StageDpi { index: 1, dpi: 900 })
        .unwrap();
    assert!(changed.dirty && changed.revision == 1);
    assert_eq!(changed.profile.buttons, original.buttons);
    assert_eq!(changed.profile.macros, original.macros);
    assert_eq!(changed.profile.lighting, original.lighting);
    assert_eq!(
        changed.profile.dpi.as_ref().unwrap().stages[0],
        original.dpi.as_ref().unwrap().stages[0]
    );
    assert_eq!(changed.changes.settings.len(), 2);
    assert!(session
        .edit(1, Edit::StageDpi { index: 1, dpi: 801 })
        .is_err());
    assert_eq!(session.snapshot().revision, 1);
}
#[test]
fn stale_edits_and_dialog_results_do_not_overwrite_newer_drafts() {
    let files = Files::new();
    let mut session = Session::demo().unwrap();
    session.edit(0, Edit::Polling { hz: Some(250) }).unwrap();
    assert!(session.edit(0, Edit::Polling { hz: Some(500) }).is_err());
    assert!(session.reset(0, true, false).is_err());
    assert!(session.open(0, true, &files.path("missing.toml")).is_err());
    assert!(session.save_new(0, &files.path("stale.toml")).is_err());
    assert!(!files.path("stale.toml").exists());
    assert_eq!(session.snapshot().profile.polling.unwrap().hz, 250);
}
#[test]
fn replacing_documents_requires_explicit_discard_and_failed_open_preserves_edits() {
    let mut session = Session::demo().unwrap();
    session.edit(0, Edit::Polling { hz: Some(500) }).unwrap();
    assert!(session.reset(1, false, false).is_err());
    assert!(session
        .open(1, true, Path::new("missing-openhyperx-profile.toml"))
        .is_err());
    assert!(session.dirty() && session.snapshot().origin == Origin::Demo);
    let snapshot = session.reset(1, true, false).unwrap();
    assert_eq!(snapshot.revision, 2);
    assert!(!snapshot.dirty && snapshot.profile.polling.is_none());
}
#[test]
fn safe_new_file_save_rebaselines_only_on_success_and_never_overwrites() {
    let files = Files::new();
    let path = files.path("profile.toml");
    let mut session = Session::demo().unwrap();
    session
        .edit(
            0,
            Edit::Name {
                name: "My raid".into(),
            },
        )
        .unwrap();
    let saved = session.save_new(1, &path).unwrap();
    let contents = fs::read_to_string(&path).unwrap();
    assert!(!saved.dirty && saved.origin == Origin::File && saved.changes.metadata.is_empty());
    session.edit(2, Edit::Polling { hz: Some(125) }).unwrap();
    assert!(session.save_new(3, &path).is_err());
    assert!(session.dirty());
    assert_eq!(session.snapshot().revision, 3);
    assert_eq!(fs::read_to_string(&path).unwrap(), contents);
}
#[test]
fn incomplete_imports_remain_partial_and_inspectable_without_defaults() {
    let files = Files::new();
    let path = files.path("partial.toml");
    fs::write(&path, "name = 'Partial'\ndevice = 'pulsefire-raid'\npartial = true\n[[unresolved_button_assignments]]\nsource_id = 'runtime:button5'\n").unwrap();
    let mut session = Session::empty();
    session.open(0, false, &path).unwrap();
    let snapshot = session
        .edit(
            1,
            Edit::SolidZone {
                zone: "logo".into(),
                color: RgbColor::BLACK,
            },
        )
        .unwrap();
    assert!(snapshot.profile.partial && snapshot.readiness.error.is_some());
    assert_eq!(snapshot.profile.lighting.unwrap().zones.len(), 1);
    assert_eq!(snapshot.profile.unresolved_button_assignments.len(), 1);
    assert!(snapshot.profile.dpi.is_none() && snapshot.profile.polling.is_none());
}
#[test]
fn forged_json_edits_are_rejected_before_mutation() {
    for json in [
        r#"{"kind":"raw-send","bytes":[1,2,3]}"#,
        r#"{"kind":"polling","hz":250,"firmware":true}"#,
        r#"{"kind":"stage-dpi","index":0,"dpi":-1}"#,
        r#"{"kind":"stage-color","index":0,"color":"€€"}"#,
        r#"{"kind":"remove-last-stage","dpi":800}"#,
        r#"{"kind":"button-binding","control":"button4"}"#,
        r#"{"kind":"button-binding","control":"button4","binding":{"type":"disabled","firmware":true}}"#,
        r#"{"kind":"macro-create","macro":{"source_id":"m","name":"M","playback":"once","events":[],"firmware":true}}"#,
        r#"{"kind":"macro-create","macro":{"source_id":"m","name":"M","playback":"once","events":[{"type":"key-down","key":"a","delay_ms":65536}]}}"#,
        r#"{"kind":"macro-create","macro":{"source_id":"m","name":"M","playback":"once","events":[{"type":"key-down","key":"a","delay_ms":20,"report":[1,2]}]}}"#,
        r#"{"kind":"macro-replace","source_id":"ab","macro":{"source_id":"ab","name":"M","playback":"once","events":[]}}"#,
        r#"{"kind":"macro-remove","source_id":"ab","confirm_references":true}"#,
    ] {
        assert!(serde_json::from_str::<Edit>(json).is_err(), "{json}");
    }
    let mut session = Session::demo().unwrap();
    for name in ["", "\n", "bad\u{1b}name", &"a".repeat(129)] {
        assert!(session.edit(0, Edit::Name { name: name.into() }).is_err());
    }
    assert_eq!(session.snapshot().revision, 0);
}

#[test]
fn macro_ipc_preserves_local_timelines_and_requires_explicit_reference_confirmation() {
    let files = Files::new();
    let mut session = Session::demo().unwrap();
    let original = session.snapshot().profile;
    let definition = original.macros[0].clone();
    let mut changed = definition.clone();
    changed.name = "Custom AB".into();
    changed.definition.events[0] = hyperx_core::MacroEvent::KeyDown {
        key: "a".into(),
        delay_ms: 37,
    };
    let json = serde_json::json!({
        "kind": "macro-replace", "source_id": definition.source_id,
        "macro": changed, "confirm_references": false,
    });
    assert!(session
        .edit(0, serde_json::from_value(json.clone()).unwrap())
        .is_err());
    assert_eq!(session.snapshot().profile, original);
    assert_eq!(session.snapshot().revision, 0);
    let mut confirmed = json;
    confirmed["confirm_references"] = true.into();
    let changed = session
        .edit(0, serde_json::from_value(confirmed).unwrap())
        .unwrap();
    assert_eq!(changed.profile.macros[0].name, "Custom AB");
    assert_eq!(
        changed.profile.macros[0].definition.events[0],
        hyperx_core::MacroEvent::KeyDown {
            key: "a".into(),
            delay_ms: 37
        }
    );
    assert_eq!(changed.profile.buttons, original.buttons);
    assert_eq!(changed.profile.dpi, original.dpi);
    assert_eq!(changed.profile.lighting, original.lighting);
    assert!(session
        .edit(
            0,
            Edit::MacroRemove {
                source_id: definition.source_id.clone()
            }
        )
        .is_err());
    assert!(session
        .edit(
            1,
            Edit::MacroRemove {
                source_id: definition.source_id
            }
        )
        .is_err());
    let mut fresh = changed.profile.macros[0].clone();
    fresh.source_id = "fresh-chord".into();
    fresh.definition.events = vec![
        hyperx_core::MacroEvent::KeyDown {
            key: "left-shift".into(),
            delay_ms: 0,
        },
        hyperx_core::MacroEvent::KeyDown {
            key: "a".into(),
            delay_ms: 49,
        },
        hyperx_core::MacroEvent::KeyUp {
            key: "a".into(),
            delay_ms: 9,
        },
        hyperx_core::MacroEvent::KeyUp {
            key: "left-shift".into(),
            delay_ms: 1,
        },
    ];
    let snapshot = session
        .edit(
            1,
            Edit::MacroCreate {
                definition: fresh.clone(),
            },
        )
        .unwrap();
    assert_eq!(snapshot.profile.macros.last().unwrap(), &fresh);
    assert_eq!(snapshot.macro_keys.len(), 120);
    assert_eq!(snapshot.macro_mouse_buttons, ["left", "right", "middle"]);
    session
        .save_new(2, &files.path("macro-profile.toml"))
        .unwrap();
    assert_eq!(
        hyperx_app::load_profile(&files.path("macro-profile.toml")).unwrap(),
        snapshot.profile
    );
    let removed = session
        .edit(
            3,
            Edit::MacroRemove {
                source_id: "fresh-chord".into(),
            },
        )
        .unwrap();
    assert_eq!(removed.profile, changed.profile);
}

#[test]
fn incomplete_macro_edit_is_preserved_but_cannot_be_newly_assigned() {
    let mut session = Session::demo().unwrap();
    let mut definition = session.snapshot().profile.macros[0].clone();
    definition.source_id = "draft".into();
    definition.definition.events.truncate(1);
    let snapshot = session
        .edit(
            0,
            Edit::MacroCreate {
                definition: definition.clone(),
            },
        )
        .unwrap();
    assert_eq!(snapshot.profile.macros.last().unwrap(), &definition);
    assert!(session
        .edit(
            1,
            Edit::ButtonBinding {
                control: "button4".into(),
                binding: Some(SoftwareButtonBinding::Macro { id: "draft".into() }),
            }
        )
        .is_err());
    assert_eq!(session.snapshot().revision, 1);
    assert!(session.snapshot().binding_choices.iter().any(|entry| {
        matches!(&entry.binding, SoftwareButtonBinding::Macro { id } if id == "draft")
            && entry.error.is_some()
    }));
}

#[test]
fn native_close_guard_includes_local_macro_edits_without_changing_file_revision() {
    let files = Files::new();
    let mut session = Session::demo().unwrap();
    let original = serde_json::to_value(session.snapshot()).unwrap();
    assert!(!session.close_guard().1);
    session.set_local_draft(true);
    let (token, dirty) = session.close_guard();
    assert!(dirty && !session.dirty());
    assert!(session.reset(0, true, false).is_err());
    assert!(session
        .open(0, true, &files.path("not-opened.toml"))
        .is_err());
    assert!(session
        .save_new(0, &files.path("not-created.toml"))
        .is_err());
    assert!(!files.path("not-created.toml").exists());
    assert_eq!(serde_json::to_value(session.snapshot()).unwrap(), original);
    session.set_local_draft(true);
    assert_ne!(session.close_guard().0, token);
    session.set_local_draft(false);
    assert!(!session.close_guard().1);
    session.edit(0, Edit::Polling { hz: Some(250) }).unwrap();
    session.set_local_draft(false);
    assert!(session.close_guard().1);
}
#[test]
fn executable_smoke_path_does_not_need_a_webview_or_device() {
    smoke_test().unwrap();
}

#[test]
fn binding_ipc_uses_target_validation_and_preserves_atomic_session_state() {
    let mut session = Session::demo().unwrap();
    let original = session.snapshot().profile;
    let json = r#"{"kind":"button-binding","control":"button4","binding":{"type":"multimedia","action":"volume-up"}}"#;
    let changed = session
        .edit(0, serde_json::from_str(json).unwrap())
        .unwrap();
    assert_eq!(changed.revision, 1);
    assert_eq!(
        changed.profile.buttons["button4"],
        SoftwareButtonBinding::Multimedia {
            action: hyperx_core::MultimediaFunction::VolumeUp
        }
    );
    assert_eq!(changed.profile.macros, original.macros);
    assert_eq!(changed.profile.dpi, original.dpi);
    for (control, binding) in [
        ("left-click", None),
        (
            "button4",
            Some(SoftwareButtonBinding::Macro {
                id: "missing".into(),
            }),
        ),
    ] {
        assert!(session
            .edit(
                1,
                Edit::ButtonBinding {
                    control: control.into(),
                    binding
                }
            )
            .is_err());
        assert_eq!(session.snapshot().revision, 1);
    }
    let omitted: Edit =
        serde_json::from_str(r#"{"kind":"button-binding","control":"button4","binding":null}"#)
            .unwrap();
    let omitted = session.edit(1, omitted).unwrap();
    assert!(!omitted.profile.buttons.contains_key("button4"));
    assert_eq!(omitted.profile.primary_buttons, original.primary_buttons);
}

#[test]
fn choices_are_deduplicated_presentation_indices_not_write_instructions() {
    let snapshot = Session::demo().unwrap().snapshot();
    assert!(snapshot.binding_choices.len() < 200);
    for control in &snapshot.controls {
        if control.primary {
            assert!(control.bindings.is_empty());
        }
        for &index in &control.bindings {
            assert!(index < snapshot.binding_choices.len());
        }
    }
}

#[test]
fn macro_assignment_round_trips_through_the_real_new_file_writer() {
    let files = Files::new();
    let mut session = Session::demo().unwrap();
    let original = session.snapshot().profile;
    session
        .edit(
            0,
            Edit::ButtonBinding {
                control: "button4".into(),
                binding: Some(SoftwareButtonBinding::Macro { id: "ab".into() }),
            },
        )
        .unwrap();
    let path = files.path("binding.toml");
    session.save_new(1, &path).unwrap();
    let saved = parse_profile(&fs::read_to_string(path).unwrap()).unwrap();
    assert_eq!(
        saved.buttons["button4"],
        SoftwareButtonBinding::Macro { id: "ab".into() }
    );
    assert_eq!(saved.macros, original.macros);
    assert_eq!(saved.dpi, original.dpi);
    assert_eq!(saved.lighting, original.lighting);
}
