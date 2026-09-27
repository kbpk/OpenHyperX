use super::*;
use crate::{
    macros::{Confirmation, Prompt},
    widgets::{Action, Hit},
};
use crossterm::event::{MouseButton, MouseEvent, MouseEventKind};
use hyperx_core::{MacroEvent, MacroPlayback, SoftwareButtonBinding, UnresolvedButtonAssignment};

fn editor(app: &App) -> &crate::macros::MacroEditor {
    let Some(Modal::Macro(editor)) = &app.modal else {
        panic!("macro editor expected")
    };
    editor
}
fn open(app: &mut App, create: bool) {
    app.tab = 2;
    app.handle_key(key(if create {
        KeyCode::Insert
    } else {
        KeyCode::F(2)
    }));
}
fn field(app: &mut App, code: KeyCode, value: &str) {
    app.handle_key(key(code));
    app.handle_paste(value);
    app.handle_key(key(KeyCode::Enter));
}
fn input(app: &mut App, value: &str) {
    app.handle_key(key(KeyCode::F(5)));
    app.handle_paste(value);
    app.handle_key(key(KeyCode::Down));
    app.handle_key(key(KeyCode::Enter));
}
fn click(app: &mut App, predicate: impl Fn(&Action) -> bool) {
    screen(app, 120, 40);
    let Hit { area, .. } = app
        .hits
        .iter()
        .find(|hit| predicate(&hit.action))
        .expect("visible hit target")
        .clone();
    app.handle_mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: area.x,
        row: area.y,
        modifiers: KeyModifiers::NONE,
    });
}

#[test]
fn new_macro_constructs_chord_and_individual_delays_without_toml_or_implicit_assignment() {
    let mut app = app();
    let before = app.document.profile().clone();
    open(&mut app, true);
    field(&mut app, KeyCode::F(2), "Shift A é");
    app.handle_key(key(KeyCode::F(3)));
    app.handle_key(key(KeyCode::F(3)));
    let events = [
        MacroEvent::KeyDown {
            key: "left-shift".into(),
            delay_ms: 0,
        },
        MacroEvent::KeyDown {
            key: "a".into(),
            delay_ms: 7,
        },
        MacroEvent::KeyUp {
            key: "a".into(),
            delay_ms: 31,
        },
        MacroEvent::KeyUp {
            key: "left-shift".into(),
            delay_ms: 0,
        },
    ];
    for event in &events {
        app.handle_key(key(KeyCode::Insert));
        if matches!(event, MacroEvent::KeyUp { .. }) {
            app.handle_key(key(KeyCode::F(4)));
        }
        let (_, value, delay) = crate::macros::event_parts(event);
        input(&mut app, value);
        field(&mut app, KeyCode::F(6), &delay.to_string());
        assert_eq!(app.document.profile(), &before);
    }
    assert_eq!(editor(&app).draft.definition.events, events);
    app.handle_key(accept());
    assert!(app.modal.is_none());
    let definition = app.document.profile().macros.last().unwrap();
    assert_eq!(definition.source_id, "macro-1");
    assert_eq!(definition.name, "Shift A é");
    assert_eq!(
        definition.definition.playback,
        MacroPlayback::RepeatWhileHeld
    );
    assert_eq!(definition.definition.events, events);
    let mut expected = before;
    expected.macros.push(definition.clone());
    assert_eq!(app.document.profile(), &expected);
    assert_eq!(
        hyperx_app::parse_profile(&hyperx_app::encode_profile(&expected).unwrap()).unwrap(),
        expected
    );
    hyperx_app::validate_button_binding(
        &expected,
        "button4",
        &SoftwareButtonBinding::Macro {
            id: "macro-1".into(),
        },
    )
    .unwrap();
    assert!(hyperx_app::validate_button_binding(
        &expected,
        "button5",
        &SoftwareButtonBinding::Macro {
            id: "macro-1".into()
        }
    )
    .is_err());
}

#[test]
fn referenced_macro_replacement_requires_confirmation_and_cancel_retains_the_draft() {
    let mut app = app();
    let mut before = app.document.profile().clone();
    before.partial = true;
    before
        .unresolved_button_assignments
        .push(UnresolvedButtonAssignment {
            source_id: "unknown-target".into(),
            macro_source_id: Some("ab".into()),
        });
    app.document.replace(before.clone());
    open(&mut app, false);
    field(&mut app, KeyCode::F(2), "Renamed AB");
    field(&mut app, KeyCode::F(6), "65535");
    app.handle_key(accept());
    assert!(matches!(
        editor(&app).prompt,
        Some(Prompt::Confirm(Confirmation::Replace))
    ));
    let output = screen(&mut app, 120, 40);
    assert!(output.contains("unknown-target") && output.contains("button5"));
    app.handle_key(key(KeyCode::Char('n')));
    assert!(editor(&app).prompt.is_none());
    assert_eq!(app.document.profile(), &before);
    app.handle_key(accept());
    app.handle_key(key(KeyCode::Char('y')));
    assert!(app.modal.is_none());
    let mut expected = before;
    expected.macros[0].name = "Renamed AB".into();
    expected.macros[0].definition.events[0] = MacroEvent::KeyDown {
        key: "a".into(),
        delay_ms: 65535,
    };
    assert_eq!(app.document.profile(), &expected);
}

