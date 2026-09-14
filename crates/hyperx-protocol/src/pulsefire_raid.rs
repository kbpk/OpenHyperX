use hyperx_core::{
    ButtonBinding, DpiProfile, DpiStage, KeyState, KeyboardMacroEvent, KeyboardUsage,
    MouseFunction, MultimediaFunction, PollingRate, RgbColor, WindowsShortcut,
};
use thiserror::Error;

pub const DIRECT_REPORT_ID: u8 = 0x07;
pub const DIRECT_REPORT_LENGTH: usize = 264;
const DIRECT_START: u8 = 0x0A;
const DIRECT_END: u8 = 0xA0;
const PROFILE_ACCESS_OPCODE: u8 = 0x03;
const PROFILE_ACCESS_TRAILER: u8 = 0x64;

const PROFILE_WRITE_OPCODE: u8 = 0x01;
const PROFILE_READ_RESPONSE_OPCODE: u8 = 0x81;
const ONBOARD_PROFILE_SECTION: u8 = 0x01;
const RUNTIME_PROFILE_SECTION: u8 = 0x04;
const POLLING_INTERVAL_OFFSET: usize = 0x18;
const DPI_X_OFFSETS: [usize; 5] = [0x19, 0x1B, 0x1D, 0x1F, 0x21];
const DPI_Y_OFFSETS: [usize; 5] = [0x25, 0x27, 0x29, 0x2B, 0x2D];
const ACTIVE_DPI_STAGE_OFFSET: usize = 0x31;
const DPI_STAGE_ENABLED_OFFSETS: [usize; 5] = [0x32, 0x33, 0x34, 0x35, 0x36];
const DPI_STAGE_COLOR_OFFSETS: [usize; 5] = [0x69, 0x6C, 0x6F, 0x72, 0x75];
const BUTTON_RECORD_LENGTH: usize = 4;
const CONFIRMED_MACRO_HEADER: [u8; 10] =
    [0x07, 0x05, 0x04, 0x04, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01];
const CONFIRMED_MACRO_BINDING: [u8; BUTTON_RECORD_LENGTH] = [0x53, 0x00, 0x00, 0x04];
const DPI_UNIT: u32 = 50;
const MIN_DPI: u32 = 200;
const MAX_DPI: u32 = 16_000;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProfileImageKind {
    HostWrite,
    DeviceReadResponse,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProfileSection {
    Onboard,
    Runtime,
}

/// Physical controls shown in the Pulsefire Raid button editor.
///
/// Kingston's manual supplies the numbered side-button names. The profile
/// offsets were established from the locally captured runtime profile and an
/// isolated Button 5 remapping sequence.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PulsefireRaidControl {
    LeftClick,
    RightClick,
    MiddleClick,
    Button4,
    Button5,
    Button7,
    Button6,
    Button8,
    Dpi,
    WheelTiltLeft,
    WheelTiltRight,
}

impl PulsefireRaidControl {
    pub const ALL: [Self; 11] = [
        Self::LeftClick,
        Self::RightClick,
        Self::MiddleClick,
        Self::Button4,
        Self::Button5,
        Self::Button7,
        Self::Button6,
        Self::Button8,
        Self::Dpi,
        Self::WheelTiltLeft,
        Self::WheelTiltRight,
    ];

    pub const fn name(self) -> &'static str {
        match self {
            Self::LeftClick => "Left click",
            Self::RightClick => "Right click",
            Self::MiddleClick => "Wheel click",
            Self::Button4 => "Button 4",
            Self::Button5 => "Button 5",
            Self::Button7 => "Button 7",
            Self::Button6 => "Button 6",
            Self::Button8 => "Button 8",
            Self::Dpi => "DPI button",
            Self::WheelTiltLeft => "Wheel tilt left",
            Self::WheelTiltRight => "Wheel tilt right",
        }
    }

    const fn profile_offset(self) -> usize {
        match self {
            Self::LeftClick => 0x7C,
            Self::RightClick => 0x80,
            Self::MiddleClick => 0x84,
            Self::Button4 => 0x88,
            Self::Button5 => 0x8C,
            Self::Button7 => 0x90,
            Self::Button6 => 0x94,
            Self::Button8 => 0x98,
            Self::Dpi => 0x9C,
            Self::WheelTiltLeft => 0xA0,
            Self::WheelTiltRight => 0xA4,
        }
    }

    const fn is_primary_click(self) -> bool {
        matches!(self, Self::LeftClick | Self::RightClick)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PerformanceProfile {
    report: [u8; DIRECT_REPORT_LENGTH],
    kind: ProfileImageKind,
    section: ProfileSection,
}

/// The one Pulsefire Raid macro definition whose complete report is confirmed.
///
/// This intentionally represents only the independently captured Button 5
/// keyboard sequences. Macro timing lives on each press/release event even
/// when NGENUITY's Standard Timing gives every event the same value.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConfirmedMacro {
    report: [u8; DIRECT_REPORT_LENGTH],
    events: Vec<KeyboardMacroEvent>,
}

#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum ConfirmedMacroError {
    #[error("expected a 264-byte Pulsefire Raid macro report, got {0} bytes")]
    WrongLength(usize),
    #[error(
        "byte 0x{offset:02X} is not part of the confirmed minimal macro: expected 0x{expected:02X}, got 0x{actual:02X}"
    )]
    UnconfirmedByte {
        offset: usize,
        expected: u8,
        actual: u8,
    },
    #[error("keyboard macro events {0:?} have not been confirmed for Pulsefire Raid macros")]
    UnconfirmedEvents(Vec<KeyboardMacroEvent>),
    #[error("minimal macro binding is confirmed only for the runtime profile")]
    UnsupportedProfileSection,
}

#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum PerformanceProfileError {
    #[error("expected a 264-byte Pulsefire Raid profile report, got {0} bytes")]
    WrongLength(usize),
    #[error("expected report ID 0x07, got 0x{0:02X}")]
    WrongReportId(u8),
    #[error("unsupported Pulsefire Raid profile opcode 0x{0:02X}")]
    UnsupportedOpcode(u8),
    #[error("unsupported Pulsefire Raid profile section 0x{0:02X}")]
    UnsupportedSection(u8),
    #[error("unknown Pulsefire Raid polling interval code 0x{0:02X}")]
    UnknownPollingInterval(u8),
    #[error("invalid enabled flag 0x{value:02X} for DPI stage {stage}")]
    InvalidDpiStageFlag { stage: usize, value: u8 },
    #[error("enabled DPI stage {stage} follows a disabled stage")]
    NonContiguousDpiStages { stage: usize },
    #[error("Pulsefire Raid profile does not contain an enabled DPI stage")]
    NoEnabledDpiStages,
    #[error("active DPI stage {active} is outside the {stage_count} enabled stages")]
    InvalidActiveDpiStage { active: usize, stage_count: usize },
    #[error("Pulsefire Raid requires between 1 and 5 DPI stages, got {0}")]
    InvalidDpiStageCount(usize),
    #[error("DPI stage {stage} {axis:?} value {dpi} must be between 200 and 16000")]
    DpiOutOfRange {
        stage: usize,
        axis: DpiAxis,
        dpi: u32,
    },
    #[error("DPI stage {stage} {axis:?} value {dpi} must be a multiple of 50")]
    DpiNotStepAligned {
        stage: usize,
        axis: DpiAxis,
        dpi: u32,
    },
    #[error("unknown Pulsefire Raid binding record {record:02X?} for {control:?}")]
    UnknownButtonBinding {
        control: PulsefireRaidControl,
        record: [u8; BUTTON_RECORD_LENGTH],
    },
    #[error("keyboard usage 0x{0:04X} is outside the captured one-byte binding range 0x01..=0xFF")]
    KeyboardUsageOutOfRange(u16),
    #[error("binding {binding:?} is not legal for Pulsefire Raid control {control:?}")]
    ButtonBindingNotAllowed {
        control: PulsefireRaidControl,
        binding: ButtonBinding,
    },
    #[error("binding {0:?} does not yet have a capture-backed Pulsefire Raid encoding")]
    UnconfirmedButtonBinding(ButtonBinding),
}

