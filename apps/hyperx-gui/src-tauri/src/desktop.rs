//! IPC + native dialogs only. Paths are selected by the operator, not supplied
//! by JavaScript; there is deliberately no Apply/Save-to-mouse command.
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Mutex,
};

use tauri::{Manager, State};
use tauri_plugin_dialog::{DialogExt, MessageDialogButtons, MessageDialogKind};

use hyperx_app::{DraftRecoveryStore, RecoveryClient};

use crate::{Edit, RecoveryDraftSummary, Session, Snapshot};

struct Managed {
    session: Mutex<Session>,
    recovery: Mutex<Option<DraftRecoveryStore>>,
    close_pending: AtomicBool,
}
fn with_session<T>(
    state: &Managed,
    action: impl FnOnce(&mut Session) -> anyhow::Result<T>,
) -> Result<T, String> {
    let mut session = state
        .session
        .lock()
        .map_err(|_| "offline document lock is unavailable".to_owned())?;
    action(&mut session).map_err(|error| format!("{error:#}"))
}

fn with_snapshot(
    state: &Managed,
    action: impl FnOnce(&mut Session) -> anyhow::Result<Snapshot>,
) -> Result<Snapshot, String> {
    let mut session = state
        .session
        .lock()
        .map_err(|_| "offline document lock is unavailable".to_owned())?;
    let prior_revision = session.revision;
    action(&mut session).map_err(|error| format!("{error:#}"))?;
    let mut recovery = state
        .recovery
        .lock()
        .map_err(|_| "draft recovery lock is unavailable".to_owned())?;
    if session.revision != prior_revision {
        if let Some(store) = recovery.as_mut() {
            session.recovery_warning = store.capture(&session.document).err().map(|error| {
                format!(
                    "FILE edit remains in memory, but crash recovery failed: {error:#}. Save NEW now."
                )
            });
        }
    }
    let mut snapshot = session.snapshot();
    if let Some(store) = recovery.as_ref() {
        match store.list() {
            Ok(paths) => {
                snapshot.pending_recovery = paths
                    .into_iter()
                    .filter(|path| store.active_path() != Some(path.as_path()))
                    .map(|path| {
                        let token = path
                            .file_name()
                            .expect("listed recovery path has a filename")
                            .to_string_lossy()
                            .into_owned();
                        match store.load(&path) {
                            Ok(document) => RecoveryDraftSummary {
                                token,
                                profile_name: Some(document.profile().name.clone()),
                                original_file: document
                                    .path()
                                    .map(|path| path.to_string_lossy().into_owned()),
                                error: None,
                            },
                            Err(error) => RecoveryDraftSummary {
                                token,
                                profile_name: None,
                                original_file: None,
                                error: Some(format!("{error:#}")),
                            },
                        }
                    })
                    .collect();
            }
            Err(error) => {
                snapshot.recovery_warning =
                    Some(format!("Cannot list offline recovery snapshots: {error:#}"));
            }
        }
    }
    Ok(snapshot)
}

