use hyperx_core::MacroEvent;
use ratatui::{
    layout::{Constraint, Layout, Margin},
    style::{Color, Style},
    text::Line,
    widgets::{Block, Borders, Clear, Paragraph, Wrap},
    Frame,
};

use crate::app::{App, Modal};
use crate::widgets::{sub, Action, Hit};

const TABS: [&str; 5] = ["Performance", "Buttons", "Macros", "Lighting", "Profiles"];

impl App {
    pub fn render(&mut self, frame: &mut Frame) {
        // Hit testing must use ONLY the currently drawn viewport, never cached
        // underlying controls after opening a modal or shrinking the terminal.
        self.hits.clear();
        if frame.area().width < 45 || frame.area().height < 12 {
            self.drag = None;
            frame.render_widget(Paragraph::new("OpenHyperX OFFLINE\nTerminal too small: use at least 45x12.\nNo HID access / Save to mouse unavailable."), frame.area());
            return;
        }
        let areas = Layout::vertical([
            Constraint::Length(2),
            Constraint::Length(3),
            Constraint::Min(1),
            Constraint::Length(2),
            Constraint::Length(2),
        ])
        .split(frame.area());
        let profile = self.document.profile();
        let readiness = hyperx_app::validate_profile(profile);
        let mode = if self.demo { "DEMO DATA" } else { "FILE DRAFT" };
        frame.render_widget(Paragraph::new(format!("OpenHyperX | OFFLINE | {mode} | {}\nTarget {:?} | profile {:?} | NOT connected/read from mouse", if self.document.dirty() { "UNSAVED" } else { "unchanged" }, profile.device, profile.name)).style(Style::default().fg(Color::Cyan)), areas[0]);
        frame.render_widget(Block::default().borders(Borders::ALL), areas[1]);
        let tab_row = areas[1].inner(Margin {
            horizontal: 1,
            vertical: 1,
        });
        let mut offset = 0;
        let labels = if tab_row.width < 55 {
            ["Perf", "Btns", "Macros", "Lights", "Profiles"]
        } else {
            TABS
        };
        for (index, tab) in labels.iter().enumerate() {
            let width = tab.len() as u16 + 3;
            self.button(
                frame,
                sub(tab_row, offset, width),
                &format!(" {tab} |"),
                Action::Tab(index),
                self.tab == index,
            );
            offset += width;
        }
        if self.tab != 0 && self.tab != 3 {
            let content = self.content();
            self.scroll = self.scroll.min(
                content
                    .lines()
                    .count()
                    .saturating_sub(1)
                    .min(usize::from(u16::MAX)) as u16,
            );
        }
        if self.tab == 0 {
            self.render_performance(frame, areas[2]);
        } else if self.tab == 3 {
            self.render_lighting(frame, areas[2]);
        } else {
            let content = self.content();
            frame.render_widget(
                Paragraph::new(safe_text(&content))
                    .block(Block::default().borders(Borders::ALL).title(TABS[self.tab]))
                    .scroll((self.scroll, 0))
                    .wrap(Wrap { trim: false }),
                areas[2],
            );
        }
        let validation = readiness.error.map_or_else(
            || "Offline supplied-field validation: OK (not hardware verification)".into(),
            |error| format!("NOT READY: {error}"),
        );
        frame.render_widget(
            Paragraph::new(format!("{validation}\n{}", safe_text(&self.status))),
            areas[3],
        );
        for (line, buttons) in [
            vec![
                ("[o Open]", 'o'),
                ("[s Save NEW]", 's'),
                ("[v Validate]", 'v'),
                ("[d Diff]", 'd'),
                ("[e TOML]", 'e'),
            ],
            vec![
                ("[m Macro]", 'm'),
                ("[r Resolve]", 'r'),
                ("[x Omit]", 'x'),
                ("[q Quit]", 'q'),
            ],
        ]
        .iter()
        .enumerate()
        {
            let mut offset = 0;
            for (label, key) in buttons {
                let row = ratatui::layout::Rect::new(
                    areas[4].x,
                    areas[4].y + line as u16,
                    areas[4].width,
                    1,
                );
                self.button(
                    frame,
                    sub(row, offset, label.len() as u16 + 1),
                    label,
                    Action::Key(crossterm::event::KeyCode::Char(*key)),
                    false,
                );
                offset += label.len() as u16 + 1;
            }
        }
        if let Some(modal) = &self.modal {
            self.hits.clear();
            self.drag = None;
            let area = frame.area().inner(Margin {
                horizontal: 2,
                vertical: 2,
            });
            frame.render_widget(Clear, area);
            match modal {
                Modal::Confirm { open } => frame.render_widget(
                    Paragraph::new(format!(
                        "Unsaved file edits. {} and discard them?\ny: confirm | n/Esc: cancel",
                        if *open { "Open another file" } else { "Quit" }
                    ))
                    .block(
                        Block::default()
                            .borders(Borders::ALL)
                            .title("Confirm discard"),
                    ),
                    area,
                ),
                Modal::Viewer {
                    title,
                    text,
                    scroll,
                } => frame.render_widget(
                    Paragraph::new(safe_text(text))
                        .block(Block::default().borders(Borders::ALL).title(title.as_str()))
                        .wrap(Wrap { trim: false })
                        .scroll((*scroll, 0)),
                    area,
                ),
                Modal::Editor {
                    title,
                    editor,
                    error,
                    ..
                } => {
                    let regions =
                        Layout::vertical([Constraint::Min(3), Constraint::Length(3)]).split(area);
                    let inner = regions[0].inner(Margin {
                        horizontal: 1,
                        vertical: 1,
                    });
                    let top = editor
                        .row
                        .saturating_sub(usize::from(inner.height.saturating_sub(1)))
                        .min(usize::from(u16::MAX)) as u16;
                    let prefix: String = editor.lines[editor.row]
                        .chars()
                        .take(editor.column)
                        .collect();
                    let column = Line::raw(prefix).width();
                    let left = column
                        .saturating_sub(usize::from(inner.width.saturating_sub(1)))
                        .min(usize::from(u16::MAX)) as u16;
                    frame.render_widget(
                        Paragraph::new(safe_text(&editor.text()))
                            .style(if editor.selected {
                                Style::default().bg(Color::Blue)
                            } else {
                                Style::default()
                            })
                            .block(Block::default().borders(Borders::ALL).title(title.as_str()))
                            .scroll((top, left)),
                        regions[0],
                    );
                    let help = ratatui::layout::Rect::new(
                        regions[1].x,
                        regions[1].y,
                        regions[1].width,
                        regions[1].height.saturating_sub(1),
                    );
                    frame.render_widget(Paragraph::new(safe_text(error.as_deref().unwrap_or("File draft only. Accept updates the draft; Save writes a NEW file. Esc cancels; omitted values do not reset hardware."))).wrap(Wrap { trim: false }), help);
                    self.hits.push(Hit {
                        area: inner,
                        action: Action::EditorCursor { top, left },
                    });
                    if inner.width > 0 && inner.height > 0 {
                        frame.set_cursor_position((
                            inner.x
                                + column
                                    .saturating_sub(usize::from(left))
                                    .min(usize::from(inner.width - 1))
                                    as u16,
                            inner.y
                                + editor
                                    .row
                                    .saturating_sub(usize::from(top))
                                    .min(usize::from(inner.height - 1))
                                    as u16,
                        ));
                    }
                }
            }
            let row = ratatui::layout::Rect::new(
                area.x + 1,
                area.y + area.height
                    - if matches!(modal, Modal::Editor { .. }) {
                        1
                    } else {
                        2
                    },
                area.width.saturating_sub(2),
                1,
            );
            use crossterm::event::KeyCode;
            frame.render_widget(Clear, row);
            let buttons = match modal {
                Modal::Confirm { .. } => vec![
                    ("[Yes: discard]", KeyCode::Char('y')),
                    ("[Cancel]", KeyCode::Esc),
                ],
                Modal::Viewer { .. } => vec![("[Close]", KeyCode::Esc)],
                Modal::Editor { .. } => vec![
                    ("[Accept draft]", KeyCode::Enter),
                    ("[Cancel]", KeyCode::Esc),
                ],
            };
            let mut offset = 0;
            for (label, key) in buttons {
                // The mouse accept action has the same semantics as Ctrl+S
                // even in a multiline TOML editor; Enter there inserts a line.
                let action = if key == KeyCode::Enter {
                    Action::AcceptEditor
                } else {
                    Action::Key(key)
                };
                self.button(
                    frame,
                    sub(row, offset, label.len() as u16 + 1),
                    label,
                    action,
                    false,
                );
                offset += label.len() as u16 + 1;
            }
        }
    }

