//! Terminal presentation only. All limits and assignment preflight come from app metadata.
use crate::{
    app::App,
    editor::Editor,
    macros::{event_parts, Confirmation, MacroEditor, Prompt, TextField},
    render::safe_text,
    widgets::{sub, Action, Hit},
};
use crossterm::event::KeyCode;
use hyperx_app::{macro_references, profile_controls};
use ratatui::{
    layout::{Margin, Rect},
    style::{Color, Style},
    text::Line,
    widgets::{Block, Borders, Paragraph, Wrap},
    Frame,
};

fn row(inner: Rect, y: u16) -> Rect {
    Rect::new(
        inner.x,
        inner.y + y.min(inner.height),
        inner.width,
        u16::from(y < inner.height),
    )
}
fn text(frame: &mut Frame, area: Rect, value: &str) {
    frame.render_widget(Paragraph::new(safe_text(value)), area);
}
fn button(
    frame: &mut Frame,
    area: Rect,
    label: &str,
    action: Action,
    hits: &mut Vec<Hit>,
    selected: bool,
) {
    if area.width == 0 || area.height == 0 {
        return;
    }
    frame.render_widget(
        Paragraph::new(safe_text(label)).style(Style::default().fg(if selected {
            Color::Yellow
        } else {
            Color::Cyan
        })),
        area,
    );
    hits.push(Hit { area, action });
}
fn buttons(frame: &mut Frame, area: Rect, values: &[(&str, KeyCode)], hits: &mut Vec<Hit>) {
    let mut x = 0;
    for (label, code) in values {
        button(
            frame,
            sub(area, x, label.len() as u16 + 1),
            label,
            Action::Key(*code),
            hits,
            false,
        );
        x += label.len() as u16 + 1;
    }
}
fn text_field(frame: &mut Frame, area: Rect, editor: &Editor, hits: &mut Vec<Hit>) {
    let column = Line::raw(
        editor.lines[0]
            .chars()
            .take(editor.column)
            .collect::<String>(),
    )
    .width();
    let left = column
        .saturating_sub(usize::from(area.width.saturating_sub(1)))
        .min(u16::MAX as usize) as u16;
    frame.render_widget(
        Paragraph::new(safe_text(&editor.text()))
            .scroll((0, left))
            .style(Style::default().bg(if editor.selected {
                Color::Blue
            } else {
                Color::DarkGray
            })),
        area,
    );
    if area.width > 0 && area.height > 0 {
        hits.push(Hit {
            area,
            action: Action::EditorCursor { top: 0, left },
        });
        frame.set_cursor_position((
            area.x
                + column
                    .saturating_sub(usize::from(left))
                    .min(usize::from(area.width - 1)) as u16,
            area.y,
        ));
    }
}

impl App {
    pub(crate) fn render_macros(&mut self, frame: &mut Frame, area: Rect) {
        frame.render_widget(
            Block::default()
                .borders(Borders::ALL)
                .title("Macros - Up/Down select; Enter/F2 edit; Insert new"),
            area,
        );
        let inner = area.inner(Margin {
            horizontal: 1,
            vertical: 1,
        });
        buttons(
            frame,
            row(inner, 0),
            &[
                ("[New]", KeyCode::Insert),
                ("[Edit]", KeyCode::F(2)),
                ("[Delete definition]", KeyCode::Delete),
            ],
            &mut self.hits,
        );
        let profile = self.document.profile();
        self.selected_macro = self
            .selected_macro
            .min(profile.macros.len().saturating_sub(1));
        if profile.macros.is_empty() {
            text(
                frame,
                row(inner, 2),
                "No macro timelines supplied; hardware macros are UNKNOWN.",
            );
            text(
                frame,
                row(inner, 3),
                "New creates a file-local draft; no recording or HID access.",
            );
            return;
        }
        let list_height = inner
            .height
            .saturating_sub(4)
            .div_ceil(3)
            .max(1)
            .min(profile.macros.len().min(u16::MAX as usize) as u16);
        let start = self
            .selected_macro
            .saturating_sub(usize::from(list_height.saturating_sub(1)));
        for (offset, (index, definition)) in profile
            .macros
            .iter()
            .enumerate()
            .skip(start)
            .take(usize::from(list_height))
            .enumerate()
        {
            let value = format!(
                "{} | {} | {} events | {} refs",
                definition.name,
                definition.definition.playback,
                definition.definition.events.len(),
                macro_references(profile, &definition.source_id).len()
            );
            button(
                frame,
                row(inner, offset as u16 + 1),
                &value,
                Action::Macro(index),
                &mut self.hits,
                index == self.selected_macro,
            );
        }
        let definition = &profile.macros[self.selected_macro];
        let offset = list_height + 2;
        let duration: u64 = definition
            .definition
            .events
            .iter()
            .map(|event| u64::from(event_parts(event).2))
            .sum();
        text(
            frame,
            row(inner, offset),
            &format!("File timeline including last delay: {duration} ms"),
        );
        for (index, event) in definition
            .definition
            .events
            .iter()
            .enumerate()
            .take(usize::from(inner.height.saturating_sub(offset + 2)))
        {
            let (kind, input, delay) = event_parts(event);
            text(
                frame,
                row(inner, offset + 1 + index as u16),
                &format!("{:>2}  {kind:<12} {input:<18} after {delay} ms", index + 1),
            );
        }
        text(
            frame,
            row(inner, inner.height.saturating_sub(1)),
            "Chords keep separate down/up rows. Draft editing != hardware execution.",
        );
    }
}

