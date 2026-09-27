//! IPC + native dialogs only. Paths are selected by the operator, not supplied
//! by JavaScript; there is deliberately no Apply/Save-to-mouse command.
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Mutex,
};

use tauri::{Manager, State};
use tauri_plugin_dialog::{DialogExt, MessageDialogButtons, MessageDialogKind};

use crate::{Edit, Session, Snapshot};

struct Managed {
    session: Mutex<Session>,
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

#[tauri::command]
fn gui_snapshot(state: State<'_, Managed>) -> Result<Snapshot, String> {
    with_session(&state, |session| Ok(session.snapshot()))
}
#[tauri::command]
fn gui_edit(
    state: State<'_, Managed>,
    expected_revision: u64,
    edit: Edit,
) -> Result<Snapshot, String> {
    with_session(&state, |session| session.edit(expected_revision, edit))
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
    with_session(&state, |session| {
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
    with_session(&state, |session| {
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
    with_session(&state, |session| {
        session.check_save(expected_revision)?;
        if let Some(file) = selected {
            session.save_new(expected_revision, &file.into_path()?)
        } else {
            Ok(session.snapshot())
        }
    })
}

pub fn run(demo: bool) {
    let session = if demo {
        Session::demo().expect("bundled demo must parse")
    } else {
        Session::empty()
    };
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(Managed { session: Mutex::new(session), close_pending: AtomicBool::new(false) })
        .invoke_handler(tauri::generate_handler![gui_snapshot, gui_edit, gui_set_local_draft, gui_reset, gui_open_profile, gui_save_profile])
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