/// Axis identifying an invalid DPI value in a profile.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DpiAxis {
    /// Horizontal sensor resolution.
    X,
    /// Vertical sensor resolution.
    Y,
}

impl PerformanceProfile {
    pub fn parse(report: &[u8]) -> Result<Self, PerformanceProfileError> {
        let report: [u8; DIRECT_REPORT_LENGTH] = report
            .try_into()
            .map_err(|_| PerformanceProfileError::WrongLength(report.len()))?;

        if report[0] != DIRECT_REPORT_ID {
            return Err(PerformanceProfileError::WrongReportId(report[0]));
        }

        let kind = match report[1] {
            PROFILE_WRITE_OPCODE => ProfileImageKind::HostWrite,
            PROFILE_READ_RESPONSE_OPCODE => ProfileImageKind::DeviceReadResponse,
            opcode => return Err(PerformanceProfileError::UnsupportedOpcode(opcode)),
        };

        let section = match report[2] {
            ONBOARD_PROFILE_SECTION => ProfileSection::Onboard,
            RUNTIME_PROFILE_SECTION => ProfileSection::Runtime,
            section => return Err(PerformanceProfileError::UnsupportedSection(section)),
        };

        Ok(Self {
            report,
            kind,
            section,
        })
    }

    pub const fn kind(&self) -> ProfileImageKind {
        self.kind
    }

    pub const fn section(&self) -> ProfileSection {
        self.section
    }

    pub fn polling_rate(&self) -> Result<PollingRate, PerformanceProfileError> {
        match self.report[POLLING_INTERVAL_OFFSET] {
            0x01 => Ok(PollingRate::Hz1000),
            0x02 => Ok(PollingRate::Hz500),
            0x04 => Ok(PollingRate::Hz250),
            0x08 => Ok(PollingRate::Hz125),
            code => Err(PerformanceProfileError::UnknownPollingInterval(code)),
        }
    }

    /// Patch the confirmed polling interval byte in an existing profile image.
    ///
    /// This is deliberately an offline operation. The device driver does not
    /// expose a profile write while the surrounding transaction is unknown.
    pub fn set_polling_rate(&mut self, polling_rate: PollingRate) {
        self.report[POLLING_INTERVAL_OFFSET] = match polling_rate {
            PollingRate::Hz1000 => 0x01,
            PollingRate::Hz500 => 0x02,
            PollingRate::Hz250 => 0x04,
            PollingRate::Hz125 => 0x08,
        };
    }

    /// Decode the enabled DPI stages and the active zero-based stage index.
    ///
    /// This is deliberately read-only. The device driver does not expose a
    /// profile write while the surrounding transaction remains unknown.
    pub fn dpi_profile(&self) -> Result<DpiProfile, PerformanceProfileError> {
        let mut stages = Vec::with_capacity(DPI_STAGE_ENABLED_OFFSETS.len());
        let mut found_disabled_stage = false;

        for (index, enabled_offset) in DPI_STAGE_ENABLED_OFFSETS.iter().copied().enumerate() {
            match self.report[enabled_offset] {
                0x00 => found_disabled_stage = true,
                0x01 if found_disabled_stage => {
                    return Err(PerformanceProfileError::NonContiguousDpiStages {
                        stage: index + 1,
                    });
                }
                0x01 => stages.push(DpiStage::new(
                    read_dpi(&self.report, DPI_X_OFFSETS[index]),
                    read_dpi(&self.report, DPI_Y_OFFSETS[index]),
                    read_color(&self.report, DPI_STAGE_COLOR_OFFSETS[index]),
                )),
                value => {
                    return Err(PerformanceProfileError::InvalidDpiStageFlag {
                        stage: index + 1,
                        value,
                    });
                }
            }
        }

        if stages.is_empty() {
            return Err(PerformanceProfileError::NoEnabledDpiStages);
        }

        let active_stage = usize::from(self.report[ACTIVE_DPI_STAGE_OFFSET]);
        if active_stage >= stages.len() {
            return Err(PerformanceProfileError::InvalidActiveDpiStage {
                active: active_stage,
                stage_count: stages.len(),
            });
        }

        Ok(DpiProfile {
            stages,
            active_stage,
        })
    }

    /// Patch all confirmed DPI fields in an existing profile image.
    ///
    /// This does not transmit anything. Values are validated before the image
    /// is changed, and fields unrelated to DPI are preserved byte-for-byte.
    pub fn set_dpi_profile(
        &mut self,
        dpi_profile: &DpiProfile,
    ) -> Result<(), PerformanceProfileError> {
        let stage_count = dpi_profile.stages.len();
        if !(1..=DPI_X_OFFSETS.len()).contains(&stage_count) {
            return Err(PerformanceProfileError::InvalidDpiStageCount(stage_count));
        }
        if dpi_profile.active_stage >= stage_count {
            return Err(PerformanceProfileError::InvalidActiveDpiStage {
                active: dpi_profile.active_stage,
                stage_count,
            });
        }

        for (index, stage) in dpi_profile.stages.iter().enumerate() {
            validate_dpi(index, DpiAxis::X, stage.x)?;
            validate_dpi(index, DpiAxis::Y, stage.y)?;
        }

        for index in 0..DPI_X_OFFSETS.len() {
            if let Some(stage) = dpi_profile.stages.get(index) {
                write_dpi(&mut self.report, DPI_X_OFFSETS[index], stage.x);
                write_dpi(&mut self.report, DPI_Y_OFFSETS[index], stage.y);
                self.report[DPI_STAGE_ENABLED_OFFSETS[index]] = 0x01;
                self.report[DPI_STAGE_COLOR_OFFSETS[index]..DPI_STAGE_COLOR_OFFSETS[index] + 3]
                    .copy_from_slice(&stage.color.bytes());
            } else {
                self.report[DPI_X_OFFSETS[index]..DPI_X_OFFSETS[index] + 2].fill(0);
                self.report[DPI_Y_OFFSETS[index]..DPI_Y_OFFSETS[index] + 2].fill(0);
                self.report[DPI_STAGE_ENABLED_OFFSETS[index]] = 0x00;
                self.report[DPI_STAGE_COLOR_OFFSETS[index]..DPI_STAGE_COLOR_OFFSETS[index] + 3]
                    .fill(0);
            }
        }
        self.report[ACTIVE_DPI_STAGE_OFFSET] = dpi_profile.active_stage as u8;

        Ok(())
    }

    /// Decode one physical control's four-byte binding record.
    pub fn button_binding(
        &self,
        control: PulsefireRaidControl,
    ) -> Result<ButtonBinding, PerformanceProfileError> {
        let offset = control.profile_offset();
        let record: [u8; BUTTON_RECORD_LENGTH] = self.report[offset..offset + BUTTON_RECORD_LENGTH]
            .try_into()
            .expect("all Pulsefire Raid button offsets fit the fixed report");

        decode_button_binding(control, record)
    }

    /// Identify the exact captured Button 5 macro reference.
    ///
    /// The runtime profile contains only this reference, not the macro event
    /// report, so callers must not infer its keys or timing from this result.
    pub fn has_confirmed_macro_reference(&self, control: PulsefireRaidControl) -> bool {
        if control != PulsefireRaidControl::Button5 {
            return false;
        }
        let offset = control.profile_offset();
        self.report[offset..offset + BUTTON_RECORD_LENGTH] == CONFIRMED_MACRO_BINDING
    }

