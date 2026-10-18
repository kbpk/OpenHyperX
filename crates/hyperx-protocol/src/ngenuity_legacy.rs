//! Read-only parser for the NGENUITY Legacy `.hxp` preset format.
//!
//! Only fields confirmed from local NGENUITY Legacy 5.38.0.0 version-40
//! presets are decoded. Parsing and conversion never perform HID I/O. The
//! format used by the current NGENUITY application is intentionally treated
//! as separate and is not supported by this module.

use hyperx_core::{
    DpiStage, MacroDefinition, MacroEvent, MacroPlayback, NamedMacro, RgbColor, SoftwareDpiProfile,
    SoftwareProfile, SoftwareProfileSource, UnresolvedButtonAssignment,
};
use thiserror::Error;

use crate::pulsefire_raid::PulsefireRaidControl;

const HEADER_ALL: u32 = 0x7D65_83CA;
const HEADER_PRESET: u32 = 0x4D2C_83CE;
const HEADER_MOUSE_PRESET: u32 = 0x1A4A_0099;
const HEADER_KEY_ASSIGNMENT: u32 = 0x2CD8_54CB;
const HEADER_MACRO: u32 = 0x2545_C654;
const HEADER_MACRO_ITEM: u32 = 0x1B35_A31E;
const SUPPORTED_PRESET_VERSION: u32 = 40;
const ACTIVE_DPI_STAGE_OFFSET: usize = 23;
const POLLING_CODE_OFFSET: usize = 27;
const LIFT_OFF_CODE_OFFSET: usize = 28;
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
    /// Zero-based stage index stored by NGENUITY Legacy version 40.
    pub active_dpi_stage: u32,
    /// Raw Legacy value; this parser does not turn it into a portable setting.
    pub polling_rate_code: u8,
    /// Raw Legacy value with no confirmed Pulsefire Raid value mapping.
    pub lift_off_distance_code: u8,
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
    /// Raw Legacy KeyCode; a physical target is model-specific.
    pub source_key_code: u16,
    /// Raw Legacy AssignmentType; its payload is not imported as a binding.
    pub assignment_type_code: u8,
    /// Active scalar field for KeyboardFunction, MouseFunction, Multimedia or
    /// WindowsShortcut. This is a Legacy enum/code, not a Raid wire value.
    pub active_function_code: Option<u16>,
    pub macro_source_id: Option<[u8; 16]>,
    /// Active only for Macro assignments; distinct from the referenced macro
    /// definition's playback mode and not translated to hardware behavior.
    pub macro_playback_option_code: Option<u8>,
    pub macro_times: Option<u16>,
}

impl NgenuityKeyAssignment {
    /// Name from Legacy's version-40 `AssignmentType` enum, not proof that the
    /// corresponding binding value can be imported or sent to a mouse.
    pub const fn assignment_type_name(&self) -> Option<&'static str> {
        match self.assignment_type_code {
            0 => Some("Default"),
            1 => Some("KeyboardFunction"),
            2 => Some("MouseFunction"),
            3 => Some("Multimedia"),
            4 => Some("Macro"),
            5 => Some("WindowsShortcut"),
            6 => Some("Open"),
            7 => Some("Disabled"),
            8 => Some("FunctionLock"),
            9 => Some("FNKey"),
            _ => None,
        }
    }

    /// Legacy enum name only; it does not establish device support or make an
    /// assignment safe to import/write. Keyboard key codes remain raw.
    pub const fn active_function_name(&self) -> Option<&'static str> {
        match (self.assignment_type_code, self.active_function_code) {
            (2, Some(1)) => Some("Left"),
            (2, Some(2)) => Some("Right"),
            (2, Some(3)) => Some("Middle"),
            (2, Some(4)) => Some("Previous"),
            (2, Some(5)) => Some("Forward"),
            (2, Some(6)) => Some("TiltL"),
            (2, Some(7)) => Some("TiltR"),
            (2, Some(8)) => Some("DPI"),
            (2, Some(9)) => Some("ScrollUp"),
            (2, Some(10)) => Some("ScrollDown"),
            (3, Some(1)) => Some("VolumeUp"),
            (3, Some(2)) => Some("VolumeDown"),
            (3, Some(3)) => Some("MuteVolume"),
            (3, Some(8)) => Some("PlayPause"),
            (3, Some(9)) => Some("NextTrack"),
            (3, Some(10)) => Some("PreviousTrack"),
            (3, Some(11)) => Some("Stop"),
            (5, Some(4)) => Some("CycleApps"),
            (5, Some(5)) => Some("SwitchApps"),
            (5, Some(7)) => Some("Cut"),
            (5, Some(8)) => Some("Copy"),
            (5, Some(9)) => Some("Paste"),
            (5, Some(10)) => Some("Undo"),
            _ => None,
        }
    }

    /// Name of the assignment-level Legacy enum, not a verified Raid mode.
    pub const fn macro_playback_option_name(&self) -> Option<&'static str> {
        match self.macro_playback_option_code {
            Some(0) => Some("None"),
            Some(1) => Some("PlayOnce"),
            Some(2) => Some("PlayMultipleTimes"),
            Some(3) => Some("UseAssignedKey"),
            Some(4) => Some("AssignedKeyPressed"),
            _ => None,
        }
    }

    /// Interpret a Legacy key code through Pulsefire Raid's `InitProfile`
    /// switch. The `.hxp` container itself does not prove the device model.
    /// Raid's extra keys 0x20B and 0x20C have no confirmed physical target.
    pub const fn pulsefire_raid_control(&self) -> Option<PulsefireRaidControl> {
        match self.source_key_code {
            0x200 => Some(PulsefireRaidControl::LeftClick),
            0x201 => Some(PulsefireRaidControl::RightClick),
            0x202 => Some(PulsefireRaidControl::MiddleClick),
            0x203 => Some(PulsefireRaidControl::Button4),
            0x204 => Some(PulsefireRaidControl::Button5),
            0x205 => Some(PulsefireRaidControl::Button6),
            0x206 => Some(PulsefireRaidControl::Button7),
            0x207 => Some(PulsefireRaidControl::Button8),
            0x208 => Some(PulsefireRaidControl::Dpi),
            0x209 => Some(PulsefireRaidControl::WheelTiltLeft),
            0x20A => Some(PulsefireRaidControl::WheelTiltRight),
            _ => None,
        }
    }
}

