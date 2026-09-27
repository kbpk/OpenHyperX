use super::*;
use crate::{
    resolution::{ResolutionPicker, Stage, Value},
    widgets::{Action, Hit},
};
use crossterm::event::{MouseButton, MouseEvent, MouseEventKind};
use hyperx_core::{MacroPlayback, SoftwareButtonBinding, UnresolvedButtonAssignment};

fn draft() -> App {
    let mut profile = app().document.profile().clone();
    profile.partial = true;
    profile.source = Some(hyperx_core::SoftwareProfileSource {
        format: "ngenuity-legacy-hxp".into(),
        format_version: 40,
    });
    profile.buttons.remove("button5");
    profile.unresolved_button_assignments = vec![
        UnresolvedButtonAssignment {
            source_id: "runtime:button5".into(),
            macro_source_id: None,
        },
        UnresolvedButtonAssignment {
            source_id: "opaque-legacy-example".into(),
            macro_source_id: Some("not-a-library-id".into()),
        },
    ];
    let mut app = App::new(ProfileDocument::from_profile(profile), false);
    app.tab = 4;
    app
}
fn picker(app: &App) -> &ResolutionPicker {
    let Some(Modal::Resolution(picker)) = &app.modal else {
        panic!("resolution selector expected")
    };
    picker
}
fn select(app: &mut App, value: impl Fn(&Value) -> bool) {
    let Some(Modal::Resolution(picker)) = &mut app.modal else {
        panic!("selector expected")
    };
    let index = picker
        .choices
        .iter()
        .position(|choice| value(&choice.value))
        .unwrap();
    picker.select(index);
}
fn source(app: &mut App, id: &str) {
    select(
        app,
        |value| matches!(value, Value::Source(source) if source.source_id == id),
    );
    app.handle_key(key(KeyCode::Enter));
}
fn target(app: &mut App, id: &str) {
    select(
        app,
        |value| matches!(value, Value::Target(target) if target == id),
    );
    app.handle_key(key(KeyCode::Enter));
}
fn definition(app: &mut App, id: &str) {
    select(
        app,
        |value| matches!(value, Value::Macro(definition) if definition.source_id == id),
    );
    app.handle_key(key(KeyCode::Enter));
}
fn review(app: &mut App) {
    app.handle_key(key(KeyCode::F(5)));
    source(app, "runtime:button5");
    target(app, "button5");
    definition(app, "ab");
    assert!(picker(app).stage == Stage::Review);
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
fn source_target_and_macro_are_explicit_previews_until_final_file_confirmation() {
    let mut app = draft();
    let before = app.document.profile().clone();
    click(&mut app, |action| {
        matches!(action, Action::Key(KeyCode::F(5)))
    });
    assert!(picker(&app).selected.is_none());
    app.handle_key(key(KeyCode::Enter));
    assert!(picker(&app).error.is_some());
    app.handle_paste("runtime:button5");
    assert!(picker(&app).selected.is_none());
    app.handle_key(key(KeyCode::Down));
    app.handle_key(key(KeyCode::Enter));
    assert!(picker(&app).stage == Stage::Target && picker(&app).selected.is_none());
    target(&mut app, "button4");
    assert!(picker(&app).stage == Stage::Target);
    assert!(picker(&app)
        .error
        .as_ref()
        .unwrap()
        .contains("belongs to button5"));
    target(&mut app, "button5");
    assert!(picker(&app).stage == Stage::Macro);
    assert!(picker(&app).selected.is_none());
    definition(&mut app, "ab");
    assert!(picker(&app).stage == Stage::Review);
    assert_eq!(app.document.profile(), &before);
    let output = screen(&mut app, 120, 40);
    assert!(output.contains("Chosen library ID: ab") && output.contains("Target: button5"));
    app.handle_key(key(KeyCode::Enter));
    assert_eq!(app.document.profile(), &before);
    app.handle_key(key(KeyCode::Char('y')));
    assert!(app.modal.is_none());
    let mut expected = before;
    expected.buttons.insert(
        "button5".into(),
        SoftwareButtonBinding::Macro { id: "ab".into() },
    );
    expected.unresolved_button_assignments.remove(0);
    assert_eq!(app.document.profile(), &expected);
    assert!(app.document.dirty());
    assert!(app
        .document
        .diff()
        .unwrap()
        .settings
        .iter()
        .any(|change| change.field.contains("button5")));
}

#[test]
fn opaque_source_hint_never_selects_a_target_or_definition_and_playback_is_target_gated() {
    let mut app = draft();
    let mut profile = app.document.profile().clone();
    profile.buttons.remove("button4");
    profile.macros[0].definition.playback = MacroPlayback::ToggleRepeat;
    app.document.replace(profile.clone());
    app.handle_key(key(KeyCode::F(5)));
    source(&mut app, "opaque-legacy-example");
    assert!(picker(&app).selected.is_none());
    target(&mut app, "button5");
    definition(&mut app, "ab");
    assert!(picker(&app).stage == Stage::Macro && picker(&app).error.is_some());
    app.handle_key(key(KeyCode::Esc));
    target(&mut app, "button4");
    definition(&mut app, "ab");
    app.handle_key(accept());
    assert!(app.modal.is_none());
    let mut expected = profile;
    expected.buttons.insert(
        "button4".into(),
        SoftwareButtonBinding::Macro { id: "ab".into() },
    );
    expected.unresolved_button_assignments.remove(1);
    assert_eq!(app.document.profile(), &expected);
}

#[test]
fn omission_is_confirmed_provenance_only_even_when_a_binding_and_macro_already_exist() {
    let mut app = draft();
    let mut supplied = app.document.profile().clone();
    supplied.buttons.insert(
        "button5".into(),
        SoftwareButtonBinding::Macro { id: "ab".into() },
    );
    app.document.replace(supplied);
    let before = app.document.profile().clone();
    app.handle_key(key(KeyCode::F(5)));
    select(
        &mut app,
        |value| matches!(value, Value::Source(source) if source.source_id == "opaque-legacy-example"),
    );
    app.handle_key(key(KeyCode::Delete));
    assert!(picker(&app).stage == Stage::Omit);
    let output = screen(&mut app, 120, 40);
    assert!(output.contains("does NOT disable/reset") && output.contains("opaque-legacy-example"));
    app.handle_key(key(KeyCode::Char('n')));
    assert!(picker(&app).stage == Stage::Source);
    assert_eq!(app.document.profile(), &before);
    select(
        &mut app,
        |value| matches!(value, Value::Source(source) if source.source_id == "opaque-legacy-example"),
    );
    click(&mut app, |action| {
        matches!(action, Action::Key(KeyCode::Delete))
    });
    click(&mut app, |action| {
        matches!(action, Action::Key(KeyCode::Char('y')))
    });
    let mut expected = before;
    expected.unresolved_button_assignments.remove(1);
    assert_eq!(app.document.profile(), &expected);
}

#[test]
fn final_confirmation_preserves_fresh_unrelated_edits_and_omission_keeps_a_supplied_reference() {
    let mut app = draft();
    review(&mut app);
    let mut changed = app.document.profile().clone();
    changed.name = "Fresh unrelated name".into();
    changed.polling.as_mut().unwrap().hz = 2000; // Unsupported, deliberately not silently repaired.
    app.document.replace(changed.clone());
    app.handle_key(key(KeyCode::Char('y')));
    let mut expected = changed;
    expected.buttons.insert(
        "button5".into(),
        SoftwareButtonBinding::Macro { id: "ab".into() },
    );
    expected.unresolved_button_assignments.remove(0);
    assert_eq!(app.document.profile(), &expected);

    expected
        .unresolved_button_assignments
        .push(UnresolvedButtonAssignment {
            source_id: "runtime:button5".into(),
            macro_source_id: None,
        });
    app.document.replace(expected.clone());
    app.handle_key(key(KeyCode::F(5)));
    select(
        &mut app,
        |value| matches!(value, Value::Source(source) if source.source_id == "runtime:button5"),
    );
    app.handle_key(key(KeyCode::Delete));
    app.handle_key(key(KeyCode::Char('y')));
    expected.unresolved_button_assignments.pop();
    assert_eq!(app.document.profile(), &expected);
    assert!(matches!(
        app.document.profile().buttons["button5"],
        SoftwareButtonBinding::Macro { .. }
    ));
}

#[test]
fn stale_source_and_macro_snapshots_are_rejected_without_trapping_cancellation() {
    for mutate_source in [true, false] {
        let mut app = draft();
        review(&mut app);
        let mut changed = app.document.profile().clone();
        if mutate_source {
            changed.unresolved_button_assignments[0].macro_source_id = Some("changed".into());
        } else {
            changed.macros[0].name = "Changed while previewing".into();
        }
        app.document.replace(changed.clone());
        app.handle_key(key(KeyCode::Char('y')));
        assert!(picker(&app).error.as_ref().unwrap().contains("changed"));
        assert_eq!(app.document.profile(), &changed);
        for _ in 0..4 {
            app.handle_key(key(KeyCode::Esc));
        }
        assert!(app.modal.is_none());
        assert_eq!(app.document.profile(), &changed);
    }
    let mut app = draft();
    review(&mut app);
    let mut changed = app.document.profile().clone();
    changed
        .buttons
        .insert("button5".into(), SoftwareButtonBinding::Disabled {});
    app.document.replace(changed.clone());
    app.handle_key(key(KeyCode::Char('y')));
    assert!(picker(&app)
        .error
        .as_ref()
        .unwrap()
        .contains("already has a supplied binding"));
    assert_eq!(app.document.profile(), &changed);
}

#[test]
fn duplicate_sources_and_definitions_cannot_be_resolved_or_removed_by_row_index() {
    let mut app = draft();
    let mut profile = app.document.profile().clone();
    profile
        .unresolved_button_assignments
        .push(profile.unresolved_button_assignments[0].clone());
    app.document.replace(profile.clone());
    app.handle_key(key(KeyCode::F(5)));
    select(
        &mut app,
        |value| matches!(value, Value::Source(source) if source.source_id == "runtime:button5"),
    );
    app.handle_key(key(KeyCode::Delete));
    assert!(picker(&app).stage == Stage::Source);
    app.handle_key(key(KeyCode::Enter));
    assert!(picker(&app).stage == Stage::Source);
    assert!(picker(&app)
        .error
        .as_ref()
        .unwrap()
        .contains("Duplicate source"));
    assert_eq!(app.document.profile(), &profile);
    let mut app = draft();
    let mut profile = app.document.profile().clone();
    profile.macros.push(profile.macros[0].clone());
    app.document.replace(profile.clone());
    app.handle_key(key(KeyCode::F(5)));
    source(&mut app, "runtime:button5");
    target(&mut app, "button5");
    definition(&mut app, "ab");
    assert!(picker(&app).stage == Stage::Macro && picker(&app).error.is_some());
    assert_eq!(app.document.profile(), &profile);
}

#[test]
fn complete_block_reason_is_scrollable_and_cannot_commit_or_leak_underlying_controls() {
    let mut app = draft();
    let before = app.document.profile().clone();
    app.handle_key(key(KeyCode::F(5)));
    source(&mut app, "runtime:button5");
    select(
        &mut app,
        |value| matches!(value, Value::Target(target) if target == "button4"),
    );
    app.handle_key(key(KeyCode::F(1)));
    assert!(picker(&app)
        .info
        .as_ref()
        .unwrap()
        .contains("belongs to button5"));
    for (width, height) in [(20, 8), (45, 12), (45, 16), (80, 24), (120, 40)] {
        screen(&mut app, width, height);
        assert!(!app
            .hits
            .iter()
            .any(|hit| matches!(hit.action, Action::ResolutionChoice(_) | Action::Tab(_))));
        for hit in &app.hits {
            assert!(hit.area.right() <= width && hit.area.bottom() <= height);
        }
    }
    app.handle_key(accept());
    app.handle_key(key(KeyCode::Char('y')));
    assert_eq!(app.document.profile(), &before);
    app.handle_key(key(KeyCode::PageDown));
    assert_eq!(picker(&app).scroll, 8);
    app.handle_mouse(MouseEvent {
        kind: MouseEventKind::ScrollDown,
        column: 0,
        row: 0,
        modifiers: KeyModifiers::NONE,
    });
    assert_eq!(picker(&app).scroll, 11);
    app.handle_key(key(KeyCode::Home));
    assert_eq!(picker(&app).scroll, 0);
    app.handle_key(key(KeyCode::Esc));
    assert!(picker(&app).info.is_none());
    assert!(picker(&app).stage == Stage::Target && picker(&app).selected.is_some());
}

#[test]
fn small_viewports_unicode_search_mouse_choices_and_release_events_only_preview() {
    let mut app = draft();
    let mut profile = app.document.profile().clone();
    for index in 0..25 {
        profile
            .unresolved_button_assignments
            .push(UnresolvedButtonAssignment {
                source_id: format!("żółty-source-{index}"),
                macro_source_id: None,
            });
    }
    app.document.replace(profile.clone());
    app.handle_key(key(KeyCode::F(5)));
    app.handle_paste("żółty-source-24");
    assert!(picker(&app).selected.is_none());
    click(&mut app, |action| {
        matches!(action, Action::ResolutionChoice(26))
    });
    let mut release = key(KeyCode::Enter);
    release.kind = KeyEventKind::Release;
    app.handle_key(release);
    assert!(picker(&app).stage == Stage::Source);
    for (width, height) in [(20, 8), (45, 12), (45, 16), (80, 24), (120, 40)] {
        screen(&mut app, width, height);
        for hit in &app.hits {
            assert!(hit.area.right() <= width && hit.area.bottom() <= height);
        }
        if height < 16 {
            assert!(app.hits.is_empty());
        }
        assert!(!app
            .hits
            .iter()
            .any(|hit| matches!(hit.action, Action::Tab(_) | Action::FileEntry(_))));
    }
    app.handle_key(key(KeyCode::Enter));
    assert!(picker(&app).stage == Stage::Target);
    assert_eq!(app.document.profile(), &profile);
    app.handle_key(key(KeyCode::Esc));
    app.handle_key(key(KeyCode::Esc));
    assert!(app.modal.is_none());
    assert_eq!(app.document.profile(), &profile);
}

#[test]
fn missing_definitions_and_unknown_models_are_visible_not_filled_with_defaults() {
    let mut app = draft();
    let mut profile = app.document.profile().clone();
    profile.macros.clear();
    app.document.replace(profile.clone());
    app.handle_key(key(KeyCode::F(5)));
    source(&mut app, "runtime:button5");
    target(&mut app, "button5");
    assert!(screen(&mut app, 120, 40).contains("No definitions"));
    app.handle_key(key(KeyCode::Enter));
    assert_eq!(app.document.profile(), &profile);
    let mut app = draft();
    let mut profile = app.document.profile().clone();
    profile.device = "unknown-model".into();
    app.document.replace(profile.clone());
    app.handle_key(key(KeyCode::F(5)));
    source(&mut app, "runtime:button5");
    assert!(picker(&app).choices.is_empty());
    assert!(screen(&mut app, 120, 40).contains("No declared targets"));
    app.handle_key(key(KeyCode::Esc));
    select(
        &mut app,
        |value| matches!(value, Value::Source(source) if source.source_id == "runtime:button5"),
    );
    app.handle_key(key(KeyCode::Delete));
    app.handle_key(key(KeyCode::Char('y')));
    let mut expected = profile;
    expected.unresolved_button_assignments.remove(0);
    assert_eq!(app.document.profile(), &expected);
    let mut app = super::app();
    app.tab = 4;
    app.handle_key(key(KeyCode::F(5)));
    assert!(screen(&mut app, 120, 40).contains("No unresolved entries"));
    app.handle_key(key(KeyCode::Delete));
    assert!(picker(&app).stage == Stage::Source);
}