    /// Patch one confirmed four-byte button record in an existing profile.
    ///
    /// This is an offline operation. Inferred members use standard USB HID
    /// usage IDs, but no profile produced here is sent by the device driver
    /// while the surrounding save transaction remains unresolved.
    pub fn set_button_binding(
        &mut self,
        control: PulsefireRaidControl,
        binding: &ButtonBinding,
    ) -> Result<(), PerformanceProfileError> {
        if control.is_primary_click()
            && !matches!(
                binding,
                ButtonBinding::Mouse(MouseFunction::LeftClick | MouseFunction::RightClick)
            )
        {
            return Err(PerformanceProfileError::ButtonBindingNotAllowed {
                control,
                binding: binding.clone(),
            });
        }

        let record = encode_button_binding(binding)?;
        let offset = control.profile_offset();
        self.report[offset..offset + BUTTON_RECORD_LENGTH].copy_from_slice(&record);
        Ok(())
    }

    /// Produce the confirmed host-write form without transmitting it.
    pub fn to_write_report(&self) -> [u8; DIRECT_REPORT_LENGTH] {
        let mut report = self.report;
        report[1] = PROFILE_WRITE_OPCODE;
        report
    }

    pub const fn as_bytes(&self) -> &[u8; DIRECT_REPORT_LENGTH] {
        &self.report
    }
}

impl ConfirmedMacro {
    /// Build a confirmed Button 5 keyboard sequence, Play Once report without I/O.
    ///
    /// Only exact event lists captured on the target unit are accepted. This
    /// prevents inferred keys, variable timings and UI bounds reaching a packet.
    pub fn button5_keyboard_once(
        events: &[KeyboardMacroEvent],
    ) -> Result<Self, ConfirmedMacroError> {
        if !confirmed_macro_event_lists()
            .iter()
            .any(|confirmed| confirmed == events)
        {
            return Err(ConfirmedMacroError::UnconfirmedEvents(events.to_vec()));
        }

        let mut report = [0_u8; DIRECT_REPORT_LENGTH];
        report[..CONFIRMED_MACRO_HEADER.len()].copy_from_slice(&CONFIRMED_MACRO_HEADER);
        let mut offset = CONFIRMED_MACRO_HEADER.len();

        for event in events {
            let [timing_high, timing_low] = event.timing_ms.to_be_bytes();
            let state = match event.state {
                KeyState::Pressed => 0x80,
                KeyState::Released => 0x00,
            };
            let usage = u8::try_from(event.usage.0)
                .expect("all confirmed macro keyboard usages fit in one byte");
            report[offset..offset + 3].copy_from_slice(&[state | timing_high, timing_low, usage]);
            offset += 3;
        }

        Ok(Self {
            report,
            events: events.to_vec(),
        })
    }

    /// Recognize only an exact local capture.
    ///
    /// Reports for other event lists or macro modes stay rejected until
    /// isolated differential captures establish their fields.
    pub fn parse(report: &[u8]) -> Result<Self, ConfirmedMacroError> {
        let report: [u8; DIRECT_REPORT_LENGTH] = report
            .try_into()
            .map_err(|_| ConfirmedMacroError::WrongLength(report.len()))?;

        for events in confirmed_macro_event_lists() {
            let expected = Self::button5_keyboard_once(&events)
                .expect("the confirmed macro table contains only supported events");
            if report == expected.report {
                return Ok(Self { report, events });
            }
        }

        let expected = Self::button5_keyboard_once(&keyboard_a_events(20))
            .expect("the reference macro is a confirmed configuration");
        let (offset, (&actual, &expected)) = report
            .iter()
            .zip(expected.report.iter())
            .enumerate()
            .find(|(_, (actual, expected))| actual != expected)
            .expect("an exact reference report would already have returned");
        Err(ConfirmedMacroError::UnconfirmedByte {
            offset,
            expected,
            actual,
        })
    }

    pub const fn control(&self) -> PulsefireRaidControl {
        PulsefireRaidControl::Button5
    }

    pub fn events(&self) -> &[KeyboardMacroEvent] {
        &self.events
    }

    pub const fn playback(&self) -> hyperx_core::MacroPlayback {
        hyperx_core::MacroPlayback::Once
    }

    /// Patch the exact captured macro reference into a runtime profile image.
    ///
    /// This remains offline. A complete profile transaction is not exposed by
    /// the device driver, and onboard use is deliberately rejected.
    pub fn apply_to_profile(
        &self,
        profile: &mut PerformanceProfile,
    ) -> Result<(), ConfirmedMacroError> {
        if profile.section != ProfileSection::Runtime {
            return Err(ConfirmedMacroError::UnsupportedProfileSection);
        }

        let offset = self.control().profile_offset();
        profile.report[offset..offset + BUTTON_RECORD_LENGTH]
            .copy_from_slice(&CONFIRMED_MACRO_BINDING);
        Ok(())
    }

    pub fn is_bound_in(&self, profile: &PerformanceProfile) -> bool {
        profile.has_confirmed_macro_reference(self.control())
    }

    pub const fn as_bytes(&self) -> &[u8; DIRECT_REPORT_LENGTH] {
        &self.report
    }
}

fn keyboard_a_events(timing_ms: u16) -> Vec<KeyboardMacroEvent> {
    vec![
        KeyboardMacroEvent::pressed(KeyboardUsage(0x04), timing_ms),
        KeyboardMacroEvent::released(KeyboardUsage(0x04), timing_ms),
    ]
}

fn keyboard_ab_events(timing_ms: u16) -> Vec<KeyboardMacroEvent> {
    let mut events = keyboard_a_events(timing_ms);
    events.extend([
        KeyboardMacroEvent::pressed(KeyboardUsage(0x05), timing_ms),
        KeyboardMacroEvent::released(KeyboardUsage(0x05), timing_ms),
    ]);
    events
}

fn confirmed_macro_event_lists() -> [Vec<KeyboardMacroEvent>; 3] {
    [
        keyboard_a_events(20),
        keyboard_a_events(300),
        keyboard_ab_events(20),
    ]
}

fn read_dpi(report: &[u8; DIRECT_REPORT_LENGTH], offset: usize) -> u32 {
    u32::from(u16::from_be_bytes([report[offset], report[offset + 1]])) * DPI_UNIT
}

fn read_color(report: &[u8; DIRECT_REPORT_LENGTH], offset: usize) -> RgbColor {
    RgbColor::new(report[offset], report[offset + 1], report[offset + 2])
}

fn validate_dpi(stage: usize, axis: DpiAxis, dpi: u32) -> Result<(), PerformanceProfileError> {
    if !(MIN_DPI..=MAX_DPI).contains(&dpi) {
        return Err(PerformanceProfileError::DpiOutOfRange {
            stage: stage + 1,
            axis,
            dpi,
        });
    }
    if dpi % DPI_UNIT != 0 {
        return Err(PerformanceProfileError::DpiNotStepAligned {
            stage: stage + 1,
            axis,
            dpi,
        });
    }
    Ok(())
}

fn write_dpi(report: &mut [u8; DIRECT_REPORT_LENGTH], offset: usize, dpi: u32) {
    let encoded = u16::try_from(dpi / DPI_UNIT)
        .expect("validated Pulsefire Raid DPI always fits in u16")
        .to_be_bytes();
    report[offset..offset + 2].copy_from_slice(&encoded);
}