pub(crate) fn render_editor(
    frame: &mut Frame,
    area: Rect,
    editor: &MacroEditor,
    profile: &hyperx_core::SoftwareProfile,
    hits: &mut Vec<Hit>,
) {
    frame.render_widget(
        Block::default()
            .borders(Borders::ALL)
            .title("Macro timeline - LOCAL DRAFT / OFFLINE"),
        area,
    );
    let inner = area.inner(Margin {
        horizontal: 1,
        vertical: 1,
    });
    if inner.height < 12 {
        text(frame, inner, "Enlarge terminal to edit a timeline (45x18 minimum).\nCtrl+S updates file; Esc then y discards, n keeps.\nNo mouse settings are changed.");
        return;
    }
    if let Some(prompt) = &editor.prompt {
        match prompt {
            Prompt::Info {
                text: report,
                scroll,
            } => {
                let body = Rect::new(
                    inner.x,
                    inner.y,
                    inner.width,
                    inner.height.saturating_sub(2),
                );
                frame.render_widget(
                    Paragraph::new(safe_text(report))
                        .wrap(Wrap { trim: false })
                        .scroll((*scroll, 0)),
                    body,
                );
                buttons(
                    frame,
                    row(inner, inner.height - 1),
                    &[("[Back to timeline]", KeyCode::Esc)],
                    hits,
                );
            }
            Prompt::Confirm(reason) => {
                let message = match reason {
                    Confirmation::Discard => "Discard the uncommitted macro timeline?",
                    Confirmation::Replace => "Replace this REFERENCED definition in the file?",
                };
                text(frame, row(inner, 0), message);
                text(
                    frame,
                    row(inner, 2),
                    &format!("{} ({})", editor.draft.name, editor.draft.source_id),
                );
                text(
                    frame,
                    row(inner, 3),
                    &format!(
                        "References: {}",
                        macro_references(profile, &editor.draft.source_id).join(", ")
                    ),
                );
                text(
                    frame,
                    row(inner, 5),
                    "y confirms; n/Esc returns to your timeline. No device writes.",
                );
                buttons(
                    frame,
                    row(inner, 7),
                    &[
                        ("[Yes]", KeyCode::Char('y')),
                        ("[Back to timeline]", KeyCode::Char('n')),
                    ],
                    hits,
                );
            }
            Prompt::Text {
                field,
                editor: input,
            } => {
                text(frame, row(inner, 0), match field { TextField::Name => "Macro name (identity stays unchanged)", TextField::Delay(_) => "Delay AFTER event: file range 0..65535 ms, hardware support checked separately" });
                text_field(frame, row(inner, 2), input, hits);
                text(frame, row(inner, 4), editor.error.as_deref().unwrap_or("Enter accepts this field locally; Esc cancels field only. Ctrl+A selects all."));
                buttons(
                    frame,
                    row(inner, 6),
                    &[
                        ("[Accept field]", KeyCode::Enter),
                        ("[Cancel field]", KeyCode::Esc),
                    ],
                    hits,
                );
            }
            Prompt::Input {
                choices,
                search,
                selected,
                ..
            } => {
                text(
                    frame,
                    row(inner, 0),
                    "Named input - type/paste to search; arrows/click preview; Enter accepts",
                );
                text_field(frame, row(inner, 1), search, hits);
                let indices = editor.input_indices();
                let count = usize::from(inner.height.saturating_sub(5));
                let start = selected
                    .and_then(|value| indices.iter().position(|index| *index == value))
                    .unwrap_or(0)
                    .saturating_sub(count.saturating_sub(1));
                for (offset, index) in indices.iter().skip(start).take(count).enumerate() {
                    button(
                        frame,
                        row(inner, 2 + offset as u16),
                        &choices[*index],
                        Action::MacroInputChoice(*index),
                        hits,
                        selected == &Some(*index),
                    );
                }
                if indices.is_empty() {
                    text(frame, row(inner, 2), "No matching inputs.");
                }
                text(
                    frame,
                    row(inner, inner.height - 2),
                    editor
                        .error
                        .as_deref()
                        .unwrap_or("Imported aliases remain unchanged unless explicitly replaced."),
                );
                buttons(
                    frame,
                    row(inner, inner.height - 1),
                    &[
                        ("[Accept input]", KeyCode::Enter),
                        ("[Cancel input]", KeyCode::Esc),
                    ],
                    hits,
                );
            }
        }
        return;
    }
    text(
        frame,
        row(inner, 0),
        &format!(
            "{} | {} | {}",
            editor.draft.name,
            editor.draft.definition.playback,
            if editor.dirty() {
                "UNCOMMITTED"
            } else {
                "unchanged"
            }
        ),
    );
    text(
        frame,
        row(inner, 1),
        &format!(
            "ID {} (fixed) | {} file references",
            editor.draft.source_id,
            macro_references(profile, &editor.draft.source_id).len()
        ),
    );
    buttons(
        frame,
        row(inner, 2),
        &[
            ("[Name F2]", KeyCode::F(2)),
            ("[Playback F3]", KeyCode::F(3)),
            ("[Preflight F9]", KeyCode::F(9)),
        ],
        hits,
    );
    buttons(
        frame,
        row(inner, 3),
        &[
            ("[Type F4]", KeyCode::F(4)),
            ("[Input F5]", KeyCode::F(5)),
            ("[Delay F6]", KeyCode::F(6)),
        ],
        hits,
    );
    buttons(
        frame,
        row(inner, 4),
        &[
            ("[Add]", KeyCode::Insert),
            ("[Remove]", KeyCode::Delete),
            ("[Move up]", KeyCode::F(7)),
            ("[Move down]", KeyCode::F(8)),
        ],
        hits,
    );
    text(
        frame,
        row(inner, 5),
        "#   Transition   Named input                   Delay AFTER event",
    );
    let height = usize::from(inner.height.saturating_sub(11));
    let start = editor.selected.saturating_sub(height.saturating_sub(1));
    for (index, event) in editor
        .draft
        .definition
        .events
        .iter()
        .enumerate()
        .skip(start)
        .take(height)
    {
        let (kind, input, delay) = event_parts(event);
        let y = 6 + (index - start) as u16;
        button(
            frame,
            row(inner, y),
            &format!(
                "{:>2}  {kind:<12} {:<25} {delay} ms",
                index + 1,
                if input.is_empty() {
                    "<choose explicitly>"
                } else {
                    input
                }
            ),
            Action::MacroRow(index),
            hits,
            index == editor.selected,
        );
    }
    if editor.draft.definition.events.is_empty() {
        text(
            frame,
            row(inner, 6),
            "Empty timeline draft. Add creates an explicit down/up transition.",
        );
    }
    let bottom = inner.height - 5;
    let mut limits = Vec::new();
    for control in profile_controls(&profile.device) {
        if let Some(caps) = control.macros {
            let modes = |values: &[hyperx_core::MacroPlayback]| {
                values
                    .iter()
                    .map(|mode| match mode {
                        hyperx_core::MacroPlayback::Once => "Once",
                        hyperx_core::MacroPlayback::ToggleRepeat => "Toggle",
                        hyperx_core::MacroPlayback::RepeatWhileHeld => "Hold",
                    })
                    .collect::<Vec<_>>()
                    .join("/")
            };
            limits.push(format!(
                "{} runtime {}; onboard {}; {} events / {} ms (encoder limits, F9 details)",
                control.name,
                modes(caps.runtime_playback),
                modes(caps.onboard_playback),
                caps.max_events,
                caps.max_delay_ms,
            ));
        }
    }
    for (index, limit) in limits.iter().take(2).enumerate() {
        text(frame, row(inner, bottom + index as u16), limit);
    }
    text(
        frame,
        row(inner, inner.height - 3),
        editor.error.as_deref().unwrap_or(
            "Ctrl+S updates FILE draft; empty/unbalanced timelines remain non-executable drafts.",
        ),
    );
    text(
        frame,
        row(inner, inner.height - 2),
        "Up/Down select; Shift+Up/Down reorder. Esc cancels; no global recorder or HID.",
    );
    button(
        frame,
        sub(row(inner, inner.height - 1), 0, 23),
        "[Update file Ctrl+S]",
        Action::AcceptEditor,
        hits,
        false,
    );
    button(
        frame,
        sub(row(inner, inner.height - 1), 24, 14),
        "[Cancel]",
        Action::Key(KeyCode::Esc),
        hits,
        false,
    );
}

pub(crate) fn render_delete(
    frame: &mut Frame,
    area: Rect,
    original: &hyperx_core::NamedMacro,
    error: Option<&str>,
    hits: &mut Vec<Hit>,
) {
    frame.render_widget(
        Block::default()
            .borders(Borders::ALL)
            .title("Confirm library deletion - OFFLINE"),
        area,
    );
    let inner = area.inner(Margin {
        horizontal: 1,
        vertical: 1,
    });
    frame.render_widget(Paragraph::new(safe_text(&format!("Delete {:?} ({}) from the file?\nMouse settings remain untouched. Referenced definitions cannot be deleted.\n{}", original.name, original.source_id, error.unwrap_or("y confirms; n/Esc cancels.")))).wrap(Wrap { trim: false }), inner);
    buttons(
        frame,
        row(inner, inner.height.saturating_sub(1)),
        &[
            ("[Yes: delete]", KeyCode::Char('y')),
            ("[Cancel]", KeyCode::Esc),
        ],
        hits,
    );
}
