//! File-client controls. Paths are presentation data, never HID device paths.
use std::path::Path;

use crossterm::event::KeyCode;
use ratatui::{
    layout::{Margin, Rect},
    style::{Color, Style},
    text::Line,
    widgets::{Block, Borders, Paragraph, Wrap},
    Frame,
};

use crate::{
    app::App,
    editor::Editor,
    files::{FileBrowser, Mode, Prompt},
    recovery_picker::{Confirmation as RecoveryConfirmation, RecoveryPicker},
    render::safe_text,
    widgets::{sub, Action, Hit},
};

fn row(area: Rect, y: u16) -> Rect {
    Rect::new(
        area.x,
        area.y.saturating_add(y).min(area.bottom()),
        area.width,
        u16::from(y < area.height),
    )
}

pub(crate) fn render_recovery(
    frame: &mut Frame,
    area: Rect,
    picker: &RecoveryPicker,
    current_dirty: bool,
    active: Option<&Path>,
    hits: &mut Vec<Hit>,
) {
    frame.render_widget(
        Block::default()
            .borders(Borders::ALL)
            .title("Offline TUI recovery - no HID access"),
        area,
    );
    let inner = area.inner(Margin {
        horizontal: 1,
        vertical: 1,
    });
    if inner.height < 4 {
        text(
            frame,
            inner,
            "Enlarge terminal for recovery actions; Esc closes.",
        );
        return;
    }
    if let Some(reason) = picker.confirmation {
        if let Some(entry) = picker.entries.get(picker.selected) {
            let action = match reason {
                RecoveryConfirmation::Restore => "Restore this unsaved FILE draft?",
                RecoveryConfirmation::Discard => "Permanently discard only this recovery snapshot?",
            };
            let explanation = match reason {
                RecoveryConfirmation::Restore if current_dirty => {
                    "Current draft stays recoverable. No FILE/USB write."
                }
                RecoveryConfirmation::Restore => {
                    "Current clean view is replaced. No FILE/USB write."
                }
                RecoveryConfirmation::Discard => {
                    "Permanent. Profile files and mouse are untouched."
                }
            };
            frame.render_widget(
                Paragraph::new(safe_text(&format!(
                    "{action}\n{explanation}\n\nSnapshot: {}\nProfile: {}",
                    entry.path.file_name().unwrap_or_default().to_string_lossy(),
                    entry.profile.as_deref().unwrap_or("UNREADABLE")
                )))
                .wrap(Wrap { trim: false }),
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
                    ("[Yes y]", KeyCode::Char('y')),
                    ("[Cancel n]", KeyCode::Char('n')),
                ],
                hits,
            );
        }
        return;
    }
    text(
        frame,
        row(inner, 0),
        "Select one snapshot. Restore and discard need confirmation.",
    );
    let visible = usize::from(inner.height.saturating_sub(5)).max(1);
    let start = picker
        .selected
        .saturating_sub(visible / 2)
        .min(picker.entries.len().saturating_sub(visible));
    if picker.entries.is_empty() {
        text(frame, row(inner, 1), "No older TUI snapshots.");
    }
    for (offset, entry) in picker.entries.iter().enumerate().skip(start).take(visible) {
        let list_row = row(inner, 1 + (offset - start) as u16);
        if list_row.height == 0 {
            break;
        }
        let active_label = if active == Some(entry.path.as_path()) {
            " [ACTIVE]"
        } else {
            ""
        };
        let label = format!(
            "{} {}{} · {}",
            if offset == picker.selected { '>' } else { ' ' },
            entry.path.file_name().unwrap_or_default().to_string_lossy(),
            active_label,
            entry.profile.as_deref().unwrap_or("UNREADABLE")
        );
        frame.render_widget(
            Paragraph::new(single_line(&label)).style(Style::default().fg(
                if offset == picker.selected {
                    Color::Yellow
                } else if entry.error.is_some() {
                    Color::Red
                } else {
                    Color::Gray
                },
            )),
            list_row,
        );
        hits.push(Hit {
            area: list_row,
            action: Action::RecoveryEntry(offset),
        });
    }
    if let Some(entry) = picker.entries.get(picker.selected) {
        let detail = entry.error.as_deref().map_or_else(
            || {
                format!(
                    "Original FILE: {}",
                    entry.original_file.as_ref().map_or_else(
                        || "none (unsaved profile)".into(),
                        |path| path.display().to_string()
                    )
                )
            },
            |error| format!("UNREADABLE: {error}"),
        );
        text(frame, row(inner, inner.height - 3), &single_line(&detail));
    }
    if let Some(error) = &picker.error {
        text(frame, row(inner, inner.height - 2), &single_line(error));
    }
    buttons(
        frame,
        row(inner, inner.height - 1),
        &[
            ("[Restore r]", KeyCode::Char('r')),
            ("[Discard d]", KeyCode::Char('d')),
            ("[F5 refresh]", KeyCode::F(5)),
            ("[Esc]", KeyCode::Esc),
        ],
        hits,
    );
}
fn text(frame: &mut Frame, area: Rect, value: &str) {
    frame.render_widget(Paragraph::new(safe_text(value)), area);
}
fn single_line(value: &str) -> String {
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
        Paragraph::new(single_line(label)).style(Style::default().fg(if selected {
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
    for (label, key) in values {
        let width = label.len() as u16 + 1;
        button(
            frame,
            sub(area, offset, width),
            label,
            Action::Key(*key),
            false,
            hits,
        );
        offset += width;
    }
}
fn field(frame: &mut Frame, area: Rect, editor: &Editor, hits: &mut Vec<Hit>) {
    let prefix: String = editor.lines[0].chars().take(editor.column).collect();
    let column = Line::raw(prefix).width();
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
    pub(crate) fn render_profiles(&mut self, frame: &mut Frame, area: Rect) {
        let title = if area.width < 70 {
            "Profiles: F2-F7 actions · ↑/↓ scroll"
        } else {
            "Profiles - FILE operations only"
        };
        frame.render_widget(Block::default().borders(Borders::ALL).title(title), area);
        let inner = area.inner(Margin {
            horizontal: 1,
            vertical: 1,
        });
        buttons(
            frame,
            row(inner, 0),
            &[
                ("[Browse F2]", KeyCode::F(2)),
                ("[Name F3]", KeyCode::F(3)),
                ("[Copy NEW F4]", KeyCode::F(4)),
            ],
            &mut self.hits,
        );
        buttons(
            frame,
            row(inner, 1),
            &[
                ("[Unresolved F5]", KeyCode::F(5)),
                ("[Overwrite FILE F6]", KeyCode::F(6)),
            ],
            &mut self.hits,
        );
        buttons(
            frame,
            row(inner, 2),
            &[
                ("[Undo u]", KeyCode::Char('u')),
                ("[Redo U]", KeyCode::Char('U')),
                ("[Recovery F7]", KeyCode::F(7)),
            ],
            &mut self.hits,
        );
        let body = Rect::new(
            inner.x,
            inner.y.saturating_add(3).min(inner.bottom()),
            inner.width,
            inner.height.saturating_sub(3),
        );
        frame.render_widget(
            Paragraph::new(safe_text(&self.content()))
                .scroll((self.scroll, 0))
                .wrap(Wrap { trim: false }),
            body,
        );
    }
}

pub(crate) fn render_browser(
    frame: &mut Frame,
    area: Rect,
    browser: &FileBrowser,
    hits: &mut Vec<Hit>,
) {
    frame.render_widget(
        Block::default()
            .borders(Borders::ALL)
            .title(match browser.mode {
                Mode::Open => "Browse profiles - OFFLINE / read only",
                Mode::SaveNew => "Copy profile to NEW file - OFFLINE / never overwrite",
            }),
        area,
    );
    let inner = area.inner(Margin {
        horizontal: 1,
        vertical: 1,
    });
    if inner.height < 9 {
        text(
            frame,
            inner,
            "Enlarge terminal to at least 45x12. Esc cancels the field/dialog. No HID access.",
        );
        return;
    }
    if let Some(document) = &browser.pending {
        let value = format!("Discard unsaved FILE edits and open the selected profile?\n\nSelected path: {}\nProfile: {}\nTarget: {}\n\ny confirms; n/Esc keeps the current document and returns to the browser.\nThe selected file was parsed before this question; no device state was read.",
            document.path().unwrap().display(), document.profile().name, document.profile().device);
        frame.render_widget(
            Paragraph::new(safe_text(&value)).wrap(Wrap { trim: false }),
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
                ("[Yes: open]", KeyCode::Char('y')),
                ("[Keep draft]", KeyCode::Char('n')),
            ],
            hits,
        );
        return;
    }
    if let Some(prompt) = &browser.prompt {
        let (label, input) = match prompt {
            Prompt::Directory(input) => (
                "Directory (absolute or relative to the displayed directory)",
                input,
            ),
            Prompt::Filename(input) => ("NEW filename in selected directory - not a path", input),
        };
        text(frame, row(inner, 0), label);
        text(
            frame,
            row(inner, 2),
            &single_line(&browser.directory.to_string_lossy()),
        );
        field(frame, row(inner, 4), input, hits);
        frame.render_widget(
            Paragraph::new(safe_text(browser.error.as_deref().unwrap_or(
                "Enter accepts; Esc cancels this field. Existing files are never overwritten.",
            )))
            .wrap(Wrap { trim: false }),
            Rect::new(
                inner.x,
                inner.y + 5,
                inner.width,
                inner.height.saturating_sub(7),
            ),
        );
        buttons(
            frame,
            row(inner, inner.height - 1),
            &[
                ("[Accept]", KeyCode::Enter),
                ("[Cancel field]", KeyCode::Esc),
            ],
            hits,
        );
        return;
    }
    text(
        frame,
        row(inner, 0),
        &single_line(&browser.directory.to_string_lossy()),
    );
    buttons(
        frame,
        row(inner, 1),
        &[
            ("[Up]", KeyCode::Backspace),
            ("[Directory p]", KeyCode::Char('p')),
            ("[Refresh F5]", KeyCode::F(5)),
        ],
        hits,
    );
    let height = usize::from(inner.height.saturating_sub(7));
    let start = browser.selected.saturating_sub(height.saturating_sub(1));
    for (offset, (index, entry)) in browser
        .entries
        .iter()
        .enumerate()
        .skip(start)
        .take(height)
        .enumerate()
    {
        let value = format!(
            "{} {}",
            if entry.directory { "[dir] " } else { "[toml]" },
            entry.path.file_name().unwrap_or_default().to_string_lossy()
        );
        button(
            frame,
            row(inner, 3 + offset as u16),
            &value,
            Action::FileEntry(index),
            index == browser.selected,
            hits,
        );
    }
    if browser.entries.is_empty() {
        text(
            frame,
            row(inner, 3),
            "No subdirectories or TOML files in this directory.",
        );
    }
    text(
        frame,
        row(inner, inner.height - 4),
        &format!(
            "Selection {}/{}; TOML files + directories only; links skipped.",
            if browser.entries.is_empty() {
                0
            } else {
                browser.selected + 1
            },
            browser.entries.len()
        ),
    );
    frame.render_widget(Paragraph::new(safe_text(browser.error.as_deref().or(browser.warning.as_deref()).unwrap_or(
        "Click/arrows preview only. Enter opens selection; no recursive scan or profile parsing while browsing."))).wrap(Wrap { trim: false }),
        Rect::new(inner.x, inner.y + inner.height - 3, inner.width, 2));
    let actions: &[(&str, KeyCode)] = match browser.mode {
        Mode::Open => &[
            ("[Open selected]", KeyCode::Enter),
            ("[Cancel]", KeyCode::Esc),
        ],
        Mode::SaveNew => &[
            ("[NEW filename F2]", KeyCode::F(2)),
            ("[Cancel]", KeyCode::Esc),
        ],
    };
    buttons(frame, row(inner, inner.height - 1), actions, hits);
}