fn decode_button_binding(
    control: PulsefireRaidControl,
    record: [u8; BUTTON_RECORD_LENGTH],
) -> Result<ButtonBinding, PerformanceProfileError> {
    let binding = match record {
        [0x00, 0x00, 0x00, 0x00] => ButtonBinding::Disabled,
        [0x00, usage, 0x00, 0x00] => ButtonBinding::Keyboard(KeyboardUsage(u16::from(usage))),
        [0x02, 0xF0, 0x00, 0x00] => ButtonBinding::Mouse(MouseFunction::LeftClick),
        [0x02, 0xF1, 0x00, 0x00] => ButtonBinding::Mouse(MouseFunction::MiddleClick),
        [0x02, 0xF2, 0x00, 0x00] => ButtonBinding::Mouse(MouseFunction::RightClick),
        [0x02, 0xF3, 0x00, 0x00] => ButtonBinding::Mouse(MouseFunction::ScrollUp),
        [0x02, 0xF4, 0x00, 0x00] => ButtonBinding::Mouse(MouseFunction::ScrollDown),
        [0x02, 0xF5, 0x00, 0x00] => ButtonBinding::Mouse(MouseFunction::TiltLeft),
        [0x02, 0xF6, 0x00, 0x00] => ButtonBinding::Mouse(MouseFunction::TiltRight),
        [0x02, 0xF8, 0x00, 0x00] | [0x02, 0xF8, 0x00, 0x03] => {
            ButtonBinding::Mouse(MouseFunction::Back)
        }
        [0x02, 0xF9, 0x00, 0x00] | [0x02, 0xF9, 0x00, 0x04] => {
            ButtonBinding::Mouse(MouseFunction::Forward)
        }
        [0x71, 0xF0, 0x00, 0x00] => ButtonBinding::Mouse(MouseFunction::DpiToggle),
        [0x04, 0x00, 0x00, 0xCD] => ButtonBinding::Multimedia(MultimediaFunction::PlayPause),
        [0x04, 0x00, 0x00, 0xB7] => ButtonBinding::Multimedia(MultimediaFunction::Stop),
        [0x04, 0x00, 0x00, 0xB5] => ButtonBinding::Multimedia(MultimediaFunction::NextTrack),
        [0x04, 0x00, 0x00, 0xB6] => ButtonBinding::Multimedia(MultimediaFunction::PreviousTrack),
        [0x04, 0x00, 0x00, 0xE2] => ButtonBinding::Multimedia(MultimediaFunction::MuteVolume),
        [0x04, 0x00, 0x00, 0xE9] => ButtonBinding::Multimedia(MultimediaFunction::VolumeUp),
        [0x04, 0x00, 0x00, 0xEA] => ButtonBinding::Multimedia(MultimediaFunction::VolumeDown),
        [0x23, 0xE2, 0x29, 0x00] => ButtonBinding::WindowsShortcut(WindowsShortcut::CycleApps),
        [0x23, 0xE2, 0x2B, 0x00] => ButtonBinding::WindowsShortcut(WindowsShortcut::SwitchApps),
        [0x23, 0xE0, 0x1B, 0x00] => ButtonBinding::WindowsShortcut(WindowsShortcut::Cut),
        [0x23, 0xE0, 0x06, 0x00] => ButtonBinding::WindowsShortcut(WindowsShortcut::Copy),
        [0x23, 0xE0, 0x19, 0x00] => ButtonBinding::WindowsShortcut(WindowsShortcut::Paste),
        [0x23, 0xE0, 0x1D, 0x00] => ButtonBinding::WindowsShortcut(WindowsShortcut::Undo),
        _ => {
            return Err(PerformanceProfileError::UnknownButtonBinding { control, record });
        }
    };

    Ok(binding)
}

fn encode_button_binding(
    binding: &ButtonBinding,
) -> Result<[u8; BUTTON_RECORD_LENGTH], PerformanceProfileError> {
    let record = match binding {
        ButtonBinding::Disabled => [0x00, 0x00, 0x00, 0x00],
        ButtonBinding::Keyboard(KeyboardUsage(usage)) => {
            let usage = u8::try_from(*usage)
                .ok()
                .filter(|usage| *usage != 0)
                .ok_or(PerformanceProfileError::KeyboardUsageOutOfRange(*usage))?;
            [0x00, usage, 0x00, 0x00]
        }
        ButtonBinding::Mouse(function) => match function {
            MouseFunction::LeftClick => [0x02, 0xF0, 0x00, 0x00],
            MouseFunction::MiddleClick => [0x02, 0xF1, 0x00, 0x00],
            MouseFunction::RightClick => [0x02, 0xF2, 0x00, 0x00],
            MouseFunction::ScrollUp => [0x02, 0xF3, 0x00, 0x00],
            MouseFunction::ScrollDown => [0x02, 0xF4, 0x00, 0x00],
            MouseFunction::TiltLeft => [0x02, 0xF5, 0x00, 0x00],
            MouseFunction::TiltRight => [0x02, 0xF6, 0x00, 0x00],
            MouseFunction::Back => [0x02, 0xF8, 0x00, 0x03],
            MouseFunction::Forward => [0x02, 0xF9, 0x00, 0x04],
            MouseFunction::DpiToggle => [0x71, 0xF0, 0x00, 0x00],
        },
        ButtonBinding::Multimedia(function) => [
            0x04,
            0x00,
            0x00,
            match function {
                MultimediaFunction::PlayPause => 0xCD,
                MultimediaFunction::Stop => 0xB7,
                MultimediaFunction::NextTrack => 0xB5,
                MultimediaFunction::PreviousTrack => 0xB6,
                MultimediaFunction::MuteVolume => 0xE2,
                MultimediaFunction::VolumeUp => 0xE9,
                MultimediaFunction::VolumeDown => 0xEA,
            },
        ],
        ButtonBinding::WindowsShortcut(shortcut) => match shortcut {
            WindowsShortcut::CycleApps => [0x23, 0xE2, 0x29, 0x00],
            WindowsShortcut::SwitchApps => [0x23, 0xE2, 0x2B, 0x00],
            WindowsShortcut::Cut => [0x23, 0xE0, 0x1B, 0x00],
            WindowsShortcut::Copy => [0x23, 0xE0, 0x06, 0x00],
            WindowsShortcut::Paste => [0x23, 0xE0, 0x19, 0x00],
            WindowsShortcut::Undo => [0x23, 0xE0, 0x1D, 0x00],
        },
        ButtonBinding::Macro(_) => {
            return Err(PerformanceProfileError::UnconfirmedButtonBinding(
                binding.clone(),
            ));
        }
    };

    Ok(record)
}

/// Encode Pulsefire Raid's volatile two-LED direct RGB feature report.
///
/// This packet is independently implemented from the report layout documented
/// by OpenRGB. It does not write onboard memory and must be periodically resent
/// if the caller wants the direct color to remain active.
pub fn encode_direct_rgb(wheel: RgbColor, logo: RgbColor) -> [u8; DIRECT_REPORT_LENGTH] {
    let mut report = [0_u8; DIRECT_REPORT_LENGTH];
    report[0] = DIRECT_REPORT_ID;
    report[1] = DIRECT_START;
    report[2..5].copy_from_slice(&wheel.bytes());
    report[5..8].copy_from_slice(&logo.bytes());
    report[8] = DIRECT_END;
    report
}

/// Encode the fixed runtime-profile access prelude observed before reads.
///
/// Its individual fields are not generalized: only the exact locally
/// repeated `07 03 04 64` report is exposed.
pub fn encode_runtime_profile_read_prelude() -> [u8; DIRECT_REPORT_LENGTH] {
    let mut report = [0_u8; DIRECT_REPORT_LENGTH];
    report[..4].copy_from_slice(&[
        DIRECT_REPORT_ID,
        PROFILE_ACCESS_OPCODE,
        RUNTIME_PROFILE_SECTION,
        PROFILE_ACCESS_TRAILER,
    ]);
    report
}

/// Encode the fixed feature-report request observed before runtime reads.
pub fn encode_profile_read_request() -> [u8; DIRECT_REPORT_LENGTH] {
    let mut report = [0_u8; DIRECT_REPORT_LENGTH];
    report[..2].copy_from_slice(&[DIRECT_REPORT_ID, PROFILE_READ_RESPONSE_OPCODE]);
    report
}

#[cfg(test)]
mod tests {
    use super::*;

