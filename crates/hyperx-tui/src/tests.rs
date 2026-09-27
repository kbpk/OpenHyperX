use crate::{
    app::{App, EditAction, Modal},
    editor::Editor,
};
use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use hyperx_app::{parse_profile, ProfileDocument, ProfileSection};
use ratatui::{backend::TestBackend, Terminal};

mod bindings;
mod mouse;

fn app() -> App {
    App::new(
        ProfileDocument::from_profile(
            parse_profile(include_str!(
                "../../../examples/profiles/pulsefire-raid.toml"
            ))
            .unwrap(),
        ),
        true,
    )
}
fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}
fn accept() -> KeyEvent {
    KeyEvent::new(KeyCode::Char('s'), KeyModifiers::CONTROL)
}
fn set_editor(app: &mut App, text: &str) {
    let Some(Modal::Editor { editor, .. }) = &mut app.modal else {
        panic!("editor should be open")
    };
    *editor = Editor::new(text, editor.single_line);
}
fn screen(app: &mut App, width: u16, height: u16) -> String {
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    terminal.draw(|frame| app.render(frame)).unwrap();
    terminal
        .backend()
        .buffer()
        .content
        .iter()
        .map(|cell| cell.symbol())
        .collect()
}

#[test]
fn every_tab_renders_model_data_and_offline_status_without_usb_details() {
    let mut app = app();
    for (tab, expected) in [
        (0, "Stage 0: X=800 Y=800"),
        (1, "wheel-tilt-right"),
        (2, "File timeline including last delay: 80 ms"),
        (3, "logo: #0000FF"),
        (4, "Save to mouse: UNAVAILABLE OFFLINE"),
    ] {
        app.tab = tab;
        let output = screen(&mut app, 120, 40);
        assert!(output.contains(expected), "{expected}: {output}");
        assert!(output.contains("OFFLINE") && output.contains("DEMO DATA"));
        assert!(!output.contains("report_id") && !output.contains("0x0951"));
    }
}

#[test]
fn empty_profiles_show_unknown_or_omitted_values_not_defaults() {
    let mut app = App::new(
        ProfileDocument::from_profile(
            parse_profile("name = 'Empty'\ndevice = 'pulsefire-raid'\npartial = true").unwrap(),
        ),
        false,
    );
    assert!(screen(&mut app, 100, 30).contains("not present / not read"));
    app.tab = 3;
    assert!(screen(&mut app, 100, 30).contains("current colors and effects unknown"));
    app.tab = 2;
    assert!(screen(&mut app, 100, 30).contains("hardware macros are UNKNOWN"));
}

#[test]
fn section_editor_accepts_valid_drafts_atomically_and_retains_parse_errors() {
    let mut app = app();
    let before = app.document.profile().clone();
    app.handle_key(key(KeyCode::Char('e')));
    set_editor(&mut app, "[polling]\nhz = 'broken'");
    app.handle_key(accept());
    assert_eq!(app.document.profile(), &before);
    assert!(matches!(
        app.modal,
        Some(Modal::Editor { error: Some(_), .. })
    ));
    set_editor(&mut app, "[polling]\nhz = 500");
    app.handle_key(accept());
    assert!(app.modal.is_none());
    assert!(app.document.dirty());
    assert_eq!(app.document.profile().polling.unwrap().hz, 500);
    assert_eq!(app.document.profile().buttons, before.buttons);
    app.handle_key(key(KeyCode::Char('d')));
    assert!(
        matches!(&app.modal, Some(Modal::Viewer { text, .. }) if text.contains("polling.hz: 1000 -> 500"))
    );
}

#[test]
fn unsupported_values_remain_visible_and_fail_validation_without_discarding_draft() {
    let mut app = app();
    app.handle_key(key(KeyCode::Char('e')));
    set_editor(&mut app, "[polling]\nhz = 2000");
    app.handle_key(accept());
    assert_eq!(app.document.profile().polling.unwrap().hz, 2000);
    assert!(screen(&mut app, 120, 35).contains("NOT READY"));
    app.handle_key(key(KeyCode::Char('v')));
    assert!(matches!(&app.modal, Some(Modal::Viewer { text, .. }) if text.contains("NOT READY")));
}

