//! File binding choices and target preflight, never a transport or device write.
use anyhow::{bail, Context, Result};
use hyperx_core::{
    MouseFunction, MultimediaFunction, SoftwareButtonBinding, SoftwareProfile, WindowsShortcut,
};
use serde::Serialize;

use crate::{profile_controls, validate_profile};

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ProfileBindingChoice {
    pub label: String,
    pub binding: SoftwareButtonBinding,
    /// Macro definitions can remain in invalid drafts. Show, but do not offer
    /// assigning, definitions rejected for this target's runtime encoding.
    pub error: Option<String>,
}

/// Validate only this assignment; unrelated invalid fields and unresolved
/// provenance must remain editable. This is offline runtime-encoding support,
/// not device verification, onboard support, or permission to apply a profile.
pub fn validate_button_binding(
    profile: &SoftwareProfile,
    control: &str,
    binding: &SoftwareButtonBinding,
) -> Result<()> {
    let target = profile_controls(&profile.device)
        .into_iter()
        .find(|target| target.id == control)
        .context("unknown physical control")?;
    if target.primary {
        bail!("use the coupled primary-button layout for left/right click");
    }
    let macros = if let SoftwareButtonBinding::Macro { id } = binding {
        let definitions: Vec<_> = profile
            .macros
            .iter()
            .filter(|definition| definition.source_id == *id)
            .cloned()
            .collect();
        if definitions.len() != 1 {
            bail!("macro ID {id:?} must identify exactly one definition in this file");
        }
        definitions
    } else {
        Vec::new()
    };
    let isolated = SoftwareProfile {
        name: "Binding preflight".into(),
        device: profile.device.clone(),
        buttons: [(control.into(), binding.clone())].into(),
        macros,
        ..Default::default()
    };
    if let Some(error) = validate_profile(&isolated).error {
        bail!("binding rejected: {error}");
    }
    Ok(())
}

/// Canonical UI names, accepted by the existing semantic keyboard parser.
/// Aliases remain valid in imported files; opening a picker never normalizes
/// them. No keyboard usage IDs or vendor bytes are exposed to a client.
pub(crate) fn keyboard_names() -> Vec<String> {
    let mut names: Vec<_> = ('a'..='z')
        .chain('0'..='9')
        .map(|key| key.to_string())
        .collect();
    names.extend((1..=24).map(|key| format!("f{key}")));
    names.extend((0..=9).map(|key| format!("keypad-{key}")));
    names.extend(
        [
            "enter",
            "escape",
            "backspace",
            "tab",
            "space",
            "minus",
            "equal",
            "left-bracket",
            "right-bracket",
            "backslash",
            "non-us-hash",
            "semicolon",
            "apostrophe",
            "grave",
            "comma",
            "period",
            "slash",
            "caps-lock",
            "print-screen",
            "scroll-lock",
            "pause",
            "insert",
            "home",
            "page-up",
            "delete",
            "end",
            "page-down",
            "arrow-right",
            "arrow-left",
            "arrow-down",
            "arrow-up",
            "num-lock",
            "keypad-divide",
            "keypad-multiply",
            "keypad-subtract",
            "keypad-add",
            "keypad-enter",
            "keypad-decimal",
            "non-us-backslash",
            "application",
            "power",
            "keypad-equal",
            "left-control",
            "left-shift",
            "left-alt",
            "left-gui",
            "right-control",
            "right-shift",
            "right-alt",
            "right-gui",
        ]
        .into_iter()
        .map(str::to_owned),
    );
    names
}

pub fn profile_binding_choices(
    profile: &SoftwareProfile,
    control: &str,
) -> Vec<ProfileBindingChoice> {
    if !profile_controls(&profile.device)
        .iter()
        .any(|target| target.id == control && !target.primary)
    {
        return Vec::new();
    }
    let mut candidates = Vec::new();
    for (action, label) in [
        (MouseFunction::LeftClick, "Left click"),
        (MouseFunction::RightClick, "Right click"),
        (MouseFunction::MiddleClick, "Middle click"),
        (MouseFunction::Back, "Back"),
        (MouseFunction::Forward, "Forward"),
        (MouseFunction::TiltLeft, "Wheel tilt left"),
        (MouseFunction::TiltRight, "Wheel tilt right"),
        (MouseFunction::DpiToggle, "DPI toggle"),
        (MouseFunction::ScrollUp, "Scroll up"),
        (MouseFunction::ScrollDown, "Scroll down"),
    ] {
        candidates.push((label.into(), SoftwareButtonBinding::Mouse { action }));
    }
    for (action, label) in [
        (MultimediaFunction::PlayPause, "Play / pause"),
        (MultimediaFunction::Stop, "Stop"),
        (MultimediaFunction::NextTrack, "Next track"),
        (MultimediaFunction::PreviousTrack, "Previous track"),
        (MultimediaFunction::MuteVolume, "Mute volume"),
        (MultimediaFunction::VolumeUp, "Volume up"),
        (MultimediaFunction::VolumeDown, "Volume down"),
    ] {
        candidates.push((label.into(), SoftwareButtonBinding::Multimedia { action }));
    }
    for (action, label) in [
        (WindowsShortcut::CycleApps, "Cycle apps"),
        (WindowsShortcut::SwitchApps, "Switch apps"),
        (WindowsShortcut::Cut, "Cut"),
        (WindowsShortcut::Copy, "Copy"),
        (WindowsShortcut::Paste, "Paste"),
        (WindowsShortcut::Undo, "Undo"),
    ] {
        candidates.push((
            label.into(),
            SoftwareButtonBinding::WindowsShortcut { action },
        ));
    }
    candidates.push(("Disabled".into(), SoftwareButtonBinding::Disabled {}));
    candidates.extend(keyboard_names().into_iter().map(|key| {
        (
            key.replace('-', " ").to_uppercase(),
            SoftwareButtonBinding::Keyboard { key },
        )
    }));
    let mut choices: Vec<_> = candidates
        .into_iter()
        .filter_map(|(label, binding)| {
            validate_button_binding(profile, control, &binding)
                .ok()
                .map(|()| ProfileBindingChoice {
                    label,
                    binding,
                    error: None,
                })
        })
        .collect();
    // Preserve duplicate IDs and rejected definitions in the visible library;
    // never silently choose the first duplicate or infer another target.
    choices.extend(profile.macros.iter().map(|definition| {
        let binding = SoftwareButtonBinding::Macro {
            id: definition.source_id.clone(),
        };
        ProfileBindingChoice {
            label: format!("{} · {}", definition.name, definition.source_id),
            error: validate_button_binding(profile, control, &binding)
                .err()
                .map(|error| error.to_string()),
            binding,
        }
    }));
    choices
}
