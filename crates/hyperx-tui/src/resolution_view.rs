use crossterm::event::KeyCode;
use ratatui::{
    layout::{Margin, Rect},
    style::{Color, Style},
    text::Line,
    widgets::{Block, Borders, Paragraph, Wrap},
    Frame,
};

use crate::{
    render::safe_text,
    resolution::{ResolutionPicker, Stage},
    widgets::{sub, Action, Hit},
};

fn row(area: Rect, offset: u16) -> Rect {
    Rect::new(
        area.x,
        area.y.saturating_add(offset).min(area.bottom()),
        area.width,
        u16::from(offset < area.height),
    )
}
fn one_line(value: &str) -> String {
    value
        .chars()
        .map(|character| {
            if character.is_control() {
                '\u{fffd}'
            } else {
                character
            }
        })
        .collect()
}
fn button(
    frame: &mut Frame,
    area: Rect,
    label: &str,
    action: Action,
    selected: bool,
    hits: &mut Vec<Hit>,
) {
    if area.width == 0 || area.height == 0 {
        return;
    }
    frame.render_widget(
        Paragraph::new(one_line(label)).style(Style::default().fg(if selected {
            Color::Yellow
        } else {
            Color::Cyan
        })),
        area,
    );
    hits.push(Hit { area, action });
}
fn buttons(frame: &mut Frame, area: Rect, values: &[(&str, KeyCode)], hits: &mut Vec<Hit>) {
    let mut offset = 0;
    for (label, code) in values {
        let width = label.len() as u16 + 1;
        button(
            frame,
            sub(area, offset, width),
            label,
            Action::Key(*code),
            false,
            hits,
        );
        offset += width;
    }
}