#[test]
fn windows_release_events_do_not_repeat_navigation_or_actions() {
    let mut app = app();
    let mut released = key(KeyCode::Tab);
    released.kind = KeyEventKind::Release;
    app.handle_key(released);
    assert_eq!(app.tab, 0);
    app.handle_key(key(KeyCode::Tab));
    assert_eq!(app.tab, 1);
    app.handle_key(key(KeyCode::BackTab));
    assert_eq!(app.tab, 0);
    app.handle_key(key(KeyCode::Char('5')));
    assert_eq!(app.tab, 4);
}

#[test]
fn dirty_document_requires_explicit_discard_and_cancel_preserves_it() {
    let mut app = app();
    let mut edited = app.document.profile().clone();
    edited.name = "Unsaved".into();
    app.document.replace(edited);
    app.handle_key(key(KeyCode::Char('q')));
    assert!(!app.quit);
    assert!(matches!(app.modal, Some(Modal::Confirm { open: false })));
    app.handle_key(key(KeyCode::Char('n')));
    assert!(app.document.dirty());
    assert!(!app.quit);
    app.handle_key(key(KeyCode::Char('o')));
    app.handle_key(key(KeyCode::Char('y')));
    set_editor(&mut app, "definitely-missing-openhyperx-profile.toml");
    app.handle_key(key(KeyCode::Enter));
    assert_eq!(app.document.profile().name, "Unsaved");
    assert!(app.document.dirty());
    assert!(matches!(
        app.modal,
        Some(Modal::Editor { error: Some(_), .. })
    ));
    app.handle_key(key(KeyCode::Esc));
    app.handle_key(key(KeyCode::Char('q')));
    app.handle_key(key(KeyCode::Char('y')));
    assert!(app.quit);
}

#[test]
fn tiny_resize_and_all_modal_types_render_without_panics() {
    let mut app = app();
    for (width, height) in [(20, 8), (45, 12), (80, 24), (160, 50)] {
        screen(&mut app, width, height);
        app.handle_key(key(KeyCode::Char('e')));
        screen(&mut app, width, height);
        app.handle_key(key(KeyCode::Esc));
        app.modal = Some(Modal::Confirm { open: false });
        screen(&mut app, width, height);
        app.modal = None;
        app.handle_key(key(KeyCode::Char('d')));
        screen(&mut app, width, height);
        app.modal = None;
    }
}

#[test]
fn unicode_editor_supports_insertion_line_splits_joining_and_cancel() {
    let mut editor = Editor::new("", false);
    editor.paste("ą中\n😀a");
    assert_eq!(editor.text(), "ą中\n😀a");
    editor.key(key(KeyCode::Home));
    editor.key(key(KeyCode::Delete));
    assert_eq!(editor.text(), "ą中\na");
    editor.key(key(KeyCode::Backspace));
    assert_eq!(editor.text(), "ą中a");
    editor.key(key(KeyCode::Home));
    editor.key(key(KeyCode::Right));
    editor.key(key(KeyCode::Enter));
    assert_eq!(editor.text(), "ą\n中a");
    let mut prompt = Editor::new("", true);
    prompt.paste("safe\npath\u{1b}[31m");
    assert_eq!(prompt.text(), "safepath[31m");
    let mut app = app();
    let before = app.document.profile().clone();
    app.handle_key(key(KeyCode::Char('e')));
    app.handle_paste("invalid draft");
    app.handle_key(key(KeyCode::Esc));
    assert_eq!(app.document.profile(), &before);
}

