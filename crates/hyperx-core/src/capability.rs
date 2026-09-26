use std::{fmt, str::FromStr};

use thiserror::Error;

use crate::MacroPlayback;

/// Implemented macro encoding support for one physical control, not a claim
/// about all hardware features or verified physical playback behavior.
/// Runtime and persistent support are intentionally independent.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MacroCapabilities {
    pub runtime_playback: &'static [MacroPlayback],
    pub onboard_playback: &'static [MacroPlayback],
    pub max_events: usize,
    pub max_delay_ms: u16,
}

impl MacroCapabilities {
    pub fn supports_runtime(self, playback: MacroPlayback) -> bool {
        self.runtime_playback.contains(&playback)
    }

    pub fn supports_onboard(self, playback: MacroPlayback) -> bool {
        self.onboard_playback.contains(&playback)
    }
}

/// A high-level feature exposed by a device.
///
/// This says what the hardware is expected to support. It does not imply that
/// OpenHyperX has implemented or verified the corresponding vendor command.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Capability {
    Dpi,
    DpiStages,
    PollingRate,
    ButtonRemapping,
    Macros,
    Lighting,
    OnboardProfile,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LightingZone {
    pub id: &'static str,
    pub name: &'static str,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CapabilitySet {
    pub features: &'static [Capability],
    pub button_count: u8,
    pub lighting_zones: &'static [LightingZone],
    pub dpi: Option<DpiCapabilities>,
    pub polling_rates: &'static [PollingRate],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DpiCapabilities {
    pub minimum: u32,
    pub maximum: u32,
    pub step: u32,
    pub max_stages: u8,
}

impl DpiCapabilities {
    pub fn validate(self, dpi: u32) -> Result<u32, DpiValidationError> {
        if dpi < self.minimum || dpi > self.maximum {
            return Err(DpiValidationError::OutOfRange {
                dpi,
                minimum: self.minimum,
                maximum: self.maximum,
            });
        }
        if self.step == 0 || (dpi - self.minimum) % self.step != 0 {
            return Err(DpiValidationError::NotStepAligned {
                dpi,
                minimum: self.minimum,
                step: self.step,
            });
        }
        Ok(dpi)
    }
}

#[derive(Clone, Copy, Debug, Error, Eq, PartialEq)]
pub enum DpiValidationError {
    #[error("DPI {dpi} is outside the supported range {minimum}..={maximum}")]
    OutOfRange {
        dpi: u32,
        minimum: u32,
        maximum: u32,
    },
    #[error("DPI {dpi} must use step {step} starting at {minimum}")]
    NotStepAligned { dpi: u32, minimum: u32, step: u32 },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PollingRate {
    Hz125,
    Hz250,
    Hz500,
    Hz1000,
}

impl PollingRate {
    pub const fn hz(self) -> u16 {
        match self {
            Self::Hz125 => 125,
            Self::Hz250 => 250,
            Self::Hz500 => 500,
            Self::Hz1000 => 1000,
        }
    }
}

impl fmt::Display for PollingRate {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}", self.hz())
    }
}

impl FromStr for PollingRate {
    type Err = PollingRateParseError;

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        match input {
            "125" => Ok(Self::Hz125),
            "250" => Ok(Self::Hz250),
            "500" => Ok(Self::Hz500),
            "1000" => Ok(Self::Hz1000),
            _ => Err(PollingRateParseError(input.to_owned())),
        }
    }
}

#[derive(Clone, Debug, Error, Eq, PartialEq)]
#[error("polling rate must be one of: 125, 250, 500, 1000 Hz; got {0}")]
pub struct PollingRateParseError(String);

#[cfg(test)]
mod tests {
    use super::*;

    const DPI: DpiCapabilities = DpiCapabilities {
        minimum: 200,
        maximum: 16_000,
        step: 50,
        max_stages: 5,
    };

    #[test]
    fn macro_runtime_and_onboard_support_are_independent() {
        let capabilities = MacroCapabilities {
            runtime_playback: &[MacroPlayback::Once, MacroPlayback::ToggleRepeat],
            onboard_playback: &[MacroPlayback::Once],
            max_events: 14,
            max_delay_ms: 9_999,
        };
        assert!(capabilities.supports_runtime(MacroPlayback::ToggleRepeat));
        assert!(!capabilities.supports_onboard(MacroPlayback::ToggleRepeat));
        assert!(capabilities.supports_onboard(MacroPlayback::Once));
        assert!(!capabilities.supports_runtime(MacroPlayback::RepeatWhileHeld));
        for (playback, name) in [
            (MacroPlayback::Once, "once"),
            (MacroPlayback::ToggleRepeat, "toggle-repeat"),
            (MacroPlayback::RepeatWhileHeld, "repeat-while-held"),
        ] {
            assert_eq!(playback.to_string(), name);
        }
    }

    #[test]
    fn validates_dpi_range_and_step() {
        assert_eq!(DPI.validate(200), Ok(200));
        assert_eq!(DPI.validate(16_000), Ok(16_000));
        assert!(matches!(
            DPI.validate(199),
            Err(DpiValidationError::OutOfRange { dpi: 199, .. })
        ));
        assert!(matches!(
            DPI.validate(225),
            Err(DpiValidationError::NotStepAligned { dpi: 225, .. })
        ));
    }

    #[test]
    fn parses_and_displays_polling_rates() {
        for (text, rate) in [
            ("125", PollingRate::Hz125),
            ("250", PollingRate::Hz250),
            ("500", PollingRate::Hz500),
            ("1000", PollingRate::Hz1000),
        ] {
            assert_eq!(text.parse::<PollingRate>(), Ok(rate));
            assert_eq!(rate.to_string(), text);
        }
        assert!("2000".parse::<PollingRate>().is_err());
    }
}
