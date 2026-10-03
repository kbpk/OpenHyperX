use super::*;
use crate::widgets::{Action, Hit};
use crossterm::event::{MouseButton, MouseEvent, MouseEventKind};
use hyperx_app::ProfileValueEdit;
use hyperx_core::{MacroPlayback, SoftwareButtonBinding};

fn select_control(app: &mut App, id: &str) {
    app.tab = 1;
    app.selected_control = hyperx_app::profile_controls(&app.document.profile().device)
        .iter()
        .position(|control| control.id == id)
        .unwrap();
    app.handle_key(key(KeyCode::Enter));
}
fn click(app: &mut App, predicate: impl Fn(&Action) -> bool) {
    screen(app, 120, 40);
    let Hit { area, .. } = app
        .hits
        .iter()
        .find(|hit| predicate(&hit.action))
        .expect("visible mouse target")
        .clone();
    app.handle_mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: area.x,
        row: area.y,
        modifiers: KeyModifiers::NONE,
    });
}
fn category(app: &mut App, predicate: impl Fn(&Option<SoftwareButtonBinding>) -> bool) {
    for _ in 0..8 {
        let Some(Modal::Binding(picker)) = &mut app.modal else {
            panic!("picker")
        };
        if let Some(first) = picker.filtered().first() {
            picker.select(*first);
            if let Ok(ProfileValueEdit::ButtonBinding { binding, .. }) = picker.edit() {
                if predicate(&binding) {
                    return;
                }
            }
        }
        app.handle_key(key(KeyCode::Right));
    }
    panic!("category not offered");
}

#[test]
fn button_table_lists_all_controls_and_primary_clicks_never_open_individual_editor() {
    let mut app = app();
    app.tab = 1;
    let before = app.document.profile().clone();
    let text = screen(&mut app, 120, 40);
    for control in hyperx_app::profile_controls(&before.device) {
        assert!(text.contains(control.id));
    }
    assert_eq!(
        app.hits
            .iter()
            .filter(|hit| matches!(hit.action, Action::Control(_)))
            .count(),
        11
    );
    click(&mut app, |action| matches!(action, Action::Control(0)));
    assert!(app.modal.is_none());
    assert!(app.status.contains("coupled pair"));
    app.handle_key(key(KeyCode::Down));
    app.handle_key(key(KeyCode::F(2)));
    assert!(app.modal.is_none());
    app.handle_key(key(KeyCode::Down));
    app.handle_key(key(KeyCode::F(2)));
    assert!(matches!(app.modal, Some(Modal::Binding(_))));
    assert_eq!(app.document.profile(), &before);
}

#[test]
fn tab_variants_navigate_picker_categories_without_changing_the_main_tab_or_file() {
    let mut app = app();
    let before = app.document.profile().clone();
    select_control(&mut app, "button4");
    let original = screen(&mut app, 120, 40);
    assert!(original.contains("[<] Mouse function [>]"));
    app.handle_key(key(KeyCode::Tab));
    assert!(screen(&mut app, 120, 40).contains("[<] Multimedia [>]"));
    app.handle_key(KeyEvent::new(KeyCode::Tab, KeyModifiers::SHIFT));
    assert!(screen(&mut app, 120, 40).contains("[<] Mouse function [>]"));
    app.handle_key(key(KeyCode::BackTab));
    assert!(screen(&mut app, 120, 40).contains("[<] Not specified (preserve) [>]"));
    assert_eq!(app.tab, 1);
    assert_eq!(app.document.profile(), &before);
}

#[test]
fn searchable_keys_preview_then_accept_one_semantic_binding_preserving_every_other_field() {
    let mut app = app();
    let before = app.document.profile().clone();
    select_control(&mut app, "button4");
    category(&mut app, |binding| {
        matches!(binding, Some(SoftwareButtonBinding::Keyboard { .. }))
    });
    app.handle_paste("left-shift");
    app.handle_key(key(KeyCode::Down));
    assert_eq!(app.document.profile(), &before);
    app.handle_key(accept());
    assert!(app.modal.is_none());
    let mut expected = before;
    expected.buttons.insert(
        "button4".into(),
        SoftwareButtonBinding::Keyboard {
            key: "left-shift".into(),
        },
    );
    assert_eq!(app.document.profile(), &expected);
    assert!(app.document.dirty());
}

#[test]
fn mouse_picker_selects_preview_and_distinguishes_disabled_from_file_omission() {
    let mut app = app();
    let before = app.document.profile().clone();
    select_control(&mut app, "button4");
    category(&mut app, |binding| {
        matches!(binding, Some(SoftwareButtonBinding::Disabled {}))
    });
    let Some(Modal::Binding(picker)) = &mut app.modal else {
        panic!("picker")
    };
    let index = picker.filtered()[0];
    picker.selected = None;
    click(
        &mut app,
        |action| matches!(action, Action::BindingChoice(value) if *value == index),
    );
    assert_eq!(app.document.profile(), &before);
    click(&mut app, |action| matches!(action, Action::AcceptEditor));
    assert_eq!(
        app.document.profile().buttons["button4"],
        SoftwareButtonBinding::Disabled {}
    );
    select_control(&mut app, "button4");
    category(&mut app, Option::is_none);
    app.handle_key(key(KeyCode::Enter));
    assert!(!app.document.profile().buttons.contains_key("button4"));
    let mut expected = before;
    expected.buttons.remove("button4");
    assert_eq!(app.document.profile(), &expected);
}