    fn profile_fixture(
        kind: ProfileImageKind,
        section: ProfileSection,
    ) -> [u8; DIRECT_REPORT_LENGTH] {
        let mut report = [0_u8; DIRECT_REPORT_LENGTH];
        report[0] = DIRECT_REPORT_ID;
        report[1] = match kind {
            ProfileImageKind::HostWrite => PROFILE_WRITE_OPCODE,
            ProfileImageKind::DeviceReadResponse => PROFILE_READ_RESPONSE_OPCODE,
        };
        report[2] = match section {
            ProfileSection::Onboard => ONBOARD_PROFILE_SECTION,
            ProfileSection::Runtime => RUNTIME_PROFILE_SECTION,
        };
        report[POLLING_INTERVAL_OFFSET] = 0x01;
        for (index, dpi) in [1000_u32, 1600, 3200].into_iter().enumerate() {
            let encoded = u16::try_from(dpi / DPI_UNIT).unwrap().to_be_bytes();
            report[DPI_X_OFFSETS[index]..DPI_X_OFFSETS[index] + 2].copy_from_slice(&encoded);
            report[DPI_Y_OFFSETS[index]..DPI_Y_OFFSETS[index] + 2].copy_from_slice(&encoded);
            report[DPI_STAGE_ENABLED_OFFSETS[index]] = 0x01;
        }
        report[DPI_STAGE_COLOR_OFFSETS[0]..DPI_STAGE_COLOR_OFFSETS[0] + 3]
            .copy_from_slice(&[0x2B, 0x00, 0xFF]);
        report[DPI_STAGE_COLOR_OFFSETS[1]..DPI_STAGE_COLOR_OFFSETS[1] + 3]
            .copy_from_slice(&[0xCD, 0x00, 0xFF]);
        report[DPI_STAGE_COLOR_OFFSETS[2]..DPI_STAGE_COLOR_OFFSETS[2] + 3]
            .copy_from_slice(&[0x32, 0xFF, 0x00]);
        report
    }

    fn captured_five_stage_profile() -> [u8; DIRECT_REPORT_LENGTH] {
        let mut report = [0_u8; DIRECT_REPORT_LENGTH];
        report[..3].copy_from_slice(&[0x07, 0x81, 0x04]);
        report[0x18..0x78].copy_from_slice(&[
            0x01, 0x00, 0x14, 0x00, 0x20, 0x00, 0x40, 0x00, // 0x18
            0x80, 0x01, 0x40, 0x00, 0x02, 0x00, 0x14, 0x00, // 0x20
            0x20, 0x00, 0x40, 0x00, 0x80, 0x01, 0x40, 0x00, // 0x28
            0x02, 0x00, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, // 0x30
            0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0xFF, 0x00, // 0x38
            0x00, 0xFF, 0x00, 0x00, 0x00, 0x00, 0xFF, 0x00, // 0x40
            0xFF, 0x00, 0xFF, 0x00, 0x00, 0xFF, 0x00, 0x00, // 0x48
            0x00, 0xFF, 0x00, 0xFF, 0x00, 0x00, 0x00, 0xFF, // 0x50
            0x00, 0x00, 0x00, 0xFF, 0x00, 0x0A, 0x0A, 0x00, // 0x58
            0x0F, 0x0F, 0x00, 0x0F, 0x0F, 0x00, 0x00, 0x00, // 0x60
            0x02, 0x2B, 0x00, 0xFF, 0xCD, 0x00, 0xFF, 0x32, // 0x68
            0xFF, 0x00, 0xFF, 0x00, 0x00, 0xFF, 0xFF, 0xFF, // 0x70
        ]);
        report
    }

    fn captured_button_profile() -> [u8; DIRECT_REPORT_LENGTH] {
        let mut report = profile_fixture(
            ProfileImageKind::DeviceReadResponse,
            ProfileSection::Runtime,
        );
        report[0x7C..0xA8].copy_from_slice(&[
            0x02, 0xF0, 0x00, 0x00, // left click
            0x02, 0xF2, 0x00, 0x00, // right click
            0x02, 0xF1, 0x00, 0x00, // middle click
            0x02, 0xF8, 0x00, 0x00, // Button 4: Back
            0x02, 0xF9, 0x00, 0x00, // Button 5: Forward
            0x04, 0x00, 0x00, 0xE9, // Button 7: Volume Up
            0x04, 0x00, 0x00, 0xEA, // Button 6: Volume Down
            0x04, 0x00, 0x00, 0xE2, // Button 8: Mute
            0x00, 0x04, 0x00, 0x00, // DPI control remapped to keyboard A
            0x02, 0xF5, 0x00, 0x00, // wheel tilt left
            0x02, 0xF6, 0x00, 0x00, // wheel tilt right
        ]);
        report
    }

    #[test]
    fn golden_direct_rgb_packet_has_exact_layout_and_zero_fill() {
        let actual = encode_direct_rgb(
            RgbColor::new(0xFF, 0x80, 0x01),
            RgbColor::new(0x02, 0x40, 0xFE),
        );
        let mut expected = [0_u8; DIRECT_REPORT_LENGTH];
        expected[..9].copy_from_slice(&[0x07, 0x0A, 0xFF, 0x80, 0x01, 0x02, 0x40, 0xFE, 0xA0]);

        assert_eq!(actual, expected);
    }

    #[test]
    fn off_is_direct_black_for_both_leds() {
        let report = encode_direct_rgb(RgbColor::BLACK, RgbColor::BLACK);
        assert_eq!(&report[..9], &[0x07, 0x0A, 0, 0, 0, 0, 0, 0, 0xA0]);
        assert!(report[9..].iter().all(|byte| *byte == 0));
    }

    #[test]
    fn golden_runtime_profile_read_reports_match_local_captures() {
        let prelude = encode_runtime_profile_read_prelude();
        assert_eq!(&prelude[..4], &[0x07, 0x03, 0x04, 0x64]);
        assert!(prelude[4..].iter().all(|byte| *byte == 0));

        let request = encode_profile_read_request();
        assert_eq!(&request[..2], &[0x07, 0x81]);
        assert!(request[2..].iter().all(|byte| *byte == 0));
    }

    #[test]
    fn golden_minimal_macro_report_matches_repeated_local_captures() {
        let a_events = keyboard_a_events(20);
        let macro_definition = ConfirmedMacro::button5_keyboard_once(&a_events).unwrap();
        let mut expected = [0_u8; DIRECT_REPORT_LENGTH];
        expected[..16].copy_from_slice(&[
            0x07, 0x05, 0x04, 0x04, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0x80, 0x14, 0x04, 0x00,
            0x14, 0x04,
        ]);

        assert_eq!(macro_definition.as_bytes(), &expected);
        assert_eq!(ConfirmedMacro::parse(&expected).unwrap(), macro_definition);
        assert_eq!(macro_definition.control(), PulsefireRaidControl::Button5);
        assert_eq!(macro_definition.events(), a_events);
        assert_eq!(
            macro_definition.playback(),
            hyperx_core::MacroPlayback::Once
        );

        let macro_300ms = ConfirmedMacro::button5_keyboard_once(&keyboard_a_events(300)).unwrap();
        let mut expected_300ms = expected;
        expected_300ms[0x0A..0x10].copy_from_slice(&[0x81, 0x2C, 0x04, 0x01, 0x2C, 0x04]);
        assert_eq!(macro_300ms.as_bytes(), &expected_300ms);
        assert_eq!(ConfirmedMacro::parse(&expected_300ms).unwrap(), macro_300ms);

        let ab_events = keyboard_ab_events(20);
        let macro_ab = ConfirmedMacro::button5_keyboard_once(&ab_events).unwrap();
        let mut expected_ab = expected;
        expected_ab[0x10..0x16].copy_from_slice(&[0x80, 0x14, 0x05, 0x00, 0x14, 0x05]);
        assert_eq!(macro_ab.as_bytes(), &expected_ab);
        assert_eq!(ConfirmedMacro::parse(&expected_ab).unwrap(), macro_ab);
        assert_eq!(macro_ab.events(), ab_events);
    }

