use serde::{Deserialize, Serialize};
use std::{fmt, str::FromStr};

/// A semantic action assigned to a programmable device button.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ButtonBinding {
    Mouse(MouseFunction),
    Keyboard(KeyboardUsage),
    Multimedia(MultimediaFunction),
    Macro(MacroBinding),
    WindowsShortcut(WindowsShortcut),
    Disabled,
}

/// Mouse actions exposed by NGENUITY Legacy for Pulsefire Raid buttons.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MouseFunction {
    LeftClick,
    RightClick,
    MiddleClick,
    Back,
    Forward,
    TiltLeft,
    TiltRight,
    DpiToggle,
    ScrollUp,
    ScrollDown,
}

/// Consumer-control actions exposed by NGENUITY Legacy.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MultimediaFunction {
    PlayPause,
    Stop,
    NextTrack,
    PreviousTrack,
    MuteVolume,
    VolumeUp,
    VolumeDown,
}

/// Named Windows actions exposed by NGENUITY Legacy.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WindowsShortcut {
    CycleApps,
    SwitchApps,
    Cut,
    Copy,
    Paste,
    Undo,
}

/// A usage ID from the USB HID Keyboard/Keypad usage page (`0x07`).
///
/// The protocol layer is responsible for translating this semantic value to
/// a device-specific representation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct KeyboardUsage(pub u16);

/// Error returned when a human-readable keyboard key name is unknown.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct KeyboardUsageParseError(pub String);

impl fmt::Display for KeyboardUsageParseError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "unknown keyboard key {:?}", self.0)
    }
}

impl std::error::Error for KeyboardUsageParseError {}

impl FromStr for KeyboardUsage {
    type Err = KeyboardUsageParseError;

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        let name = input.trim().to_ascii_lowercase().replace('_', "-");

        if let [letter @ b'a'..=b'z'] = name.as_bytes() {
            return Ok(Self(u16::from(*letter - b'a' + 0x04)));
        }
        if let [digit @ b'1'..=b'9'] = name.as_bytes() {
            return Ok(Self(u16::from(*digit - b'1' + 0x1E)));
        }
        if name == "0" {
            return Ok(Self(0x27));
        }
        if let Some(number) = name
            .strip_prefix('f')
            .and_then(|value| value.parse::<u16>().ok())
        {
            return match number {
                1..=12 => Ok(Self(0x3A + number - 1)),
                13..=24 => Ok(Self(0x68 + number - 13)),
                _ => Err(KeyboardUsageParseError(input.to_owned())),
            };
        }
        if let Some(number) = name
            .strip_prefix("keypad-")
            .and_then(|value| value.parse::<u16>().ok())
        {
            return match number {
                1..=9 => Ok(Self(0x59 + number - 1)),
                0 => Ok(Self(0x62)),
                _ => Err(KeyboardUsageParseError(input.to_owned())),
            };
        }

        let usage = match name.as_str() {
            "enter" | "return" => 0x28,
            "escape" | "esc" => 0x29,
            "backspace" => 0x2A,
            "tab" => 0x2B,
            "space" => 0x2C,
            "minus" => 0x2D,
            "equal" | "equals" => 0x2E,
            "left-bracket" => 0x2F,
            "right-bracket" => 0x30,
            "backslash" => 0x31,
            "non-us-hash" => 0x32,
            "semicolon" => 0x33,
            "apostrophe" | "quote" => 0x34,
            "grave" | "backtick" => 0x35,
            "comma" => 0x36,
            "period" | "dot" => 0x37,
            "slash" => 0x38,
            "caps-lock" => 0x39,
            "print-screen" => 0x46,
            "scroll-lock" => 0x47,
            "pause" => 0x48,
            "insert" => 0x49,
            "home" => 0x4A,
            "page-up" => 0x4B,
            "delete" => 0x4C,
            "end" => 0x4D,
            "page-down" => 0x4E,
            "arrow-right" | "right-arrow" => 0x4F,
            "arrow-left" | "left-arrow" => 0x50,
            "arrow-down" | "down-arrow" => 0x51,
            "arrow-up" | "up-arrow" => 0x52,
            "num-lock" => 0x53,
            "keypad-divide" => 0x54,
            "keypad-multiply" => 0x55,
            "keypad-subtract" => 0x56,
            "keypad-add" => 0x57,
            "keypad-enter" => 0x58,
            "keypad-decimal" => 0x63,
            "non-us-backslash" => 0x64,
            "application" | "menu" => 0x65,
            "power" => 0x66,
            "keypad-equal" => 0x67,
            "left-control" | "left-ctrl" => 0xE0,
            "left-shift" => 0xE1,
            "left-alt" => 0xE2,
            "left-gui" | "left-windows" | "left-win" => 0xE3,
            "right-control" | "right-ctrl" => 0xE4,
            "right-shift" => 0xE5,
            "right-alt" => 0xE6,
            "right-gui" | "right-windows" | "right-win" => 0xE7,
            _ => return Err(KeyboardUsageParseError(input.to_owned())),
        };
        Ok(Self(usage))
    }
}

