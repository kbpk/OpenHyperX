//! Offline color shortcuts for a draft field. Selection never commits a value.

use crossterm::event::KeyCode;
use hyperx_core::RgbColor;
use ratatui::{
    layout::Rect,
    style::{Color, Style},
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};

use crate::{
    app::EditAction,
    editor::Editor,
    widgets::{Action, Hit},
};

const COLORS: [(&str, RgbColor); 8] = [
    ("Off", RgbColor::new(0, 0, 0)),
    ("White", RgbColor::new(255, 255, 255)),
    ("Red", RgbColor::new(255, 0, 0)),
    ("Green", RgbColor::new(0, 255, 0)),
    ("Blue", RgbColor::new(0, 0, 255)),
    ("Yellow", RgbColor::new(255, 255, 0)),
    ("Cyan", RgbColor::new(0, 255, 255)),
    ("Magenta", RgbColor::new(255, 0, 255)),
];
const SHORT_NAMES: [&str; 8] = [
    "Off", "White", "Red", "Green", "Blue", "Yell", "Cyan", "Mgta",
];

pub(crate) fn is_color_action(action: EditAction) -> bool {
    matches!(action, EditAction::StageColor(_) | EditAction::ZoneColor(_))
}

pub(crate) fn preview_key(code: KeyCode, editor: &mut Editor) -> bool {
    let KeyCode::F(number @ 1..=8) = code else {
        return false;
    };
    let color = COLORS[usize::from(number - 1)].1;
    *editor = Editor::new(&color.to_string(), true);
    true
}

pub(crate) fn render(frame: &mut Frame, area: Rect, editor: &Editor, hits: &mut Vec<Hit>) {
    if area.height < 2 || area.width < 4 {
        return;
    }
    let width = area.width / 4;
    for (index, (name, color)) in COLORS.iter().enumerate() {
        let column = (index % 4) as u16;
        let row = (index / 4) as u16;
        let slot = Rect::new(
            area.x + column * width,
            area.y + row,
            if column == 3 {
                area.width - column * width
            } else {
                width
            },
            1,
        );
        let (swatch, swatch_color) = if *color == RgbColor::BLACK {
            ("□", Color::Gray)
        } else {
            ("■", Color::Rgb(color.red, color.green, color.blue))
        };
        let selected = editor.text().eq_ignore_ascii_case(&color.to_string());
        let label = if slot.width < 12 {
            SHORT_NAMES[index]
        } else {
            name
        };
        frame.render_widget(
            Paragraph::new(Line::from(vec![
                Span::styled(format!("F{} ", index + 1), Style::default().fg(Color::Cyan)),
                Span::styled(swatch, Style::default().fg(swatch_color)),
                Span::raw(format!(" {label}")),
            ]))
            .style(if selected {
                Style::default().bg(Color::DarkGray)
            } else {
                Style::default()
            }),
            slot,
        );
        hits.push(Hit {
            area: slot,
            action: Action::PaletteColor(index as u8 + 1),
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shortcuts_preview_exact_hex_without_committing() {
        let mut editor = Editor::new("#123456", true);
        assert!(preview_key(KeyCode::F(3), &mut editor));
        assert_eq!(editor.text(), "#FF0000");
        assert!(editor.selected);
        assert!(!preview_key(KeyCode::F(9), &mut editor));
        assert_eq!(editor.text(), "#FF0000");
    }
}
