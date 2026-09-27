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
fn executable_smoke_path_does_not_need_a_webview_or_device() {
    smoke_test().unwrap();
}
