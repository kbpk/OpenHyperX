use super::*;
use crate::{
    files::{FileBrowser, Mode, Prompt},
    widgets::{Action, Hit},
};
use crossterm::event::{MouseButton, MouseEvent, MouseEventKind};
use std::{
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicUsize, Ordering},
};

struct Files(PathBuf);
impl Files {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "openhyperx-tui-browser-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(fs::canonicalize(path).unwrap())
    }
    fn path(&self, name: &str) -> PathBuf {
        self.0.join(name)
    }
    fn profile(&self, name: &str) -> PathBuf {
        let path = self.path(name);
        hyperx_app::save_profile_new(&path, app().document.profile(), &[]).unwrap();
        path
    }
}
impl Drop for Files {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}
fn browser(app: &App) -> &FileBrowser {
    let Some(Modal::Files(browser)) = &app.modal else {
        panic!("file browser expected")
    };
    browser
}
fn browse(app: &mut App, directory: &Path, mode: Mode) {
    app.modal = Some(Modal::Files(Box::new(
        FileBrowser::new(directory, mode, app.document.path()).unwrap(),
    )));
}
fn choose(app: &mut App, name: &str) {
    let Some(Modal::Files(browser)) = &mut app.modal else {
        panic!("file browser expected")
    };
    let index = browser
        .entries
        .iter()
        .position(|entry| entry.path.file_name().unwrap() == name)
        .unwrap();
    browser.select(index);
}
fn set_field(app: &mut App, text: &str) {
    let Some(Modal::Files(browser)) = &mut app.modal else {
        panic!("file browser expected")
    };
    let editor = browser.editor_mut().unwrap();
    *editor = Editor::new(text, true);
}

#[test]
fn overwrite_file_requires_explicit_confirmation_and_keeps_recovery_copy() {
    let files = Files::new();
    let path = files.profile("original.toml");
    let old = fs::read(&path).unwrap();
    let mut app = App::new(ProfileDocument::open(&path).unwrap(), false);
    app.tab = 4;
    let mut edited = app.document.profile().clone();
    edited.name = "Overwritten draft".into();
    app.document.replace(edited.clone());
    app.handle_key(key(KeyCode::F(6)));
    assert!(matches!(app.modal, Some(Modal::Overwrite { .. })));
    app.handle_key(key(KeyCode::Char('n')));
    assert!(app.modal.is_none());
    assert_eq!(fs::read(&path).unwrap(), old);
    assert!(app.document.dirty());

    click(&mut app, |action| {
        matches!(action, Action::Key(KeyCode::F(6)))
    });
    click(&mut app, |action| {
        matches!(action, Action::Key(KeyCode::Char('y')))
    });
    assert!(app.modal.is_none());
    assert!(!app.document.dirty());
    assert_eq!(hyperx_app::load_profile(&path).unwrap(), edited);
    let backup = files.path("original.toml.openhyperx-backup-0001/original.toml");
    assert_eq!(fs::read(&backup).unwrap(), old);
    assert!(app.status.contains("No mouse settings changed"));
}

#[test]
fn overwrite_file_rejects_stale_disk_state_without_losing_the_draft() {
    let files = Files::new();
    let path = files.profile("stale.toml");
    let mut app = App::new(ProfileDocument::open(&path).unwrap(), false);
    app.tab = 4;
    let mut edited = app.document.profile().clone();
    edited.name = "Local edit".into();
    app.document.replace(edited);
    fs::write(
        &path,
        format!("# external change\n{}", fs::read_to_string(&path).unwrap()),
    )
    .unwrap();
    let external = fs::read(&path).unwrap();
    app.handle_key(key(KeyCode::F(6)));
    app.handle_key(key(KeyCode::Char('y')));
    assert!(
        matches!(&app.modal, Some(Modal::Overwrite { error: Some(error), .. }) if error.contains("changed on disk"))
    );
    assert_eq!(fs::read(&path).unwrap(), external);
    assert!(app.document.dirty());
    app.handle_key(key(KeyCode::Esc));
    assert!(app.modal.is_none());
}
fn click(app: &mut App, action: impl Fn(&Action) -> bool) {
    screen(app, 120, 40);
    let Hit { area, .. } = app
        .hits
        .iter()
        .find(|hit| action(&hit.action))
        .unwrap()
        .clone();
    app.handle_mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: area.x,
        row: area.y,
        modifiers: KeyModifiers::NONE,
    });
}