#[derive(Debug, Error, Eq, PartialEq)]
pub enum NgenuityPresetError {
    #[error("NGENUITY Legacy preset is truncated while reading {context} at offset 0x{offset:X}")]
    Truncated {
        context: &'static str,
        offset: usize,
    },
    #[error("unknown NGENUITY Legacy preset header 0x{0:08X}")]
    UnknownHeader(u32),
    #[error("embedded preset length {declared} exceeds the {available} available bytes")]
    InvalidEmbeddedLength { declared: usize, available: usize },
    #[error("unsupported NGENUITY Legacy preset version {0}; only version 40 is decoded")]
    UnsupportedVersion(u32),
    #[error("NGENUITY Legacy string length uses an invalid 7-bit integer")]
    InvalidStringLength,
    #[error("NGENUITY Legacy preset contains invalid UTF-8 in {0}")]
    InvalidUtf8(&'static str),
    #[error("NGENUITY Legacy mouse preset block was not found")]
    MissingMousePreset,
    #[error("NGENUITY Legacy collection {name} contains an unreasonable item count {count}")]
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
    #[error("macro {name:?} uses unconfirmed NGENUITY Legacy playback mode {mode}")]
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

/// Return the embedded versioned preset bytes without the optional export
/// wrapper and footer.
pub fn ngenuity_legacy_embedded_payload(bytes: &[u8]) -> Result<&[u8], NgenuityPresetError> {
    select_payload(bytes).map(|(_, payload)| payload)
}

pub fn parse_ngenuity_legacy_preset(bytes: &[u8]) -> Result<NgenuityPreset, NgenuityPresetError> {
    let (container, payload) = select_payload(bytes)?;

    expect_header(payload, 0, HEADER_PRESET, "preset")?;
    let version = read_u32(payload, 4, "preset version")?;
    if version != SUPPORTED_PRESET_VERSION {
        return Err(NgenuityPresetError::UnsupportedVersion(version));
    }
    let (name, _) = read_string(payload, 16, "preset name")?;

    let mouse_offset =
        find_header(payload, HEADER_MOUSE_PRESET).ok_or(NgenuityPresetError::MissingMousePreset)?;
    // MousePreset.Deserialize reads three flags after the 16-byte GUID, then
    // DPIStage (Int32), polling (Byte), lift-off (Byte), and the DPI collection.
    // The Int32 after that collection is the effect count, not DPIStage.
    let active_dpi_stage = read_u32(
        payload,
        mouse_offset + ACTIVE_DPI_STAGE_OFFSET,
        "active DPI stage",
    )?;
    let polling_rate_code = read_u8(
        payload,
        mouse_offset + POLLING_CODE_OFFSET,
        "polling rate code",
    )?;
    let lift_off_distance_code = read_u8(
        payload,
        mouse_offset + LIFT_OFF_CODE_OFFSET,
        "lift-off distance code",
    )?;
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
    let macros = parse_macros(payload)?;
    let key_assignments = parse_key_assignments(payload, &macros)?;

    Ok(NgenuityPreset {
        container,
        embedded_length: payload.len(),
        version,
        name,
        dpi_stages,
        active_dpi_stage,
        polling_rate_code,
        lift_off_distance_code,
        macros,
        key_assignments,
    })
}

fn select_payload(bytes: &[u8]) -> Result<(NgenuityContainer, &[u8]), NgenuityPresetError> {
    let outer_header = read_u32(bytes, 0, "container header")?;
    match outer_header {
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
            Ok((NgenuityContainer::Export, &bytes[8..8 + length]))
        }
        HEADER_PRESET => Ok((NgenuityContainer::InternalPreset, bytes)),
        other => Err(NgenuityPresetError::UnknownHeader(other)),
    }
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
                source_id: format_ngenuity_legacy_id(&assignment.source_id),
                macro_source_id: assignment
                    .macro_source_id
                    .as_ref()
                    .map(format_ngenuity_legacy_id),
            })
            .collect();

        Ok(SoftwareProfile {
            name: self.name.clone(),
            device: device.into(),
            partial: true,
            source: Some(SoftwareProfileSource {
                format: "ngenuity-legacy-hxp".to_owned(),
                format_version: self.version,
            }),
            dpi: Some(dpi),
            macros,
            unresolved_button_assignments,
            ..SoftwareProfile::default()
        })
    }
}