#[test]
fn field_errors_cancel_and_discard_are_separate_atomic_transitions() {
    let mut app = app();
    let before = app.document.profile().clone();
    open(&mut app, false);
    app.handle_key(key(KeyCode::F(6)));
    app.handle_paste("65536");
    app.handle_key(key(KeyCode::Enter));
    assert!(matches!(editor(&app).prompt, Some(Prompt::Text { .. })));
    assert!(editor(&app).error.as_ref().unwrap().contains("0..65535"));
    assert_eq!(editor(&app).draft, before.macros[0]);
    app.handle_key(key(KeyCode::Esc));
    assert!(editor(&app).prompt.is_none());
    field(&mut app, KeyCode::F(2), "");
    // Editor preselects old name; Delete empties it before accepting the field.
    app.handle_key(key(KeyCode::F(2)));
    app.handle_key(key(KeyCode::Delete));
    app.handle_key(key(KeyCode::Enter));
    app.handle_key(accept());
    app.handle_key(key(KeyCode::Char('y')));
    assert!(editor(&app).error.as_ref().unwrap().contains("name"));
    assert_eq!(app.document.profile(), &before);
    app.handle_key(key(KeyCode::Esc));
    assert!(matches!(
        editor(&app).prompt,
        Some(Prompt::Confirm(Confirmation::Discard))
    ));
    app.handle_key(key(KeyCode::Esc));
    assert!(editor(&app).prompt.is_none());
    app.handle_key(key(KeyCode::Esc));
    app.handle_key(key(KeyCode::Char('y')));
    assert!(app.modal.is_none());
    assert_eq!(app.document.profile(), &before);
}

#[test]
fn mouse_controls_reorder_remove_and_choose_inputs_without_changing_the_file_early() {
    let mut app = app();
    let before = app.document.profile().clone();
    app.tab = 2;
    click(&mut app, |action| matches!(action, Action::Macro(0)));
    click(&mut app, |action| matches!(action, Action::MacroRow(2)));
    click(&mut app, |action| {
        matches!(action, Action::Key(KeyCode::F(7)))
    });
    assert_eq!(
        editor(&app).draft.definition.events[1],
        before.macros[0].definition.events[2]
    );
    click(&mut app, |action| {
        matches!(action, Action::Key(KeyCode::Delete))
    });
    click(&mut app, |action| {
        matches!(action, Action::Key(KeyCode::Insert))
    });
    click(&mut app, |action| {
        matches!(action, Action::Key(KeyCode::F(4)))
    });
    click(&mut app, |action| {
        matches!(action, Action::Key(KeyCode::F(4)))
    });
    click(&mut app, |action| {
        matches!(action, Action::Key(KeyCode::F(5)))
    });
    app.handle_paste("middle");
    let selected = editor(&app).input_indices()[0];
    click(
        &mut app,
        |action| matches!(action, Action::MacroInputChoice(index) if *index == selected),
    );
    assert_eq!(app.document.profile(), &before);
    click(&mut app, |action| {
        matches!(action, Action::Key(KeyCode::Enter))
    });
    assert!(
        matches!(editor(&app).draft.definition.events.last(), Some(MacroEvent::MouseButtonDown { button, delay_ms: 0 }) if button == "middle")
    );
    click(&mut app, |action| matches!(action, Action::AcceptEditor));
    click(&mut app, |action| {
        matches!(action, Action::Key(KeyCode::Char('y')))
    });
    assert_eq!(app.document.profile().macros[0].definition.events.len(), 4);
    assert_eq!(app.document.profile().buttons, before.buttons);
}

