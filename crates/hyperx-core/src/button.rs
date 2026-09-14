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

/// Mouse actions exposed by NGENUITY for Pulsefire Raid buttons.
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

/// Consumer-control actions exposed by NGENUITY.
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

/// Named Windows actions exposed by NGENUITY.
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

/// Macro playback policies observed in NGENUITY.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MacroPlayback {
    Once,
    ToggleRepeat,
    RepeatWhileHeld,
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
}
