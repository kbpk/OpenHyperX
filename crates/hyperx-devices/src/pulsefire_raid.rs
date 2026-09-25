use std::time::Duration;

use hyperx_core::{
    ButtonBinding, Capability, CapabilitySet, DeviceDescriptor, DpiCapabilities, DpiProfile,
    DpiValidationError, InterfaceSelector, KeyboardUsage, LightingZone, MacroDefinition,
    MacroEvent, MacroPlayback, MouseFunction, MultimediaFunction, PollingRate, PrimaryButtonLayout,
    RgbColor, UsbId, WindowsShortcut,
};
use hyperx_hid::{HidError, HidTransport};
use hyperx_protocol::pulsefire_raid::{
    encode_direct_rgb, encode_onboard_profile_read_prelude, encode_onboard_static_lighting_reports,
    encode_profile_read_request, encode_runtime_profile_read_prelude, PerformanceProfile,
    PerformanceProfileError, ProfileImageKind, ProfileSection, PulsefireRaidControl,
    PulsefireRaidMacro, PulsefireRaidMacroError, PulsefireRaidMacroEvent, PulsefireRaidMacroInput,
    PulsefireRaidMacroMouseButton, PulsefireRaidMacroState, DIRECT_REPORT_ID, DIRECT_REPORT_LENGTH,
};
use thiserror::Error;

const PROFILE_PRELUDE_DELAY: Duration = Duration::from_millis(65);
const PROFILE_READ_DELAY: Duration = Duration::from_millis(110);
const ONBOARD_COMMIT_DELAY: Duration = Duration::from_secs(1);
const SAVE_ACK_LENGTH: usize = 8;
const SAVE_ACK_TIMEOUT_MS: i32 = 500;

pub const PULSEFIRE_RAID_DPI: DpiCapabilities = DpiCapabilities {
    minimum: 200,
    maximum: 16_000,
    step: 50,
    max_stages: 5,
};

const POLLING_RATES: &[PollingRate] = &[
    PollingRate::Hz125,
    PollingRate::Hz250,
    PollingRate::Hz500,
    PollingRate::Hz1000,
];

const FEATURES: &[Capability] = &[
    Capability::Dpi,
    Capability::DpiStages,
    Capability::PollingRate,
    Capability::ButtonRemapping,
    Capability::Macros,
    Capability::Lighting,
    Capability::OnboardProfile,
];

const LIGHTING_ZONES: &[LightingZone] = &[
    LightingZone {
        id: "wheel",
        name: "Scroll Wheel",
    },
    LightingZone {
        id: "logo",
        name: "Logo",
    },
];

/// Pulsefire Raid identity and hardware-level capabilities.
///
/// Protocol support is tracked separately and must never be inferred from
/// this hardware declaration.
pub const PULSEFIRE_RAID: DeviceDescriptor = DeviceDescriptor {
    id: "pulsefire-raid",
    name: "HyperX Pulsefire Raid",
    usb_id: UsbId {
        vendor_id: 0x0951,
        product_id: 0x16E4,
    },
    capabilities: CapabilitySet {
        features: FEATURES,
        button_count: 11,
        lighting_zones: LIGHTING_ZONES,
        dpi: Some(PULSEFIRE_RAID_DPI),
        polling_rates: POLLING_RATES,
    },
    configuration_interface: InterfaceSelector {
        interface_number: 1,
        usage_page: 0xFF01,
        usage: 0x0001,
    },
};

/// Vendor collection carrying eight-byte acknowledgements for profile saves.
///
/// Feature reports remain on the configuration collection. Windows exposes
/// endpoint `0x83` through this separate HID interface, as confirmed by
/// USBPcap and the physical device topology.
pub const PULSEFIRE_RAID_ACKNOWLEDGEMENT_INTERFACE: InterfaceSelector = InterfaceSelector {
    interface_number: 2,
    usage_page: 0xFF00,
    usage: 0xFF00,
};

/// One validated, capture-backed runtime button assignment.
///
/// The fields are private so every client, including a future GUI, must pass
/// the target-specific evidence gate before opening the device.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PulsefireRaidRuntimeAssignment {
    control: PulsefireRaidControl,
    action: PulsefireRaidRuntimeAction,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum PulsefireRaidRuntimeAction {
    Ordinary(ButtonBinding),
    Macro(MacroDefinition),
}

impl PulsefireRaidRuntimeAssignment {
    /// Validate an ordinary binding against captured record evidence and the
    /// target control's observed NGENUITY Legacy capabilities.
    ///
    /// This performs no discovery or I/O. Binding records seen on only one
    /// physical control remain target-specific.
    pub fn ordinary(
        control: PulsefireRaidControl,
        binding: ButtonBinding,
    ) -> Result<Self, PulsefireRaidError> {
        let general_control = matches!(
            control,
            PulsefireRaidControl::MiddleClick
                | PulsefireRaidControl::Button4
                | PulsefireRaidControl::Button5
                | PulsefireRaidControl::Button7
                | PulsefireRaidControl::Button6
                | PulsefireRaidControl::Button8
                | PulsefireRaidControl::Dpi
                | PulsefireRaidControl::WheelTiltLeft
                | PulsefireRaidControl::WheelTiltRight
        );
        let portable_binding = match &binding {
            ButtonBinding::Disabled
            | ButtonBinding::Mouse(
                MouseFunction::LeftClick
                | MouseFunction::RightClick
                | MouseFunction::MiddleClick
                | MouseFunction::Back
                | MouseFunction::Forward
                | MouseFunction::TiltLeft
                | MouseFunction::TiltRight
                | MouseFunction::DpiToggle
                | MouseFunction::ScrollUp
                | MouseFunction::ScrollDown,
            )
            | ButtonBinding::Multimedia(
                MultimediaFunction::PlayPause
                | MultimediaFunction::Stop
                | MultimediaFunction::NextTrack
                | MultimediaFunction::PreviousTrack
                | MultimediaFunction::MuteVolume
                | MultimediaFunction::VolumeUp
                | MultimediaFunction::VolumeDown,
            )
            | ButtonBinding::WindowsShortcut(
                WindowsShortcut::CycleApps
                | WindowsShortcut::SwitchApps
                | WindowsShortcut::Cut
                | WindowsShortcut::Copy
                | WindowsShortcut::Paste
                | WindowsShortcut::Undo,
            ) => true,
            ButtonBinding::Keyboard(usage) => is_supported_runtime_keyboard_usage(*usage),
            _ => false,
        };
        let confirmed = general_control && portable_binding;
        if !confirmed {
            return Err(PulsefireRaidError::UnconfirmedRuntimeButtonBinding { control, binding });
        }
        Ok(Self {
            control,
            action: PulsefireRaidRuntimeAction::Ordinary(binding),
        })
    }

    /// Validate a software macro for the capture-backed runtime path.
    ///
    /// This performs no discovery or I/O, allowing clients to reject invalid
    /// files before opening the device.
    pub fn macro_timeline(
        control: PulsefireRaidControl,
        definition: MacroDefinition,
    ) -> Result<Self, PulsefireRaidError> {
        if !matches!(
            control,
            PulsefireRaidControl::Button4 | PulsefireRaidControl::Button5
        ) {
            return Err(PulsefireRaidError::UnconfirmedMacroControl(control));
        }
        let assignment = Self {
            control,
            action: PulsefireRaidRuntimeAction::Macro(definition),
        };
        assignment.encoded_macro()?;
        Ok(assignment)
    }

    pub const fn control(&self) -> PulsefireRaidControl {
        self.control
    }

    pub fn ordinary_binding(&self) -> Option<&ButtonBinding> {
        match &self.action {
            PulsefireRaidRuntimeAction::Ordinary(binding) => Some(binding),
            PulsefireRaidRuntimeAction::Macro(_) => None,
        }
    }

    pub fn macro_definition(&self) -> Option<&MacroDefinition> {
        match &self.action {
            PulsefireRaidRuntimeAction::Ordinary(_) => None,
            PulsefireRaidRuntimeAction::Macro(definition) => Some(definition),
        }
    }

    fn encoded_macro(&self) -> Result<Option<PulsefireRaidMacro>, PulsefireRaidError> {
        let PulsefireRaidRuntimeAction::Macro(definition) = &self.action else {
            return Ok(None);
        };
        Ok(Some(encode_macro_definition(self.control, definition)?))
    }
}

fn encode_macro_definition(
    control: PulsefireRaidControl,
    definition: &MacroDefinition,
) -> Result<PulsefireRaidMacro, PulsefireRaidError> {
    if definition.playback != MacroPlayback::Once {
        return Err(PulsefireRaidError::UnsupportedMacroPlayback(
            definition.playback,
        ));
    }
    let events = definition
        .events
        .iter()
        .map(encode_macro_event)
        .collect::<Result<Vec<_>, _>>()?;
    Ok(PulsefireRaidMacro::play_once_for(control, &events)?)
}

const fn is_supported_runtime_keyboard_usage(usage: KeyboardUsage) -> bool {
    // This is exactly the named key set accepted by KeyboardUsage::from_str.
    // The isolated A -> B capture confirms that ordinary bindings carry the
    // standard one-byte Keyboard/Keypad usage in record byte 1.
    matches!(usage.0, 0x04..=0x73 | 0xE0..=0xE7)
}