#[tauri::command]
fn gui_snapshot(state: State<'_, Managed>) -> Result<Snapshot, String> {
    with_snapshot(&state, |session| Ok(session.snapshot()))
}
#[tauri::command]
fn gui_edit(
    state: State<'_, Managed>,
    expected_revision: u64,
    edit: Edit,
) -> Result<Snapshot, String> {
    with_snapshot(&state, |session| session.edit(expected_revision, edit))
}
#[tauri::command]
fn gui_set_local_draft(state: State<'_, Managed>, pending: bool) -> Result<(), String> {
    with_session(&state, |session| {
        session.set_local_draft(pending);
        Ok(())
    })
}
#[tauri::command]
fn gui_reset(
    state: State<'_, Managed>,
    expected_revision: u64,
    discard_changes: bool,
    demo: bool,
) -> Result<Snapshot, String> {
    with_snapshot(&state, |session| {
        session.reset(expected_revision, discard_changes, demo)
    })
}
#[tauri::command]
async fn gui_open_profile(
    app: tauri::AppHandle,
    state: State<'_, Managed>,
    expected_revision: u64,
    discard_changes: bool,
) -> Result<Snapshot, String> {
    with_session(&state, |session| {
        session.check_discard(expected_revision, discard_changes)
    })?;
    // Blocking dialog helpers run off the webview thread; this also works on
    // platforms where the plugin dispatches its native UI to the main thread.
    let selected = tauri::async_runtime::spawn_blocking(move || {
        app.dialog()
            .file()
            .set_title("Open OpenHyperX profile (offline)")
            .add_filter("OpenHyperX TOML", &["toml"])
            .blocking_pick_file()
    })
    .await
    .map_err(|error| error.to_string())?;
    with_snapshot(&state, |session| {
        session.check_discard(expected_revision, discard_changes)?;
        if let Some(file) = selected {
            session.open(expected_revision, discard_changes, &file.into_path()?)
        } else {
            Ok(session.snapshot())
        }
    })
}
#[tauri::command]
async fn gui_save_profile(
    app: tauri::AppHandle,
    state: State<'_, Managed>,
    expected_revision: u64,
) -> Result<Snapshot, String> {
    with_session(&state, |session| session.check_save(expected_revision))?;
    let selected = tauri::async_runtime::spawn_blocking(move || {
        app.dialog()
            .file()
            .set_title("Save NEW OpenHyperX profile — existing files are never overwritten")
            .set_file_name("openhyperx-profile.toml")
            .add_filter("OpenHyperX TOML", &["toml"])
            .blocking_save_file()
    })
    .await
    .map_err(|error| error.to_string())?;
    with_snapshot(&state, |session| {
        session.check_save(expected_revision)?;
        if let Some(file) = selected {
            session.save_new(expected_revision, &file.into_path()?)
        } else {
            Ok(session.snapshot())
        }
    })
}

#[tauri::command]
fn gui_overwrite_profile(
    state: State<'_, Managed>,
    expected_revision: u64,
    confirmed: bool,
) -> Result<Snapshot, String> {
    with_snapshot(&state, |session| {
        session.overwrite_file(expected_revision, confirmed)
    })
}

#[tauri::command]
fn gui_undo_file_edit(
    state: State<'_, Managed>,
    expected_revision: u64,
) -> Result<Snapshot, String> {
    with_snapshot(&state, |session| session.undo_file_edit(expected_revision))
}

#[tauri::command]
fn gui_redo_file_edit(
    state: State<'_, Managed>,
    expected_revision: u64,
) -> Result<Snapshot, String> {
    with_snapshot(&state, |session| session.redo_file_edit(expected_revision))
}

#[tauri::command]
fn gui_restore_recovery(
    state: State<'_, Managed>,
    expected_revision: u64,
    token: String,
) -> Result<Snapshot, String> {
    restore_recovery(&state, expected_revision, &token)
}

fn restore_recovery(
    state: &Managed,
    expected_revision: u64,
    token: &str,
) -> Result<Snapshot, String> {
    if token.is_empty()
        || token.contains('/')
        || token.contains('\\')
        || token == "."
        || token == ".."
    {
        return Err("invalid recovery token".into());
    }
    with_snapshot(state, |session| {
        session.check_discard(expected_revision, false)?;
        let mut recovery = state
            .recovery
            .lock()
            .map_err(|_| anyhow::anyhow!("draft recovery lock is unavailable"))?;
        let store = recovery
            .as_mut()
            .ok_or_else(|| anyhow::anyhow!("draft recovery storage is unavailable"))?;
        let path = store.directory().join(&token);
        let document = store.adopt(&path)?;
        session.restore_recovery(expected_revision, document)
    })
}

#[cfg(test)]
mod recovery_tests {
    use std::{
        fs,
        sync::{
            atomic::{AtomicBool, AtomicUsize, Ordering},
            Mutex,
        },
    };

    use hyperx_app::{parse_profile, DraftRecoveryStore, ProfileDocument, RecoveryClient};

    use super::{restore_recovery, with_snapshot, Managed};
    use crate::{Origin, Session};