#[test]
fn browsing_is_lazy_and_failed_opens_keep_the_dirty_document_before_confirmation() {
    let files = Files::new();
    let original = files.profile("original.toml");
    let target = files.profile("żółty profil.TOML");
    fs::write(files.path("bad.toml"), "[broken").unwrap();
    fs::write(
        files.path("large.toml"),
        "x".repeat(hyperx_app::MAX_PROFILE_BYTES + 1),
    )
    .unwrap();
    fs::write(files.path("non-utf8.toml"), [0xff, 0xfe]).unwrap();
    let original_bytes = fs::read(&original).unwrap();
    let mut app = App::new(ProfileDocument::open(&original).unwrap(), false);
    let mut edited = app.document.profile().clone();
    edited.name = "Unsaved".into();
    app.document.replace(edited.clone());
    app.tab = 4;
    app.handle_key(key(KeyCode::F(2)));
    assert!(browser(&app).error.is_none());
    for name in ["bad.toml", "large.toml", "non-utf8.toml"] {
        choose(&mut app, name);
        assert_eq!(app.document.profile(), &edited);
        app.handle_key(key(KeyCode::Enter));
        assert!(browser(&app).error.is_some());
        assert!(browser(&app).pending.is_none());
        assert_eq!(app.document.profile(), &edited);
        assert_eq!(app.document.path(), Some(original.as_path()));
        assert!(app.document.dirty());
    }
    choose(&mut app, "żółty profil.TOML");
    app.handle_key(key(KeyCode::Enter));
    assert!(browser(&app).pending.is_some());
    assert_eq!(app.document.profile(), &edited);
    assert!(screen(&mut app, 120, 40).contains("Discard unsaved FILE edits"));
    app.handle_key(key(KeyCode::Char('n')));
    assert!(browser(&app).pending.is_none());
    assert_eq!(app.document.profile(), &edited);
    app.handle_key(key(KeyCode::Enter));
    // Confirmation adopts exactly the parsed snapshot, not a second unseen read.
    fs::write(&target, "malformed now").unwrap();
    app.handle_key(key(KeyCode::Char('y')));
    assert!(app.modal.is_none());
    assert_eq!(app.document.path(), Some(target.as_path()));
    assert_eq!(app.document.profile().name, "Raid example");
    assert!(!app.document.dirty() && !app.demo);
    assert_eq!(fs::read(original).unwrap(), original_bytes);
}

#[test]
fn rename_and_new_copy_preserve_all_settings_provenance_and_existing_files() {
    let files = Files::new();
    let original = files.profile("original.toml");
    let existing = files.profile("existing.toml");
    let original_bytes = fs::read(&original).unwrap();
    let existing_bytes = fs::read(&existing).unwrap();
    let mut app = App::new(ProfileDocument::open(&original).unwrap(), false);
    let mut draft = app.document.profile().clone();
    draft.partial = true;
    draft
        .unresolved_button_assignments
        .push(hyperx_core::UnresolvedButtonAssignment {
            source_id: "imported-unknown".into(),
            macro_source_id: Some("ab".into()),
        });
    app.document.replace(draft.clone());
    app.tab = 4;
    click(&mut app, |action| {
        matches!(action, Action::Key(KeyCode::F(3)))
    });
    set_editor(&mut app, "   ");
    app.handle_key(key(KeyCode::Enter));
    assert!(matches!(
        app.modal,
        Some(Modal::Editor { error: Some(_), .. })
    ));
    assert_eq!(app.document.profile(), &draft);
    set_editor(&mut app, "Nowy profil żółty");
    app.handle_key(key(KeyCode::Enter));
    draft.name = "Nowy profil żółty".into();
    assert_eq!(app.document.profile(), &draft);
    assert_eq!(app.document.path(), Some(original.as_path()));
    assert!(app.document.diff().unwrap().settings.is_empty());
    click(&mut app, |action| {
        matches!(action, Action::Key(KeyCode::F(4)))
    });
    app.handle_key(key(KeyCode::F(2)));
    assert_eq!(
        browser(&app)
            .prompt
            .as_ref()
            .map(|prompt| match prompt {
                Prompt::Filename(editor) | Prompt::Directory(editor) => editor.text(),
            })
            .unwrap(),
        "original-copy.toml"
    );
    set_field(&mut app, "existing.toml");
    app.handle_key(key(KeyCode::Enter));
    assert!(browser(&app)
        .error
        .as_ref()
        .unwrap()
        .contains("never overwritten"));
    assert_eq!(app.document.profile(), &draft);
    assert_eq!(app.document.path(), Some(original.as_path()));
    assert!(app.document.dirty());
    for name in [
        "",
        "  ",
        ".",
        "..",
        "../outside.toml",
        "sub\\outside.toml",
        "C:outside.toml",
        "a/b.toml",
    ] {
        set_field(&mut app, name);
        app.handle_key(key(KeyCode::Enter));
        assert!(browser(&app)
            .error
            .as_ref()
            .unwrap()
            .contains("one new filename"));
        assert_eq!(app.document.profile(), &draft);
    }
    set_field(&mut app, "hidden-copy.txt");
    app.handle_key(key(KeyCode::Enter));
    assert!(browser(&app)
        .error
        .as_ref()
        .unwrap()
        .contains(".toml filename"));
    assert!(!files.path("hidden-copy.txt").exists());
    set_field(&mut app, "żółta kopia.toml");
    app.handle_key(key(KeyCode::Enter));
    assert!(app.modal.is_none());
    let copy = files.path("żółta kopia.toml");
    assert_eq!(app.document.path(), Some(copy.as_path()));
    assert_eq!(hyperx_app::load_profile(&copy).unwrap(), draft);
    assert_eq!(app.document.profile(), &draft);
    assert!(!app.document.dirty());
    assert_eq!(fs::read(&original).unwrap(), original_bytes);
    assert_eq!(fs::read(&existing).unwrap(), existing_bytes);
}