pub fn format_ngenuity_legacy_id(id: &[u8; 16]) -> String {
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
        source_id: format_ngenuity_legacy_id(&source.source_id),
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
        // KeyAssignment.Deserialize reads KeyCode and AssignmentType directly
        // after the GUID; MacroID is exactly the 16 bytes at +27. Searching
        // the whole record could mistake an unrelated GUID for a macro link.
        let source_key_code = read_u16(record, 20, "key assignment source key")?;
        let assignment_type_code = read_u8(record, 22, "key assignment type")?;
        // KeyAssignment.Deserialize always reads all fields. Only the field
        // selected by AssignmentType is semantically active; stale values in
        // other fields must not appear as a current binding.
        let active_function_code = match assignment_type_code {
            1 => Some(read_u16(record, 23, "keyboard function")?),
            2 => Some(u16::from(read_u8(record, 25, "mouse function")?)),
            3 => Some(u16::from(read_u8(record, 26, "multimedia function")?)),
            5 => Some(u16::from(read_u8(record, 46, "Windows shortcut")?)),
            _ => None,
        };
        let macro_id = read_id(record, 27, "key assignment macro identifier")?;
        let macro_source_id = if assignment_type_code == 4 {
            macros
                .iter()
                .find(|source_macro| source_macro.source_id == macro_id)
                .map(|source_macro| source_macro.source_id)
        } else {
            None
        };
        let (macro_playback_option_code, macro_times) = if assignment_type_code == 4 {
            (
                Some(read_u8(record, 43, "macro assignment playback option")?),
                Some(read_u16(record, 44, "macro assignment play count")?),
            )
        } else {
            (None, None)
        };
        assignments.push(NgenuityKeyAssignment {
            source_id,
            source_key_code,
            assignment_type_code,
            active_function_code,
            macro_source_id,
            macro_playback_option_code,
            macro_times,
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
        let preset = parse_ngenuity_legacy_preset(&bytes).unwrap();

        assert_eq!(preset.container, NgenuityContainer::Export);
        assert_eq!(preset.version, 40);
        assert_eq!(preset.name, "Test preset");
        assert_eq!(preset.dpi_stages.len(), 1);
        assert_eq!(preset.dpi_stages[0].dpi, 800);
        assert_eq!(preset.dpi_stages[0].color, RgbColor::new(0x2B, 0, 0xFF));
        assert_eq!(preset.active_dpi_stage, 0);
        assert_eq!(preset.polling_rate_code, 0x01);
        assert_eq!(preset.lift_off_distance_code, 0x01);
        assert_eq!(preset.macros.len(), 1);
        assert_eq!(preset.macros[0].items.len(), 2);
        assert_eq!(preset.key_assignments[0].source_key_code, 0x203);
        assert_eq!(preset.key_assignments[0].assignment_type_code, 4);
        assert_eq!(
            preset.key_assignments[0].assignment_type_name(),
            Some("Macro")
        );
        assert_eq!(
            preset.key_assignments[0].pulsefire_raid_control(),
            Some(PulsefireRaidControl::Button4)
        );
        assert_eq!(preset.key_assignments[0].macro_source_id, Some(MACRO_ID));
        assert_eq!(preset.key_assignments[0].active_function_code, None);
        assert_eq!(
            preset.key_assignments[0].macro_playback_option_code,
            Some(1)
        );
        assert_eq!(
            preset.key_assignments[0].macro_playback_option_name(),
            Some("PlayOnce")
        );
        assert_eq!(preset.key_assignments[0].macro_times, Some(1));

        let imported = preset.to_software_profile("pulsefire-raid").unwrap();
        assert!(imported.partial);
        assert_eq!(
            imported
                .source
                .as_ref()
                .map(|source| source.format.as_str()),
            Some("ngenuity-legacy-hxp")
        );
        let dpi = imported.dpi.unwrap();
        assert_eq!(dpi.active_stage, None);
        assert_eq!(dpi.source_active_stage, Some(0));
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
        let preset = parse_ngenuity_legacy_preset(&bytes).unwrap();
        assert_eq!(preset.container, NgenuityContainer::InternalPreset);
        assert_eq!(preset.embedded_length, bytes.len());
        assert_eq!(ngenuity_legacy_embedded_payload(&bytes).unwrap(), bytes);
    }

    #[test]
    fn macro_reference_uses_only_the_deserialized_macro_id_field() {
        let mut bytes = fixture(false);
        let first = find_header(&bytes, HEADER_KEY_ASSIGNMENT).unwrap();
        bytes[first + 27..first + 43].fill(0);
        bytes[first + 50..first + 66].copy_from_slice(&MACRO_ID);

        let preset = parse_ngenuity_legacy_preset(&bytes).unwrap();
        assert_eq!(preset.key_assignments[0].macro_source_id, None);
        assert_eq!(
            preset.key_assignments[0].macro_playback_option_code,
            Some(1)
        );
        assert_eq!(preset.key_assignments[0].macro_times, Some(1));
    }

    #[test]
    fn inactive_macro_id_does_not_become_an_active_binding_reference() {
        let mut bytes = fixture(false);
        let first = find_header(&bytes, HEADER_KEY_ASSIGNMENT).unwrap();
        bytes[first + 22] = 2; // MouseFunction, with the old MacroID still stored.

        let preset = parse_ngenuity_legacy_preset(&bytes).unwrap();
        assert_eq!(
            preset.key_assignments[0].assignment_type_name(),
            Some("MouseFunction")
        );
        assert_eq!(preset.key_assignments[0].macro_source_id, None);
        assert_eq!(preset.key_assignments[0].macro_playback_option_code, None);
        assert_eq!(preset.key_assignments[0].macro_times, None);
    }

    #[test]
    fn assignment_value_comes_only_from_the_active_legacy_field() {
        for (assignment_type, expected_code, expected_name) in [
            (1, Some(0x0123), None),
            (2, Some(4), Some("Previous")),
            (3, Some(2), Some("VolumeDown")),
            (4, None, None),
            (5, Some(10), Some("Undo")),
            (7, None, None),
            (0xFF, None, None),
        ] {
            let mut bytes = fixture(false);
            let first = find_header(&bytes, HEADER_KEY_ASSIGNMENT).unwrap();
            bytes[first + 22] = assignment_type;
            bytes[first + 23..first + 25].copy_from_slice(&0x0123_u16.to_le_bytes());
            bytes[first + 25] = 4;
            bytes[first + 26] = 2;
            bytes[first + 43] = 2;
            bytes[first + 44..first + 46].copy_from_slice(&3_u16.to_le_bytes());
            bytes[first + 46] = 10;
            let preset = parse_ngenuity_legacy_preset(&bytes).unwrap();
            let assignment = &preset.key_assignments[0];
            assert_eq!(assignment.active_function_code, expected_code);
            assert_eq!(assignment.active_function_name(), expected_name);
            assert_eq!(assignment.macro_source_id.is_some(), assignment_type == 4);
            assert_eq!(
                assignment.macro_playback_option_code,
                (assignment_type == 4).then_some(2)
            );
            assert_eq!(
                assignment.macro_playback_option_name(),
                (assignment_type == 4).then_some("PlayMultipleTimes")
            );
            assert_eq!(assignment.macro_times, (assignment_type == 4).then_some(3));
        }
    }

    #[test]
    fn unknown_legacy_function_codes_stay_raw() {
        let mut bytes = fixture(false);
        let first = find_header(&bytes, HEADER_KEY_ASSIGNMENT).unwrap();
        bytes[first + 22] = 2;
        bytes[first + 25] = 0xFE;
        let preset = parse_ngenuity_legacy_preset(&bytes).unwrap();
        let assignment = &preset.key_assignments[0];
        assert_eq!(assignment.active_function_code, Some(0xFE));
        assert_eq!(assignment.active_function_name(), None);
    }

    #[test]
    fn unknown_macro_playback_option_stays_raw_and_unimported() {
        let mut bytes = fixture(false);
        let first = find_header(&bytes, HEADER_KEY_ASSIGNMENT).unwrap();
        bytes[first + 43] = 0xFE;
        let preset = parse_ngenuity_legacy_preset(&bytes).unwrap();
        let assignment = &preset.key_assignments[0];
        assert_eq!(assignment.macro_playback_option_code, Some(0xFE));
        assert_eq!(assignment.macro_playback_option_name(), None);
        assert!(
            preset
                .to_software_profile("pulsefire-raid")
                .unwrap()
                .partial
        );
    }

    #[test]
    fn raid_key_codes_map_only_to_the_eleven_known_controls() {
        for (code, control) in [
            (0x200, PulsefireRaidControl::LeftClick),
            (0x201, PulsefireRaidControl::RightClick),
            (0x202, PulsefireRaidControl::MiddleClick),
            (0x203, PulsefireRaidControl::Button4),
            (0x204, PulsefireRaidControl::Button5),
            (0x205, PulsefireRaidControl::Button6),
            (0x206, PulsefireRaidControl::Button7),
            (0x207, PulsefireRaidControl::Button8),
            (0x208, PulsefireRaidControl::Dpi),
            (0x209, PulsefireRaidControl::WheelTiltLeft),
            (0x20A, PulsefireRaidControl::WheelTiltRight),
        ] {
            let assignment = NgenuityKeyAssignment {
                source_id: [0; 16],
                source_key_code: code,
                assignment_type_code: 0,
                active_function_code: None,
                macro_source_id: None,
                macro_playback_option_code: None,
                macro_times: None,
            };
            assert_eq!(assignment.pulsefire_raid_control(), Some(control));
        }
        for code in [0x000, 0x20B, 0x20C, 0xFFFF] {
            let assignment = NgenuityKeyAssignment {
                source_id: [0; 16],
                source_key_code: code,
                assignment_type_code: 0,
                active_function_code: None,
                macro_source_id: None,
                macro_playback_option_code: None,
                macro_times: None,
            };
            assert_eq!(assignment.pulsefire_raid_control(), None);
        }
    }

    #[test]
    fn extracts_export_payload_without_footer_or_wrapper() {
        let wrapped = fixture(true);
        let internal = fixture(false);
        assert_eq!(
            ngenuity_legacy_embedded_payload(&wrapped).unwrap(),
            internal
        );
    }

    #[test]
    fn recorded_timing_and_mouse_actions_are_preserved() {
        let mut bytes = fixture(true);
        let preset = parse_ngenuity_legacy_preset(&bytes).unwrap();
        let first = find_header(&bytes, HEADER_MACRO).unwrap();
        let name_end = first + 20 + 1 + "Test macro".len();
        bytes[name_end] = 0;
        let first_item = name_end + 15;
        bytes[first_item + 32..first_item + 36].copy_from_slice(&953_u32.to_le_bytes());
        bytes[first_item + 20..first_item + 24].copy_from_slice(&2_u32.to_le_bytes());
        bytes[first_item + 24] = 1;
        bytes[first_item + 25] = 2;

        let changed = parse_ngenuity_legacy_preset(&bytes).unwrap();
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
            parse_ngenuity_legacy_preset(&unknown),
            Err(NgenuityPresetError::UnsupportedVersion(41))
        );

        let truncated = HEADER_ALL.to_le_bytes();
        assert!(matches!(
            parse_ngenuity_legacy_preset(&truncated),
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
        // Three flags, zero-based active stage 0, polling code 1, lift-off 1.
        // The effect-count Int32 after the DPI record is deliberately 1 so a
        // regression back to that old offset fails the assertions above.
        inner.extend_from_slice(&[0; 7]);
        inner.extend_from_slice(&[0x01, 0x01]);
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
        assignment[20..22].copy_from_slice(&0x203_u16.to_le_bytes());
        assignment[22] = 4;
        assignment[27..43].copy_from_slice(&MACRO_ID);
        assignment[43] = 1;
        assignment[44..46].copy_from_slice(&1_u16.to_le_bytes());
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