fn encode_macro_event(event: &MacroEvent) -> Result<PulsefireRaidMacroEvent, PulsefireRaidError> {
    let (input, state, delay_ms) = match event {
        MacroEvent::KeyDown { key, delay_ms } => (
            PulsefireRaidMacroInput::Keyboard(parse_macro_key(key)?),
            PulsefireRaidMacroState::Pressed,
            *delay_ms,
        ),
        MacroEvent::KeyUp { key, delay_ms } => (
            PulsefireRaidMacroInput::Keyboard(parse_macro_key(key)?),
            PulsefireRaidMacroState::Released,
            *delay_ms,
        ),
        MacroEvent::MouseButtonDown { button, delay_ms } => (
            PulsefireRaidMacroInput::MouseButton(parse_macro_mouse_button(button)?),
            PulsefireRaidMacroState::Pressed,
            *delay_ms,
        ),
        MacroEvent::MouseButtonUp { button, delay_ms } => (
            PulsefireRaidMacroInput::MouseButton(parse_macro_mouse_button(button)?),
            PulsefireRaidMacroState::Released,
            *delay_ms,
        ),
    };
    Ok(PulsefireRaidMacroEvent::new(input, state, delay_ms))
}

fn parse_macro_key(key: &str) -> Result<KeyboardUsage, PulsefireRaidError> {
    key.parse()
        .map_err(|_| PulsefireRaidError::UnknownMacroKey(key.to_owned()))
}

fn parse_macro_mouse_button(
    button: &str,
) -> Result<PulsefireRaidMacroMouseButton, PulsefireRaidError> {
    match button
        .trim()
        .to_ascii_lowercase()
        .replace('_', "-")
        .as_str()
    {
        "left" => Ok(PulsefireRaidMacroMouseButton::Left),
        "right" => Ok(PulsefireRaidMacroMouseButton::Right),
        "middle" | "wheel" => Ok(PulsefireRaidMacroMouseButton::Middle),
        _ => Err(PulsefireRaidError::UnknownMacroMouseButton(
            button.to_owned(),
        )),
    }
}

