//! Visible hit regions and typed offline controls. No vendor protocol knowledge.
use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use hyperx_app::{device_descriptor, edit_profile_value, ProfileValueEdit};
use hyperx_core::{DpiCapabilities, PrimaryButtonLayout, RgbColor};
use ratatui::{
    layout::{Position, Rect},
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Gauge, Paragraph},
    Frame,
};

use crate::app::{App, EditAction, Modal};

#[derive(Clone, Debug)]
pub enum Action {
    Key(KeyCode),
    AcceptEditor,
    Tab(usize),
    Control(usize),
    BindingCategory(i8),
    BindingChoice(usize),
    Macro(usize),
    MacroRow(usize),
    MacroInputChoice(usize),
    FileEntry(usize),
    ResolutionChoice(usize),
    Stage(usize),
    Slider(usize),
    AdjustDpi { index: usize, delta: i64 },
    DpiInput(usize),
    ColorInput(usize),
    PaletteColor(u8),
    AddStage,
    ZoneColor(&'static str),
    Value(ProfileValueEdit),
    EditorCursor { top: u16, left: u16 },
}
#[derive(Clone, Debug)]
pub struct Hit {
    pub area: Rect,
    pub action: Action,
}

/// Terminal-cell coordinates are clamped and snapped to the model's DPI step.
pub fn slider_value(caps: DpiCapabilities, area: Rect, column: u16) -> u32 {
    let width = u64::from(area.width.saturating_sub(1).max(1));
    let offset = u64::from(column.saturating_sub(area.x)).min(width);
    let steps = u64::from(caps.maximum.saturating_sub(caps.minimum) / caps.step.max(1));
    caps.minimum + (((offset * steps + width / 2) / width) as u32) * caps.step.max(1)
}

impl App {
    pub fn clear_mouse_layout(&mut self) {
        self.hits.clear();
        self.drag = None;
    }