    #[test]
    fn minimal_macro_binding_matches_forward_reverse_captures() {
        let macro_definition =
            ConfirmedMacro::button5_keyboard_once(&keyboard_ab_events(20)).unwrap();
        let mut profile = PerformanceProfile::parse(&captured_button_profile()).unwrap();
        profile
            .set_button_binding(
                PulsefireRaidControl::Button5,
                &ButtonBinding::Mouse(MouseFunction::Forward),
            )
            .unwrap();
        let before = *profile.as_bytes();

        macro_definition.apply_to_profile(&mut profile).unwrap();

        assert_eq!(
            changed_offsets(&before, profile.as_bytes()),
            vec![0x8C, 0x8D]
        );
        assert_eq!(&profile.as_bytes()[0x8C..0x90], &[0x53, 0x00, 0x00, 0x04]);
        assert!(profile.has_confirmed_macro_reference(PulsefireRaidControl::Button5));
        assert!(!profile.has_confirmed_macro_reference(PulsefireRaidControl::Dpi));
        assert!(macro_definition.is_bound_in(&profile));

        profile
            .set_button_binding(
                PulsefireRaidControl::Button5,
                &ButtonBinding::Mouse(MouseFunction::Forward),
            )
            .unwrap();
        assert_eq!(profile.as_bytes(), &before);
        assert!(!macro_definition.is_bound_in(&profile));
    }

    #[test]
    fn minimal_macro_rejects_unconfirmed_variants_and_onboard_binding() {
        assert_eq!(
            ConfirmedMacro::parse(&[0_u8; 16]),
            Err(ConfirmedMacroError::WrongLength(16))
        );

        let unconfirmed_events = keyboard_a_events(50);
        assert_eq!(
            ConfirmedMacro::button5_keyboard_once(&unconfirmed_events),
            Err(ConfirmedMacroError::UnconfirmedEvents(
                unconfirmed_events.clone()
            ))
        );

        let mut unconfirmed_timing = *ConfirmedMacro::button5_keyboard_once(&keyboard_a_events(20))
            .unwrap()
            .as_bytes();
        unconfirmed_timing[0x0A..0x10].copy_from_slice(&[0x80, 0x32, 0x04, 0x00, 0x32, 0x04]);
        assert_eq!(
            ConfirmedMacro::parse(&unconfirmed_timing),
            Err(ConfirmedMacroError::UnconfirmedByte {
                offset: 0x0B,
                expected: 0x14,
                actual: 0x32,
            })
        );

        for (offset, actual) in [(0x09, 0x02), (0x0C, 0x05), (0x20, 0x01)] {
            let mut report = *ConfirmedMacro::button5_keyboard_once(&keyboard_a_events(20))
                .unwrap()
                .as_bytes();
            let expected = report[offset];
            report[offset] = actual;
            assert_eq!(
                ConfirmedMacro::parse(&report),
                Err(ConfirmedMacroError::UnconfirmedByte {
                    offset,
                    expected,
                    actual,
                })
            );
        }

        let macro_definition =
            ConfirmedMacro::button5_keyboard_once(&keyboard_a_events(20)).unwrap();
        let original = profile_fixture(ProfileImageKind::HostWrite, ProfileSection::Onboard);
        let mut onboard = PerformanceProfile::parse(&original).unwrap();
        assert_eq!(
            macro_definition.apply_to_profile(&mut onboard),
            Err(ConfirmedMacroError::UnsupportedProfileSection)
        );
        assert_eq!(onboard.as_bytes(), &original);
    }

    #[test]
    fn parses_captured_performance_fields() {
        let report = profile_fixture(
            ProfileImageKind::DeviceReadResponse,
            ProfileSection::Runtime,
        );
        let profile = PerformanceProfile::parse(&report).unwrap();

        assert_eq!(profile.kind(), ProfileImageKind::DeviceReadResponse);
        assert_eq!(profile.section(), ProfileSection::Runtime);
        assert_eq!(profile.polling_rate().unwrap(), PollingRate::Hz1000);
        assert_eq!(
            profile.dpi_profile().unwrap(),
            DpiProfile {
                stages: vec![
                    DpiStage::new(1000, 1000, RgbColor::new(0x2B, 0x00, 0xFF)),
                    DpiStage::new(1600, 1600, RgbColor::new(0xCD, 0x00, 0xFF)),
                    DpiStage::new(3200, 3200, RgbColor::new(0x32, 0xFF, 0x00)),
                ],
                active_stage: 0,
            }
        );
    }

    #[test]
    fn parses_captured_onboard_profile_section() {
        let report = profile_fixture(ProfileImageKind::HostWrite, ProfileSection::Onboard);
        let profile = PerformanceProfile::parse(&report).unwrap();

        assert_eq!(profile.kind(), ProfileImageKind::HostWrite);
        assert_eq!(profile.section(), ProfileSection::Onboard);
    }

    #[test]
    fn polling_codes_match_repeated_capture_matrix() {
        for (code, expected) in [
            (0x01, PollingRate::Hz1000),
            (0x02, PollingRate::Hz500),
            (0x04, PollingRate::Hz250),
            (0x08, PollingRate::Hz125),
        ] {
            let mut report = profile_fixture(ProfileImageKind::HostWrite, ProfileSection::Runtime);
            report[POLLING_INTERVAL_OFFSET] = code;
            let profile = PerformanceProfile::parse(&report).unwrap();
            assert_eq!(profile.polling_rate().unwrap(), expected);
        }
    }

    #[test]
    fn offline_polling_patch_changes_only_confirmed_offset() {
        let report = profile_fixture(ProfileImageKind::HostWrite, ProfileSection::Runtime);
        let mut profile = PerformanceProfile::parse(&report).unwrap();

        profile.set_polling_rate(PollingRate::Hz125);

        let changed_offsets: Vec<_> = report
            .iter()
            .zip(profile.as_bytes())
            .enumerate()
            .filter_map(|(index, (before, after))| (before != after).then_some(index))
            .collect();
        assert_eq!(changed_offsets, vec![POLLING_INTERVAL_OFFSET]);
        assert_eq!(profile.as_bytes()[POLLING_INTERVAL_OFFSET], 0x08);
    }

    #[test]
    fn decodes_captured_button_records() {
        let profile = PerformanceProfile::parse(&captured_button_profile()).unwrap();

        assert_eq!(
            profile
                .button_binding(PulsefireRaidControl::Button4)
                .unwrap(),
            ButtonBinding::Mouse(MouseFunction::Back)
        );
        assert_eq!(
            profile
                .button_binding(PulsefireRaidControl::Button5)
                .unwrap(),
            ButtonBinding::Mouse(MouseFunction::Forward)
        );
        assert_eq!(
            profile
                .button_binding(PulsefireRaidControl::Button7)
                .unwrap(),
            ButtonBinding::Multimedia(MultimediaFunction::VolumeUp)
        );
        assert_eq!(
            profile
                .button_binding(PulsefireRaidControl::Button6)
                .unwrap(),
            ButtonBinding::Multimedia(MultimediaFunction::VolumeDown)
        );
        assert_eq!(
            profile
                .button_binding(PulsefireRaidControl::Button8)
                .unwrap(),
            ButtonBinding::Multimedia(MultimediaFunction::MuteVolume)
        );
        assert_eq!(
            profile.button_binding(PulsefireRaidControl::Dpi).unwrap(),
            ButtonBinding::Keyboard(KeyboardUsage(0x04))
        );
    }