    #[test]
    fn backend_lists_and_restores_only_a_selected_private_snapshot() {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let root = std::env::temp_dir().join(format!(
            "openhyperx-gui-recovery-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let directory = root.join("private");
        let mut producer = DraftRecoveryStore::new(&directory, RecoveryClient::Gui).unwrap();
        let profile = parse_profile(include_str!(
            "../../../../examples/profiles/pulsefire-raid.toml"
        ))
        .unwrap();
        let mut document = ProfileDocument::from_profile(profile);
        let mut edited = document.profile().clone();
        edited.name = "GUI recovered draft".into();
        document.replace(edited);
        let old_snapshot = producer.capture(&document).unwrap().unwrap();
        drop(producer);
        let managed = Managed {
            session: Mutex::new(Session::empty()),
            recovery: Mutex::new(Some(
                DraftRecoveryStore::new(&directory, RecoveryClient::Gui).unwrap(),
            )),
            close_pending: AtomicBool::new(false),
        };
        let initial = with_snapshot(&managed, |session| Ok(session.snapshot())).unwrap();
        assert_eq!(initial.pending_recovery.len(), 1);
        let token = initial.pending_recovery[0].token.clone();
        assert_eq!(
            initial.pending_recovery[0].profile_name.as_deref(),
            Some("GUI recovered draft")
        );
        assert!(restore_recovery(&managed, 0, "../not-a-snapshot.toml").is_err());
        let restored = restore_recovery(&managed, 0, &token).unwrap();
        assert_eq!(restored.origin, Origin::Recovered);
        assert!(restored.dirty);
        assert_eq!(restored.profile.name, "GUI recovered draft");
        assert!(restored.pending_recovery.is_empty());
        assert!(!old_snapshot.exists());
        let saved = with_snapshot(&managed, |session| {
            session.save_new(restored.revision, &root.join("saved.toml"))
        })
        .unwrap();
        assert!(!saved.dirty);
        assert!(saved.pending_recovery.is_empty());
        assert!(managed
            .recovery
            .lock()
            .unwrap()
            .as_ref()
            .unwrap()
            .list()
            .unwrap()
            .is_empty());
        fs::remove_dir_all(root).unwrap();
    }
}

pub fn run(demo: bool) {
    let mut session = if demo {
        Session::demo().expect("bundled demo must parse")
    } else {
        Session::empty()
    };
    let recovery = DraftRecoveryStore::default_directory()
        .and_then(|directory| DraftRecoveryStore::new(&directory, RecoveryClient::Gui));
    let recovery = match recovery {
        Ok(store) => Some(store),
        Err(error) => {
            session.recovery_warning = Some(format!(
                "Crash recovery is unavailable: {error:#}. Save NEW frequently."
            ));
            None
        }
    };
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(Managed { session: Mutex::new(session), recovery: Mutex::new(recovery), close_pending: AtomicBool::new(false) })
        .invoke_handler(tauri::generate_handler![gui_snapshot, gui_edit, gui_set_local_draft, gui_reset, gui_open_profile, gui_save_profile, gui_overwrite_profile, gui_undo_file_edit, gui_redo_file_edit, gui_restore_recovery])
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                let state = window.state::<Managed>();
                let (revision, dirty) = state.session.lock()
                    .map(|session| { let (token, dirty) = session.close_guard(); (Some(token), dirty) })
                    .unwrap_or((None, true));
                if !dirty { return; }
                api.prevent_close();
                if state.close_pending.swap(true, Ordering::SeqCst) { return; }
                let app = window.app_handle().clone();
                let window = window.clone();
                std::thread::spawn(move || {
                    let confirmed = app.dialog().message("Discard unsaved file and macro timeline edits and close OpenHyperX? No mouse settings have been changed.")
                        .title("Unsaved offline profile")
                        .kind(MessageDialogKind::Warning)
                        .buttons(MessageDialogButtons::OkCancel)
                        .blocking_show();
                    // A native dialog must not authorize discarding edits made
                    // after it opened. Poisoned locks fail closed as well.
                    if confirmed {
                        let managed = app.state::<Managed>();
                        if let Ok(session) = managed.session.lock() {
                            if Some(session.close_guard().0) == revision {
                                let _ = window.destroy();
                            }
                        };
                    }
                    app.state::<Managed>().close_pending.store(false, Ordering::SeqCst);
                });
            }
        })
        .run(tauri::generate_context!())
        .expect("OpenHyperX desktop runtime failed");
}
