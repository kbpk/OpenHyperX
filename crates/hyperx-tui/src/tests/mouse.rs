use super::*;
use crate::widgets::{slider_value, Action, Hit};
use crossterm::event::{MouseButton, MouseEvent, MouseEventKind};
use hyperx_app::ProfileValueEdit as E;
use ratatui::layout::Rect;

fn mouse(app: &mut App, kind: MouseEventKind, x: u16, y: u16) {
    app.handle_mouse(MouseEvent {
        kind,
        column: x,
        row: y,
        modifiers: KeyModifiers::NONE,
    });
}
fn hit(app: &mut App, predicate: impl Fn(&Action) -> bool) -> Hit {
    screen(app, 120, 40);
    app.hits
        .iter()
        .find(|hit| predicate(&hit.action))
        .expect("visible mouse target")
        .clone()
}
fn click(app: &mut App, predicate: impl Fn(&Action) -> bool) {
    let target = hit(app, predicate);
    mouse(
        app,
        MouseEventKind::Down(MouseButton::Left),
        target.area.x,
        target.area.y,
    );
    mouse(
        app,
        MouseEventKind::Up(MouseButton::Left),
        target.area.x,
        target.area.y,
    );
}

#[test]
fn slider_click_drag_clamps_and_snaps_without_changing_other_settings() {
    let mut app = app();
    let original = app.document.profile().clone();
    let slider = hit(&mut app, |action| matches!(action, Action::Slider(1)));
    mouse(
        &mut app,
        MouseEventKind::Down(MouseButton::Left),
        slider.area.x,
        slider.area.y,
    );
    assert_eq!(
        app.document.profile().dpi.as_ref().unwrap().stages[1].x,
        200
    );
    mouse(
        &mut app,
        MouseEventKind::Drag(MouseButton::Left),
        u16::MAX,
        0,
    );
    assert_eq!(
        app.document.profile().dpi.as_ref().unwrap().stages[1].x,
        16000
    );
    mouse(&mut app, MouseEventKind::Drag(MouseButton::Left), 0, 0);
    assert_eq!(
        app.document.profile().dpi.as_ref().unwrap().stages[1].x,
        200
    );
    mouse(
        &mut app,
        MouseEventKind::Up(MouseButton::Left),
        slider.area.x,
        slider.area.y,
    );
    mouse(
        &mut app,
        MouseEventKind::Drag(MouseButton::Left),
        u16::MAX,
        0,
    );
    let mut expected = original;
    expected.dpi.as_mut().unwrap().stages[1].x = 200;
    expected.dpi.as_mut().unwrap().stages[1].y = 200;
    assert_eq!(app.document.profile(), &expected);
    let caps = hyperx_app::device_descriptor("pulsefire-raid")
        .unwrap()
        .capabilities
        .dpi
        .unwrap();
    for width in [1, 2, 3, 19, 99, 1000] {
        for column in 0..1100 {
            let value = slider_value(caps, Rect::new(10, 0, width, 1), column);
            assert!(caps.validate(value).is_ok());
        }
    }
}

#[test]
fn mouse_tabs_and_polling_buttons_edit_only_the_requested_field() {
    let mut app = app();
    let original = app.document.profile().clone();
    click(&mut app, |action| matches!(action, Action::Tab(3)));
    assert_eq!(app.tab, 3);
    click(&mut app, |action| matches!(action, Action::Tab(0)));
    for rate in [125, 250, 500, 1000] {
        click(
            &mut app,
            |action| matches!(action, Action::Value(E::Polling(Some(hz))) if *hz == rate),
        );
        let mut expected = original.clone();
        expected.polling = Some(hyperx_core::SoftwarePollingProfile { hz: rate });
        assert_eq!(app.document.profile(), &expected);
    }
}

#[test]
fn direct_numeric_input_is_selected_and_invalid_values_retain_the_dialog() {
    let mut app = app();
    let original = app.document.profile().clone();
    click(&mut app, |action| matches!(action, Action::DpiInput(0)));
    app.handle_paste("801");
    click(&mut app, |action| matches!(action, Action::AcceptEditor));
    assert_eq!(app.document.profile(), &original);
    assert!(matches!(
        app.modal,
        Some(Modal::Editor { error: Some(_), .. })
    ));
    app.handle_key(KeyEvent::new(KeyCode::Char('a'), KeyModifiers::CONTROL));
    app.handle_paste("900");
    click(&mut app, |action| matches!(action, Action::AcceptEditor));
    assert!(app.modal.is_none());
    assert_eq!(
        app.document.profile().dpi.as_ref().unwrap().stages[0].x,
        900
    );
    click(&mut app, |action| matches!(action, Action::DpiInput(0)));
    app.handle_paste("16000");
    click(&mut app, |action| {
        matches!(action, Action::Key(KeyCode::Esc))
    });
    assert_eq!(
        app.document.profile().dpi.as_ref().unwrap().stages[0].x,
        900
    );
}