#[derive(Debug, Error)]
pub enum PulsefireRaidError {
    #[error("Pulsefire Raid configuration requires HID interface 1, got {0}")]
    WrongInterface(i32),
    #[error("feature report write was short: expected {expected} bytes, wrote {actual}")]
    ShortFeatureWrite { expected: usize, actual: usize },
    #[error("expected a 264-byte Pulsefire Raid profile response, got {0} bytes")]
    WrongProfileResponseLength(usize),
    #[error("unexpected profile read response: got {kind:?} {section:?}")]
    WrongProfileResponse {
        kind: ProfileImageKind,
        section: ProfileSection,
    },
    #[error("save acknowledgement for opcode 0x{opcode:02X} was {actual} bytes, expected 8")]
    WrongSaveAcknowledgementLength { opcode: u8, actual: usize },
    #[error("unexpected save acknowledgement for opcode 0x{opcode:02X}: {actual:02X?}")]
    UnexpectedSaveAcknowledgement {
        opcode: u8,
        actual: [u8; SAVE_ACK_LENGTH],
    },
    #[error("runtime Button 5 references a macro; provide its definition to save it onboard")]
    MissingOnboardMacroDefinition,
    #[error("a macro definition was supplied, but runtime Button 5 does not reference a macro")]
    UnexpectedOnboardMacroDefinition,
    #[error("Pulsefire Raid save acknowledgements require HID interface 2, got {0}")]
    WrongAcknowledgementInterface(i32),
    #[error("DPI stage {stage} does not exist; the runtime profile has {stage_count} stage(s)")]
    DpiStageNotFound { stage: usize, stage_count: usize },
    #[error("DPI stage update must change its DPI, color, or active state")]
    EmptyDpiStageUpdate,
    #[error("Pulsefire Raid supports at most {0} DPI stages")]
    TooManyDpiStages(u8),
    #[error("the only remaining DPI stage cannot be removed")]
    CannotRemoveOnlyDpiStage,
    #[error("runtime binding {binding:?} is not capture-backed for {control:?}")]
    UnconfirmedRuntimeButtonBinding {
        control: PulsefireRaidControl,
        binding: ButtonBinding,
    },
    #[error(
        "runtime macros are not capture-backed for {0:?}; only Button 4 and Button 5 are supported"
    )]
    UnconfirmedMacroControl(PulsefireRaidControl),
    #[error("Pulsefire Raid macro playback {0:?} is not capture-backed; only once is supported")]
    UnsupportedMacroPlayback(MacroPlayback),
    #[error("unknown macro keyboard key {0:?}")]
    UnknownMacroKey(String),
    #[error("unknown or unconfirmed macro mouse button {0:?}; use left, right or middle")]
    UnknownMacroMouseButton(String),
    #[error(transparent)]
    InvalidMacro(#[from] PulsefireRaidMacroError),
    #[error(transparent)]
    InvalidDpi(#[from] DpiValidationError),
    #[error(transparent)]
    InvalidProfile(#[from] PerformanceProfileError),
    #[error(transparent)]
    Transport(#[from] HidError),
}

/// Confirmed settings included in one completed onboard save transaction.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PulsefireRaidOnboardSaveResult {
    pub polling_rate: PollingRate,
    pub dpi_profile: DpiProfile,
    pub macro_saved: bool,
}

pub struct PulsefireRaid<T> {
    transport: T,
}

impl<T: HidTransport> PulsefireRaid<T> {
    pub fn new(transport: T) -> Result<Self, PulsefireRaidError> {
        let interface = transport.interface_number();
        if interface != PULSEFIRE_RAID.configuration_interface.interface_number {
            return Err(PulsefireRaidError::WrongInterface(interface));
        }
        Ok(Self { transport })
    }

    /// Set volatile direct colors for both physical LEDs.
    ///
    /// The device returns to its stored effect unless this is periodically
    /// refreshed. This operation does not save to onboard memory.
    pub fn set_volatile_direct_rgb(
        &mut self,
        wheel: RgbColor,
        logo: RgbColor,
    ) -> Result<(), PulsefireRaidError> {
        let report = encode_direct_rgb(wheel, logo);
        self.send_feature_report(&report)?;
        std::thread::sleep(std::time::Duration::from_millis(10));
        Ok(())
    }

    /// Read the current volatile performance/button profile.
    ///
    /// This replays the exact read sequence observed twice in one local
    /// capture and in earlier isolated setting captures. It performs no
    /// profile write and deliberately does not retry an ambiguous failure.
    pub fn runtime_profile(&mut self) -> Result<PerformanceProfile, PulsefireRaidError> {
        self.runtime_profile_with_wait(std::thread::sleep)
    }

    fn runtime_profile_with_wait(
        &mut self,
        mut wait: impl FnMut(Duration),
    ) -> Result<PerformanceProfile, PulsefireRaidError> {
        let prelude = encode_runtime_profile_read_prelude();
        self.send_feature_report(&prelude)?;
        wait(PROFILE_PRELUDE_DELAY);

        let request = encode_profile_read_request();
        self.send_feature_report(&request)?;
        wait(PROFILE_READ_DELAY);

        let mut response = [0_u8; DIRECT_REPORT_LENGTH];
        response[0] = DIRECT_REPORT_ID;
        let response_length = self.transport.get_feature_report(&mut response)?;
        if response_length != DIRECT_REPORT_LENGTH {
            return Err(PulsefireRaidError::WrongProfileResponseLength(
                response_length,
            ));
        }

        let profile = PerformanceProfile::parse(&response)?;
        if profile.kind() != ProfileImageKind::DeviceReadResponse
            || profile.section() != ProfileSection::Runtime
        {
            return Err(PulsefireRaidError::WrongProfileResponse {
                kind: profile.kind(),
                section: profile.section(),
            });
        }

        Ok(profile)
    }

    /// Persist the confirmed runtime DPI, polling and button records to the
    /// mouse's onboard profile.
    ///
    /// `wheel` and `logo` are the capture-confirmed static lighting snapshot
    /// carried by NGENUITY Legacy's save transaction and hardware-verified
    /// across a power-cycle. If runtime Button 5 contains the known macro
    /// reference, its definition is required because the profile image contains
    /// no macro timeline to read back.
    pub fn save_runtime_to_onboard<A: HidTransport>(
        &mut self,
        acknowledgements: &mut A,
        wheel: RgbColor,
        logo: RgbColor,
        macro_definition: Option<&MacroDefinition>,
    ) -> Result<PulsefireRaidOnboardSaveResult, PulsefireRaidError> {
        self.save_runtime_to_onboard_with_wait(
            acknowledgements,
            wheel,
            logo,
            macro_definition,
            std::thread::sleep,
        )
    }

    fn save_runtime_to_onboard_with_wait<A: HidTransport>(
        &mut self,
        acknowledgements: &mut A,
        wheel: RgbColor,
        logo: RgbColor,
        macro_definition: Option<&MacroDefinition>,
        mut wait: impl FnMut(Duration),
    ) -> Result<PulsefireRaidOnboardSaveResult, PulsefireRaidError> {
        if acknowledgements.interface_number()
            != PULSEFIRE_RAID_ACKNOWLEDGEMENT_INTERFACE.interface_number
        {
            return Err(PulsefireRaidError::WrongAcknowledgementInterface(
                acknowledgements.interface_number(),
            ));
        }
        // Validate the optional macro before opening the persistent part of
        // the transaction. Runtime fields are validated immediately after the
        // read and before selecting the onboard section.
        let macro_report = macro_definition
            .map(|definition| encode_macro_definition(PulsefireRaidControl::Button5, definition))
            .transpose()?;
        let runtime = self.read_runtime_profile_for_save(acknowledgements, &mut wait)?;
        runtime.validate_confirmed_runtime_settings()?;

        let has_macro_reference =
            runtime.has_confirmed_macro_reference(PulsefireRaidControl::Button5);
        match (has_macro_reference, macro_report.is_some()) {
            (true, false) => return Err(PulsefireRaidError::MissingOnboardMacroDefinition),
            (false, true) => return Err(PulsefireRaidError::UnexpectedOnboardMacroDefinition),
            _ => {}
        }

        let polling_rate = runtime.polling_rate()?;
        let dpi_profile = runtime.dpi_profile()?;

        self.send_feature_report_with_save_ack(
            acknowledgements,
            &encode_onboard_profile_read_prelude(),
        )?;
        for report in encode_onboard_static_lighting_reports(wheel, logo) {
            self.send_feature_report_with_save_ack(acknowledgements, &report)?;
        }
        self.send_feature_report_with_save_ack(acknowledgements, &encode_profile_read_request())?;
        wait(PROFILE_READ_DELAY);

        let mut onboard = self.read_profile_response(ProfileSection::Onboard)?;
        onboard.copy_confirmed_runtime_settings_from(&runtime)?;

        if let Some(macro_report) = &macro_report {
            self.send_feature_report_with_save_ack(
                acknowledgements,
                &macro_report.to_onboard_report()?,
            )?;
        }
        self.send_feature_report_with_save_ack(acknowledgements, &onboard.to_write_report())?;
        wait(ONBOARD_COMMIT_DELAY);
        self.send_feature_report_with_save_ack(
            acknowledgements,
            &encode_runtime_profile_read_prelude(),
        )?;

        Ok(PulsefireRaidOnboardSaveResult {
            polling_rate,
            dpi_profile,
            macro_saved: macro_report.is_some(),
        })
    }

    fn read_runtime_profile_for_save<A: HidTransport>(
        &mut self,
        acknowledgements: &mut A,
        wait: &mut impl FnMut(Duration),
    ) -> Result<PerformanceProfile, PulsefireRaidError> {
        self.send_feature_report_with_save_ack(
            acknowledgements,
            &encode_runtime_profile_read_prelude(),
        )?;
        self.send_feature_report_with_save_ack(acknowledgements, &encode_profile_read_request())?;
        wait(PROFILE_READ_DELAY);
        self.read_profile_response(ProfileSection::Runtime)
    }

    fn read_profile_response(
        &mut self,
        expected_section: ProfileSection,
    ) -> Result<PerformanceProfile, PulsefireRaidError> {
        let mut response = [0_u8; DIRECT_REPORT_LENGTH];
        response[0] = DIRECT_REPORT_ID;
        let response_length = self.transport.get_feature_report(&mut response)?;
        if response_length != DIRECT_REPORT_LENGTH {
            return Err(PulsefireRaidError::WrongProfileResponseLength(
                response_length,
            ));
        }
        let profile = PerformanceProfile::parse(&response)?;
        if profile.kind() != ProfileImageKind::DeviceReadResponse
            || profile.section() != expected_section
        {
            return Err(PulsefireRaidError::WrongProfileResponse {
                kind: profile.kind(),
                section: profile.section(),
            });
        }
        Ok(profile)
    }

    /// Change both axes of the active DPI stage in the runtime profile.
    ///
    /// Every other stage, color, binding and unknown profile byte is preserved.
    /// This does not write the onboard profile.
    pub fn set_runtime_active_dpi(&mut self, dpi: u32) -> Result<DpiProfile, PulsefireRaidError> {
        self.set_runtime_active_dpi_with_wait(dpi, std::thread::sleep)
    }

    fn set_runtime_active_dpi_with_wait(
        &mut self,
        dpi: u32,
        wait: impl FnMut(Duration),
    ) -> Result<DpiProfile, PulsefireRaidError> {
        let dpi = PULSEFIRE_RAID_DPI.validate(dpi)?;
        let mut profile = self.runtime_profile_with_wait(wait)?;
        let mut dpi_profile = profile.dpi_profile()?;
        let stage = &mut dpi_profile.stages[dpi_profile.active_stage];
        stage.x = dpi;
        stage.y = dpi;
        profile.set_dpi_profile(&dpi_profile)?;
        self.write_runtime_profile(&profile)?;
        Ok(dpi_profile)
    }

    /// Change one existing runtime DPI stage and optionally make it active.
    ///
    /// Stage indexes are zero-based. Omitted DPI or color values are preserved,
    /// as are every field unrelated to the DPI profile. This does not write the
    /// onboard profile.
    pub fn set_runtime_dpi_stage(
        &mut self,
        stage_index: usize,
        dpi: Option<u32>,
        color: Option<RgbColor>,
        activate: bool,
    ) -> Result<DpiProfile, PulsefireRaidError> {
        self.set_runtime_dpi_stage_with_wait(stage_index, dpi, color, activate, std::thread::sleep)
    }

    fn set_runtime_dpi_stage_with_wait(
        &mut self,
        stage_index: usize,
        dpi: Option<u32>,
        color: Option<RgbColor>,
        activate: bool,
        wait: impl FnMut(Duration),
    ) -> Result<DpiProfile, PulsefireRaidError> {
        if dpi.is_none() && color.is_none() && !activate {
            return Err(PulsefireRaidError::EmptyDpiStageUpdate);
        }
        let dpi = dpi
            .map(|value| PULSEFIRE_RAID_DPI.validate(value))
            .transpose()?;
        let mut profile = self.runtime_profile_with_wait(wait)?;
        let mut dpi_profile = profile.dpi_profile()?;
        let stage_count = dpi_profile.stages.len();
        let stage = dpi_profile.stages.get_mut(stage_index).ok_or(
            PulsefireRaidError::DpiStageNotFound {
                stage: stage_index + 1,
                stage_count,
            },
        )?;
        if let Some(dpi) = dpi {
            stage.x = dpi;
            stage.y = dpi;
        }
        if let Some(color) = color {
            stage.color = color;
        }
        if activate {
            dpi_profile.active_stage = stage_index;
        }
        profile.set_dpi_profile(&dpi_profile)?;
        self.write_runtime_profile(&profile)?;
        Ok(dpi_profile)
    }

    /// Append one runtime DPI stage. This does not write the onboard profile.
    pub fn add_runtime_dpi_stage(
        &mut self,
        dpi: u32,
        color: RgbColor,
        activate: bool,
    ) -> Result<DpiProfile, PulsefireRaidError> {
        self.add_runtime_dpi_stage_with_wait(dpi, color, activate, std::thread::sleep)
    }

    fn add_runtime_dpi_stage_with_wait(
        &mut self,
        dpi: u32,
        color: RgbColor,
        activate: bool,
        wait: impl FnMut(Duration),
    ) -> Result<DpiProfile, PulsefireRaidError> {
        let dpi = PULSEFIRE_RAID_DPI.validate(dpi)?;
        let mut profile = self.runtime_profile_with_wait(wait)?;
        let mut dpi_profile = profile.dpi_profile()?;
        if dpi_profile.stages.len() >= usize::from(PULSEFIRE_RAID_DPI.max_stages) {
            return Err(PulsefireRaidError::TooManyDpiStages(
                PULSEFIRE_RAID_DPI.max_stages,
            ));
        }
        dpi_profile
            .stages
            .push(hyperx_core::DpiStage::new(dpi, dpi, color));
        if activate {
            dpi_profile.active_stage = dpi_profile.stages.len() - 1;
        }
        profile.set_dpi_profile(&dpi_profile)?;
        self.write_runtime_profile(&profile)?;
        Ok(dpi_profile)
    }

    /// Remove the final runtime DPI stage, preserving at least one stage.
    ///
    /// Removing only the final stage matches the device's contiguous stage
    /// layout and the captured NGENUITY Legacy add/remove behavior.
    pub fn remove_runtime_last_dpi_stage(&mut self) -> Result<DpiProfile, PulsefireRaidError> {
        self.remove_runtime_last_dpi_stage_with_wait(std::thread::sleep)
    }

    fn remove_runtime_last_dpi_stage_with_wait(
        &mut self,
        wait: impl FnMut(Duration),
    ) -> Result<DpiProfile, PulsefireRaidError> {
        let mut profile = self.runtime_profile_with_wait(wait)?;
        let mut dpi_profile = profile.dpi_profile()?;
        if dpi_profile.stages.len() == 1 {
            return Err(PulsefireRaidError::CannotRemoveOnlyDpiStage);
        }
        dpi_profile.stages.pop();
        if dpi_profile.active_stage >= dpi_profile.stages.len() {
            dpi_profile.active_stage = dpi_profile.stages.len() - 1;
        }
        profile.set_dpi_profile(&dpi_profile)?;
        self.write_runtime_profile(&profile)?;
        Ok(dpi_profile)
    }

    /// Select an existing runtime DPI stage without changing its values.
    pub fn set_runtime_active_dpi_stage(
        &mut self,
        stage_index: usize,
    ) -> Result<DpiProfile, PulsefireRaidError> {
        self.set_runtime_dpi_stage(stage_index, None, None, true)
    }

    /// Assign one target-specific, capture-backed action in runtime memory.
    ///
    /// Button 4/5 macros transmit their validated definition immediately before
    /// the profile reference, matching the captured NGENUITY Legacy transaction.
    /// No variant writes the onboard profile.
    pub fn set_runtime_button_assignment(
        &mut self,
        assignment: PulsefireRaidRuntimeAssignment,
    ) -> Result<PulsefireRaidRuntimeAssignment, PulsefireRaidError> {
        self.set_runtime_button_assignment_with_wait(assignment, std::thread::sleep)
    }

    fn set_runtime_button_assignment_with_wait(
        &mut self,
        assignment: PulsefireRaidRuntimeAssignment,
        wait: impl FnMut(Duration),
    ) -> Result<PulsefireRaidRuntimeAssignment, PulsefireRaidError> {
        let control = assignment.control();
        let macro_definition = assignment.encoded_macro()?;
        let ordinary_binding = assignment.ordinary_binding();
        let mut profile = self.runtime_profile_with_wait(wait)?;

        if let Some(macro_definition) = macro_definition {
            macro_definition.apply_to_profile(&mut profile)?;
            self.send_feature_report(macro_definition.as_bytes())?;
        } else {
            profile.set_button_binding(
                control,
                ordinary_binding.expect("every non-macro assignment has an ordinary binding"),
            )?;
        }

        self.write_runtime_profile(&profile)?;
        Ok(assignment)
    }

    /// Switch the physical left/right buttons as one runtime-only update.
    ///
    /// NGENUITY Legacy exposes these records as a coupled pair. This method
    /// preserves that invariant and never writes onboard memory.
    pub fn set_runtime_primary_button_layout(
        &mut self,
        layout: PrimaryButtonLayout,
    ) -> Result<PrimaryButtonLayout, PulsefireRaidError> {
        self.set_runtime_primary_button_layout_with_wait(layout, std::thread::sleep)
    }

    fn set_runtime_primary_button_layout_with_wait(
        &mut self,
        layout: PrimaryButtonLayout,
        wait: impl FnMut(Duration),
    ) -> Result<PrimaryButtonLayout, PulsefireRaidError> {
        let mut profile = self.runtime_profile_with_wait(wait)?;
        profile.set_primary_button_layout(layout);
        self.write_runtime_profile(&profile)?;
        Ok(layout)
    }

    /// Change the polling rate in the runtime profile only.
    pub fn set_runtime_polling_rate(
        &mut self,
        polling_rate: PollingRate,
    ) -> Result<PollingRate, PulsefireRaidError> {
        self.set_runtime_polling_rate_with_wait(polling_rate, std::thread::sleep)
    }

    fn set_runtime_polling_rate_with_wait(
        &mut self,
        polling_rate: PollingRate,
        wait: impl FnMut(Duration),
    ) -> Result<PollingRate, PulsefireRaidError> {
        let mut profile = self.runtime_profile_with_wait(wait)?;
        profile.set_polling_rate(polling_rate);
        self.write_runtime_profile(&profile)?;
        Ok(polling_rate)
    }

    fn write_runtime_profile(
        &mut self,
        profile: &PerformanceProfile,
    ) -> Result<(), PulsefireRaidError> {
        if profile.section() != ProfileSection::Runtime {
            return Err(PulsefireRaidError::WrongProfileResponse {
                kind: profile.kind(),
                section: profile.section(),
            });
        }
        self.send_feature_report(&profile.to_write_report())?;
        Ok(())
    }

    fn send_feature_report(&mut self, report: &[u8]) -> Result<(), PulsefireRaidError> {
        let actual = self.transport.send_feature_report(report)?;
        if actual != report.len() {
            return Err(PulsefireRaidError::ShortFeatureWrite {
                expected: report.len(),
                actual,
            });
        }
        Ok(())
    }

    fn send_feature_report_with_save_ack<A: HidTransport>(
        &mut self,
        acknowledgements: &mut A,
        report: &[u8],
    ) -> Result<(), PulsefireRaidError> {
        self.send_feature_report(report)?;
        let opcode = report[1];
        let mut actual = [0_u8; SAVE_ACK_LENGTH];
        let length = acknowledgements.read_timeout(&mut actual, SAVE_ACK_TIMEOUT_MS)?;
        if length != SAVE_ACK_LENGTH {
            return Err(PulsefireRaidError::WrongSaveAcknowledgementLength {
                opcode,
                actual: length,
            });
        }
        let expected = save_acknowledgement(opcode);
        if actual != expected {
            return Err(PulsefireRaidError::UnexpectedSaveAcknowledgement { opcode, actual });
        }
        Ok(())
    }

    pub fn into_transport(self) -> T {
        self.transport
    }
}

fn save_acknowledgement(opcode: u8) -> [u8; SAVE_ACK_LENGTH] {
    [
        0x00,
        0x00,
        DIRECT_REPORT_ID,
        opcode,
        u8::from(opcode == 0x81) * 0xFF,
        0x00,
        0x00,
        0x00,
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use hyperx_hid::testing::MockHidTransport;

    fn captured_ab_macro() -> MacroDefinition {
        MacroDefinition {
            playback: MacroPlayback::Once,
            events: ["a", "b"]
                .into_iter()
                .flat_map(|key| {
                    [
                        MacroEvent::KeyDown {
                            key: key.to_owned(),
                            delay_ms: 20,
                        },
                        MacroEvent::KeyUp {
                            key: key.to_owned(),
                            delay_ms: 20,
                        },
                    ]
                })
                .collect(),
        }
    }

    fn captured_coverage_macro() -> MacroDefinition {
        MacroDefinition {
            playback: MacroPlayback::Once,
            events: vec![
                MacroEvent::KeyDown {
                    key: "left-shift".to_owned(),
                    delay_ms: 20,
                },
                MacroEvent::KeyDown {
                    key: "a".to_owned(),
                    delay_ms: 953,
                },
                MacroEvent::KeyUp {
                    key: "a".to_owned(),
                    delay_ms: 141,
                },
                MacroEvent::KeyUp {
                    key: "left-shift".to_owned(),
                    delay_ms: 766,
                },
                MacroEvent::KeyDown {
                    key: "left-control".to_owned(),
                    delay_ms: 1171,
                },
                MacroEvent::KeyDown {
                    key: "b".to_owned(),
                    delay_ms: 1110,
                },
                MacroEvent::KeyUp {
                    key: "b".to_owned(),
                    delay_ms: 344,
                },
                MacroEvent::KeyUp {
                    key: "left-control".to_owned(),
                    delay_ms: 453,
                },
                MacroEvent::MouseButtonDown {
                    button: "left".to_owned(),
                    delay_ms: 1890,
                },
                MacroEvent::MouseButtonUp {
                    button: "left".to_owned(),
                    delay_ms: 110,
                },
                MacroEvent::MouseButtonDown {
                    button: "right".to_owned(),
                    delay_ms: 1203,
                },
                MacroEvent::MouseButtonUp {
                    button: "right".to_owned(),
                    delay_ms: 94,
                },
                MacroEvent::MouseButtonDown {
                    button: "middle".to_owned(),
                    delay_ms: 718,
                },
                MacroEvent::MouseButtonUp {
                    button: "middle".to_owned(),
                    delay_ms: 172,
                },
            ],
        }
    }

    #[test]
    fn configuration_collection_matches_openrgb_detector() {
        assert_eq!(PULSEFIRE_RAID.configuration_interface.interface_number, 1);
        assert_eq!(PULSEFIRE_RAID.configuration_interface.usage_page, 0xFF01);
        assert_eq!(PULSEFIRE_RAID.configuration_interface.usage, 0x0001);
        assert_eq!(PULSEFIRE_RAID.capabilities.dpi, Some(PULSEFIRE_RAID_DPI));
        assert_eq!(PULSEFIRE_RAID.capabilities.polling_rates, POLLING_RATES);
    }

    #[test]
    fn driver_sends_exact_direct_rgb_feature_report() {
        let mut expected = [0_u8; DIRECT_REPORT_LENGTH];
        expected[..9].copy_from_slice(&[0x07, 0x0A, 0xFF, 0x80, 0x01, 0x02, 0x40, 0xFE, 0xA0]);
        let mut transport = MockHidTransport::new(1);
        transport.expect_feature_report(expected);

        let mut device = PulsefireRaid::new(transport).unwrap();
        device
            .set_volatile_direct_rgb(
                RgbColor::new(0xFF, 0x80, 0x01),
                RgbColor::new(0x02, 0x40, 0xFE),
            )
            .unwrap();
        device.into_transport().assert_drained();
    }

    #[test]
    fn driver_reads_runtime_profile_with_exact_sequence_and_timing() {
        let prelude = encode_runtime_profile_read_prelude();
        let request = encode_profile_read_request();
        let mut response = [0_u8; DIRECT_REPORT_LENGTH];
        response[..3].copy_from_slice(&[0x07, 0x81, 0x04]);

        let mut transport = MockHidTransport::new(1);
        transport.expect_feature_report(prelude);
        transport.expect_feature_report(request);
        transport.queue_feature_response(response);

        let mut device = PulsefireRaid::new(transport).unwrap();
        let mut waits = Vec::new();
        let profile = device
            .runtime_profile_with_wait(|duration| waits.push(duration))
            .unwrap();

        assert_eq!(profile.kind(), ProfileImageKind::DeviceReadResponse);
        assert_eq!(profile.section(), ProfileSection::Runtime);
        assert_eq!(waits, [PROFILE_PRELUDE_DELAY, PROFILE_READ_DELAY]);
        device.into_transport().assert_drained();
    }

    fn performance_profile_response() -> [u8; DIRECT_REPORT_LENGTH] {
        let mut response = [0_u8; DIRECT_REPORT_LENGTH];
        response[..3].copy_from_slice(&[0x07, 0x81, 0x04]);
        response[0x18] = 0x02;
        response[0x19..0x1B].copy_from_slice(&[0x00, 0x10]);
        response[0x25..0x27].copy_from_slice(&[0x00, 0x10]);
        response[0x32] = 0x01;
        response[0x69..0x6C].copy_from_slice(&[0x2B, 0x00, 0xFF]);
        response
    }

    fn save_profile_response(
        section: ProfileSection,
        with_macro: bool,
    ) -> [u8; DIRECT_REPORT_LENGTH] {
        let mut response = performance_profile_response();
        response[2] = match section {
            ProfileSection::Onboard => 0x01,
            ProfileSection::Runtime => 0x04,
        };
        response[0x7C..0xA8].copy_from_slice(&[
            0x02, 0xF0, 0x00, 0x00, // left click
            0x02, 0xF2, 0x00, 0x00, // right click
            0x02, 0xF1, 0x00, 0x00, // middle click
            0x02, 0xF8, 0x00, 0x03, // Button 4: Back
            0x02, 0xF9, 0x00, 0x04, // Button 5: Forward
            0x04, 0x00, 0x00, 0xE9, // Button 7: Volume Up
            0x04, 0x00, 0x00, 0xEA, // Button 6: Volume Down
            0x04, 0x00, 0x00, 0xE2, // Button 8: Mute
            0x71, 0xF0, 0x00, 0x00, // DPI toggle
            0x02, 0xF5, 0x00, 0x00, // wheel tilt left
            0x02, 0xF6, 0x00, 0x00, // wheel tilt right
        ]);
        if with_macro {
            response[0x8C..0x90].copy_from_slice(&[0x53, 0x00, 0x00, 0x04]);
        }
        response
    }

    fn expect_acked_feature(
        configuration: &mut MockHidTransport,
        acknowledgements: &mut MockHidTransport,
        report: [u8; DIRECT_REPORT_LENGTH],
    ) {
        let opcode = report[1];
        configuration.expect_feature_report(report);
        acknowledgements.queue_input_report(save_acknowledgement(opcode));
    }

    fn script_onboard_save(
        configuration: &mut MockHidTransport,
        acknowledgements: &mut MockHidTransport,
        runtime: [u8; DIRECT_REPORT_LENGTH],
        onboard: [u8; DIRECT_REPORT_LENGTH],
        wheel: RgbColor,
        logo: RgbColor,
        macro_definition: Option<&MacroDefinition>,
    ) {
        expect_acked_feature(
            configuration,
            acknowledgements,
            encode_runtime_profile_read_prelude(),
        );
        expect_acked_feature(
            configuration,
            acknowledgements,
            encode_profile_read_request(),
        );
        configuration.queue_feature_response(runtime);

        expect_acked_feature(
            configuration,
            acknowledgements,
            encode_onboard_profile_read_prelude(),
        );
        for report in encode_onboard_static_lighting_reports(wheel, logo) {
            expect_acked_feature(configuration, acknowledgements, report);
        }
        expect_acked_feature(
            configuration,
            acknowledgements,
            encode_profile_read_request(),
        );
        configuration.queue_feature_response(onboard);

        let runtime_profile = PerformanceProfile::parse(&runtime).unwrap();
        let mut onboard_profile = PerformanceProfile::parse(&onboard).unwrap();
        onboard_profile
            .copy_confirmed_runtime_settings_from(&runtime_profile)
            .unwrap();
        if let Some(definition) = macro_definition {
            let macro_report =
                encode_macro_definition(PulsefireRaidControl::Button5, definition).unwrap();
            expect_acked_feature(
                configuration,
                acknowledgements,
                macro_report.to_onboard_report().unwrap(),
            );
        }
        expect_acked_feature(
            configuration,
            acknowledgements,
            onboard_profile.to_write_report(),
        );
        expect_acked_feature(
            configuration,
            acknowledgements,
            encode_runtime_profile_read_prelude(),
        );
    }

    fn two_stage_profile_response() -> [u8; DIRECT_REPORT_LENGTH] {
        let mut response = performance_profile_response();
        response[0x1B..0x1D].copy_from_slice(&[0x00, 0x20]);
        response[0x27..0x29].copy_from_slice(&[0x00, 0x20]);
        response[0x33] = 0x01;
        response[0x6C..0x6F].copy_from_slice(&[0xCD, 0x00, 0xFF]);
        response
    }

    fn five_stage_profile_response() -> [u8; DIRECT_REPORT_LENGTH] {
        let mut response = performance_profile_response();
        response[0x1B..0x23].copy_from_slice(&[0x00, 0x20, 0x00, 0x40, 0x00, 0x80, 0x01, 0x40]);
        response[0x27..0x2F].copy_from_slice(&[0x00, 0x20, 0x00, 0x40, 0x00, 0x80, 0x01, 0x40]);
        response[0x32..0x37].fill(0x01);
        response
    }

    #[test]
    fn driver_saves_confirmed_runtime_fields_with_exact_acked_sequence() {
        let runtime = save_profile_response(ProfileSection::Runtime, false);
        let mut onboard = save_profile_response(ProfileSection::Onboard, false);
        onboard[0x18] = 0x08;
        onboard[0x19..0x1B].copy_from_slice(&[0x00, 0x20]);
        onboard[0x25..0x27].copy_from_slice(&[0x00, 0x20]);
        onboard[0x10] = 0xA5;
        let wheel = RgbColor::new(0xFF, 0x00, 0x00);
        let logo = RgbColor::BLACK;

        let mut configuration = MockHidTransport::new(1);
        let mut acknowledgements = MockHidTransport::new(2);
        script_onboard_save(
            &mut configuration,
            &mut acknowledgements,
            runtime,
            onboard,
            wheel,
            logo,
            None,
        );

        let mut device = PulsefireRaid::new(configuration).unwrap();
        let mut waits = Vec::new();
        let result = device
            .save_runtime_to_onboard_with_wait(
                &mut acknowledgements,
                wheel,
                logo,
                None,
                |duration| waits.push(duration),
            )
            .unwrap();

        assert_eq!(result.polling_rate, PollingRate::Hz500);
        assert_eq!(result.dpi_profile.stages[0].x, 800);
        assert!(!result.macro_saved);
        assert_eq!(
            waits,
            [PROFILE_READ_DELAY, PROFILE_READ_DELAY, ONBOARD_COMMIT_DELAY]
        );
        device.into_transport().assert_drained();
        acknowledgements.assert_drained();
    }

    #[test]
    fn driver_saves_capture_backed_macro_in_onboard_section() {
        let runtime = save_profile_response(ProfileSection::Runtime, true);
        let onboard = save_profile_response(ProfileSection::Onboard, false);
        let macro_definition = captured_coverage_macro();
        let wheel = RgbColor::BLACK;
        let logo = RgbColor::new(0x00, 0x00, 0xFF);

        let mut configuration = MockHidTransport::new(1);
        let mut acknowledgements = MockHidTransport::new(2);
        script_onboard_save(
            &mut configuration,
            &mut acknowledgements,
            runtime,
            onboard,
            wheel,
            logo,
            Some(&macro_definition),
        );

        let mut device = PulsefireRaid::new(configuration).unwrap();
        let result = device
            .save_runtime_to_onboard_with_wait(
                &mut acknowledgements,
                wheel,
                logo,
                Some(&macro_definition),
                |_| {},
            )
            .unwrap();
        assert!(result.macro_saved);
        device.into_transport().assert_drained();
        acknowledgements.assert_drained();
    }

    #[test]
    fn driver_refuses_missing_macro_before_selecting_onboard_memory() {
        let runtime = save_profile_response(ProfileSection::Runtime, true);
        let mut configuration = MockHidTransport::new(1);
        let mut acknowledgements = MockHidTransport::new(2);
        expect_acked_feature(
            &mut configuration,
            &mut acknowledgements,
            encode_runtime_profile_read_prelude(),
        );
        expect_acked_feature(
            &mut configuration,
            &mut acknowledgements,
            encode_profile_read_request(),
        );
        configuration.queue_feature_response(runtime);

        let mut device = PulsefireRaid::new(configuration).unwrap();
        assert!(matches!(
            device.save_runtime_to_onboard_with_wait(
                &mut acknowledgements,
                RgbColor::BLACK,
                RgbColor::BLACK,
                None,
                |_| {}
            ),
            Err(PulsefireRaidError::MissingOnboardMacroDefinition)
        ));
        device.into_transport().assert_drained();
        acknowledgements.assert_drained();
    }

    #[test]
    fn driver_refuses_button4_macro_before_selecting_onboard_memory() {
        let mut runtime = save_profile_response(ProfileSection::Runtime, true);
        runtime[0x88..0x8C].copy_from_slice(&[0x53, 0x00, 0x00, 0x03]);
        let mut configuration = MockHidTransport::new(1);
        let mut acknowledgements = MockHidTransport::new(2);
        expect_acked_feature(
            &mut configuration,
            &mut acknowledgements,
            encode_runtime_profile_read_prelude(),
        );
        expect_acked_feature(
            &mut configuration,
            &mut acknowledgements,
            encode_profile_read_request(),
        );
        configuration.queue_feature_response(runtime);

        let macro_definition = captured_coverage_macro();
        let mut device = PulsefireRaid::new(configuration).unwrap();
        assert!(matches!(
            device.save_runtime_to_onboard_with_wait(
                &mut acknowledgements,
                RgbColor::BLACK,
                RgbColor::BLACK,
                Some(&macro_definition),
                |_| {}
            ),
            Err(PulsefireRaidError::InvalidProfile(
                PerformanceProfileError::UnconfirmedOnboardMacroControl(
                    PulsefireRaidControl::Button4
                )
            ))
        ));
        device.into_transport().assert_drained();
        acknowledgements.assert_drained();
    }

    #[test]
    fn driver_aborts_on_an_unexpected_save_acknowledgement() {
        let mut configuration = MockHidTransport::new(1);
        configuration.expect_feature_report(encode_runtime_profile_read_prelude());
        let mut acknowledgements = MockHidTransport::new(2);
        acknowledgements.queue_input_report([0x00; SAVE_ACK_LENGTH]);

        let mut device = PulsefireRaid::new(configuration).unwrap();
        assert!(matches!(
            device.save_runtime_to_onboard_with_wait(
                &mut acknowledgements,
                RgbColor::BLACK,
                RgbColor::BLACK,
                None,
                |_| {}
            ),
            Err(PulsefireRaidError::UnexpectedSaveAcknowledgement { opcode: 0x03, .. })
        ));
        device.into_transport().assert_drained();
        acknowledgements.assert_drained();
    }

    #[test]
    fn driver_rejects_the_wrong_acknowledgement_interface_before_io() {
        let configuration = MockHidTransport::new(1);
        let mut wrong_acknowledgements = MockHidTransport::new(1);
        let mut device = PulsefireRaid::new(configuration).unwrap();

        assert!(matches!(
            device.save_runtime_to_onboard_with_wait(
                &mut wrong_acknowledgements,
                RgbColor::BLACK,
                RgbColor::BLACK,
                None,
                |_| {}
            ),
            Err(PulsefireRaidError::WrongAcknowledgementInterface(1))
        ));
        device.into_transport().assert_drained();
        wrong_acknowledgements.assert_drained();
    }

    #[test]
    fn driver_changes_only_active_dpi_fields_and_write_opcode() {
        let response = performance_profile_response();
        let mut expected_write = response;
        expected_write[1] = 0x01;
        expected_write[0x19..0x1B].copy_from_slice(&[0x00, 0x12]);
        expected_write[0x25..0x27].copy_from_slice(&[0x00, 0x12]);

        let mut transport = MockHidTransport::new(1);
        transport.expect_feature_report(encode_runtime_profile_read_prelude());
        transport.expect_feature_report(encode_profile_read_request());
        transport.queue_feature_response(response);
        transport.expect_feature_report(expected_write);

        let mut device = PulsefireRaid::new(transport).unwrap();
        let dpi_profile = device
            .set_runtime_active_dpi_with_wait(900, |_| {})
            .unwrap();

        assert_eq!(dpi_profile.active().unwrap().x, 900);
        assert_eq!(dpi_profile.active().unwrap().y, 900);
        device.into_transport().assert_drained();
    }

    #[test]
    fn driver_rejects_invalid_dpi_before_any_hid_io() {
        let transport = MockHidTransport::new(1);
        let mut device = PulsefireRaid::new(transport).unwrap();

        assert!(matches!(
            device.set_runtime_active_dpi_with_wait(225, |_| {}),
            Err(PulsefireRaidError::InvalidDpi(
                DpiValidationError::NotStepAligned { dpi: 225, .. }
            ))
        ));
        device.into_transport().assert_drained();
    }

    #[test]
    fn driver_updates_one_dpi_stage_and_preserves_other_fields() {
        let response = two_stage_profile_response();
        let mut expected_write = response;
        expected_write[1] = 0x01;
        expected_write[0x1B..0x1D].copy_from_slice(&[0x00, 0x22]);
        expected_write[0x27..0x29].copy_from_slice(&[0x00, 0x22]);
        expected_write[0x31] = 0x01;
        expected_write[0x6C..0x6F].copy_from_slice(&[0x01, 0x02, 0x03]);

        let mut transport = MockHidTransport::new(1);
        transport.expect_feature_report(encode_runtime_profile_read_prelude());
        transport.expect_feature_report(encode_profile_read_request());
        transport.queue_feature_response(response);
        transport.expect_feature_report(expected_write);

        let mut device = PulsefireRaid::new(transport).unwrap();
        let dpi_profile = device
            .set_runtime_dpi_stage_with_wait(
                1,
                Some(1700),
                Some(RgbColor::new(1, 2, 3)),
                true,
                |_| {},
            )
            .unwrap();

        assert_eq!(dpi_profile.active_stage, 1);
        assert_eq!(dpi_profile.stages[1].x, 1700);
        assert_eq!(dpi_profile.stages[1].y, 1700);
        assert_eq!(dpi_profile.stages[1].color, RgbColor::new(1, 2, 3));
        device.into_transport().assert_drained();
    }

    #[test]
    fn driver_adds_a_contiguous_dpi_stage() {
        let response = performance_profile_response();
        let mut expected_write = response;
        expected_write[1] = 0x01;
        expected_write[0x1B..0x1D].copy_from_slice(&[0x00, 0x20]);
        expected_write[0x27..0x29].copy_from_slice(&[0x00, 0x20]);
        expected_write[0x31] = 0x01;
        expected_write[0x33] = 0x01;
        expected_write[0x6C..0x6F].copy_from_slice(&[0xFF, 0x00, 0x00]);

        let mut transport = MockHidTransport::new(1);
        transport.expect_feature_report(encode_runtime_profile_read_prelude());
        transport.expect_feature_report(encode_profile_read_request());
        transport.queue_feature_response(response);
        transport.expect_feature_report(expected_write);

        let mut device = PulsefireRaid::new(transport).unwrap();
        let dpi_profile = device
            .add_runtime_dpi_stage_with_wait(1600, RgbColor::new(0xFF, 0, 0), true, |_| {})
            .unwrap();

        assert_eq!(dpi_profile.stages.len(), 2);
        assert_eq!(dpi_profile.active_stage, 1);
        device.into_transport().assert_drained();
    }

    #[test]
    fn driver_removes_and_clears_only_the_final_dpi_stage() {
        let mut response = two_stage_profile_response();
        response[0x31] = 0x01;
        let mut expected_write = response;
        expected_write[1] = 0x01;
        expected_write[0x1B..0x1D].fill(0);
        expected_write[0x27..0x29].fill(0);
        expected_write[0x31] = 0x00;
        expected_write[0x33] = 0x00;
        expected_write[0x6C..0x6F].fill(0);

        let mut transport = MockHidTransport::new(1);
        transport.expect_feature_report(encode_runtime_profile_read_prelude());
        transport.expect_feature_report(encode_profile_read_request());
        transport.queue_feature_response(response);
        transport.expect_feature_report(expected_write);

        let mut device = PulsefireRaid::new(transport).unwrap();
        let dpi_profile = device
            .remove_runtime_last_dpi_stage_with_wait(|_| {})
            .unwrap();

        assert_eq!(dpi_profile.stages.len(), 1);
        assert_eq!(dpi_profile.active_stage, 0);
        device.into_transport().assert_drained();
    }

    #[test]
    fn driver_rejects_missing_or_empty_dpi_stage_updates() {
        let transport = MockHidTransport::new(1);
        let mut device = PulsefireRaid::new(transport).unwrap();
        assert!(matches!(
            device.set_runtime_dpi_stage_with_wait(0, None, None, false, |_| {}),
            Err(PulsefireRaidError::EmptyDpiStageUpdate)
        ));
        device.into_transport().assert_drained();

        let response = performance_profile_response();
        let mut transport = MockHidTransport::new(1);
        transport.expect_feature_report(encode_runtime_profile_read_prelude());
        transport.expect_feature_report(encode_profile_read_request());
        transport.queue_feature_response(response);
        let mut device = PulsefireRaid::new(transport).unwrap();
        assert!(matches!(
            device.set_runtime_dpi_stage_with_wait(1, Some(1600), None, false, |_| {}),
            Err(PulsefireRaidError::DpiStageNotFound {
                stage: 2,
                stage_count: 1
            })
        ));
        device.into_transport().assert_drained();
    }

    #[test]
    fn driver_rejects_stage_count_overflow_and_removing_the_only_stage() {
        let response = five_stage_profile_response();
        let mut transport = MockHidTransport::new(1);
        transport.expect_feature_report(encode_runtime_profile_read_prelude());
        transport.expect_feature_report(encode_profile_read_request());
        transport.queue_feature_response(response);
        let mut device = PulsefireRaid::new(transport).unwrap();
        assert!(matches!(
            device.add_runtime_dpi_stage_with_wait(16000, RgbColor::BLACK, false, |_| {}),
            Err(PulsefireRaidError::TooManyDpiStages(5))
        ));
        device.into_transport().assert_drained();

        let response = performance_profile_response();
        let mut transport = MockHidTransport::new(1);
        transport.expect_feature_report(encode_runtime_profile_read_prelude());
        transport.expect_feature_report(encode_profile_read_request());
        transport.queue_feature_response(response);
        let mut device = PulsefireRaid::new(transport).unwrap();
        assert!(matches!(
            device.remove_runtime_last_dpi_stage_with_wait(|_| {}),
            Err(PulsefireRaidError::CannotRemoveOnlyDpiStage)
        ));
        device.into_transport().assert_drained();
    }

    #[test]
    fn driver_writes_only_the_captured_button5_record_for_an_ordinary_assignment() {
        let mut response = performance_profile_response();
        response[0x8C..0x90].copy_from_slice(&[0x53, 0x00, 0x00, 0x04]);
        let mut expected_write = response;
        expected_write[1] = 0x01;
        expected_write[0x8C..0x90].copy_from_slice(&[0x02, 0xF9, 0x00, 0x04]);

        let mut transport = MockHidTransport::new(1);
        transport.expect_feature_report(encode_runtime_profile_read_prelude());
        transport.expect_feature_report(encode_profile_read_request());
        transport.queue_feature_response(response);
        transport.expect_feature_report(expected_write);

        let assignment = PulsefireRaidRuntimeAssignment::ordinary(
            PulsefireRaidControl::Button5,
            ButtonBinding::Mouse(MouseFunction::Forward),
        )
        .unwrap();
        let mut device = PulsefireRaid::new(transport).unwrap();
        assert_eq!(
            device
                .set_runtime_button_assignment_with_wait(assignment.clone(), |_| {})
                .unwrap(),
            assignment
        );
        device.into_transport().assert_drained();
    }

    #[test]
    fn driver_swaps_primary_buttons_as_one_runtime_profile_write() {
        let mut response = performance_profile_response();
        response[0x7C..0x84].copy_from_slice(&[
            0x02, 0xF0, 0x00, 0x00, // physical left: Left Click
            0x02, 0xF2, 0x00, 0x02, // physical right: Right Click
        ]);
        let mut expected_write = response;
        expected_write[1] = 0x01;
        expected_write[0x7C..0x84].copy_from_slice(&[
            0x02, 0xF2, 0x00, 0x02, // physical left: Right Click
            0x02, 0xF0, 0x00, 0x00, // physical right: Left Click
        ]);

        let mut transport = MockHidTransport::new(1);
        transport.expect_feature_report(encode_runtime_profile_read_prelude());
        transport.expect_feature_report(encode_profile_read_request());
        transport.queue_feature_response(response);
        transport.expect_feature_report(expected_write);

        let mut device = PulsefireRaid::new(transport).unwrap();
        assert_eq!(
            device
                .set_runtime_primary_button_layout_with_wait(PrimaryButtonLayout::Swapped, |_| {})
                .unwrap(),
            PrimaryButtonLayout::Swapped
        );
        device.into_transport().assert_drained();
    }

    #[test]
    fn driver_writes_only_the_captured_button4_scroll_up_record() {
        let mut response = performance_profile_response();
        response[0x88..0x8C].copy_from_slice(&[0x02, 0xF8, 0x00, 0x03]);
        assert_eq!(&response[0x88..0x8C], &[0x02, 0xF8, 0x00, 0x03]);
        let mut expected_write = response;
        expected_write[1] = 0x01;
        expected_write[0x88..0x8C].copy_from_slice(&[0x02, 0xF4, 0x00, 0x00]);

        let mut transport = MockHidTransport::new(1);
        transport.expect_feature_report(encode_runtime_profile_read_prelude());
        transport.expect_feature_report(encode_profile_read_request());
        transport.queue_feature_response(response);
        transport.expect_feature_report(expected_write);

        let assignment = PulsefireRaidRuntimeAssignment::ordinary(
            PulsefireRaidControl::Button4,
            ButtonBinding::Mouse(MouseFunction::ScrollUp),
        )
        .unwrap();
        let mut device = PulsefireRaid::new(transport).unwrap();
        assert_eq!(
            device
                .set_runtime_button_assignment_with_wait(assignment.clone(), |_| {})
                .unwrap(),
            assignment
        );
        device.into_transport().assert_drained();
    }

    #[test]
    fn driver_writes_only_the_captured_button4_play_pause_record() {
        let mut response = performance_profile_response();
        response[0x88..0x8C].copy_from_slice(&[0x02, 0xF8, 0x00, 0x03]);
        let mut expected_write = response;
        expected_write[1] = 0x01;
        expected_write[0x88..0x8C].copy_from_slice(&[0x04, 0x00, 0x00, 0xCD]);

        let mut transport = MockHidTransport::new(1);
        transport.expect_feature_report(encode_runtime_profile_read_prelude());
        transport.expect_feature_report(encode_profile_read_request());
        transport.queue_feature_response(response);
        transport.expect_feature_report(expected_write);

        let assignment = PulsefireRaidRuntimeAssignment::ordinary(
            PulsefireRaidControl::Button4,
            ButtonBinding::Multimedia(MultimediaFunction::PlayPause),
        )
        .unwrap();
        let mut device = PulsefireRaid::new(transport).unwrap();
        assert_eq!(
            device
                .set_runtime_button_assignment_with_wait(assignment.clone(), |_| {})
                .unwrap(),
            assignment
        );
        device.into_transport().assert_drained();
    }

    #[test]
    fn driver_writes_only_the_captured_button4_cycle_apps_record() {
        let mut response = performance_profile_response();
        response[0x88..0x8C].copy_from_slice(&[0x02, 0xF8, 0x00, 0x03]);
        let mut expected_write = response;
        expected_write[1] = 0x01;
        expected_write[0x88..0x8C].copy_from_slice(&[0x23, 0xE3, 0x2B, 0x00]);

        let mut transport = MockHidTransport::new(1);
        transport.expect_feature_report(encode_runtime_profile_read_prelude());
        transport.expect_feature_report(encode_profile_read_request());
        transport.queue_feature_response(response);
        transport.expect_feature_report(expected_write);

        let assignment = PulsefireRaidRuntimeAssignment::ordinary(
            PulsefireRaidControl::Button4,
            ButtonBinding::WindowsShortcut(WindowsShortcut::CycleApps),
        )
        .unwrap();
        let mut device = PulsefireRaid::new(transport).unwrap();
        assert_eq!(
            device
                .set_runtime_button_assignment_with_wait(assignment.clone(), |_| {})
                .unwrap(),
            assignment
        );
        device.into_transport().assert_drained();
    }

    #[test]
    fn driver_writes_only_the_captured_dpi_control_record() {
        let mut response = performance_profile_response();
        response[0x9C..0xA0].copy_from_slice(&[0x00, 0x04, 0x00, 0x00]);
        let mut expected_write = response;
        expected_write[1] = 0x01;
        expected_write[0x9C..0xA0].copy_from_slice(&[0x71, 0xF0, 0x00, 0x00]);

        let mut transport = MockHidTransport::new(1);
        transport.expect_feature_report(encode_runtime_profile_read_prelude());
        transport.expect_feature_report(encode_profile_read_request());
        transport.queue_feature_response(response);
        transport.expect_feature_report(expected_write);

        let assignment = PulsefireRaidRuntimeAssignment::ordinary(
            PulsefireRaidControl::Dpi,
            ButtonBinding::Mouse(MouseFunction::DpiToggle),
        )
        .unwrap();
        let mut device = PulsefireRaid::new(transport).unwrap();
        assert_eq!(
            device
                .set_runtime_button_assignment_with_wait(assignment.clone(), |_| {})
                .unwrap(),
            assignment
        );
        device.into_transport().assert_drained();
    }

    #[test]
    fn driver_writes_only_the_captured_button7_record() {
        let mut response = performance_profile_response();
        response[0x90..0x94].copy_from_slice(&[0x04, 0x00, 0x00, 0xE9]);
        let mut expected_write = response;
        expected_write[1] = 0x01;
        expected_write[0x90..0x94].copy_from_slice(&[0x04, 0x00, 0x00, 0xEA]);

        let mut transport = MockHidTransport::new(1);
        transport.expect_feature_report(encode_runtime_profile_read_prelude());
        transport.expect_feature_report(encode_profile_read_request());
        transport.queue_feature_response(response);
        transport.expect_feature_report(expected_write);

        let assignment = PulsefireRaidRuntimeAssignment::ordinary(
            PulsefireRaidControl::Button7,
            ButtonBinding::Multimedia(MultimediaFunction::VolumeDown),
        )
        .unwrap();
        let mut device = PulsefireRaid::new(transport).unwrap();
        assert_eq!(
            device
                .set_runtime_button_assignment_with_wait(assignment.clone(), |_| {})
                .unwrap(),
            assignment
        );
        device.into_transport().assert_drained();
    }

    #[test]
    fn driver_applies_a_portable_record_to_another_general_control() {
        let mut response = performance_profile_response();
        response[0x94..0x98].copy_from_slice(&[0x04, 0x00, 0x00, 0xEA]);
        let mut expected_write = response;
        expected_write[1] = 0x01;
        expected_write[0x94..0x98].copy_from_slice(&[0x02, 0xF8, 0x00, 0x03]);

        let mut transport = MockHidTransport::new(1);
        transport.expect_feature_report(encode_runtime_profile_read_prelude());
        transport.expect_feature_report(encode_profile_read_request());
        transport.queue_feature_response(response);
        transport.expect_feature_report(expected_write);

        let assignment = PulsefireRaidRuntimeAssignment::ordinary(
            PulsefireRaidControl::Button6,
            ButtonBinding::Mouse(MouseFunction::Back),
        )
        .unwrap();
        let mut device = PulsefireRaid::new(transport).unwrap();
        assert_eq!(
            device
                .set_runtime_button_assignment_with_wait(assignment.clone(), |_| {})
                .unwrap(),
            assignment
        );
        device.into_transport().assert_drained();
    }

    #[test]
    fn driver_sends_captured_chord_mouse_macro_before_its_button5_profile_reference() {
        let mut response = performance_profile_response();
        response[0x8C..0x90].copy_from_slice(&[0x02, 0xF9, 0x00, 0x04]);

        let mut expected_macro = [0_u8; DIRECT_REPORT_LENGTH];
        expected_macro[..52].copy_from_slice(&[
            0x07, 0x05, 0x04, 0x04, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0x80, 0x14, 0xE1, 0x83,
            0xB9, 0x04, 0x00, 0x8D, 0x04, 0x02, 0xFE, 0xE1, 0x84, 0x93, 0xE0, 0x84, 0x56, 0x05,
            0x01, 0x58, 0x05, 0x01, 0xC5, 0xE0, 0x87, 0x62, 0xB7, 0x00, 0x6E, 0xB7, 0x84, 0xB3,
            0xB8, 0x00, 0x5E, 0xB8, 0x82, 0xCE, 0xB9, 0x00, 0xAC, 0xB9,
        ]);
        let mut expected_write = response;
        expected_write[1] = 0x01;
        expected_write[0x8C..0x90].copy_from_slice(&[0x53, 0x00, 0x00, 0x04]);

        let mut transport = MockHidTransport::new(1);
        transport.expect_feature_report(encode_runtime_profile_read_prelude());
        transport.expect_feature_report(encode_profile_read_request());
        transport.queue_feature_response(response);
        transport.expect_feature_report(expected_macro);
        transport.expect_feature_report(expected_write);

        let assignment = PulsefireRaidRuntimeAssignment::macro_timeline(
            PulsefireRaidControl::Button5,
            captured_coverage_macro(),
        )
        .unwrap();
        let mut device = PulsefireRaid::new(transport).unwrap();
        assert_eq!(
            device
                .set_runtime_button_assignment_with_wait(assignment.clone(), |_| {})
                .unwrap(),
            assignment
        );
        device.into_transport().assert_drained();
    }

    #[test]
    fn driver_sends_captured_button4_ab_macro_without_changing_button5() {
        let response = save_profile_response(ProfileSection::Runtime, true);
        let mut expected_macro = [0_u8; DIRECT_REPORT_LENGTH];
        expected_macro[..22].copy_from_slice(&[
            0x07, 0x05, 0x04, 0x03, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0x80, 0x14, 0x04, 0x00,
            0x14, 0x04, 0x80, 0x14, 0x05, 0x00, 0x14, 0x05,
        ]);
        let mut expected_write = response;
        expected_write[1] = 0x01;
        expected_write[0x88..0x8C].copy_from_slice(&[0x53, 0x00, 0x00, 0x03]);
        assert_eq!(&expected_write[0x8C..0x90], &[0x53, 0x00, 0x00, 0x04]);

        let mut transport = MockHidTransport::new(1);
        transport.expect_feature_report(encode_runtime_profile_read_prelude());
        transport.expect_feature_report(encode_profile_read_request());
        transport.queue_feature_response(response);
        transport.expect_feature_report(expected_macro);
        transport.expect_feature_report(expected_write);

        let assignment = PulsefireRaidRuntimeAssignment::macro_timeline(
            PulsefireRaidControl::Button4,
            captured_ab_macro(),
        )
        .unwrap();
        let mut device = PulsefireRaid::new(transport).unwrap();
        assert_eq!(
            device
                .set_runtime_button_assignment_with_wait(assignment.clone(), |_| {})
                .unwrap(),
            assignment
        );
        device.into_transport().assert_drained();
    }

    #[test]
    fn runtime_assignment_gate_separates_portable_and_target_specific_evidence() {
        let general_controls = [
            PulsefireRaidControl::MiddleClick,
            PulsefireRaidControl::Button4,
            PulsefireRaidControl::Button5,
            PulsefireRaidControl::Button7,
            PulsefireRaidControl::Button6,
            PulsefireRaidControl::Button8,
            PulsefireRaidControl::Dpi,
            PulsefireRaidControl::WheelTiltLeft,
            PulsefireRaidControl::WheelTiltRight,
        ];
        let portable_bindings = [
            ButtonBinding::Disabled,
            ButtonBinding::Mouse(MouseFunction::LeftClick),
            ButtonBinding::Mouse(MouseFunction::RightClick),
            ButtonBinding::Mouse(MouseFunction::MiddleClick),
            ButtonBinding::Mouse(MouseFunction::Back),
            ButtonBinding::Mouse(MouseFunction::Forward),
            ButtonBinding::Mouse(MouseFunction::TiltLeft),
            ButtonBinding::Mouse(MouseFunction::TiltRight),
            ButtonBinding::Mouse(MouseFunction::DpiToggle),
            ButtonBinding::Mouse(MouseFunction::ScrollUp),
            ButtonBinding::Mouse(MouseFunction::ScrollDown),
            ButtonBinding::Multimedia(MultimediaFunction::PlayPause),
            ButtonBinding::Multimedia(MultimediaFunction::Stop),
            ButtonBinding::Multimedia(MultimediaFunction::NextTrack),
            ButtonBinding::Multimedia(MultimediaFunction::PreviousTrack),
            ButtonBinding::Multimedia(MultimediaFunction::MuteVolume),
            ButtonBinding::Multimedia(MultimediaFunction::VolumeUp),
            ButtonBinding::Multimedia(MultimediaFunction::VolumeDown),
            ButtonBinding::WindowsShortcut(WindowsShortcut::CycleApps),
            ButtonBinding::WindowsShortcut(WindowsShortcut::SwitchApps),
            ButtonBinding::WindowsShortcut(WindowsShortcut::Cut),
            ButtonBinding::WindowsShortcut(WindowsShortcut::Copy),
            ButtonBinding::WindowsShortcut(WindowsShortcut::Paste),
            ButtonBinding::WindowsShortcut(WindowsShortcut::Undo),
            ButtonBinding::Keyboard(KeyboardUsage(0x04)),
            ButtonBinding::Keyboard(KeyboardUsage(0x05)),
            ButtonBinding::Keyboard(KeyboardUsage(0x2C)),
            ButtonBinding::Keyboard(KeyboardUsage(0xE1)),
        ];
        for control in general_controls {
            for binding in portable_bindings.iter().cloned() {
                assert!(PulsefireRaidRuntimeAssignment::ordinary(control, binding).is_ok());
            }
        }
        for (control, binding) in [
            (PulsefireRaidControl::LeftClick, ButtonBinding::Disabled),
            (
                PulsefireRaidControl::Dpi,
                ButtonBinding::Keyboard(KeyboardUsage(0x74)),
            ),
        ] {
            assert!(matches!(
                PulsefireRaidRuntimeAssignment::ordinary(control, binding),
                Err(PulsefireRaidError::UnconfirmedRuntimeButtonBinding { .. })
            ));
        }

        let macro_assignment = PulsefireRaidRuntimeAssignment::macro_timeline(
            PulsefireRaidControl::Button5,
            captured_coverage_macro(),
        )
        .unwrap();
        assert!(macro_assignment.ordinary_binding().is_none());
        assert!(macro_assignment.encoded_macro().unwrap().is_some());
        assert!(PulsefireRaidRuntimeAssignment::macro_timeline(
            PulsefireRaidControl::Button4,
            captured_ab_macro(),
        )
        .is_ok());
        assert!(matches!(
            PulsefireRaidRuntimeAssignment::macro_timeline(
                PulsefireRaidControl::Dpi,
                captured_coverage_macro(),
            ),
            Err(PulsefireRaidError::UnconfirmedMacroControl(
                PulsefireRaidControl::Dpi
            ))
        ));

        let mut unsupported_playback = captured_coverage_macro();
        unsupported_playback.playback = MacroPlayback::ToggleRepeat;
        assert!(matches!(
            PulsefireRaidRuntimeAssignment::macro_timeline(
                PulsefireRaidControl::Button5,
                unsupported_playback,
            ),
            Err(PulsefireRaidError::UnsupportedMacroPlayback(
                MacroPlayback::ToggleRepeat
            ))
        ));
    }

    #[test]
    fn driver_changes_only_polling_field_and_write_opcode() {
        let response = performance_profile_response();
        let mut expected_write = response;
        expected_write[1] = 0x01;
        expected_write[0x18] = 0x01;

        let mut transport = MockHidTransport::new(1);
        transport.expect_feature_report(encode_runtime_profile_read_prelude());
        transport.expect_feature_report(encode_profile_read_request());
        transport.queue_feature_response(response);
        transport.expect_feature_report(expected_write);

        let mut device = PulsefireRaid::new(transport).unwrap();
        assert_eq!(
            device
                .set_runtime_polling_rate_with_wait(PollingRate::Hz1000, |_| {})
                .unwrap(),
            PollingRate::Hz1000
        );
        device.into_transport().assert_drained();
    }

    #[test]
    fn driver_rejects_a_truncated_profile_response() {
        let mut transport = MockHidTransport::new(1);
        transport.expect_feature_report(encode_runtime_profile_read_prelude());
        transport.expect_feature_report(encode_profile_read_request());
        transport.queue_feature_response([0x07, 0x81, 0x04]);

        let mut device = PulsefireRaid::new(transport).unwrap();
        let error = device
            .runtime_profile_with_wait(|_| {})
            .expect_err("a truncated profile must be rejected");

        assert!(matches!(
            error,
            PulsefireRaidError::WrongProfileResponseLength(3)
        ));
    }

    #[test]
    fn driver_rejects_the_standard_mouse_interface() {
        let transport = MockHidTransport::new(0);
        assert!(matches!(
            PulsefireRaid::new(transport),
            Err(PulsefireRaidError::WrongInterface(0))
        ));
    }
}