    pub(crate) fn edit_value(&mut self, edit: ProfileValueEdit) -> Result<()> {
        let edited = edit_profile_value(self.document.profile(), edit)?;
        self.document.replace(edited);
        self.status =
            "Updated file draft. No HID access; s saves a NEW file, not the mouse.".into();
        Ok(())
    }
    pub(crate) fn button(
        &mut self,
        frame: &mut Frame,
        area: Rect,
        label: &str,
        action: Action,
        selected: bool,
    ) {
        if area.width == 0 || area.height == 0 {
            return;
        }
        frame.render_widget(
            Paragraph::new(label).style(Style::default().fg(if selected {
                Color::Yellow
            } else {
                Color::Cyan
            })),
            area,
        );
        self.hits.push(Hit { area, action });
    }
    fn color_button(
        &mut self,
        frame: &mut Frame,
        area: Rect,
        label: String,
        color: RgbColor,
        action: Action,
    ) {
        if area.width == 0 || area.height == 0 {
            return;
        }
        // Keep the exact hex value visible even when truecolor is unsupported.
        // An outlined, neutral swatch makes explicit black distinguishable from
        // an unknown/missing color on dark terminal backgrounds.
        let (swatch, swatch_color) = if color == RgbColor::BLACK {
            ("□", Color::Gray)
        } else {
            ("■", Color::Rgb(color.red, color.green, color.blue))
        };
        frame.render_widget(
            Paragraph::new(Line::from(vec![
                Span::styled(label, Style::default().fg(Color::Cyan)),
                Span::styled(swatch, Style::default().fg(swatch_color)),
            ])),
            area,
        );
        self.hits.push(Hit { area, action });
    }
    fn caps(&self) -> Option<DpiCapabilities> {
        device_descriptor(&self.document.profile().device)?
            .capabilities
            .dpi
    }
    fn change_dpi(&mut self, index: usize, delta: i64) {
        let Some(caps) = self.caps() else {
            return;
        };
        let Some(stage) = self
            .document
            .profile()
            .dpi
            .as_ref()
            .and_then(|value| value.stages.get(index))
        else {
            return;
        };
        let value = (i64::from(stage.x) + delta)
            .clamp(i64::from(caps.minimum), i64::from(caps.maximum)) as u32;
        // Imported unsupported values can be explicitly repaired with a control;
        // no unrelated level or provenance is normalized.
        let value = caps.minimum + ((value - caps.minimum) / caps.step.max(1)) * caps.step.max(1);
        if let Err(error) = self.edit_value(ProfileValueEdit::StageDpi { index, dpi: value }) {
            self.status = error.to_string();
        }
    }
    pub(crate) fn performance_key(&mut self, key: KeyEvent) -> bool {
        if self.tab == 3 && self.modal.is_none() {
            if let KeyCode::F(index @ 2..=3) = key.code {
                if let Some(zone) =
                    device_descriptor(&self.document.profile().device).and_then(|device| {
                        device
                            .capabilities
                            .lighting_zones
                            .get(usize::from(index - 2))
                    })
                {
                    self.action(Action::ZoneColor(zone.id));
                }
                return true;
            }
        }
        if self.tab != 0 || self.modal.is_some() {
            return false;
        }
        let count = self
            .document
            .profile()
            .dpi
            .as_ref()
            .map_or(0, |value| value.stages.len());
        self.selected_stage = self.selected_stage.min(count.saturating_sub(1));
        match key.code {
            KeyCode::Left | KeyCode::Right | KeyCode::Char('+') | KeyCode::Char('-') => {
                let step = self.caps().map_or(0, |caps| i64::from(caps.step));
                let sign = if matches!(key.code, KeyCode::Left | KeyCode::Char('-')) {
                    -1
                } else {
                    1
                };
                self.change_dpi(
                    self.selected_stage,
                    sign * step
                        * if key.modifiers.contains(KeyModifiers::SHIFT) {
                            10
                        } else {
                            1
                        },
                );
            }
            KeyCode::Char('[') => self.selected_stage = self.selected_stage.saturating_sub(1),
            KeyCode::Char(']') => {
                self.selected_stage = (self.selected_stage + 1).min(count.saturating_sub(1))
            }
            KeyCode::F(2) => self.action(Action::DpiInput(self.selected_stage)),
            KeyCode::F(3) => self.action(Action::ColorInput(self.selected_stage)),
            KeyCode::Insert => self.action(Action::AddStage),
            KeyCode::Delete => self.action(Action::Value(ProfileValueEdit::RemoveLastStage)),
            KeyCode::Enter if count > 0 => self.action(Action::Value(
                ProfileValueEdit::ActiveStage(Some(self.selected_stage)),
            )),
            KeyCode::F(4) => {
                if let Some(device) = device_descriptor(&self.document.profile().device) {
                    let rates = device.capabilities.polling_rates;
                    if !rates.is_empty() {
                        let next = self
                            .document
                            .profile()
                            .polling
                            .and_then(|value| rates.iter().position(|rate| rate.hz() == value.hz))
                            .map_or(0, |index| (index + 1) % rates.len());
                        self.action(Action::Value(ProfileValueEdit::Polling(Some(
                            rates[next].hz(),
                        ))));
                    }
                }
            }
            _ => return false,
        }
        true
    }
    fn action(&mut self, action: Action) {
        match action {
            Action::Key(key) => self.handle_key(KeyEvent::new(key, KeyModifiers::NONE)),
            Action::AcceptEditor => {
                self.handle_key(KeyEvent::new(KeyCode::Char('s'), KeyModifiers::CONTROL))
            }
            Action::PaletteColor(number) => {
                self.handle_key(KeyEvent::new(KeyCode::F(number), KeyModifiers::NONE))
            }
            Action::AdjustDpi { index, delta } => self.change_dpi(index, delta),
            Action::Tab(tab) => {
                self.tab = tab;
                self.scroll = 0;
                self.drag = None;
            }
            Action::Stage(index) => self.selected_stage = index,
            Action::FileEntry(index) => {
                if let Some(Modal::Files(browser)) = &mut self.modal {
                    browser.select(index);
                }
            }
            Action::ResolutionChoice(index) => {
                if let Some(Modal::Resolution(picker)) = &mut self.modal {
                    picker.select(index);
                }
            }
            Action::Macro(index) => {
                self.selected_macro = index;
                self.open_macro(false);
            }
            Action::MacroRow(index) => {
                if let Some(Modal::Macro(editor)) = &mut self.modal {
                    if editor.prompt.is_none() && index < editor.draft.definition.events.len() {
                        editor.selected = index;
                    }
                }
            }
            Action::MacroInputChoice(index) => {
                if let Some(Modal::Macro(editor)) = &mut self.modal {
                    editor.select_input(index);
                }
            }
            Action::Control(index) => {
                self.selected_control = index;
                self.open_binding_picker();
            }
            Action::BindingCategory(delta) => {
                if let Some(Modal::Binding(picker)) = &mut self.modal {
                    picker.category_move(delta);
                }
            }
            Action::BindingChoice(index) => {
                if let Some(Modal::Binding(picker)) = &mut self.modal {
                    picker.select(index);
                }
            }
            Action::DpiInput(index) => {
                self.selected_stage = index;
                if let Some(stage) = self
                    .document
                    .profile()
                    .dpi
                    .as_ref()
                    .and_then(|value| value.stages.get(index))
                {
                    self.editor(
                        "DPI value (X=Y) - Enter accept, Esc cancel",
                        EditAction::StageDpi(index),
                        stage.x.to_string(),
                        true,
                    );
                }
            }
            Action::ColorInput(index) => {
                self.selected_stage = index;
                if let Some(stage) = self
                    .document
                    .profile()
                    .dpi
                    .as_ref()
                    .and_then(|value| value.stages.get(index))
                {
                    self.editor(
                        "Stage color #RRGGBB - Enter accept, Esc cancel",
                        EditAction::StageColor(index),
                        stage.color.to_string(),
                        true,
                    );
                }
            }
            Action::AddStage => {
                self.editor(
                    "New DPI stage value; WHITE color - Enter explicitly adds",
                    EditAction::AddStage,
                    "800".into(),
                    true,
                );
            }
            Action::ZoneColor(zone) => {
                let color = self
                    .document
                    .profile()
                    .lighting
                    .as_ref()
                    .and_then(|value| value.zones.get(zone))
                    .map_or_else(String::new, ToString::to_string);
                self.editor(
                    "Solid zone color #RRGGBB - Enter accept, Esc cancel",
                    EditAction::ZoneColor(zone),
                    color,
                    true,
                );
            }
            Action::Value(edit) => {
                if let Err(error) = self.edit_value(edit) {
                    self.status = error.to_string();
                }
            }
            Action::Slider(_) | Action::EditorCursor { .. } => {}
        }
    }
    fn drag_dpi(&mut self, index: usize, area: Rect, column: u16) {
        if let Some(caps) = self.caps() {
            if let Err(error) = self.edit_value(ProfileValueEdit::StageDpi {
                index,
                dpi: slider_value(caps, area, column),
            }) {
                self.status = error.to_string();
            }
        }
    }
    pub fn handle_mouse(&mut self, event: MouseEvent) {
        match event.kind {
            MouseEventKind::Up(MouseButton::Left) => {
                self.drag = None;
                return;
            }
            MouseEventKind::Drag(MouseButton::Left) if self.modal.is_none() => {
                if let Some((index, area)) = self.drag {
                    self.drag_dpi(index, area, event.column);
                }
                return;
            }
            _ => {}
        }
        let hit = self
            .hits
            .iter()
            .rev()
            .find(|hit| hit.area.contains(Position::new(event.column, event.row)))
            .cloned();
        if matches!(
            event.kind,
            MouseEventKind::ScrollUp | MouseEventKind::ScrollDown
        ) {
            let down = event.kind == MouseEventKind::ScrollDown;
            match &mut self.modal {
                Some(Modal::Viewer { scroll, .. }) => {
                    *scroll = if down {
                        scroll.saturating_add(3)
                    } else {
                        scroll.saturating_sub(3)
                    }
                }
                Some(Modal::Editor { editor, .. }) => {
                    for _ in 0..3 {
                        editor.key(KeyEvent::new(
                            if down { KeyCode::Down } else { KeyCode::Up },
                            KeyModifiers::NONE,
                        ));
                    }
                }
                Some(Modal::Binding(picker)) => picker.move_selection(if down { 3 } else { -3 }),
                Some(Modal::Macro(editor)) => editor.move_selection(if down { 3 } else { -3 }),
                Some(Modal::Files(browser)) => browser.move_selection(if down { 3 } else { -3 }),
                Some(Modal::Resolution(picker)) => picker.move_selection(if down { 3 } else { -3 }),
                Some(Modal::MacroDelete { .. }) => {}
                Some(Modal::Confirm { .. } | Modal::Overwrite { .. }) => {}
                None => {
                    if let Some(Hit {
                        action: Action::Slider(index),
                        ..
                    }) = hit
                    {
                        let step = self.caps().map_or(0, |caps| i64::from(caps.step));
                        self.change_dpi(index, if down { -step } else { step });
                    } else if self.tab == 2 {
                        self.macros_key(KeyEvent::new(
                            if down { KeyCode::Down } else { KeyCode::Up },
                            KeyModifiers::NONE,
                        ));
                    } else {
                        self.scroll = if down {
                            self.scroll.saturating_add(1)
                        } else {
                            self.scroll.saturating_sub(1)
                        };
                    }
                }
            }
            return;
        }
        if event.kind != MouseEventKind::Down(MouseButton::Left) {
            return;
        }
        self.drag = None;
        if let Some(hit) = hit {
            match hit.action {
                Action::Slider(index) => {
                    self.selected_stage = index;
                    self.drag = Some((index, hit.area));
                    self.drag_dpi(index, hit.area, event.column);
                }
                Action::EditorCursor { top, left } => {
                    let editor = match &mut self.modal {
                        Some(Modal::Editor { editor, .. }) => Some(editor),
                        Some(Modal::Macro(editor)) => editor.text_editor_mut(),
                        Some(Modal::Files(browser)) => browser.editor_mut(),
                        Some(Modal::Resolution(picker)) => picker.editor_mut(),
                        _ => None,
                    };
                    if let Some(editor) = editor {
                        editor.selected = false;
                        editor.row = (usize::from(top)
                            + usize::from(event.row.saturating_sub(hit.area.y)))
                        .min(editor.lines.len() - 1);
                        let target = usize::from(left)
                            + usize::from(event.column.saturating_sub(hit.area.x));
                        let mut width = 0;
                        editor.column = 0;
                        for character in editor.lines[editor.row].chars() {
                            width += Line::raw(character.to_string()).width();
                            if width > target {
                                break;
                            }
                            editor.column += 1;
                        }
                    }
                }
                action => self.action(action),
            }
        }
    }
    pub(crate) fn render_performance(&mut self, frame: &mut Frame, area: Rect) {
        frame.render_widget(
            Block::default()
                .borders(Borders::ALL)
                .title("Performance - click/drag; arrows: DPI step; F2 value; F3 color"),
            area,
        );
        let inner = area.inner(ratatui::layout::Margin {
            horizontal: 1,
            vertical: 1,
        });
        let dpi = self.document.profile().dpi.clone();
        let descriptor = device_descriptor(&self.document.profile().device);
        let caps = descriptor.and_then(|value| value.capabilities.dpi);
        let all_stages = dpi
            .as_ref()
            .map_or(&[][..], |value| value.stages.as_slice());
        // Keep invalid large imported drafts intact; render at most the model's
        // supported stage count. Hidden extra stages stay in TOML and validation.
        let visible = all_stages
            .len()
            .min(caps.map_or(0, |value| usize::from(value.max_stages)));
        let stages = &all_stages[..visible];
        self.selected_stage = self.selected_stage.min(stages.len().saturating_sub(1));
        let after = (stages.len() as u16 * 3 + 1).max(3);
        let length = after + 8;
        self.scroll = self.scroll.min(length.saturating_sub(inner.height));
        if let Some(row) = form_row(inner, 0, self.scroll) {
            frame.render_widget(
                Paragraph::new(format!(
                    "File values only; X=Y edits. {} of {} levels shown; click values/colors.",
                    stages.len(),
                    all_stages.len()
                )),
                row,
            );
        }
        if stages.is_empty() {
            if let Some(row) = form_row(inner, 1, self.scroll) {
                frame.render_widget(Paragraph::new("DPI stages: <not present / not read>"), row);
            }
        }
        for (index, stage) in stages.iter().enumerate() {
            let row_number = 1 + index as u16 * 3;
            if let Some(row) = form_row(inner, row_number, self.scroll) {
                let part = row.width.saturating_sub(23);
                self.button(
                    frame,
                    sub(row, 0, part),
                    &format!("Stage {index}: X={} Y={}", stage.x, stage.y),
                    Action::Stage(index),
                    self.selected_stage == index,
                );
                self.button(
                    frame,
                    sub(row, part + 1, 10),
                    if dpi.as_ref().unwrap().active_stage == Some(index) {
                        "[ACTIVE]"
                    } else {
                        "[Activate]"
                    },
                    Action::Value(ProfileValueEdit::ActiveStage(Some(index))),
                    false,
                );
                self.color_button(
                    frame,
                    sub(row, part + 12, 10),
                    format!("[{}]", stage.color),
                    stage.color,
                    Action::ColorInput(index),
                );
            }
            if let Some(row) = form_row(inner, row_number + 1, self.scroll) {
                self.button(
                    frame,
                    sub(row, 0, 3),
                    "[-]",
                    Action::AdjustDpi {
                        index,
                        delta: -i64::from(caps.map_or(0, |value| value.step)),
                    },
                    false,
                );
                self.button(
                    frame,
                    sub(row, 4, 8),
                    &format!("[{}]", stage.x),
                    Action::DpiInput(index),
                    false,
                );
                self.button(
                    frame,
                    sub(row, 13, 3),
                    "[+]",
                    Action::AdjustDpi {
                        index,
                        delta: i64::from(caps.map_or(0, |value| value.step)),
                    },
                    false,
                );
                if let Some(caps) = caps {
                    let slider = sub(row, 18, row.width.saturating_sub(18));
                    if slider.width >= 2 {
                        let ratio =
                            f64::from(stage.x.clamp(caps.minimum, caps.maximum) - caps.minimum)
                                / f64::from((caps.maximum - caps.minimum).max(1));
                        frame.render_widget(
                            Gauge::default()
                                .ratio(ratio)
                                .label(format!(
                                    "{}..{} step {}",
                                    caps.minimum, caps.maximum, caps.step
                                ))
                                .gauge_style(Style::default().fg(Color::Blue).bg(Color::DarkGray)),
                            slider,
                        );
                        self.hits.push(Hit {
                            area: slider,
                            action: Action::Slider(index),
                        });
                    }
                }
            }
        }
        if let Some(row) = form_row(inner, after, self.scroll) {
            if caps.is_some_and(|value| stages.len() < usize::from(value.max_stages)) {
                self.button(
                    frame,
                    sub(row, 0, 15),
                    "[+ Add level]",
                    Action::AddStage,
                    false,
                );
            }
            if stages.len() > 1 {
                self.button(
                    frame,
                    sub(row, 16, 18),
                    "[- Remove last]",
                    Action::Value(ProfileValueEdit::RemoveLastStage),
                    false,
                );
            }
        }
        if let Some(row) = form_row(inner, after + 1, self.scroll) {
            let text = dpi.as_ref().map_or_else(
                || "No supplied active stage".into(),
                |value| {
                    format!(
                        "Active: {}; source active: {} (provenance)",
                        value
                            .active_stage
                            .map_or_else(|| "<not present>".into(), |stage| stage.to_string()),
                        value
                            .source_active_stage
                            .map_or_else(|| "<not present>".into(), |stage| stage.to_string())
                    )
                },
            );
            frame.render_widget(Paragraph::new(text), row);
        }
        let polling = self.document.profile().polling;
        if let Some(row) = form_row(inner, after + 3, self.scroll) {
            frame.render_widget(
                Paragraph::new(polling.map_or_else(
                    || "Polling: <not present / not read>".into(),
                    |value| format!("Polling: {} Hz", value.hz),
                )),
                row,
            );
        }
        if let (Some(row), Some(descriptor)) = (form_row(inner, after + 4, self.scroll), descriptor)
        {
            for (index, rate) in descriptor.capabilities.polling_rates.iter().enumerate() {
                self.button(
                    frame,
                    sub(row, index as u16 * 8, 8),
                    &format!("[{}]", rate.hz()),
                    Action::Value(ProfileValueEdit::Polling(Some(rate.hz()))),
                    polling.is_some_and(|value| value.hz == rate.hz()),
                );
            }
        }
        if let Some(row) = form_row(inner, after + 6, self.scroll) {
            frame.render_widget(
                Paragraph::new(format!(
                    "Primary layout: {} (coupled pair)",
                    self.document
                        .profile()
                        .primary_buttons
                        .map_or_else(|| "<not present>".into(), |layout| layout.to_string())
                )),
                row,
            );
        }
        if let Some(row) = form_row(inner, after + 7, self.scroll) {
            self.button(
                frame,
                sub(row, 0, 13),
                "[Standard]",
                Action::Value(ProfileValueEdit::PrimaryButtons(Some(
                    PrimaryButtonLayout::Standard,
                ))),
                false,
            );
            self.button(
                frame,
                sub(row, 14, 13),
                "[Swapped]",
                Action::Value(ProfileValueEdit::PrimaryButtons(Some(
                    PrimaryButtonLayout::Swapped,
                ))),
                false,
            );
        }
    }
    pub(crate) fn render_lighting(&mut self, frame: &mut Frame, area: Rect) {
        frame.render_widget(
            Block::default()
                .borders(Borders::ALL)
                .title("Lighting - Solid colors in file; click to edit"),
            area,
        );
        let inner = area.inner(ratatui::layout::Margin {
            horizontal: 1,
            vertical: 1,
        });
        let lighting = self.document.profile().lighting.clone();
        self.scroll = self.scroll.min(8_u16.saturating_sub(inner.height));
        if let Some(row) = form_row(inner, 0, self.scroll) {
            frame.render_widget(
                Paragraph::new("Independent zones. Black #000000 means off. No hardware writes."),
                row,
            );
        }
        let zones = device_descriptor(&self.document.profile().device)
            .map_or(&[][..], |device| device.capabilities.lighting_zones);
        for (index, descriptor) in zones.iter().enumerate() {
            let zone = descriptor.id;
            if let Some(row) = form_row(inner, index as u16 * 2 + 2, self.scroll) {
                let color = lighting
                    .as_ref()
                    .and_then(|value| value.zones.get(zone))
                    .copied();
                if let Some(color) = color {
                    self.color_button(
                        frame,
                        row,
                        format!("[{zone}: {color}] "),
                        color,
                        Action::ZoneColor(zone),
                    );
                } else {
                    self.button(
                        frame,
                        row,
                        &format!("[{zone}: <not present / not read>]"),
                        Action::ZoneColor(zone),
                        false,
                    );
                }
            }
        }
        if lighting.is_none() {
            if let Some(row) = form_row(inner, 6, self.scroll) {
                frame.render_widget(
                    Paragraph::new(
                        "current colors and effects unknown; supply BOTH colors explicitly.",
                    ),
                    row,
                );
            }
        }
    }
}

pub fn sub(row: Rect, offset: u16, width: u16) -> Rect {
    Rect::new(
        row.x.saturating_add(offset.min(row.width)),
        row.y,
        width.min(row.width.saturating_sub(offset)),
        row.height,
    )
}
fn form_row(inner: Rect, logical: u16, scroll: u16) -> Option<Rect> {
    let offset = logical.checked_sub(scroll)?;
    (offset < inner.height).then_some(Rect::new(inner.x, inner.y + offset, inner.width, 1))
}
