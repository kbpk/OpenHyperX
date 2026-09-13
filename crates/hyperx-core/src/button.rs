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
}