#[test]
fn imported_aliases_overlong_timelines_and_non_hardware_delays_are_preserved() {
    let mut app = app();
    let mut before = app.document.profile().clone();
    before.macros[0].definition.events = vec![
        MacroEvent::KeyDown {
            key: "RETURN".into(),
            delay_ms: 10000
        };
        15
    ];
    app.document.replace(before.clone());
    open(&mut app, false);
    assert_eq!(editor(&app).draft, before.macros[0]);
    app.handle_key(key(KeyCode::F(5)));
    assert!(screen(&mut app, 120, 40).contains("RETURN"));
    app.handle_key(key(KeyCode::Enter));
    app.handle_key(accept());
    assert!(app.modal.is_none());
    assert_eq!(app.document.profile(), &before);
    open(&mut app, false);
    app.handle_key(key(KeyCode::End));
    assert_eq!(editor(&app).selected, 14);
    app.handle_key(key(KeyCode::Insert));
    app.handle_key(accept());
    app.handle_key(key(KeyCode::Char('y')));
    assert_eq!(app.document.profile().macros[0].definition.events.len(), 16);
    assert_eq!(
        app.document.profile().macros[0].definition.events[0],
        before.macros[0].definition.events[0]
    );
    assert!(hyperx_app::validate_profile(app.document.profile())
        .error
        .is_some());
}

#[test]
fn library_identity_reserves_import_references_and_removal_is_reference_and_revision_safe() {
    let mut app = app();
    let mut before = app.document.profile().clone();
    before
        .unresolved_button_assignments
        .push(UnresolvedButtonAssignment {
            source_id: "reserved".into(),
            macro_source_id: Some("macro-1".into()),
        });
    before.buttons.insert(
        "button6".into(),
        SoftwareButtonBinding::Macro {
            id: "macro-2".into(),
        },
    );
    app.document.replace(before.clone());
    open(&mut app, true);
    assert_eq!(editor(&app).draft.source_id, "macro-3");
    app.handle_key(accept());
    assert_eq!(app.document.profile().macros.len(), 2);
    app.selected_macro = 0;
    app.handle_key(key(KeyCode::Delete));
    assert!(app.modal.is_none() && app.status.contains("Cannot delete"));
    app.selected_macro = 1;
    app.handle_key(key(KeyCode::Delete));
    assert!(matches!(app.modal, Some(Modal::MacroDelete { .. })));
    let mut updated = app.document.profile().clone();
    updated.macros[1].name = "Changed outside pending confirmation".into();
    app.document.replace(updated.clone());
    app.handle_key(key(KeyCode::Char('y')));
    assert!(matches!(
        app.modal,
        Some(Modal::MacroDelete { error: Some(_), .. })
    ));
    assert_eq!(app.document.profile(), &updated);
    app.handle_key(key(KeyCode::Esc));
    app.handle_key(key(KeyCode::Delete));
    app.handle_key(key(KeyCode::Char('y')));
    assert_eq!(app.document.profile(), &before);
}

#[test]
fn stale_source_and_duplicate_ids_are_not_silently_replaced_or_removed() {
    let mut app = app();
    open(&mut app, false);
    field(&mut app, KeyCode::F(2), "Local retained name");
    let mut updated = app.document.profile().clone();
    updated.macros[0].name = "External update".into();
    app.document.replace(updated.clone());
    app.handle_key(accept());
    assert!(editor(&app)
        .error
        .as_ref()
        .unwrap()
        .contains("changed while editing"));
    assert_eq!(editor(&app).draft.name, "Local retained name");
    assert_eq!(app.document.profile(), &updated);
    app.handle_key(key(KeyCode::Esc));
    app.handle_key(key(KeyCode::Char('y')));
    updated.macros.push(updated.macros[0].clone());
    app.document.replace(updated.clone());
    app.handle_key(key(KeyCode::F(2)));
    assert!(app.modal.is_none() && app.status.contains("Duplicate"));
    app.handle_key(key(KeyCode::Delete));
    assert!(app.modal.is_none());
    assert_eq!(app.document.profile(), &updated);
}

#[test]
fn small_windows_nested_prompts_and_huge_timelines_keep_hits_in_bounds_and_preserve_data() {
    let mut app = app();
    let mut before = app.document.profile().clone();
    before.macros[0].definition.events = vec![
        MacroEvent::KeyDown {
            key: "imported-alias".into(),
            delay_ms: u16::MAX
        };
        500
    ];
    app.document.replace(before.clone());
    open(&mut app, false);
    app.handle_key(key(KeyCode::End));
    for code in [KeyCode::Null, KeyCode::F(2), KeyCode::F(5), KeyCode::F(6)] {
        if code != KeyCode::Null {
            app.handle_key(key(code));
        }
        for (width, height) in [(120, 40), (45, 12), (45, 18), (60, 30), (20, 8)] {
            screen(&mut app, width, height);
            assert!(app
                .hits
                .iter()
                .all(|hit| hit.area.x.saturating_add(hit.area.width) <= width
                    && hit.area.y.saturating_add(hit.area.height) <= height));
            assert!(!app.hits.iter().any(|hit| matches!(
                hit.action,
                Action::Macro(_) | Action::Value(_) | Action::Slider(_)
            )));
        }
        if code != KeyCode::Null {
            app.handle_key(key(KeyCode::Esc));
        }
    }
    assert_eq!(editor(&app).draft.definition.events.len(), 500);
    assert_eq!(app.document.profile(), &before);
    app.handle_key(key(KeyCode::Esc));
    assert!(app.modal.is_none());
}

