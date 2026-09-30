use std::{
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicUsize, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
use hyperx_app::{DraftRecoveryStore, RecoveryClient};

use super::{app, key};
use crate::{
    app::{App, Modal},
    handle_event,
    local_recovery::LocalRecoveryStore,
    recovery_picker::EntryKind,
};

static NEXT: AtomicUsize = AtomicUsize::new(0);

fn directory() -> PathBuf {
    std::env::temp_dir().join(format!(
        "openhyperx-tui-local-{}-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ))
}
fn attach(tui: &mut App, dir: &Path) {
    tui.recovery = Some(DraftRecoveryStore::new(dir, RecoveryClient::Tui).unwrap());
    tui.local_recovery = Some(LocalRecoveryStore::new(dir).unwrap());
}
fn local_path(tui: &App) -> PathBuf {
    tui.local_recovery
        .as_ref()
        .unwrap()
        .list()
        .unwrap()
        .pop()
        .unwrap()
}
fn choose_local(tui: &mut App, path: &Path) {
    tui.tab = 4;
    tui.handle_key(key(KeyCode::F(7)));
    let Modal::Recovery(picker) = tui.modal.as_mut().unwrap() else {
        panic!()
    };
    picker.select(
        picker
            .entries
            .iter()
            .position(|entry| entry.path == path)
            .unwrap(),
    );
    assert_eq!(picker.entries[picker.selected].kind, EntryKind::LocalEditor);
    handle_event(tui, Event::Key(key(KeyCode::Char('r'))));
    handle_event(tui, Event::Key(key(KeyCode::Char('y'))));
}

#[test]
fn unaccepted_profile_name_is_restored_only_into_the_matching_document() {
    let dir = directory();
    let mut first = app();
    attach(&mut first, &dir);
    first.tab = 4;
    handle_event(&mut first, Event::Key(key(KeyCode::F(3))));
    handle_event(&mut first, Event::Paste("Recovered name".into()));
    assert_eq!(first.document.profile().name, "Raid example");
    let saved = local_path(&first);
    assert_eq!(
        first
            .local_recovery
            .as_ref()
            .unwrap()
            .load(&saved)
            .unwrap()
            .kind(),
        "unfinished field"
    );
    drop(first);

    let mut other = app();
    other.document.replace({
        let mut changed = other.document.profile().clone();
        changed.name = "Different document".into();
        changed
    });
    attach(&mut other, &dir);
    choose_local(&mut other, &saved);
    assert_eq!(other.document.profile().name, "Different document");
    let Modal::Recovery(picker) = other.modal.as_ref().unwrap() else {
        panic!()
    };
    assert!(picker
        .error
        .as_deref()
        .unwrap()
        .contains("matching profile"));
    assert!(saved.exists());
    drop(other);

    let mut restored = app();
    attach(&mut restored, &dir);
    choose_local(&mut restored, &saved);
    let Modal::Editor { editor, .. } = restored.modal.as_ref().unwrap() else {
        panic!()
    };
    assert_eq!(editor.text(), "Recovered name");
    assert_eq!(restored.document.profile().name, "Raid example");
    handle_event(&mut restored, Event::Key(key(KeyCode::Enter)));
    assert_eq!(restored.document.profile().name, "Recovered name");
    assert!(restored
        .local_recovery
        .as_ref()
        .unwrap()
        .list()
        .unwrap()
        .is_empty());
    assert_eq!(restored.recovery.as_ref().unwrap().list().unwrap().len(), 1);
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn unaccepted_macro_timeline_and_nested_name_field_survive_restart() {
    let dir = directory();
    let mut first = app();
    attach(&mut first, &dir);
    first.tab = 2;
    handle_event(&mut first, Event::Key(key(KeyCode::Insert)));
    handle_event(&mut first, Event::Key(key(KeyCode::Insert)));
    handle_event(&mut first, Event::Key(key(KeyCode::F(3))));
    handle_event(&mut first, Event::Key(key(KeyCode::F(2))));
    handle_event(&mut first, Event::Paste("Recovered macro".into()));
    assert_eq!(first.document.profile().macros.len(), 1);
    let saved = local_path(&first);
    assert_eq!(
        first
            .local_recovery
            .as_ref()
            .unwrap()
            .load(&saved)
            .unwrap()
            .kind(),
        "unfinished macro timeline"
    );
    drop(first);

    let mut restored = app();
    attach(&mut restored, &dir);
    choose_local(&mut restored, &saved);
    let Modal::Macro(editor) = restored.modal.as_ref().unwrap() else {
        panic!()
    };
    assert_eq!(editor.draft.definition.events.len(), 1);
    assert_eq!(
        editor.draft.definition.playback,
        hyperx_core::MacroPlayback::ToggleRepeat
    );
    let Some(crate::macros::Prompt::Text { editor: name, .. }) = &editor.prompt else {
        panic!()
    };
    assert_eq!(name.text(), "Recovered macro");
    assert_eq!(restored.document.profile().macros.len(), 1);
    handle_event(&mut restored, Event::Key(key(KeyCode::Enter)));
    handle_event(
        &mut restored,
        Event::Key(KeyEvent::new(KeyCode::Char('s'), KeyModifiers::CONTROL)),
    );
    assert_eq!(restored.document.profile().macros.len(), 2);
    assert_eq!(
        restored.document.profile().macros[1].name,
        "Recovered macro"
    );
    assert_eq!(
        restored.document.profile().macros[1]
            .definition
            .events
            .len(),
        1
    );
    assert!(restored
        .local_recovery
        .as_ref()
        .unwrap()
        .list()
        .unwrap()
        .is_empty());
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn unfinished_macro_input_search_remains_a_preview_after_restore() {
    let dir = directory();
    let mut first = app();
    attach(&mut first, &dir);
    first.tab = 2;
    handle_event(&mut first, Event::Key(key(KeyCode::Insert)));
    handle_event(&mut first, Event::Key(key(KeyCode::Insert)));
    handle_event(&mut first, Event::Key(key(KeyCode::F(5))));
    handle_event(&mut first, Event::Paste("shift".into()));
    let saved = local_path(&first);
    drop(first);

    let mut restored = app();
    attach(&mut restored, &dir);
    choose_local(&mut restored, &saved);
    let Modal::Macro(editor) = restored.modal.as_ref().unwrap() else {
        panic!()
    };
    let Some(crate::macros::Prompt::Input {
        search, selected, ..
    }) = &editor.prompt
    else {
        panic!()
    };
    assert_eq!(search.text(), "shift");
    assert_eq!(*selected, None);
    assert_eq!(restored.document.profile().macros.len(), 1);
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn failed_snapshot_write_keeps_editor_in_memory_and_previous_copy_on_disk() {
    let dir = directory();
    let mut tui = app();
    attach(&mut tui, &dir);
    tui.tab = 4;
    handle_event(&mut tui, Event::Key(key(KeyCode::F(3))));
    handle_event(&mut tui, Event::Paste("First version".into()));
    let old = local_path(&tui);
    let moved = dir.with_extension("away");
    fs::rename(&dir, &moved).unwrap();
    handle_event(&mut tui, Event::Paste(" second".into()));
    let Modal::Editor { editor, .. } = tui.modal.as_ref().unwrap() else {
        panic!()
    };
    assert_eq!(editor.text(), "First version second");
    assert!(tui.status.contains("LOCAL EDITOR RECOVERY FAILED"));
    fs::rename(&moved, &dir).unwrap();
    assert!(old.exists());
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn restored_open_path_cannot_reuse_a_previous_discard_confirmation() {
    let dir = directory();
    let mut first = app();
    let mut changed = first.document.profile().clone();
    changed.name = "Unsaved source".into();
    first.document.replace(changed);
    attach(&mut first, &dir);
    handle_event(&mut first, Event::Key(key(KeyCode::Char('o'))));
    handle_event(&mut first, Event::Key(key(KeyCode::Char('y'))));
    handle_event(&mut first, Event::Paste("never-open-this.toml".into()));
    let saved = local_path(&first);
    drop(first);

    let mut restored = app();
    let mut changed = restored.document.profile().clone();
    changed.name = "Unsaved source".into();
    restored.document.replace(changed);
    attach(&mut restored, &dir);
    choose_local(&mut restored, &saved);
    handle_event(&mut restored, Event::Key(key(KeyCode::Enter)));
    assert_eq!(restored.document.profile().name, "Unsaved source");
    let Modal::Editor { error, .. } = restored.modal.as_ref().unwrap() else {
        panic!()
    };
    assert!(error
        .as_deref()
        .unwrap()
        .contains("normal Open confirmation"));
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn cancel_retires_only_this_sessions_local_snapshot() {
    let dir = directory();
    let mut tui = app();
    attach(&mut tui, &dir);
    tui.tab = 4;
    handle_event(&mut tui, Event::Key(key(KeyCode::F(3))));
    handle_event(&mut tui, Event::Paste("Unaccepted".into()));
    let active = local_path(&tui);
    assert!(active.exists());
    handle_event(&mut tui, Event::Key(key(KeyCode::Esc)));
    assert!(tui.modal.is_none());
    assert_eq!(tui.document.profile().name, "Raid example");
    assert!(!active.exists());
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn corrupt_local_snapshot_is_visible_but_never_restored_implicitly() {
    let dir = directory();
    let mut tui = app();
    attach(&mut tui, &dir);
    let corrupt = dir.join("local-tui-corrupt.json");
    fs::write(&corrupt, "{invalid").unwrap();
    tui.tab = 4;
    tui.handle_key(key(KeyCode::F(7)));
    let Modal::Recovery(picker) = tui.modal.as_ref().unwrap() else {
        panic!()
    };
    assert_eq!(picker.entries.len(), 1);
    assert!(picker.entries[0].error.is_some());
    tui.handle_key(key(KeyCode::Char('r')));
    let Modal::Recovery(picker) = tui.modal.as_ref().unwrap() else {
        panic!()
    };
    assert!(picker.error.as_deref().unwrap().contains("Unreadable"));
    assert!(corrupt.exists());
    tui.handle_key(key(KeyCode::Char('d')));
    tui.handle_key(key(KeyCode::Char('y')));
    assert!(!corrupt.exists());
    assert_eq!(tui.document.profile().name, "Raid example");
    fs::remove_dir_all(dir).unwrap();
}