/// Press/release state for one keyboard event in a macro.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum KeyState {
    Pressed,
    Released,
}

/// One keyboard event and its associated timing value.
///
/// Device protocols decide whether the timing is applied before or after the
/// event. Keeping it on each event supports both fixed and recorded timings.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct KeyboardMacroEvent {
    pub usage: KeyboardUsage,
    pub state: KeyState,
    pub timing_ms: u16,
}

impl KeyboardMacroEvent {
    pub const fn pressed(usage: KeyboardUsage, timing_ms: u16) -> Self {
        Self {
            usage,
            state: KeyState::Pressed,
            timing_ms,
        }
    }

    pub const fn released(usage: KeyboardUsage, timing_ms: u16) -> Self {
        Self {
            usage,
            state: KeyState::Released,
            timing_ms,
        }
    }
}

/// Reference to an application or onboard macro and its playback policy.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MacroBinding {
    pub id: String,
    pub playback: MacroPlayback,
}

/// Macro playback policies observed in NGENUITY Legacy.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum MacroPlayback {
    Once,
    ToggleRepeat,
    RepeatWhileHeld,
}

/// Platform-independent software macro definition.
///
/// Device drivers validate which event shapes, keys, timings and playback
/// modes they can encode. A definition being parseable does not imply that a
/// particular device can execute it.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct MacroDefinition {
    pub playback: MacroPlayback,
    pub events: Vec<MacroEvent>,
}

/// One input transition followed by an exact delay before the next event.
///
/// Separate down/up events allow chords: press a modifier, use a zero delay,
/// press another key, then release both explicitly. Mouse-button variants are
/// modeled for future capture-backed device encoders.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum MacroEvent {
    KeyDown { key: String, delay_ms: u16 },
    KeyUp { key: String, delay_ms: u16 },
    MouseButtonDown { button: String, delay_ms: u16 },
    MouseButtonUp { button: String, delay_ms: u16 },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn represents_each_observed_binding_category() {
        let bindings = [
            ButtonBinding::Mouse(MouseFunction::Back),
            ButtonBinding::Keyboard(KeyboardUsage(0x04)),
            ButtonBinding::Multimedia(MultimediaFunction::VolumeUp),
            ButtonBinding::Macro(MacroBinding {
                id: "test-macro".to_owned(),
                playback: MacroPlayback::RepeatWhileHeld,
            }),
            ButtonBinding::WindowsShortcut(WindowsShortcut::Copy),
            ButtonBinding::Disabled,
        ];

        assert_eq!(bindings.len(), 6);
        assert_ne!(bindings[0], bindings[5]);
    }

    #[test]
    fn macro_timings_are_stored_per_event() {
        let events = [
            KeyboardMacroEvent::pressed(KeyboardUsage(0x04), 20),
            KeyboardMacroEvent::released(KeyboardUsage(0x04), 40),
        ];

        assert_eq!(events[0].state, KeyState::Pressed);
        assert_eq!(events[0].timing_ms, 20);
        assert_eq!(events[1].state, KeyState::Released);
        assert_eq!(events[1].timing_ms, 40);
    }

    #[test]
    fn parses_human_keyboard_names_to_standard_hid_usages() {
        assert_eq!("a".parse(), Ok(KeyboardUsage(0x04)));
        assert_eq!("B".parse(), Ok(KeyboardUsage(0x05)));
        assert_eq!("0".parse(), Ok(KeyboardUsage(0x27)));
        assert_eq!("f12".parse(), Ok(KeyboardUsage(0x45)));
        assert_eq!("F24".parse(), Ok(KeyboardUsage(0x73)));
        assert_eq!("left_shift".parse(), Ok(KeyboardUsage(0xE1)));
        assert_eq!("right-win".parse(), Ok(KeyboardUsage(0xE7)));
        assert!("definitely-not-a-key".parse::<KeyboardUsage>().is_err());
    }
}