    pub fn content(&self) -> String {
        let profile = self.document.profile();
        let mut text =
            "Only supplied file settings; <not present> means preserve, not default/reset.\n\n"
                .to_owned();
        match self.tab {
            0 => {
                text.push_str(&format!(
                    "Polling: {}\nPrimary layout: {:?}\n",
                    profile.polling.map_or_else(
                        || "<not present>".into(),
                        |value| format!("{} Hz", value.hz)
                    ),
                    profile.primary_buttons
                ));
                if let Some(dpi) = &profile.dpi {
                    text.push_str(&format!(
                        "Active stage: {:?} (zero-based); source active: {:?} (provenance)\n",
                        dpi.active_stage, dpi.source_active_stage
                    ));
                    for (index, stage) in dpi.stages.iter().enumerate() {
                        text.push_str(&format!(
                            "Stage {index}: X={} Y={} {}\n",
                            stage.x, stage.y, stage.color
                        ));
                    }
                } else {
                    text.push_str("DPI stages: <not present / not read>\n");
                }
                if let Some(device) = hyperx_app::device_descriptor(&profile.device) {
                    text.push_str(&format!(
                        "\nDeclared model DPI capabilities: {:?}\nNot live device measurements.\n",
                        device.capabilities.dpi
                    ));
                }
            }
            1 => {
                let controls = hyperx_app::profile_controls(&profile.device);
                for control in &controls {
                    let value = if control.primary {
                        format!("coupled pair {:?}", profile.primary_buttons)
                    } else {
                        profile.buttons.get(control.id).map_or_else(
                            || "<not present / not read>".into(),
                            |value| format!("{value:?}"),
                        )
                    };
                    text.push_str(&format!("{} ({}) -> {value}\n", control.id, control.name));
                }
                for (id, value) in &profile.buttons {
                    if !controls.iter().any(|control| control.id == id) {
                        text.push_str(&format!("UNKNOWN CONTROL {id:?} -> {value:?}\n"));
                    }
                }
                text.push_str("\nPrimary buttons are one atomic Standard/Swapped layout.\nMacro references do not reveal hardware timelines.\n");
            }
            2 => {
                if profile.macros.is_empty() {
                    text.push_str(
                        "No macro timelines supplied. Existing hardware macros are UNKNOWN.\n",
                    );
                }
                for named in &profile.macros {
                    text.push_str(&format!(
                        "Macro {:?}: {:?} | {}\n",
                        named.source_id, named.name, named.definition.playback
                    ));
                    let mut elapsed = 0u64;
                    for (index, event) in named.definition.events.iter().enumerate() {
                        let delay = match event {
                            MacroEvent::KeyDown { delay_ms, .. }
                            | MacroEvent::KeyUp { delay_ms, .. }
                            | MacroEvent::MouseButtonDown { delay_ms, .. }
                            | MacroEvent::MouseButtonUp { delay_ms, .. } => *delay_ms,
                        };
                        text.push_str(&format!("  {} @ {elapsed} ms: {event:?}\n", index + 1));
                        elapsed += u64::from(delay);
                    }
                    text.push_str(&format!(
                        "  File timeline including last delay: {elapsed} ms\n\n"
                    ));
                }
                text.push_str("Chords: separate key-down/up events with zero delays between simultaneous presses.\nLimits are target-specific implemented encodings, not proven hardware maxima:\n");
                for control in hyperx_app::profile_controls(&profile.device) {
                    if let Some(caps) = control.macros {
                        text.push_str(&format!(
                            "{}: runtime {:?}; onboard {:?}; {} events; max delay {} ms\n",
                            control.id,
                            caps.runtime_playback,
                            caps.onboard_playback,
                            caps.max_events,
                            caps.max_delay_ms
                        ));
                    }
                }
            }
            3 => {
                if let Some(lighting) = &profile.lighting {
                    text.push_str(&format!("Mode: {:?}\n", lighting.mode));
                    for (zone, color) in &lighting.zones {
                        text.push_str(&format!("{zone}: {color}\n"));
                    }
                } else {
                    text.push_str("Lighting: <not present / current colors and effects unknown>\n");
                }
                text.push_str("\nProfile lighting currently supports Solid with both wheel and logo.\nBlack means off ONLY when explicitly supplied.\nSoftware effects/rainbow are separate runtime commands, not inferred here.\nNo LED changes are sent from this offline editor.\n");
            }
            _ => {
                text.push_str(&format!("Name: {:?}\nDevice target: {:?}\nPartial: {}\nSource: {:?}\nOpened/saved path: {:?}\n", profile.name, profile.device, profile.partial, profile.source, self.document.path()));
                for assignment in &profile.unresolved_button_assignments {
                    text.push_str(&format!(
                        "UNRESOLVED {:?}: macro source {:?}\n",
                        assignment.source_id, assignment.macro_source_id
                    ));
                }
                text.push_str("\nUnresolved entries block apply; r resolves an explicit target/library macro.\nx deliberately omits one entry; no source ID is guessed into a physical target.\ns writes a NEW TOML file, never overwrites. Comments are not preserved.\nSave to mouse: UNAVAILABLE OFFLINE. No device discovered/opened.\n");
            }
        }
        text
    }
}

fn safe_text(text: &str) -> String {
    text.chars()
        .map(|character| {
            if character.is_control() && !matches!(character, '\n' | '\t') {
                '\u{fffd}'
            } else {
                character
            }
        })
        .collect()
}