#[test]
fn stage_add_activate_remove_and_colors_use_no_toml_editor() {
    let mut app = app();
    let original = app.document.profile().clone();
    click(&mut app, |action| matches!(action, Action::AddStage));
    app.handle_paste("6400");
    click(&mut app, |action| matches!(action, Action::AcceptEditor));
    assert_eq!(app.document.profile().dpi.as_ref().unwrap().stages.len(), 4);
    click(&mut app, |action| matches!(action, Action::ColorInput(3)));
    app.handle_paste("FF8000");
    click(&mut app, |action| matches!(action, Action::AcceptEditor));
    assert_eq!(
        app.document.profile().dpi.as_ref().unwrap().stages[3]
            .color
            .to_string(),
        "#FF8000"
    );
    click(&mut app, |action| {
        matches!(action, Action::Value(E::ActiveStage(Some(3))))
    });
    click(&mut app, |action| {
        matches!(action, Action::Value(E::RemoveLastStage))
    });
    assert_eq!(app.document.profile().dpi.as_ref().unwrap().stages.len(), 4);
    assert!(app.status.contains("choose another active"));
    click(&mut app, |action| {
        matches!(action, Action::Value(E::ActiveStage(Some(0))))
    });
    click(&mut app, |action| {
        matches!(action, Action::Value(E::RemoveLastStage))
    });
    assert_eq!(app.document.profile(), &original);
    click(&mut app, |action| matches!(action, Action::Tab(3)));
    click(&mut app, |action| {
        matches!(action, Action::ZoneColor("wheel"))
    });
    app.handle_paste("FF0000");
    click(&mut app, |action| matches!(action, Action::AcceptEditor));
    let colors = &app.document.profile().lighting.as_ref().unwrap().zones;
    assert_eq!(colors["wheel"].to_string(), "#FF0000");
    assert_eq!(colors["logo"], original.lighting.unwrap().zones["logo"]);
}

#[test]
fn mouse_modal_blocks_underlying_sliders_and_resize_discards_stale_targets() {
    let mut app = app();
    let slider = hit(&mut app, |action| matches!(action, Action::Slider(0)));
    app.handle_key(key(KeyCode::Char('v')));
    screen(&mut app, 120, 40);
    let original = app.document.profile().clone();
    mouse(
        &mut app,
        MouseEventKind::Down(MouseButton::Left),
        slider.area.x,
        slider.area.y,
    );
    mouse(&mut app, MouseEventKind::Drag(MouseButton::Left), 100, 0);
    assert_eq!(app.document.profile(), &original);
    click(&mut app, |action| {
        matches!(action, Action::Key(KeyCode::Esc))
    });
    app.clear_mouse_layout();
    mouse(
        &mut app,
        MouseEventKind::Down(MouseButton::Left),
        slider.area.x,
        slider.area.y,
    );
    assert_eq!(app.document.profile(), &original);
    screen(&mut app, 20, 8);
    assert!(app.hits.is_empty());
}

#[test]
fn scroll_slider_keyboard_and_row_buttons_have_the_same_step_and_limits() {
    let mut app = app();
    let slider = hit(&mut app, |action| matches!(action, Action::Slider(0)));
    mouse(
        &mut app,
        MouseEventKind::ScrollUp,
        slider.area.x,
        slider.area.y,
    );
    assert_eq!(
        app.document.profile().dpi.as_ref().unwrap().stages[0].x,
        850
    );
    app.handle_key(key(KeyCode::Left));
    assert_eq!(
        app.document.profile().dpi.as_ref().unwrap().stages[0].x,
        800
    );
    app.handle_key(KeyEvent::new(KeyCode::Right, KeyModifiers::SHIFT));
    assert_eq!(
        app.document.profile().dpi.as_ref().unwrap().stages[0].x,
        1300
    );
    click(&mut app, |action| {
        matches!(
            action,
            Action::AdjustDpi {
                index: 1,
                delta: 50
            }
        )
    });
    assert_eq!(
        app.document.profile().dpi.as_ref().unwrap().stages[1].x,
        1650
    );
    assert_eq!(
        app.document.profile().dpi.as_ref().unwrap().stages[0].x,
        1300
    );
    // Right clicks and movement do not activate a value.
    let before = app.document.profile().clone();
    mouse(
        &mut app,
        MouseEventKind::Down(MouseButton::Right),
        slider.area.x,
        slider.area.y,
    );
    mouse(
        &mut app,
        MouseEventKind::Moved,
        slider.area.x,
        slider.area.y,
    );
    assert_eq!(app.document.profile(), &before);
}