    #[test]
    fn golden_button_five_sequence_matches_local_captures() {
        let original = captured_button_profile();
        let mut profile = PerformanceProfile::parse(&original).unwrap();

        profile
            .set_button_binding(PulsefireRaidControl::Button5, &ButtonBinding::Disabled)
            .unwrap();
        assert_eq!(
            changed_offsets(&original, profile.as_bytes()),
            vec![0x8C, 0x8D]
        );
        assert_eq!(&profile.as_bytes()[0x8C..0x90], &[0x00; 4]);

        let before = *profile.as_bytes();
        profile
            .set_button_binding(
                PulsefireRaidControl::Button5,
                &ButtonBinding::Mouse(MouseFunction::Forward),
            )
            .unwrap();
        assert_eq!(
            changed_offsets(&before, profile.as_bytes()),
            vec![0x8C, 0x8D, 0x8F]
        );
        assert_eq!(&profile.as_bytes()[0x8C..0x90], &[0x02, 0xF9, 0x00, 0x04]);

        let before = *profile.as_bytes();
        profile
            .set_button_binding(
                PulsefireRaidControl::Button5,
                &ButtonBinding::Mouse(MouseFunction::Back),
            )
            .unwrap();
        assert_eq!(
            changed_offsets(&before, profile.as_bytes()),
            vec![0x8D, 0x8F]
        );
        assert_eq!(&profile.as_bytes()[0x8C..0x90], &[0x02, 0xF8, 0x00, 0x03]);

        let before = *profile.as_bytes();
        profile
            .set_button_binding(
                PulsefireRaidControl::Button5,
                &ButtonBinding::Multimedia(MultimediaFunction::VolumeUp),
            )
            .unwrap();
        assert_eq!(
            changed_offsets(&before, profile.as_bytes()),
            vec![0x8C, 0x8D, 0x8F]
        );
        assert_eq!(&profile.as_bytes()[0x8C..0x90], &[0x04, 0x00, 0x00, 0xE9]);

        let before = *profile.as_bytes();
        profile
            .set_button_binding(
                PulsefireRaidControl::Button5,
                &ButtonBinding::WindowsShortcut(WindowsShortcut::Copy),
            )
            .unwrap();
        assert_eq!(
            changed_offsets(&before, profile.as_bytes()),
            vec![0x8C, 0x8D, 0x8E, 0x8F]
        );
        assert_eq!(&profile.as_bytes()[0x8C..0x90], &[0x23, 0xE0, 0x06, 0x00]);

        let before = *profile.as_bytes();
        profile
            .set_button_binding(
                PulsefireRaidControl::Button5,
                &ButtonBinding::Keyboard(KeyboardUsage(0x04)),
            )
            .unwrap();
        assert_eq!(
            changed_offsets(&before, profile.as_bytes()),
            vec![0x8C, 0x8D, 0x8E]
        );
        assert_eq!(&profile.as_bytes()[0x8C..0x90], &[0x00, 0x04, 0x00, 0x00]);
    }

    #[test]
    fn golden_dpi_toggle_record_matches_two_local_control_slots() {
        let original = captured_button_profile();
        let dpi_toggle = ButtonBinding::Mouse(MouseFunction::DpiToggle);
        let mut profile = PerformanceProfile::parse(&original).unwrap();

        profile
            .set_button_binding(PulsefireRaidControl::Dpi, &dpi_toggle)
            .unwrap();
        assert_eq!(
            changed_offsets(&original, profile.as_bytes()),
            vec![0x9C, 0x9D]
        );
        assert_eq!(&profile.as_bytes()[0x9C..0xA0], &[0x71, 0xF0, 0x00, 0x00]);
        assert_eq!(
            profile.button_binding(PulsefireRaidControl::Dpi).unwrap(),
            dpi_toggle
        );

        let before = *profile.as_bytes();
        profile
            .set_button_binding(PulsefireRaidControl::Button5, &dpi_toggle)
            .unwrap();
        assert_eq!(
            changed_offsets(&before, profile.as_bytes()),
            vec![0x8C, 0x8D]
        );
        assert_eq!(&profile.as_bytes()[0x8C..0x90], &[0x71, 0xF0, 0x00, 0x00]);

        let before = *profile.as_bytes();
        profile
            .set_button_binding(
                PulsefireRaidControl::Button5,
                &ButtonBinding::Mouse(MouseFunction::Forward),
            )
            .unwrap();
        assert_eq!(
            changed_offsets(&before, profile.as_bytes()),
            vec![0x8C, 0x8D, 0x8F]
        );
        assert_eq!(&profile.as_bytes()[0x8C..0x90], &[0x02, 0xF9, 0x00, 0x04]);
    }

    #[test]
    fn standard_hid_binding_families_round_trip_offline() {
        let mut profile = PerformanceProfile::parse(&captured_button_profile()).unwrap();
        let bindings = [
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
            ButtonBinding::Keyboard(KeyboardUsage(0x2C)),
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
            ButtonBinding::Disabled,
        ];

        for binding in bindings {
            profile
                .set_button_binding(PulsefireRaidControl::Button5, &binding)
                .unwrap();
            assert_eq!(
                profile
                    .button_binding(PulsefireRaidControl::Button5)
                    .unwrap(),
                binding
            );
        }
    }

    #[test]
    fn rejects_unconfirmed_or_illegal_button_bindings_without_mutating() {
        use hyperx_core::{MacroBinding, MacroPlayback};

        let original = captured_button_profile();
        let mut profile = PerformanceProfile::parse(&original).unwrap();

        let disabled = ButtonBinding::Disabled;
        assert_eq!(
            profile.set_button_binding(PulsefireRaidControl::LeftClick, &disabled),
            Err(PerformanceProfileError::ButtonBindingNotAllowed {
                control: PulsefireRaidControl::LeftClick,
                binding: disabled,
            })
        );
        assert_eq!(profile.as_bytes(), &original);

        for usage in [0, 0x100] {
            assert_eq!(
                profile.set_button_binding(
                    PulsefireRaidControl::Button5,
                    &ButtonBinding::Keyboard(KeyboardUsage(usage)),
                ),
                Err(PerformanceProfileError::KeyboardUsageOutOfRange(usage))
            );
            assert_eq!(profile.as_bytes(), &original);
        }

        let macro_binding = ButtonBinding::Macro(MacroBinding {
            id: "capture-required".to_owned(),
            playback: MacroPlayback::Once,
        });
        assert_eq!(
            profile.set_button_binding(PulsefireRaidControl::Button5, &macro_binding),
            Err(PerformanceProfileError::UnconfirmedButtonBinding(
                macro_binding
            ))
        );
        assert_eq!(profile.as_bytes(), &original);
    }

    #[test]
    fn reports_unknown_button_records_without_guessing() {
        let mut report = captured_button_profile();
        report[0x8C..0x90].copy_from_slice(&[0xFF, 0xEE, 0xDD, 0xCC]);
        let profile = PerformanceProfile::parse(&report).unwrap();

        assert_eq!(
            profile.button_binding(PulsefireRaidControl::Button5),
            Err(PerformanceProfileError::UnknownButtonBinding {
                control: PulsefireRaidControl::Button5,
                record: [0xFF, 0xEE, 0xDD, 0xCC],
            })
        );
    }

    #[test]
    fn decodes_all_five_captured_dpi_stages() {
        let report = captured_five_stage_profile();
        let profile = PerformanceProfile::parse(&report).unwrap();
        let dpi = profile.dpi_profile().unwrap();

        assert_eq!(dpi.stages.len(), 5);
        assert_eq!(dpi.active_stage, 0);
        assert_eq!(
            dpi.stages,
            vec![
                DpiStage::new(1000, 1000, RgbColor::new(0x2B, 0x00, 0xFF)),
                DpiStage::new(1600, 1600, RgbColor::new(0xCD, 0x00, 0xFF)),
                DpiStage::new(3200, 3200, RgbColor::new(0x32, 0xFF, 0x00)),
                DpiStage::new(6400, 6400, RgbColor::new(0xFF, 0x00, 0x00)),
                DpiStage::new(16_000, 16_000, RgbColor::new(0xFF, 0xFF, 0xFF)),
            ]
        );
        assert_eq!(dpi.active(), dpi.stages.first());
    }