#[test]
fn directory_navigation_mouse_previews_and_refresh_never_modify_a_profile() {
    let files = Files::new();
    fs::create_dir(files.path("subdir")).unwrap();
    fs::write(files.path("ignored.txt"), "data").unwrap();
    files.profile("Z.toml");
    files.profile("A.TOML");
    let mut app = app();
    let before = app.document.profile().clone();
    browse(&mut app, &files.0, Mode::Open);
    assert_eq!(browser(&app).entries.len(), 3);
    assert!(browser(&app).entries[0].directory);
    click(&mut app, |action| matches!(action, Action::FileEntry(2)));
    assert_eq!(browser(&app).selected, 2);
    assert_eq!(app.document.profile(), &before);
    app.handle_mouse(MouseEvent {
        kind: MouseEventKind::ScrollUp,
        column: 0,
        row: 0,
        modifiers: KeyModifiers::NONE,
    });
    assert_eq!(browser(&app).selected, 0);
    app.handle_key(key(KeyCode::Enter));
    assert_eq!(browser(&app).directory, files.path("subdir"));
    assert!(browser(&app).entries.is_empty());
    assert!(screen(&mut app, 120, 40).contains("No subdirectories or TOML files"));
    app.handle_key(key(KeyCode::Backspace));
    assert_eq!(browser(&app).directory, files.0);
    app.handle_key(key(KeyCode::Char('p')));
    set_field(&mut app, "does-not-exist");
    app.handle_key(key(KeyCode::Enter));
    assert!(browser(&app).error.is_some());
    assert_eq!(browser(&app).directory, files.0);
    app.handle_key(key(KeyCode::Esc));
    choose(&mut app, "subdir");
    app.handle_key(key(KeyCode::Right));
    fs::remove_dir(files.path("subdir")).unwrap();
    app.handle_key(key(KeyCode::F(5)));
    assert!(browser(&app).error.is_some());
    assert_eq!(browser(&app).directory, files.path("subdir"));
    assert_eq!(app.document.profile(), &before);
    app.handle_key(key(KeyCode::Esc));
    assert!(app.modal.is_none());
}