#[test]
fn keyboard_forms_edit_values_without_opening_a_toml_section() {
    let mut app = app();
    app.handle_key(key(KeyCode::F(4)));
    assert_eq!(app.document.profile().polling.unwrap().hz, 125);
    app.handle_key(key(KeyCode::Char(']')));
    app.handle_key(key(KeyCode::F(2)));
    app.handle_paste("1200");
    app.handle_key(key(KeyCode::Enter));
    assert_eq!(
        app.document.profile().dpi.as_ref().unwrap().stages[1].x,
        1200
    );
    app.handle_key(key(KeyCode::F(3)));
    let before = app.document.profile().clone();
    app.handle_paste("€€");
    app.handle_key(key(KeyCode::Enter));
    assert_eq!(app.document.profile(), &before);
    assert!(matches!(
        app.modal,
        Some(Modal::Editor { error: Some(_), .. })
    ));
    app.handle_key(key(KeyCode::Esc));
    app.handle_key(key(KeyCode::Enter));
    assert_eq!(
        app.document.profile().dpi.as_ref().unwrap().active_stage,
        Some(1)
    );
    app.tab = 3;
    app.handle_key(key(KeyCode::F(2)));
    app.handle_paste("FFFFFF");
    app.handle_key(key(KeyCode::Enter));
    assert_eq!(
        app.document.profile().lighting.as_ref().unwrap().zones["wheel"].to_string(),
        "#FFFFFF"
    );
}

#[test]
fn mouse_scrolling_narrow_viewports_never_targets_hidden_controls() {
    let mut app = app();
    let mut profile = app.document.profile().clone();
    let extra_stage = profile.dpi.as_ref().unwrap().stages[0];
    profile.dpi.as_mut().unwrap().stages.resize(5, extra_stage);
    app.document.replace(profile);
    screen(&mut app, 45, 12);
    assert!(
        app.hits
            .iter()
            .filter(|hit| matches!(hit.action, Action::Tab(_)))
            .count()
            == 5
    );
    assert!(!app
        .hits
        .iter()
        .any(|hit| matches!(hit.action, Action::Slider(_))));
    let mut found = false;
    for _ in 0..25 {
        // Scroll over the label, not the bar: the wheel over a DPI bar changes
        // its value; elsewhere it scrolls the viewport.
        mouse(&mut app, MouseEventKind::ScrollDown, 2, 6);
        screen(&mut app, 45, 12);
        if let Some(slider) = app
            .hits
            .iter()
            .find(|hit| matches!(hit.action, Action::Slider(4)))
            .cloned()
        {
            mouse(
                &mut app,
                MouseEventKind::Down(MouseButton::Left),
                slider.area.x,
                slider.area.y,
            );
            mouse(
                &mut app,
                MouseEventKind::Up(MouseButton::Left),
                slider.area.x,
                slider.area.y,
            );
            assert_eq!(
                app.document.profile().dpi.as_ref().unwrap().stages[4].x,
                200
            );
            found = true;
            break;
        }
    }
    assert!(
        found,
        "last level must be reachable with the mouse in a small viewport"
    );
    for (index, level) in app
        .document
        .profile()
        .dpi
        .as_ref()
        .unwrap()
        .stages
        .iter()
        .enumerate()
    {
        if index != 4 {
            assert_ne!(level.x, 200);
        }
    }
}

#[test]
fn clicking_in_unicode_fields_and_discard_dialogs_uses_only_visible_actions() {
    let mut app = app();
    app.handle_key(key(KeyCode::Char('s')));
    set_editor(&mut app, "ą中a");
    let cursor = hit(&mut app, |action| {
        matches!(action, Action::EditorCursor { .. })
    });
    mouse(
        &mut app,
        MouseEventKind::Down(MouseButton::Left),
        cursor.area.x + 1,
        cursor.area.y,
    );
    assert!(
        matches!(&app.modal, Some(Modal::Editor { editor, .. }) if editor.column == 1 && !editor.selected)
    );
    app.handle_key(key(KeyCode::Char('X')));
    assert!(matches!(&app.modal, Some(Modal::Editor { editor, .. }) if editor.text() == "ąX中a"));
    click(&mut app, |action| {
        matches!(action, Action::Key(KeyCode::Esc))
    });
    app.handle_key(key(KeyCode::Right));
    click(&mut app, |action| {
        matches!(action, Action::Key(KeyCode::Char('q')))
    });
    assert!(!app.quit && app.document.dirty());
    click(&mut app, |action| {
        matches!(action, Action::Key(KeyCode::Esc))
    });
    assert!(!app.quit && app.document.dirty());
    click(&mut app, |action| {
        matches!(action, Action::Key(KeyCode::Char('q')))
    });
    click(&mut app, |action| {
        matches!(action, Action::Key(KeyCode::Char('y')))
    });
    assert!(app.quit);
}

#[test]
fn huge_unsupported_stage_list_is_rendered_without_truncating_profile_data() {
    let mut app = app();
    let mut profile = app.document.profile().clone();
    profile.dpi.as_mut().unwrap().stages = vec![profile.dpi.as_ref().unwrap().stages[0]; 25000];
    app.document.replace(profile.clone());
    let text = screen(&mut app, 120, 40);
    assert!(text.contains("5 of 25000 levels shown") && text.contains("NOT READY"));
    assert_eq!(app.document.profile(), &profile);
    assert!(!app
        .hits
        .iter()
        .any(|hit| matches!(hit.action, Action::Slider(index) if index > 4)));
}