#[test]
fn library_macro_assignment_uses_target_gate_and_never_changes_library_definitions() {
    let mut app = app();
    let before = app.document.profile().clone();
    assert!(!before.macros.is_empty());
    select_control(&mut app, "button4");
    category(&mut app, |binding| {
        matches!(binding, Some(SoftwareButtonBinding::Macro { .. }))
    });
    app.handle_key(key(KeyCode::Enter));
    assert!(app.modal.is_none());
    assert_eq!(app.document.profile().macros, before.macros);
    assert!(matches!(
        app.document.profile().buttons["button4"],
        SoftwareButtonBinding::Macro { .. }
    ));

    let mut invalid = before.clone();
    invalid.macros[0].definition.playback = MacroPlayback::ToggleRepeat;
    app.document.replace(invalid.clone());
    select_control(&mut app, "dpi");
    // Rejected library entries are still visible for inspection, not silently
    // offered on a target with no corresponding implemented runtime encoding.
    for _ in 0..6 {
        app.handle_key(key(KeyCode::Right));
    }
    let choices = hyperx_app::profile_binding_choices(&invalid, "dpi");
    let index = choices
        .iter()
        .position(|choice| matches!(choice.binding, SoftwareButtonBinding::Macro { .. }))
        .unwrap()
        + 1;
    let Some(Modal::Binding(picker)) = &mut app.modal else {
        panic!("picker")
    };
    for _ in 0..8 {
        if picker.filtered().contains(&index) {
            break;
        }
        picker.category_move(1);
    }
    picker.select(index);
    app.handle_key(key(KeyCode::Enter));
    assert!(matches!(&app.modal, Some(Modal::Binding(picker)) if picker.error.is_some()));
    assert_eq!(app.document.profile(), &invalid);
}

#[test]
fn category_and_search_changes_do_not_implicitly_assign_and_cancel_preserves_imported_alias() {
    let mut app = app();
    let mut before = app.document.profile().clone();
    before.buttons.insert(
        "button4".into(),
        SoftwareButtonBinding::Keyboard {
            key: "RETURN".into(),
        },
    );
    app.document.replace(before.clone());
    select_control(&mut app, "button4");
    assert!(screen(&mut app, 120, 40).contains("RETURN"));
    app.handle_key(key(KeyCode::Enter));
    assert!(matches!(&app.modal, Some(Modal::Binding(picker)) if picker.error.is_some()));
    app.handle_key(key(KeyCode::Right));
    app.handle_key(key(KeyCode::Enter));
    assert!(matches!(&app.modal, Some(Modal::Binding(picker)) if picker.error.is_some()));
    app.handle_paste("not-a-real-key");
    assert!(screen(&mut app, 120, 40).contains("No matching choices"));
    app.handle_key(key(KeyCode::Esc));
    assert!(app.modal.is_none());
    assert_eq!(app.document.profile(), &before);
}

#[test]
fn small_and_resized_binding_modals_never_leak_table_hits_or_panic() {
    let mut app = app();
    select_control(&mut app, "button4");
    for (width, height) in [(120, 40), (45, 12), (45, 18), (60, 30), (20, 8)] {
        let output = screen(&mut app, width, height);
        if (width, height) == (45, 12) {
            assert!(!output.contains("Enlarge terminal"));
            assert!(app
                .hits
                .iter()
                .any(|hit| matches!(hit.action, Action::BindingChoice(_))));
            assert!(app
                .hits
                .iter()
                .any(|hit| matches!(hit.action, Action::AcceptEditor)));
        }
        assert!(!app
            .hits
            .iter()
            .any(|hit| matches!(hit.action, Action::Control(_) | Action::Value(_))));
        assert!(app
            .hits
            .iter()
            .all(|hit| hit.area.x.saturating_add(hit.area.width) <= width
                && hit.area.y.saturating_add(hit.area.height) <= height));
    }
    app.clear_mouse_layout();
    assert!(app.hits.is_empty());
}

#[test]
fn compact_binding_category_mouse_target_changes_only_the_local_preview() {
    let mut app = app();
    let original = app.document.profile().clone();
    select_control(&mut app, "button4");
    let initial_screen = screen(&mut app, 45, 12);
    let hit = app
        .hits
        .iter()
        .find(|hit| matches!(hit.action, Action::BindingCategory(1)))
        .unwrap()
        .clone();
    app.handle_mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: hit.area.x,
        row: hit.area.y,
        modifiers: KeyModifiers::NONE,
    });
    assert!(matches!(&app.modal, Some(Modal::Binding(_))));
    assert_ne!(screen(&mut app, 45, 12), initial_screen);
    assert_eq!(app.document.profile(), &original);
}