#[test]
fn copy_is_exclusive_even_if_destination_appears_after_browsing_and_cancellation_keeps_baseline() {
    let files = Files::new();
    let source = files.profile("source.toml");
    let mut app = App::new(ProfileDocument::open(&source).unwrap(), false);
    let mut draft = app.document.profile().clone();
    draft.name = "Unsaved".into();
    app.document.replace(draft.clone());
    browse(&mut app, &files.0, Mode::SaveNew);
    app.handle_key(key(KeyCode::F(2)));
    set_field(&mut app, "raced.toml");
    fs::write(files.path("raced.toml"), "external data").unwrap();
    app.handle_key(accept());
    assert!(browser(&app).error.is_some());
    assert_eq!(
        fs::read_to_string(files.path("raced.toml")).unwrap(),
        "external data"
    );
    assert_eq!(app.document.path(), Some(source.as_path()));
    assert_eq!(app.document.profile(), &draft);
    assert!(app.document.dirty());
    app.handle_key(key(KeyCode::Esc));
    assert!(browser(&app).prompt.is_none());
    app.handle_key(key(KeyCode::Esc));
    assert!(app.modal.is_none());
    assert_eq!(app.document.profile(), &draft);
    assert!(app.document.dirty());
}

#[test]
fn bounded_listing_and_small_resizes_keep_paths_and_mouse_actions_isolated() {
    let files = Files::new();
    for number in 0..12 {
        fs::write(files.path(&format!("{number:02}.toml")), "bad TOML").unwrap();
    }
    let (limited, truncated) = crate::files::limited_listing(&files.0, 3).unwrap();
    assert_eq!(limited.len(), 3);
    assert!(truncated);
    let mut app = app();
    let before = app.document.profile().clone();
    browse(&mut app, &files.0, Mode::Open);
    app.handle_key(key(KeyCode::End));
    assert_eq!(browser(&app).selected, 11);
    for (width, height) in [(20, 8), (45, 12), (45, 15), (80, 24), (120, 40)] {
        screen(&mut app, width, height);
        for hit in &app.hits {
            assert!(hit.area.right() <= width && hit.area.bottom() <= height);
            assert!(matches!(
                hit.action,
                Action::Key(_) | Action::FileEntry(_) | Action::EditorCursor { .. }
            ));
        }
        if height < 15 {
            assert!(app.hits.is_empty());
        }
    }
    app.handle_key(key(KeyCode::Char('p')));
    for (width, height) in [(45, 12), (45, 15), (120, 40)] {
        screen(&mut app, width, height);
        assert!(!app
            .hits
            .iter()
            .any(|hit| matches!(hit.action, Action::FileEntry(_))));
    }
    set_field(&mut app, "bad-directory");
    let mut release = key(KeyCode::Enter);
    release.kind = KeyEventKind::Release;
    app.handle_key(release);
    assert!(browser(&app).error.is_none());
    app.handle_key(key(KeyCode::Enter));
    assert!(browser(&app).error.is_some());
    app.handle_key(key(KeyCode::Esc));
    assert_eq!(app.document.profile(), &before);
}

#[cfg(target_os = "linux")]
#[test]
fn raw_os_filenames_are_not_reconstructed_from_lossy_labels_and_links_are_skipped() {
    use std::{
        ffi::OsString,
        os::unix::{ffi::OsStringExt, fs::symlink},
    };
    let files = Files::new();
    let name = OsString::from_vec(b"raw-\xff.toml".to_vec());
    let path = files.0.join(name);
    hyperx_app::save_profile_new(&path, app().document.profile(), &[]).unwrap();
    symlink(&path, files.path("linked.toml")).unwrap();
    symlink(&files.0, files.path("directory-link")).unwrap();
    let mut app = app();
    browse(&mut app, &files.0, Mode::Open);
    assert_eq!(browser(&app).entries.len(), 1);
    assert_eq!(browser(&app).entries[0].path, path);
    screen(&mut app, 120, 40);
    app.handle_key(key(KeyCode::Enter));
    assert!(app.modal.is_none());
    assert_eq!(app.document.path(), Some(path.as_path()));
}

#[test]
fn unsupported_targets_remain_inspectable_and_renamable_without_inventing_settings() {
    let files = Files::new();
    fs::write(
        files.path("future.toml"),
        "name = 'Future'\ndevice = 'unknown-mouse'\npartial = true",
    )
    .unwrap();
    let mut app = app();
    browse(&mut app, &files.0, Mode::Open);
    app.handle_key(key(KeyCode::Enter));
    let before = app.document.profile().clone();
    app.tab = 4;
    app.handle_key(key(KeyCode::F(3)));
    set_editor(&mut app, "Different name");
    app.handle_key(key(KeyCode::Enter));
    let mut expected = before;
    expected.name = "Different name".into();
    assert_eq!(app.document.profile(), &expected);
    assert!(screen(&mut app, 120, 40).contains("NOT READY"));
}