#[test]
fn named_input_search_never_defaults_to_a_choice_and_windows_releases_are_ignored() {
    let mut app = app();
    open(&mut app, true);
    app.handle_key(key(KeyCode::Insert));
    let mut released = key(KeyCode::Insert);
    released.kind = KeyEventKind::Release;
    app.handle_key(released);
    assert_eq!(editor(&app).draft.definition.events.len(), 1);
    app.handle_key(key(KeyCode::F(5)));
    app.handle_paste("shift");
    assert_eq!(editor(&app).input_indices().len(), 2);
    app.handle_key(key(KeyCode::Enter));
    assert!(editor(&app).error.is_some());
    assert!(editor(&app).prompt.is_some());
    assert_eq!(
        crate::macros::event_parts(&editor(&app).draft.definition.events[0]).1,
        ""
    );
    app.handle_key(key(KeyCode::Down));
    app.handle_key(key(KeyCode::Enter));
    assert_eq!(
        crate::macros::event_parts(&editor(&app).draft.definition.events[0]).1,
        "left-shift"
    );
    let selected = editor(&app).selected;
    app.handle_key(key(KeyCode::F(7)));
    assert_eq!(editor(&app).selected, selected);
    app.handle_key(key(KeyCode::Delete));
    app.handle_key(key(KeyCode::F(4)));
    app.handle_key(key(KeyCode::F(5)));
    app.handle_key(key(KeyCode::F(6)));
    assert!(editor(&app).draft.definition.events.is_empty());
}

#[test]
fn local_preflight_uses_real_target_encoder_and_keeps_incomplete_drafts_out_of_the_file() {
    let mut app = app();
    let before = app.document.profile().clone();
    open(&mut app, false);
    app.handle_key(key(KeyCode::F(3)));
    app.handle_key(key(KeyCode::F(9)));
    let Some(Prompt::Info { text, .. }) = &editor(&app).prompt else {
        panic!("preflight report")
    };
    assert!(text.contains("Button 4") && text.contains("Button 5"));
    assert!(text.contains("Runtime encoder accepts") && text.contains("Rejected:"));
    assert!(text.contains("0..9999") && text.contains("0..65535"));
    assert!(text.contains("not hardware verification"));
    assert_eq!(app.document.profile(), &before);
    app.handle_key(accept());
    assert!(editor(&app).prompt.is_some());
    app.handle_key(key(KeyCode::PageDown));
    app.handle_key(key(KeyCode::Home));
    assert!(matches!(
        editor(&app).prompt,
        Some(Prompt::Info { scroll: 0, .. })
    ));
    app.handle_key(key(KeyCode::Esc));
    assert_eq!(
        editor(&app).draft.definition.playback,
        MacroPlayback::ToggleRepeat
    );
    assert!(editor(&app).prompt.is_none());
    field(&mut app, KeyCode::F(6), "10000");
    app.handle_key(key(KeyCode::F(9)));
    let Some(Prompt::Info { text, .. }) = &editor(&app).prompt else {
        panic!("preflight report")
    };
    assert!(!text.contains("Runtime encoder accepts"));
    assert_eq!(app.document.profile(), &before);
}

#[test]
fn macro_text_fields_support_unicode_cursor_clicks_and_mouse_scroll_only_previews() {
    let mut app = app();
    let before = app.document.profile().clone();
    open(&mut app, false);
    app.handle_key(key(KeyCode::F(2)));
    click(&mut app, |action| {
        matches!(action, Action::EditorCursor { .. })
    });
    let Some(Prompt::Text { editor: input, .. }) = &editor(&app).prompt else {
        panic!("name field")
    };
    assert_eq!(input.column, 0);
    app.handle_paste("Ż");
    app.handle_key(key(KeyCode::Enter));
    assert_eq!(
        editor(&app).draft.name,
        format!("Ż{}", before.macros[0].name)
    );
    screen(&mut app, 120, 40);
    assert!(screen(&mut app, 120, 40).contains("LOCAL MACRO DRAFT"));
    app.handle_mouse(MouseEvent {
        kind: MouseEventKind::ScrollDown,
        column: 20,
        row: 15,
        modifiers: KeyModifiers::NONE,
    });
    assert_eq!(editor(&app).selected, 3);
    let mut up = key(KeyCode::Up);
    up.modifiers = KeyModifiers::SHIFT;
    app.handle_key(up);
    assert_eq!(editor(&app).selected, 2);
    assert_eq!(
        editor(&app).draft.definition.events[2],
        before.macros[0].definition.events[3]
    );
    assert_eq!(app.document.profile(), &before);
}
