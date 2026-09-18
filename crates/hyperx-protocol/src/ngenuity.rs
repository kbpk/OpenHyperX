//! Read-only parser for the NGENUITY `.hxp` preset format.
//!
//! Only fields confirmed from local NGENUITY 5.38.0.0 version-40 presets are
//! decoded. Parsing and conversion never perform HID I/O.

use hyperx_core::{
    DpiStage, MacroDefinition, MacroEvent, MacroPlayback, NamedMacro, RgbColor, SoftwareDpiProfile,
    SoftwareProfile, SoftwareProfileSource, UnresolvedButtonAssignment,
};
use thiserror::Error;

const HEADER_ALL: u32 = 0x7D65_83CA;
const HEADER_PRESET: u32 = 0x4D2C_83CE;
const HEADER_MOUSE_PRESET: u32 = 0x1A4A_0099;
const HEADER_KEY_ASSIGNMENT: u32 = 0x2CD8_54CB;
const HEADER_MACRO: u32 = 0x2545_C654;
const HEADER_MACRO_ITEM: u32 = 0x1B35_A31E;
const SUPPORTED_PRESET_VERSION: u32 = 40;
const DPI_COUNT_OFFSET: usize = 29;
const DPI_RECORD_OFFSET: usize = 33;
const DPI_RECORD_LENGTH: usize = 64;
const KEY_ASSIGNMENT_LENGTH: usize = 93;
const MACRO_ITEM_LENGTH: usize = 38;
const MAX_COLLECTION_ITEMS: usize = 1_024;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NgenuityContainer {
    Export,
    InternalPreset,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NgenuityPreset {
    pub container: NgenuityContainer,
    pub embedded_length: usize,
    pub version: u32,
    pub name: String,
    pub dpi_stages: Vec<NgenuityDpiStage>,
    /// The exact one-based value stored by NGENUITY version 40.
    pub active_dpi_stage: u32,
    pub macros: Vec<NgenuityMacro>,
    pub key_assignments: Vec<NgenuityKeyAssignment>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NgenuityDpiStage {
    pub source_id: [u8; 16],
    pub dpi: u32,
    pub alpha: u8,
    pub color: RgbColor,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NgenuityMacro {
    pub source_id: [u8; 16],
    pub name: String,
    pub use_standard_timing: bool,
    pub standard_timing_ms: u32,
    pub expanded: bool,
    pub playback_mode: u32,
    pub play_times: u8,
    pub items: Vec<NgenuityMacroItem>,
}

impl NgenuityMacro {
    pub fn effective_timing_ms(&self, item: &NgenuityMacroItem) -> u32 {
        if self.use_standard_timing {
            self.standard_timing_ms
        } else {
            item.timing_ms
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NgenuityMacroItem {
    pub source_id: [u8; 16],
    pub item_type: u32,
    pub action: u8,
    pub state: u8,
    pub data: u32,
    pub timing_ms: u32,
    pub key_data: u16,
}

impl NgenuityMacroItem {
    pub fn decoded_action(&self) -> Option<NgenuityInputAction> {
        match (self.item_type, self.action, self.state) {
            (1, 0, 1) => Some(NgenuityInputAction::Keyboard {
                usage: self.key_data,
                pressed: true,
            }),
            (1, 1, 1) => Some(NgenuityInputAction::Keyboard {
                usage: self.key_data,
                pressed: false,
            }),
            (2, 1, 2) => Some(NgenuityInputAction::MouseButton {
                button: NgenuityMouseButton::Left,
                pressed: true,
            }),
            (2, 2, 2) => Some(NgenuityInputAction::MouseButton {
                button: NgenuityMouseButton::Left,
                pressed: false,
            }),
            (2, 4, 2) => Some(NgenuityInputAction::MouseButton {
                button: NgenuityMouseButton::Right,
                pressed: true,
            }),
            (2, 5, 2) => Some(NgenuityInputAction::MouseButton {
                button: NgenuityMouseButton::Right,
                pressed: false,
            }),
            (2, 7, 2) => Some(NgenuityInputAction::MouseButton {
                button: NgenuityMouseButton::Middle,
                pressed: true,
            }),
            (2, 8, 2) => Some(NgenuityInputAction::MouseButton {
                button: NgenuityMouseButton::Middle,
                pressed: false,
            }),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NgenuityInputAction {
    Keyboard {
        usage: u16,
        pressed: bool,
    },
    MouseButton {
        button: NgenuityMouseButton,
        pressed: bool,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NgenuityMouseButton {
    Left,
    Right,
    Middle,
}

impl NgenuityMouseButton {
    pub const fn name(self) -> &'static str {
        match self {
            Self::Left => "left",
            Self::Right => "right",
            Self::Middle => "middle",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NgenuityKeyAssignment {
    pub source_id: [u8; 16],
    pub macro_source_id: Option<[u8; 16]>,
}

#[derive(Debug, Error, Eq, PartialEq)]
pub enum NgenuityPresetError {
    #[error("NGENUITY preset is truncated while reading {context} at offset 0x{offset:X}")]
    Truncated {
        context: &'static str,
        offset: usize,
    },
    #[error("unknown NGENUITY preset header 0x{0:08X}")]
    UnknownHeader(u32),
    #[error("embedded preset length {declared} exceeds the {available} available bytes")]
    InvalidEmbeddedLength { declared: usize, available: usize },
    #[error("unsupported NGENUITY preset version {0}; only version 40 is decoded")]
    UnsupportedVersion(u32),
    #[error("NGENUITY string length uses an invalid 7-bit integer")]
    InvalidStringLength,
    #[error("NGENUITY preset contains invalid UTF-8 in {0}")]
    InvalidUtf8(&'static str),
    #[error("NGENUITY mouse preset block was not found")]
    MissingMousePreset,
    #[error("NGENUITY collection {name} contains an unreasonable item count {count}")]
    InvalidItemCount { name: &'static str, count: usize },
    #[error("expected {name} header 0x{expected:08X} at offset 0x{offset:X}")]
    WrongObjectHeader {
        name: &'static str,
        expected: u32,
        offset: usize,
    },
}

#[derive(Debug, Error, Eq, PartialEq)]
pub enum NgenuityImportError {
    #[error("macro {name:?} uses unconfirmed NGENUITY playback mode {mode}")]
    UnsupportedPlayback { name: String, mode: u32 },
    #[error("macro {name:?} event {event} has unsupported type/action/state {item_type}/{action}/{state}")]
    UnsupportedMacroItem {
        name: String,
        event: usize,
        item_type: u32,
        action: u8,
        state: u8,
    },
    #[error("macro {name:?} event {event} uses unknown keyboard usage 0x{usage:04X}")]
    UnknownKeyboardUsage {
        name: String,
        event: usize,
        usage: u16,
    },
    #[error(
        "macro {name:?} event {event} timing {timing_ms} ms exceeds the portable 16-bit range"
    )]
    TimingOutOfRange {
        name: String,
        event: usize,
        timing_ms: u32,
    },
}

pub fn parse_ngenuity_preset(bytes: &[u8]) -> Result<NgenuityPreset, NgenuityPresetError> {
    let outer_header = read_u32(bytes, 0, "container header")?;
    let (container, payload) = match outer_header {
        HEADER_ALL => {
            let length = usize::try_from(read_u32(bytes, 4, "embedded preset length")?)
                .expect("u32 always fits usize on supported targets");
            let available = bytes.len().saturating_sub(8);
            if length > available {
                return Err(NgenuityPresetError::InvalidEmbeddedLength {
                    declared: length,
                    available,
                });
            }
            (NgenuityContainer::Export, &bytes[8..8 + length])
        }
        HEADER_PRESET => (NgenuityContainer::InternalPreset, bytes),
        other => return Err(NgenuityPresetError::UnknownHeader(other)),
    };

    expect_header(payload, 0, HEADER_PRESET, "preset")?;
    let version = read_u32(payload, 4, "preset version")?;
    if version != SUPPORTED_PRESET_VERSION {
        return Err(NgenuityPresetError::UnsupportedVersion(version));
    }
    let (name, _) = read_string(payload, 16, "preset name")?;

    let mouse_offset =
        find_header(payload, HEADER_MOUSE_PRESET).ok_or(NgenuityPresetError::MissingMousePreset)?;
    let dpi_count = usize::try_from(read_u32(
        payload,
        mouse_offset + DPI_COUNT_OFFSET,
        "DPI stage count",
    )?)
    .expect("u32 always fits usize on supported targets");
    validate_count("DPI stages", dpi_count)?;
    let mut dpi_stages = Vec::with_capacity(dpi_count);
    for index in 0..dpi_count {
        let offset = mouse_offset + DPI_RECORD_OFFSET + index * DPI_RECORD_LENGTH;
        let source_id = read_id(payload, offset, "DPI stage identifier")?;
        let dpi = read_u32(payload, offset + 16, "DPI value")?;
        let color = read_bytes(payload, offset + 36, 4, "DPI stage color")?;
        dpi_stages.push(NgenuityDpiStage {
            source_id,
            dpi,
            alpha: color[0],
            color: RgbColor::new(color[1], color[2], color[3]),
        });
    }
    let active_offset = mouse_offset + DPI_RECORD_OFFSET + dpi_count * DPI_RECORD_LENGTH;
    let active_dpi_stage = read_u32(payload, active_offset, "active DPI stage")?;

    let macros = parse_macros(payload)?;
    let key_assignments = parse_key_assignments(payload, &macros)?;

    Ok(NgenuityPreset {
        container,
        embedded_length: payload.len(),
        version,
        name,
        dpi_stages,
        active_dpi_stage,
        macros,
        key_assignments,
    })
}

impl NgenuityPreset {
    /// Convert every currently understood field to a portable software profile.
    ///
    /// The device slug is supplied by the caller because no decoded version-40
    /// field proves the physical model. Unknown button targets remain explicit
    /// unresolved source assignments.
    pub fn to_software_profile(
        &self,
        device: impl Into<String>,
    ) -> Result<SoftwareProfile, NgenuityImportError> {
        let dpi = SoftwareDpiProfile {
            stages: self
                .dpi_stages
                .iter()
                .map(|stage| DpiStage::new(stage.dpi, stage.dpi, stage.color))
                .collect(),
            active_stage: None,
            source_active_stage: Some(self.active_dpi_stage),
        };
        let macros = self
            .macros
            .iter()
            .map(import_macro)
            .collect::<Result<Vec<_>, _>>()?;
        let unresolved_button_assignments = self
            .key_assignments
            .iter()
            .map(|assignment| UnresolvedButtonAssignment {
                source_id: format_ngenuity_id(&assignment.source_id),
                macro_source_id: assignment.macro_source_id.as_ref().map(format_ngenuity_id),
            })
            .collect();

        Ok(SoftwareProfile {
            name: self.name.clone(),
            device: device.into(),
            partial: true,
            source: Some(SoftwareProfileSource {
                format: "ngenuity-hxp".to_owned(),
                format_version: self.version,
            }),
            dpi: Some(dpi),
            macros,
            unresolved_button_assignments,
        })
    }
}

pub fn format_ngenuity_id(id: &[u8; 16]) -> String {
    let mut output = String::with_capacity(32);
    for byte in id {
        use std::fmt::Write;
        write!(&mut output, "{byte:02x}").expect("writing to a String cannot fail");
    }
    output
}

pub fn keyboard_usage_name(usage: u16) -> Option<String> {
    let name = match usage {
        0x04..=0x1D => {
            return char::from_u32(u32::from(b'a') + u32::from(usage - 0x04)).map(|c| c.to_string())
        }
        0x1E..=0x26 => return Some((usage - 0x1E + 1).to_string()),
        0x27 => return Some("0".to_owned()),
        0x28 => "enter",
        0x29 => "escape",
        0x2A => "backspace",
        0x2B => "tab",
        0x2C => "space",
        0x2D => "minus",
        0x2E => "equal",
        0x2F => "left-bracket",
        0x30 => "right-bracket",
        0x31 => "backslash",
        0x32 => "non-us-hash",
        0x33 => "semicolon",
        0x34 => "apostrophe",
        0x35 => "grave",
        0x36 => "comma",
        0x37 => "period",
        0x38 => "slash",
        0x39 => "caps-lock",
        0x3A..=0x45 => return Some(format!("f{}", usage - 0x3A + 1)),
        0x46 => "print-screen",
        0x47 => "scroll-lock",
        0x48 => "pause",
        0x49 => "insert",
        0x4A => "home",
        0x4B => "page-up",
        0x4C => "delete",
        0x4D => "end",
        0x4E => "page-down",
        0x4F => "arrow-right",
        0x50 => "arrow-left",
        0x51 => "arrow-down",
        0x52 => "arrow-up",
        0x53 => "num-lock",
        0x54 => "keypad-divide",
        0x55 => "keypad-multiply",
        0x56 => "keypad-subtract",
        0x57 => "keypad-add",
        0x58 => "keypad-enter",
        0x59..=0x61 => return Some(format!("keypad-{}", usage - 0x59 + 1)),
        0x62 => "keypad-0",
        0x63 => "keypad-decimal",
        0x64 => "non-us-backslash",
        0x65 => "application",
        0x66 => "power",
        0x67 => "keypad-equal",
        0x68..=0x73 => return Some(format!("f{}", usage - 0x68 + 13)),
        0xE0 => "left-control",
        0xE1 => "left-shift",
        0xE2 => "left-alt",
        0xE3 => "left-windows",
        0xE4 => "right-control",
        0xE5 => "right-shift",
        0xE6 => "right-alt",
        0xE7 => "right-windows",
        _ => return None,
    };
    Some(name.to_owned())
}

fn import_macro(source: &NgenuityMacro) -> Result<NamedMacro, NgenuityImportError> {
    if source.playback_mode != 1 {
        return Err(NgenuityImportError::UnsupportedPlayback {
            name: source.name.clone(),
            mode: source.playback_mode,
        });
    }
    let events = source
        .items
        .iter()
        .enumerate()
        .map(|(index, item)| import_macro_item(source, item, index))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(NamedMacro {
        source_id: format_ngenuity_id(&source.source_id),
        name: source.name.clone(),
        definition: MacroDefinition {
            playback: MacroPlayback::Once,
            events,
        },
    })
}

fn import_macro_item(
    source: &NgenuityMacro,
    item: &NgenuityMacroItem,
    index: usize,
) -> Result<MacroEvent, NgenuityImportError> {
    let event = index + 1;
    let timing_ms = source.effective_timing_ms(item);
    let delay_ms = u16::try_from(timing_ms).map_err(|_| NgenuityImportError::TimingOutOfRange {
        name: source.name.clone(),
        event,
        timing_ms,
    })?;
    match item.decoded_action() {
        Some(NgenuityInputAction::Keyboard { usage, pressed }) => {
            let key = keyboard_usage_name(usage).ok_or_else(|| {
                NgenuityImportError::UnknownKeyboardUsage {
                    name: source.name.clone(),
                    event,
                    usage,
                }
            })?;
            Ok(if pressed {
                MacroEvent::KeyDown { key, delay_ms }
            } else {
                MacroEvent::KeyUp { key, delay_ms }
            })
        }
        Some(NgenuityInputAction::MouseButton { button, pressed }) => {
            let button = button.name().to_owned();
            Ok(if pressed {
                MacroEvent::MouseButtonDown { button, delay_ms }
            } else {
                MacroEvent::MouseButtonUp { button, delay_ms }
            })
        }
        None => Err(NgenuityImportError::UnsupportedMacroItem {
            name: source.name.clone(),
            event,
            item_type: item.item_type,
            action: item.action,
            state: item.state,
        }),
    }
}

fn parse_key_assignments(
    payload: &[u8],
    macros: &[NgenuityMacro],
) -> Result<Vec<NgenuityKeyAssignment>, NgenuityPresetError> {
    let Some(first) = find_header(payload, HEADER_KEY_ASSIGNMENT) else {
        return Ok(Vec::new());
    };
    let count_offset = first.checked_sub(4).ok_or(NgenuityPresetError::Truncated {
        context: "key assignment count",
        offset: first,
    })?;
    let count = usize::try_from(read_u32(payload, count_offset, "key assignment count")?)
        .expect("u32 always fits usize on supported targets");
    validate_count("key assignments", count)?;
    let mut assignments = Vec::with_capacity(count);
    for index in 0..count {
        let offset = first + index * KEY_ASSIGNMENT_LENGTH;
        expect_header(payload, offset, HEADER_KEY_ASSIGNMENT, "key assignment")?;
        let record = read_bytes(payload, offset, KEY_ASSIGNMENT_LENGTH, "key assignment")?;
        let source_id = read_id(payload, offset + 4, "key assignment identifier")?;
        let macro_source_id = macros.iter().find_map(|source_macro| {
            record
                .windows(source_macro.source_id.len())
                .any(|window| window == source_macro.source_id)
                .then_some(source_macro.source_id)
        });
        assignments.push(NgenuityKeyAssignment {
            source_id,
            macro_source_id,
        });
    }
    Ok(assignments)
}

fn parse_macros(payload: &[u8]) -> Result<Vec<NgenuityMacro>, NgenuityPresetError> {
    let Some(first) = find_header(payload, HEADER_MACRO) else {
        return Ok(Vec::new());
    };
    let count_offset = first.checked_sub(4).ok_or(NgenuityPresetError::Truncated {
        context: "macro count",
        offset: first,
    })?;
    let count = usize::try_from(read_u32(payload, count_offset, "macro count")?)
        .expect("u32 always fits usize on supported targets");
    validate_count("macros", count)?;
    let mut macros = Vec::with_capacity(count);
    let mut offset = first;
    for _ in 0..count {
        expect_header(payload, offset, HEADER_MACRO, "macro")?;
        let source_id = read_id(payload, offset + 4, "macro identifier")?;
        let (name, mut cursor) = read_string(payload, offset + 20, "macro name")?;
        let use_standard_timing = read_u8(payload, cursor, "standard timing flag")? != 0;
        cursor += 1;
        let standard_timing_ms = read_u32(payload, cursor, "standard timing")?;
        cursor += 4;
        let expanded = read_u8(payload, cursor, "macro expanded flag")? != 0;
        cursor += 1;
        let playback_mode = read_u32(payload, cursor, "macro playback mode")?;
        cursor += 4;
        let play_times = read_u8(payload, cursor, "macro play count")?;
        cursor += 1;
        let item_count = usize::try_from(read_u32(payload, cursor, "macro item count")?)
            .expect("u32 always fits usize on supported targets");
        cursor += 4;
        validate_count("macro items", item_count)?;
        let mut items = Vec::with_capacity(item_count);
        for _ in 0..item_count {
            expect_header(payload, cursor, HEADER_MACRO_ITEM, "macro item")?;
            items.push(NgenuityMacroItem {
                source_id: read_id(payload, cursor + 4, "macro item identifier")?,
                item_type: read_u32(payload, cursor + 20, "macro item type")?,
                action: read_u8(payload, cursor + 24, "macro item action")?,
                state: read_u8(payload, cursor + 25, "macro item state")?,
                data: read_u32(payload, cursor + 28, "macro item data")?,
                timing_ms: read_u32(payload, cursor + 32, "macro item timing")?,
                key_data: read_u16(payload, cursor + 36, "macro item key data")?,
            });
            cursor += MACRO_ITEM_LENGTH;
        }
        macros.push(NgenuityMacro {
            source_id,
            name,
            use_standard_timing,
            standard_timing_ms,
            expanded,
            playback_mode,
            play_times,
            items,
        });
        offset = cursor;
    }
    Ok(macros)
}

fn validate_count(name: &'static str, count: usize) -> Result<(), NgenuityPresetError> {
    if count > MAX_COLLECTION_ITEMS {
        Err(NgenuityPresetError::InvalidItemCount { name, count })
    } else {
        Ok(())
    }
}

fn find_header(bytes: &[u8], header: u32) -> Option<usize> {
    let needle = header.to_le_bytes();
    bytes
        .windows(needle.len())
        .position(|window| window == needle)
}

fn expect_header(
    bytes: &[u8],
    offset: usize,
    expected: u32,
    name: &'static str,
) -> Result<(), NgenuityPresetError> {
    let actual = read_u32(bytes, offset, name)?;
    if actual == expected {
        Ok(())
    } else {
        Err(NgenuityPresetError::WrongObjectHeader {
            name,
            expected,
            offset,
        })
    }
}

fn read_string(
    bytes: &[u8],
    offset: usize,
    context: &'static str,
) -> Result<(String, usize), NgenuityPresetError> {
    let (length, cursor) = read_7bit_length(bytes, offset)?;
    let value = read_bytes(bytes, cursor, length, context)?;
    let value = std::str::from_utf8(value)
        .map_err(|_| NgenuityPresetError::InvalidUtf8(context))?
        .to_owned();
    Ok((value, cursor + length))
}

fn read_7bit_length(
    bytes: &[u8],
    mut offset: usize,
) -> Result<(usize, usize), NgenuityPresetError> {
    let mut value = 0_usize;
    for shift in (0..35).step_by(7) {
        let byte = read_u8(bytes, offset, "7-bit string length")?;
        offset += 1;
        value |= usize::from(byte & 0x7F) << shift;
        if byte & 0x80 == 0 {
            return Ok((value, offset));
        }
    }
    Err(NgenuityPresetError::InvalidStringLength)
}

fn read_id(
    bytes: &[u8],
    offset: usize,
    context: &'static str,
) -> Result<[u8; 16], NgenuityPresetError> {
    read_bytes(bytes, offset, 16, context)?
        .try_into()
        .map_err(|_| NgenuityPresetError::Truncated { context, offset })
}

fn read_u8(bytes: &[u8], offset: usize, context: &'static str) -> Result<u8, NgenuityPresetError> {
    bytes
        .get(offset)
        .copied()
        .ok_or(NgenuityPresetError::Truncated { context, offset })
}

fn read_u16(
    bytes: &[u8],
    offset: usize,
    context: &'static str,
) -> Result<u16, NgenuityPresetError> {
    let value: [u8; 2] = read_bytes(bytes, offset, 2, context)?
        .try_into()
        .expect("validated two-byte slice");
    Ok(u16::from_le_bytes(value))
}

fn read_u32(
    bytes: &[u8],
    offset: usize,
    context: &'static str,
) -> Result<u32, NgenuityPresetError> {
    let value: [u8; 4] = read_bytes(bytes, offset, 4, context)?
        .try_into()
        .expect("validated four-byte slice");
    Ok(u32::from_le_bytes(value))
}

fn read_bytes<'a>(
    bytes: &'a [u8],
    offset: usize,
    length: usize,
    context: &'static str,
) -> Result<&'a [u8], NgenuityPresetError> {
    bytes
        .get(offset..offset.saturating_add(length))
        .ok_or(NgenuityPresetError::Truncated { context, offset })
}

#[cfg(test)]
mod tests {
    use super::*;

    const MACRO_ID: [u8; 16] = [0x22; 16];

    #[test]
    fn parses_export_and_imports_confirmed_fields() {
        let bytes = fixture(true);
        let preset = parse_ngenuity_preset(&bytes).unwrap();

        assert_eq!(preset.container, NgenuityContainer::Export);
        assert_eq!(preset.version, 40);
        assert_eq!(preset.name, "Test preset");
        assert_eq!(preset.dpi_stages.len(), 1);
        assert_eq!(preset.dpi_stages[0].dpi, 800);
        assert_eq!(preset.dpi_stages[0].color, RgbColor::new(0x2B, 0, 0xFF));
        assert_eq!(preset.active_dpi_stage, 1);
        assert_eq!(preset.macros.len(), 1);
        assert_eq!(preset.macros[0].items.len(), 2);
        assert_eq!(preset.key_assignments[0].macro_source_id, Some(MACRO_ID));

        let imported = preset.to_software_profile("pulsefire-raid").unwrap();
        assert!(imported.partial);
        let dpi = imported.dpi.unwrap();
        assert_eq!(dpi.active_stage, None);
        assert_eq!(dpi.source_active_stage, Some(1));
        assert_eq!(imported.macros[0].definition.playback, MacroPlayback::Once);
        assert_eq!(
            imported.macros[0].definition.events,
            vec![
                MacroEvent::KeyDown {
                    key: "a".to_owned(),
                    delay_ms: 300,
                },
                MacroEvent::KeyUp {
                    key: "a".to_owned(),
                    delay_ms: 300,
                },
            ]
        );
    }

    #[test]
    fn parses_internal_preset_without_export_wrapper() {
        let bytes = fixture(false);
        let preset = parse_ngenuity_preset(&bytes).unwrap();
        assert_eq!(preset.container, NgenuityContainer::InternalPreset);
        assert_eq!(preset.embedded_length, bytes.len());
    }

    #[test]
    fn recorded_timing_and_mouse_actions_are_preserved() {
        let mut bytes = fixture(true);
        let preset = parse_ngenuity_preset(&bytes).unwrap();
        let first = find_header(&bytes, HEADER_MACRO).unwrap();
        let name_end = first + 20 + 1 + "Test macro".len();
        bytes[name_end] = 0;
        let first_item = name_end + 15;
        bytes[first_item + 32..first_item + 36].copy_from_slice(&953_u32.to_le_bytes());
        bytes[first_item + 20..first_item + 24].copy_from_slice(&2_u32.to_le_bytes());
        bytes[first_item + 24] = 1;
        bytes[first_item + 25] = 2;

        let changed = parse_ngenuity_preset(&bytes).unwrap();
        assert_eq!(
            preset.macros[0].effective_timing_ms(&preset.macros[0].items[0]),
            300
        );
        assert_eq!(
            changed.macros[0].effective_timing_ms(&changed.macros[0].items[0]),
            953
        );
        assert_eq!(
            changed.macros[0].items[0].decoded_action(),
            Some(NgenuityInputAction::MouseButton {
                button: NgenuityMouseButton::Left,
                pressed: true,
            })
        );
    }

    #[test]
    fn rejects_unknown_version_and_truncation() {
        let mut unknown = fixture(false);
        unknown[4..8].copy_from_slice(&41_u32.to_le_bytes());
        assert_eq!(
            parse_ngenuity_preset(&unknown),
            Err(NgenuityPresetError::UnsupportedVersion(41))
        );

        let truncated = HEADER_ALL.to_le_bytes();
        assert!(matches!(
            parse_ngenuity_preset(&truncated),
            Err(NgenuityPresetError::Truncated { .. })
        ));
    }

    #[test]
    fn canonical_keyboard_names_round_trip_through_core_parser() {
        for usage in [
            0x04, 0x1D, 0x27, 0x3A, 0x45, 0x59, 0x62, 0x68, 0x73, 0xE0, 0xE7,
        ] {
            let name = keyboard_usage_name(usage).unwrap();
            assert_eq!(name.parse(), Ok(hyperx_core::KeyboardUsage(usage)));
        }
        assert_eq!(keyboard_usage_name(0xFFFF), None);
    }

    fn fixture(export: bool) -> Vec<u8> {
        let mut inner = Vec::new();
        push_u32(&mut inner, HEADER_PRESET);
        push_u32(&mut inner, SUPPORTED_PRESET_VERSION);
        inner.extend_from_slice(&[0; 8]);
        push_string(&mut inner, "Test preset");
        inner.extend_from_slice(&[0xAA; 16]);

        push_u32(&mut inner, HEADER_MOUSE_PRESET);
        inner.extend_from_slice(&[0x10; 16]);
        inner.extend_from_slice(&[0; 9]);
        push_u32(&mut inner, 1);
        let mut dpi = [0_u8; DPI_RECORD_LENGTH];
        dpi[..16].fill(0x11);
        dpi[16..20].copy_from_slice(&800_u32.to_le_bytes());
        dpi[36..40].copy_from_slice(&[0xFF, 0x2B, 0, 0xFF]);
        inner.extend_from_slice(&dpi);
        push_u32(&mut inner, 1);

        push_u32(&mut inner, 1);
        let mut assignment = [0_u8; KEY_ASSIGNMENT_LENGTH];
        assignment[..4].copy_from_slice(&HEADER_KEY_ASSIGNMENT.to_le_bytes());
        assignment[4..20].fill(0x33);
        assignment[27..43].copy_from_slice(&MACRO_ID);
        inner.extend_from_slice(&assignment);

        push_u32(&mut inner, 1);
        push_u32(&mut inner, HEADER_MACRO);
        inner.extend_from_slice(&MACRO_ID);
        push_string(&mut inner, "Test macro");
        inner.push(1);
        push_u32(&mut inner, 300);
        inner.push(0);
        push_u32(&mut inner, 1);
        inner.push(1);
        push_u32(&mut inner, 2);
        push_macro_item(&mut inner, 0, 20, 0x04, 0x44);
        push_macro_item(&mut inner, 1, 78, 0x04, 0x55);

        if !export {
            return inner;
        }
        let mut outer = Vec::new();
        push_u32(&mut outer, HEADER_ALL);
        push_u32(&mut outer, inner.len() as u32);
        outer.extend_from_slice(&inner);
        outer
    }

    fn push_macro_item(output: &mut Vec<u8>, action: u8, timing_ms: u32, key_data: u16, id: u8) {
        push_u32(output, HEADER_MACRO_ITEM);
        output.extend_from_slice(&[id; 16]);
        push_u32(output, 1);
        output.push(action);
        output.push(1);
        output.extend_from_slice(&[0; 2]);
        push_u32(output, 0);
        push_u32(output, timing_ms);
        output.extend_from_slice(&key_data.to_le_bytes());
    }

    fn push_string(output: &mut Vec<u8>, value: &str) {
        assert!(value.len() < 0x80);
        output.push(value.len() as u8);
        output.extend_from_slice(value.as_bytes());
    }

    fn push_u32(output: &mut Vec<u8>, value: u32) {
        output.extend_from_slice(&value.to_le_bytes());
    }
}