pub(crate) fn render(
    frame: &mut Frame,
    area: Rect,
    picker: &ResolutionPicker,
    hits: &mut Vec<Hit>,
) {
    let title = match picker.stage {
        Stage::Source => "Unresolved sources - choose exact entry",
        Stage::Target => "Resolution - choose model-declared target",
        Stage::Macro => "Resolution - choose real library definition",
        Stage::Review => "Confirm resolution - FILE ONLY / OFFLINE",
        Stage::Omit => "Confirm omission - provenance ONLY / OFFLINE",
    };
    frame.render_widget(Block::default().borders(Borders::ALL).title(title), area);
    let inner = area.inner(Margin {
        horizontal: 1,
        vertical: 1,
    });
    if inner.height < 10 {
        frame.render_widget(Paragraph::new("Enlarge terminal to 45x12. Esc returns/back, then closes. No profile edits or HID access."), inner);
        return;
    }
    if let Some(report) = &picker.info {
        frame.render_widget(
            Paragraph::new(safe_text(report))
                .wrap(Wrap { trim: false })
                .scroll((picker.scroll, 0)),
            Rect::new(
                inner.x,
                inner.y,
                inner.width,
                inner.height.saturating_sub(2),
            ),
        );
        buttons(
            frame,
            row(inner, inner.height - 1),
            &[("[Back Esc]", KeyCode::Esc)],
            hits,
        );
        return;
    }
    if picker.reviewing() {
        let source = picker.source.as_ref().unwrap();
        let text = if picker.stage == Stage::Omit {
            format!("Remove exactly this unresolved provenance entry from the FILE?\n\nSource: {}\nMacro source hint: {}\n\nThis does NOT disable/reset any button, remove a supplied binding, delete a macro, or change the mouse. An omitted binding preserves device state.\nOther unresolved entries and imported metadata stay intact.\n\ny / Ctrl+S confirms; n / Esc returns. Enter does not confirm.", source.source_id, source.macro_source_id.as_deref().unwrap_or("<unknown>"))
        } else {
            let definition = picker.definition.as_ref().unwrap();
            format!("Resolve exactly one source into an explicit FILE assignment?\n\nSource: {}\nMacro source hint: {} (provenance, not inferred identity)\nTarget: {}\nChosen library ID: {}\nName: {}\nPlayback: {}\nEvents: {}\n\nThe chosen timeline was validated for this target. Only its reference is assigned; no library definition is replaced or uploaded. Other unresolved entries/settings stay intact. This is not measured playback or hardware verification.\n\ny / Ctrl+S confirms; n / Esc returns. Enter does not confirm.", source.source_id, source.macro_source_id.as_deref().unwrap_or("<unknown>"), picker.target.as_deref().unwrap(), definition.source_id, definition.name, definition.definition.playback, definition.definition.events.len())
        };
        let report = format!(
            "{}\n\n{}",
            picker
                .error
                .as_deref()
                .unwrap_or("Up/Down/Page keys or wheel scroll the complete summary."),
            text
        );
        frame.render_widget(
            Paragraph::new(safe_text(&report))
                .wrap(Wrap { trim: false })
                .scroll((picker.scroll, 0)),
            Rect::new(
                inner.x,
                inner.y,
                inner.width,
                inner.height.saturating_sub(2),
            ),
        );
        buttons(
            frame,
            row(inner, inner.height - 1),
            &[
                ("[Confirm y]", KeyCode::Char('y')),
                ("[Back n]", KeyCode::Char('n')),
            ],
            hits,
        );
        return;
    }
    let context = picker.source.as_ref().map_or_else(
        || "No source selected; IDs are never guessed into buttons.".into(),
        |source| {
            format!(
                "Source {} | target {}",
                source.source_id,
                picker.target.as_deref().unwrap_or("<choose explicitly>")
            )
        },
    );
    frame.render_widget(Paragraph::new(one_line(&context)), row(inner, 0));
    frame.render_widget(
        Paragraph::new("Type filters; arrows/click preview; Enter next; F1 full reason."),
        row(inner, 1),
    );
    let search_area = row(inner, 2);
    let column = Line::raw(
        picker.search.lines[0]
            .chars()
            .take(picker.search.column)
            .collect::<String>(),
    )
    .width();
    let left = column
        .saturating_sub(usize::from(search_area.width.saturating_sub(1)))
        .min(u16::MAX as usize) as u16;
    frame.render_widget(
        Paragraph::new(safe_text(&picker.search.text()))
            .scroll((0, left))
            .style(Style::default().bg(if picker.search.selected {
                Color::Blue
            } else {
                Color::DarkGray
            })),
        search_area,
    );
    if search_area.width > 0 {
        hits.push(Hit {
            area: search_area,
            action: Action::EditorCursor { top: 0, left },
        });
        frame.set_cursor_position((
            search_area.x
                + column
                    .saturating_sub(usize::from(left))
                    .min(usize::from(search_area.width - 1)) as u16,
            search_area.y,
        ));
    }
    let indices = picker.filtered();
    let height = usize::from(inner.height.saturating_sub(8));
    let start = picker
        .selected
        .and_then(|index| indices.iter().position(|value| *value == index))
        .unwrap_or(0)
        .saturating_sub(height.saturating_sub(1));
    for (offset, index) in indices.iter().skip(start).take(height).enumerate() {
        let choice = &picker.choices[*index];
        button(
            frame,
            row(inner, 3 + offset as u16),
            &format!(
                "{}{}",
                if choice.error.is_some() {
                    "BLOCKED | "
                } else {
                    ""
                },
                choice.label
            ),
            Action::ResolutionChoice(*index),
            picker.selected == Some(*index),
            hits,
        );
    }
    if indices.is_empty() {
        let message = match picker.stage {
            Stage::Macro if picker.choices.is_empty() => {
                "No definitions. Close and create/import a real timeline in Macros."
            }
            Stage::Target if picker.choices.is_empty() => {
                "No declared targets for this model. No buttons are invented."
            }
            Stage::Source if picker.choices.is_empty() => {
                "No unresolved entries in this FILE; no device was read."
            }
            _ => "No matching choices. Clear the search to inspect the complete list.",
        };
        frame.render_widget(
            Paragraph::new(message).wrap(Wrap { trim: false }),
            Rect::new(inner.x, inner.y + 3, inner.width, height as u16),
        );
    }
    let reason = picker.error.as_deref().or_else(|| picker.selected.and_then(|index| picker.choices[index].error.as_deref()))
        .unwrap_or("Preview only. No file edit until the final confirmation. Source hints never select a macro automatically.");
    frame.render_widget(
        Paragraph::new(safe_text(reason)).wrap(Wrap { trim: false }),
        Rect::new(inner.x, inner.y + inner.height - 4, inner.width, 3),
    );
    let actions: &[(&str, KeyCode)] = if picker.stage == Stage::Source {
        &[
            ("[Next]", KeyCode::Enter),
            ("[Omit Del]", KeyCode::Delete),
            ("[Why F1]", KeyCode::F(1)),
            ("[Close]", KeyCode::Esc),
        ]
    } else {
        &[
            ("[Next Enter]", KeyCode::Enter),
            ("[Why F1]", KeyCode::F(1)),
            ("[Back Esc]", KeyCode::Esc),
        ]
    };
    buttons(frame, row(inner, inner.height - 1), actions, hits);
}