    #[test]
    fn offline_stage_two_patch_matches_forward_and_reverse_captures() {
        let original = captured_five_stage_profile();
        let mut profile = PerformanceProfile::parse(&original).unwrap();
        let mut dpi = profile.dpi_profile().unwrap();
        dpi.stages[1].x = 1700;
        dpi.stages[1].y = 1700;
        dpi.active_stage = 1;

        profile.set_dpi_profile(&dpi).unwrap();

        assert_eq!(
            changed_offsets(&original, profile.as_bytes()),
            vec![0x1C, 0x28, 0x31]
        );
        assert_eq!(profile.as_bytes()[0x1C], 0x22);
        assert_eq!(profile.as_bytes()[0x28], 0x22);
        assert_eq!(profile.as_bytes()[0x31], 0x01);

        dpi.stages[1].x = 1600;
        dpi.stages[1].y = 1600;
        dpi.active_stage = 0;
        profile.set_dpi_profile(&dpi).unwrap();
        assert_eq!(profile.as_bytes(), &original);
    }

    #[test]
    fn offline_removal_of_fifth_stage_matches_capture() {
        let original = captured_five_stage_profile();
        let mut profile = PerformanceProfile::parse(&original).unwrap();
        let mut dpi = profile.dpi_profile().unwrap();
        dpi.stages.pop();

        profile.set_dpi_profile(&dpi).unwrap();

        assert_eq!(
            changed_offsets(&original, profile.as_bytes()),
            vec![0x21, 0x22, 0x2D, 0x2E, 0x36, 0x75, 0x76, 0x77]
        );
        assert!(profile.as_bytes()[0x21..0x23]
            .iter()
            .chain(&profile.as_bytes()[0x2D..0x2F])
            .chain(&profile.as_bytes()[0x36..0x37])
            .chain(&profile.as_bytes()[0x75..0x78])
            .all(|byte| *byte == 0));
    }

    #[test]
    fn offline_profile_patch_validates_every_value_before_mutating() {
        let original = captured_five_stage_profile();
        let mut profile = PerformanceProfile::parse(&original).unwrap();
        let mut dpi = profile.dpi_profile().unwrap();
        dpi.stages[2].x = 1625;

        assert_eq!(
            profile.set_dpi_profile(&dpi),
            Err(PerformanceProfileError::DpiNotStepAligned {
                stage: 3,
                axis: DpiAxis::X,
                dpi: 1625,
            })
        );
        assert_eq!(profile.as_bytes(), &original);

        dpi.stages[2].x = 16_050;
        assert_eq!(
            profile.set_dpi_profile(&dpi),
            Err(PerformanceProfileError::DpiOutOfRange {
                stage: 3,
                axis: DpiAxis::X,
                dpi: 16_050,
            })
        );
        assert_eq!(profile.as_bytes(), &original);

        dpi.stages[2].x = 150;
        assert_eq!(
            profile.set_dpi_profile(&dpi),
            Err(PerformanceProfileError::DpiOutOfRange {
                stage: 3,
                axis: DpiAxis::X,
                dpi: 150,
            })
        );
        assert_eq!(profile.as_bytes(), &original);

        dpi.stages[2].x = 200;
        profile.set_dpi_profile(&dpi).unwrap();
        assert_eq!(&profile.as_bytes()[0x1D..0x1F], &[0x00, 0x04]);
    }

    #[test]
    fn offline_profile_patch_validates_stage_count_and_active_index() {
        let original = captured_five_stage_profile();
        let mut profile = PerformanceProfile::parse(&original).unwrap();
        let mut dpi = profile.dpi_profile().unwrap();
        dpi.stages.clear();
        assert_eq!(
            profile.set_dpi_profile(&dpi),
            Err(PerformanceProfileError::InvalidDpiStageCount(0))
        );

        dpi.stages.push(DpiStage::new(800, 800, RgbColor::BLACK));
        dpi.active_stage = 1;
        assert_eq!(
            profile.set_dpi_profile(&dpi),
            Err(PerformanceProfileError::InvalidActiveDpiStage {
                active: 1,
                stage_count: 1,
            })
        );
        assert_eq!(profile.as_bytes(), &original);
    }

    #[test]
    fn write_report_changes_only_the_confirmed_opcode() {
        let original = captured_five_stage_profile();
        let profile = PerformanceProfile::parse(&original).unwrap();
        let write_report = profile.to_write_report();

        assert_eq!(changed_offsets(&original, &write_report), vec![0x01]);
        assert_eq!(&write_report[..3], &[0x07, 0x01, 0x04]);
    }

    #[test]
    fn validates_dpi_stage_flags_and_active_index() {
        let mut report = profile_fixture(ProfileImageKind::HostWrite, ProfileSection::Runtime);
        report[DPI_STAGE_ENABLED_OFFSETS[1]] = 0;
        assert_eq!(
            PerformanceProfile::parse(&report).unwrap().dpi_profile(),
            Err(PerformanceProfileError::NonContiguousDpiStages { stage: 3 })
        );

        let mut report = profile_fixture(ProfileImageKind::HostWrite, ProfileSection::Runtime);
        report[DPI_STAGE_ENABLED_OFFSETS[1]] = 0x02;
        assert_eq!(
            PerformanceProfile::parse(&report).unwrap().dpi_profile(),
            Err(PerformanceProfileError::InvalidDpiStageFlag {
                stage: 2,
                value: 0x02,
            })
        );

        let mut report = profile_fixture(ProfileImageKind::HostWrite, ProfileSection::Runtime);
        report[ACTIVE_DPI_STAGE_OFFSET] = 3;
        assert_eq!(
            PerformanceProfile::parse(&report).unwrap().dpi_profile(),
            Err(PerformanceProfileError::InvalidActiveDpiStage {
                active: 3,
                stage_count: 3,
            })
        );

        let mut report = profile_fixture(ProfileImageKind::HostWrite, ProfileSection::Runtime);
        for offset in DPI_STAGE_ENABLED_OFFSETS {
            report[offset] = 0;
        }
        assert_eq!(
            PerformanceProfile::parse(&report).unwrap().dpi_profile(),
            Err(PerformanceProfileError::NoEnabledDpiStages)
        );
    }

    #[test]
    fn rejects_non_profile_reports_and_unknown_polling_codes() {
        assert_eq!(
            PerformanceProfile::parse(&[0; 10]),
            Err(PerformanceProfileError::WrongLength(10))
        );

        let mut report = profile_fixture(ProfileImageKind::HostWrite, ProfileSection::Runtime);
        report[0] = 0x06;
        assert_eq!(
            PerformanceProfile::parse(&report),
            Err(PerformanceProfileError::WrongReportId(0x06))
        );

        let mut report = profile_fixture(ProfileImageKind::HostWrite, ProfileSection::Runtime);
        report[1] = 0x10;
        assert_eq!(
            PerformanceProfile::parse(&report),
            Err(PerformanceProfileError::UnsupportedOpcode(0x10))
        );

        let mut report = profile_fixture(ProfileImageKind::HostWrite, ProfileSection::Runtime);
        report[2] = 0x03;
        assert_eq!(
            PerformanceProfile::parse(&report),
            Err(PerformanceProfileError::UnsupportedSection(0x03))
        );

        let mut report = profile_fixture(ProfileImageKind::HostWrite, ProfileSection::Runtime);
        report[POLLING_INTERVAL_OFFSET] = 0x03;
        let profile = PerformanceProfile::parse(&report).unwrap();
        assert_eq!(
            profile.polling_rate(),
            Err(PerformanceProfileError::UnknownPollingInterval(0x03))
        );
    }

    fn changed_offsets(before: &[u8], after: &[u8]) -> Vec<usize> {
        before
            .iter()
            .zip(after)
            .enumerate()
            .filter_map(|(index, (before, after))| (before != after).then_some(index))
            .collect()
    }
}
