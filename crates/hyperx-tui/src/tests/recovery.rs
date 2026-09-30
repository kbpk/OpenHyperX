use std::{
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicUsize, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

use crossterm::event::{Event, KeyCode, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use hyperx_app::{DraftRecoveryStore, ProfileDocument, RecoveryClient};

use super::{app, key, screen};
use crate::{app::Modal, handle_event, local_recovery::LocalRecoveryStore, widgets::Action};

static NEXT: AtomicUsize = AtomicUsize::new(0);

fn directory() -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "openhyperx-tui-picker-{}-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    path
}

fn snapshot(path: &Path, name: &str) -> PathBuf {
    let mut profile = app().document.profile().clone();
    let mut document = ProfileDocument::from_profile(profile.clone());
    profile.name = name.into();
    document.replace(profile);
    let mut store = DraftRecoveryStore::new(path, RecoveryClient::Tui).unwrap();
    store.capture(&document).unwrap().unwrap()
}

#[test]
fn recovery_panel_requires_confirmation_and_retains_displaced_dirty_draft() {
    let dir = directory();
    let previous = snapshot(&dir, "Earlier profile");
    let mut tui = app();
    let mut edited = tui.document.profile().clone();
    edited.name = "Current unsaved profile".into();
    tui.document.replace(edited);
    let mut current = DraftRecoveryStore::new(&dir, RecoveryClient::Tui).unwrap();
    let current_path = current.capture(&tui.document).unwrap().unwrap();
    tui.recovery = Some(current);
    tui.local_recovery = Some(LocalRecoveryStore::new(&dir).unwrap());
    tui.tab = 4;
    tui.handle_key(key(KeyCode::F(7)));
    let Modal::Recovery(picker) = tui.modal.as_mut().unwrap() else {
        panic!("recovery picker must be open")
    };
    picker.select(
        picker
            .entries
            .iter()
            .position(|entry| entry.path == previous)
            .unwrap(),
    );
    assert!(screen(&mut tui, 45, 12).contains("[Restore r]"));
    handle_event(&mut tui, Event::Key(key(KeyCode::Char('r'))));
    assert!(screen(&mut tui, 45, 12).contains("stays recoverable"));
    handle_event(&mut tui, Event::Key(key(KeyCode::Char('n'))));
    assert_eq!(tui.document.profile().name, "Current unsaved profile");
    handle_event(&mut tui, Event::Key(key(KeyCode::Char('r'))));
    handle_event(&mut tui, Event::Key(key(KeyCode::Char('y'))));
    assert_eq!(tui.document.profile().name, "Earlier profile");
    assert!(tui.document.dirty());
    assert!(
        !current_path.exists(),
        "fresh current snapshot supersedes stale active copy"
    );
    assert!(
        !previous.exists(),
        "adopted snapshot is superseded atomically"
    );
    let store = tui.recovery.as_ref().unwrap();
    let paths = store.list().unwrap();
    assert_eq!(paths.len(), 2);
    assert!(paths
        .iter()
        .any(|path| store.load(path).unwrap().profile().name == "Current unsaved profile"));
    assert!(paths
        .iter()
        .any(|path| store.load(path).unwrap().profile().name == "Earlier profile"));
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn recovery_panel_refuses_active_discard_and_can_discard_a_corrupt_old_snapshot() {
    let dir = directory();
    let mut tui = app();
    let mut edited = tui.document.profile().clone();
    edited.name = "Current unsaved profile".into();
    tui.document.replace(edited);
    let mut store = DraftRecoveryStore::new(&dir, RecoveryClient::Tui).unwrap();
    let active = store.capture(&tui.document).unwrap().unwrap();
    let corrupt = dir.join("draft-tui-corrupt.toml");
    fs::write(&corrupt, "not a recovery snapshot").unwrap();
    tui.recovery = Some(store);
    tui.local_recovery = Some(LocalRecoveryStore::new(&dir).unwrap());
    tui.tab = 4;
    tui.handle_key(key(KeyCode::F(7)));
    {
        let Modal::Recovery(picker) = tui.modal.as_mut().unwrap() else {
            panic!()
        };
        picker.select(
            picker
                .entries
                .iter()
                .position(|entry| entry.path == active)
                .unwrap(),
        );
    }
    tui.handle_key(key(KeyCode::Char('d')));
    tui.handle_key(key(KeyCode::Char('y')));
    assert!(active.exists());
    let Modal::Recovery(picker) = tui.modal.as_ref().unwrap() else {
        panic!()
    };
    assert!(picker.error.as_deref().unwrap().contains("active"));
    {
        let Modal::Recovery(picker) = tui.modal.as_mut().unwrap() else {
            panic!()
        };
        picker.select(
            picker
                .entries
                .iter()
                .position(|entry| entry.path == corrupt)
                .unwrap(),
        );
    }
    tui.handle_key(key(KeyCode::Char('r')));
    let Modal::Recovery(picker) = tui.modal.as_ref().unwrap() else {
        panic!()
    };
    assert!(picker.error.as_deref().unwrap().contains("Unreadable"));
    tui.handle_key(key(KeyCode::Char('d')));
    tui.handle_key(key(KeyCode::Char('y')));
    assert!(!corrupt.exists());
    assert!(active.exists());
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn restore_refuses_to_displace_a_dirty_draft_when_fresh_snapshot_fails() {
    let dir = directory();
    let _previous = snapshot(&dir, "Earlier profile");
    let mut tui = app();
    let mut edited = tui.document.profile().clone();
    edited.name = "Unsaved latest edit".into();
    tui.document.replace(edited);
    tui.recovery = Some(DraftRecoveryStore::new(&dir, RecoveryClient::Tui).unwrap());
    tui.local_recovery = Some(LocalRecoveryStore::new(&dir).unwrap());
    tui.tab = 4;
    tui.handle_key(key(KeyCode::F(7)));
    let moved = dir.with_extension("away");
    fs::rename(&dir, &moved).unwrap();
    tui.handle_key(key(KeyCode::Char('r')));
    tui.handle_key(key(KeyCode::Char('y')));
    assert_eq!(tui.document.profile().name, "Unsaved latest edit");
    assert!(tui.document.dirty());
    let Modal::Recovery(picker) = tui.modal.as_ref().unwrap() else {
        panic!()
    };
    assert!(picker.error.as_deref().unwrap().contains("save NEW"));
    fs::rename(&moved, &dir).unwrap();
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn mouse_selects_visible_snapshot_without_restoring_it() {
    let dir = directory();
    let _first = snapshot(&dir, "First profile");
    let second = snapshot(&dir, "Second profile");
    let mut tui = app();
    tui.recovery = Some(DraftRecoveryStore::new(&dir, RecoveryClient::Tui).unwrap());
    tui.local_recovery = Some(LocalRecoveryStore::new(&dir).unwrap());
    tui.tab = 4;
    tui.handle_key(key(KeyCode::F(7)));
    screen(&mut tui, 45, 12);
    let hit = tui
        .hits
        .iter()
        .find(|hit| matches!(hit.action, Action::RecoveryEntry(1)))
        .unwrap()
        .area;
    tui.handle_mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: hit.x,
        row: hit.y,
        modifiers: KeyModifiers::NONE,
    });
    let Modal::Recovery(picker) = tui.modal.as_ref().unwrap() else {
        panic!()
    };
    assert_eq!(picker.entries[picker.selected].path, second);
    assert_eq!(tui.document.profile().name, "Raid example");
    fs::remove_dir_all(dir).unwrap();
}