#[test]
fn resolution_and_omission_use_shared_target_gates_and_preserve_other_settings() {
    let mut app = app();
    let mut original = app.document.profile().clone();
    original.buttons.remove("button5");
    original
        .unresolved_button_assignments
        .push(hyperx_core::UnresolvedButtonAssignment {
            source_id: "runtime:button5".into(),
            macro_source_id: None,
        });
    app.document.replace(original.clone());
    app.handle_key(key(KeyCode::Char('r')));
    set_editor(
        &mut app,
        "source_id = 'runtime:button5'\ncontrol = 'dpi'\nmacro_id = 'ab'",
    );
    app.handle_key(accept());
    assert_eq!(app.document.profile(), &original);
    set_editor(
        &mut app,
        "source_id = 'runtime:button5'\ncontrol = 'button5'\nmacro_id = 'ab'",
    );
    app.handle_key(accept());
    assert!(app
        .document
        .profile()
        .unresolved_button_assignments
        .is_empty());
    assert_eq!(app.document.profile().dpi, original.dpi);
    app.document.replace(original.clone());
    app.handle_key(key(KeyCode::Char('x')));
    set_editor(&mut app, "runtime:button5");
    app.handle_key(key(KeyCode::Enter));
    assert!(!app.document.profile().buttons.contains_key("button5"));
    assert!(app
        .document
        .profile()
        .unresolved_button_assignments
        .is_empty());
}

#[test]
fn section_shortcuts_select_the_correct_shared_edit_scope() {
    let mut app = app();
    for (tab, scope) in [
        (0, ProfileSection::Performance),
        (1, ProfileSection::Buttons),
        (2, ProfileSection::Macros),
        (3, ProfileSection::Lighting),
        (4, ProfileSection::All),
    ] {
        app.tab = tab;
        app.handle_key(key(KeyCode::Char('e')));
        assert!(
            matches!(app.modal, Some(Modal::Editor { action: EditAction::Section(value), .. }) if value == scope)
        );
        app.handle_key(key(KeyCode::Esc));
    }
}

#[test]
fn large_paste_is_bounded_and_preserves_unicode_cursor_and_existing_suffix() {
    let mut editor = Editor::new("suffix", false);
    editor.paste(&"ą".repeat(100_000));
    assert_eq!(editor.column, 100_000);
    assert!(editor.text().ends_with("suffix"));
    let before = editor.text();
    editor.paste(&"x".repeat(hyperx_app::MAX_PROFILE_BYTES));
    assert_eq!(editor.text(), before);
    editor.paste("\n中\n");
    assert_eq!((editor.row, editor.column), (2, 0));
    assert_eq!(editor.lines[2], "suffix");
}

#[test]
fn save_as_uses_new_files_and_keeps_failed_drafts_and_existing_files_intact() {
    struct Files(std::path::PathBuf);
    impl Drop for Files {
        fn drop(&mut self) {
            for file in std::fs::read_dir(&self.0).unwrap() {
                std::fs::remove_file(file.unwrap().path()).unwrap();
            }
            std::fs::remove_dir(&self.0).unwrap();
        }
    }
    let directory = std::env::temp_dir().join(format!(
        "openhyperx-tui-save-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir(&directory).unwrap();
    let _files = Files(directory.clone());
    let destination = directory.join("saved.toml");
    let mut app = app();
    let mut edited = app.document.profile().clone();
    edited.name = "First draft".into();
    app.document.replace(edited);
    app.handle_key(key(KeyCode::Char('s')));
    set_editor(&mut app, destination.to_str().unwrap());
    app.handle_key(key(KeyCode::Enter));
    assert!(!app.document.dirty());
    assert!(app.modal.is_none());
    let saved = std::fs::read_to_string(&destination).unwrap();
    assert!(saved.contains("First draft"));
    let mut edited = app.document.profile().clone();
    edited.name = "Second draft".into();
    app.document.replace(edited);
    app.handle_key(key(KeyCode::Char('s')));
    set_editor(&mut app, destination.to_str().unwrap());
    app.handle_key(key(KeyCode::Enter));
    assert!(app.document.dirty());
    assert!(matches!(
        app.modal,
        Some(Modal::Editor { error: Some(_), .. })
    ));
    assert_eq!(std::fs::read_to_string(&destination).unwrap(), saved);
}
