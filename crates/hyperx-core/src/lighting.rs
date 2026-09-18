use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::RgbColor;

/// Number of integer RGB steps in one full red-green-blue spectrum cycle.
pub const SPECTRUM_STEPS: u16 = 6 * 256;

const MIN_PROGRAM_DURATION_SECONDS: u64 = 1;
const MAX_PROGRAM_DURATION_SECONDS: u64 = 3_600;
const MIN_FRAME_INTERVAL_MS: u64 = 20;
const MAX_FRAME_INTERVAL_MS: u64 = 1_000;
const MIN_PERIOD_MS: u64 = 100;
const MAX_PERIOD_MS: u64 = 60_000;
const MIN_CONFETTI_STEP_MS: u64 = 50;
const MAX_CONFETTI_STEP_MS: u64 = 5_000;
const MIN_FADE_MS: u64 = 50;
const MAX_FADE_MS: u64 = 10_000;

const SUN_PALETTE: [RgbColor; 4] = [
    RgbColor::new(0xFF, 0x20, 0x00),
    RgbColor::new(0xFF, 0x8C, 0x00),
    RgbColor::new(0xFF, 0xE0, 0xA0),
    RgbColor::new(0xFF, 0x70, 0x00),
];
const TWILIGHT_PALETTE: [RgbColor; 4] = [
    RgbColor::new(0x10, 0x00, 0x30),
    RgbColor::new(0x50, 0x00, 0xA0),
    RgbColor::new(0x00, 0x40, 0xA0),
    RgbColor::new(0xA0, 0x00, 0x70),
];

/// A foreground software-lighting program keyed by device-defined zone IDs.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SoftwareLightingProgram {
    pub duration_seconds: u64,
    #[serde(default = "default_frame_interval_ms")]
    pub frame_interval_ms: u64,
    pub zones: BTreeMap<String, SoftwareLightingEffect>,
}

impl SoftwareLightingProgram {
    /// Validate all portable constraints before discovery or device I/O.
    pub fn validate(&self) -> Result<(), SoftwareLightingError> {
        if !(MIN_PROGRAM_DURATION_SECONDS..=MAX_PROGRAM_DURATION_SECONDS)
            .contains(&self.duration_seconds)
        {
            return Err(SoftwareLightingError::DurationOutOfRange(
                self.duration_seconds,
            ));
        }
        if !(MIN_FRAME_INTERVAL_MS..=MAX_FRAME_INTERVAL_MS).contains(&self.frame_interval_ms) {
            return Err(SoftwareLightingError::FrameIntervalOutOfRange(
                self.frame_interval_ms,
            ));
        }
        if self.zones.is_empty() {
            return Err(SoftwareLightingError::NoZones);
        }
        for (zone, effect) in &self.zones {
            effect
                .validate()
                .map_err(|source| SoftwareLightingError::InvalidZoneEffect {
                    zone: zone.clone(),
                    source: Box::new(source),
                })?;
        }
        Ok(())
    }

    pub fn uses_triggers(&self) -> bool {
        self.zones
            .values()
            .any(SoftwareLightingEffect::uses_trigger)
    }
}

const fn default_frame_interval_ms() -> u64 {
    50
}

const fn default_period_ms() -> u64 {
    5_000
}

const fn default_confetti_step_ms() -> u64 {
    250
}

const fn default_fade_ms() -> u64 {
    1_000
}

/// One portable software-rendered effect.
///
/// Names mirror familiar NGENUITY Legacy choices, but animation curves and palettes
/// are explicitly OpenHyperX definitions until captures establish otherwise.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "effect", rename_all = "kebab-case", deny_unknown_fields)]
pub enum SoftwareLightingEffect {
    Off,
    Solid {
        color: RgbColor,
    },
    Cycle {
        #[serde(default = "default_period_ms")]
        period_ms: u64,
        #[serde(default)]
        phase_degrees: u16,
    },
    Pulse {
        color: RgbColor,
        #[serde(default = "default_period_ms")]
        period_ms: u64,
        #[serde(default)]
        phase_degrees: u16,
    },
    Breathing {
        color: RgbColor,
        #[serde(default = "default_period_ms")]
        period_ms: u64,
        #[serde(default)]
        phase_degrees: u16,
    },
    TriggeredFade {
        color: RgbColor,
        #[serde(default = "default_fade_ms")]
        fade_ms: u64,
    },
    Confetti {
        #[serde(default = "default_confetti_step_ms")]
        step_ms: u64,
        #[serde(default)]
        seed: u64,
    },
    Sun {
        #[serde(default = "default_period_ms")]
        period_ms: u64,
        #[serde(default)]
        phase_degrees: u16,
    },
    Twilight {
        #[serde(default = "default_period_ms")]
        period_ms: u64,
        #[serde(default)]
        phase_degrees: u16,
    },
}

impl SoftwareLightingEffect {
    pub fn validate(&self) -> Result<(), SoftwareLightingError> {
        match *self {
            Self::Off | Self::Solid { .. } => Ok(()),
            Self::Cycle {
                period_ms,
                phase_degrees,
            }
            | Self::Pulse {
                period_ms,
                phase_degrees,
                ..
            }
            | Self::Breathing {
                period_ms,
                phase_degrees,
                ..
            }
            | Self::Sun {
                period_ms,
                phase_degrees,
            }
            | Self::Twilight {
                period_ms,
                phase_degrees,
            } => {
                validate_period(period_ms)?;
                validate_phase(phase_degrees)
            }
            Self::TriggeredFade { fade_ms, .. } => {
                if (MIN_FADE_MS..=MAX_FADE_MS).contains(&fade_ms) {
                    Ok(())
                } else {
                    Err(SoftwareLightingError::FadeOutOfRange(fade_ms))
                }
            }
            Self::Confetti { step_ms, .. } => {
                if (MIN_CONFETTI_STEP_MS..=MAX_CONFETTI_STEP_MS).contains(&step_ms) {
                    Ok(())
                } else {
                    Err(SoftwareLightingError::ConfettiStepOutOfRange(step_ms))
                }
            }
        }
    }

    pub const fn uses_trigger(&self) -> bool {
        matches!(self, Self::TriggeredFade { .. })
    }

    /// Render one frame at `elapsed_ms`.
    ///
    /// `last_trigger_ms` is required only for `triggered-fade`; it is the
    /// elapsed timestamp of the latest input trigger in the same time domain.
    pub fn color_at(
        &self,
        elapsed_ms: u64,
        last_trigger_ms: Option<u64>,
    ) -> Result<RgbColor, SoftwareLightingError> {
        self.validate()?;
        Ok(match *self {
            Self::Off => RgbColor::BLACK,
            Self::Solid { color } => color,
            Self::Cycle {
                period_ms,
                phase_degrees,
            } => spectrum_color(cycle_spectrum_phase(elapsed_ms, period_ms, phase_degrees)),
            Self::Pulse {
                color,
                period_ms,
                phase_degrees,
            } => {
                let progress = cycle_progress(elapsed_ms, period_ms, phase_degrees);
                scale_color(color, u8::MAX - progress)
            }
            Self::Breathing {
                color,
                period_ms,
                phase_degrees,
            } => {
                let progress = cycle_progress(elapsed_ms, period_ms, phase_degrees);
                let triangle = if progress < 128 {
                    progress.saturating_mul(2)
                } else {
                    (u8::MAX - progress).saturating_mul(2)
                };
                scale_color(color, smoothstep(triangle))
            }
            Self::TriggeredFade { color, fade_ms } => {
                let Some(trigger_ms) = last_trigger_ms else {
                    return Ok(RgbColor::BLACK);
                };
                let age_ms = elapsed_ms.saturating_sub(trigger_ms);
                if age_ms >= fade_ms {
                    RgbColor::BLACK
                } else {
                    let brightness = u8::MAX
                        - u8::try_from(age_ms * u64::from(u8::MAX) / fade_ms)
                            .expect("validated fade progress fits in u8");
                    scale_color(color, brightness)
                }
            }
            Self::Confetti { step_ms, seed } => {
                let bucket = elapsed_ms / step_ms;
                let random = splitmix64(seed.wrapping_add(bucket));
                spectrum_color((random % u64::from(SPECTRUM_STEPS)) as u16)
            }
            Self::Sun {
                period_ms,
                phase_degrees,
            } => palette_color(
                &SUN_PALETTE,
                cycle_progress_u16(elapsed_ms, period_ms, phase_degrees, 1024),
            ),
            Self::Twilight {
                period_ms,
                phase_degrees,
            } => palette_color(
                &TWILIGHT_PALETTE,
                cycle_progress_u16(elapsed_ms, period_ms, phase_degrees, 1024),
            ),
        })
    }
}

fn validate_period(period_ms: u64) -> Result<(), SoftwareLightingError> {
    if (MIN_PERIOD_MS..=MAX_PERIOD_MS).contains(&period_ms) {
        Ok(())
    } else {
        Err(SoftwareLightingError::PeriodOutOfRange(period_ms))
    }
}

fn validate_phase(phase_degrees: u16) -> Result<(), SoftwareLightingError> {
    if phase_degrees < 360 {
        Ok(())
    } else {
        Err(SoftwareLightingError::PhaseOutOfRange(phase_degrees))
    }
}

fn cycle_spectrum_phase(elapsed_ms: u64, period_ms: u64, phase_degrees: u16) -> u16 {
    cycle_progress_u16(elapsed_ms, period_ms, phase_degrees, SPECTRUM_STEPS)
}

fn cycle_progress(elapsed_ms: u64, period_ms: u64, phase_degrees: u16) -> u8 {
    cycle_progress_u16(elapsed_ms, period_ms, phase_degrees, 256) as u8
}

fn cycle_progress_u16(elapsed_ms: u64, period_ms: u64, phase_degrees: u16, steps: u16) -> u16 {
    let timed = u128::from(elapsed_ms % period_ms) * u128::from(steps) / u128::from(period_ms);
    let offset = u128::from(phase_degrees) * u128::from(steps) / 360;
    ((timed + offset) % u128::from(steps)) as u16
}

fn scale_color(color: RgbColor, brightness: u8) -> RgbColor {
    const fn scale(channel: u8, brightness: u8) -> u8 {
        ((channel as u16 * brightness as u16 + 127) / 255) as u8
    }
    RgbColor::new(
        scale(color.red, brightness),
        scale(color.green, brightness),
        scale(color.blue, brightness),
    )
}

fn smoothstep(value: u8) -> u8 {
    let value = u32::from(value);
    let squared = value * value;
    ((squared * (3 * 255 - 2 * value) + 255 * 255 / 2) / (255 * 255)) as u8
}

fn palette_color(palette: &[RgbColor; 4], progress: u16) -> RgbColor {
    let segment = usize::from(progress / 256) % palette.len();
    let next = (segment + 1) % palette.len();
    let amount = (progress % 256) as u8;
    mix_color(palette[segment], palette[next], amount)
}

fn mix_color(start: RgbColor, end: RgbColor, amount: u8) -> RgbColor {
    fn mix(start: u8, end: u8, amount: u8) -> u8 {
        let inverse = u16::from(u8::MAX - amount);
        ((u16::from(start) * inverse + u16::from(end) * u16::from(amount) + 127) / 255) as u8
    }
    RgbColor::new(
        mix(start.red, end.red, amount),
        mix(start.green, end.green, amount),
        mix(start.blue, end.blue, amount),
    )
}

fn splitmix64(mut value: u64) -> u64 {
    value = value.wrapping_add(0x9E37_79B9_7F4A_7C15);
    value = (value ^ (value >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    value = (value ^ (value >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    value ^ (value >> 31)
}

/// Return one saturated color from a deterministic RGB spectrum.
pub const fn spectrum_color(phase: u16) -> RgbColor {
    let phase = phase % SPECTRUM_STEPS;
    let segment = phase / 256;
    let offset = (phase % 256) as u8;
    let inverse = u8::MAX - offset;

    match segment {
        0 => RgbColor::new(u8::MAX, offset, 0),
        1 => RgbColor::new(inverse, u8::MAX, 0),
        2 => RgbColor::new(0, u8::MAX, offset),
        3 => RgbColor::new(0, inverse, u8::MAX),
        4 => RgbColor::new(offset, 0, u8::MAX),
        _ => RgbColor::new(u8::MAX, 0, inverse),
    }
}

#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum SoftwareLightingError {
    #[error("lighting duration must be between 1 and 3600 seconds; got {0}")]
    DurationOutOfRange(u64),
    #[error("frame interval must be between 20 and 1000 ms; got {0}")]
    FrameIntervalOutOfRange(u64),
    #[error("lighting program must contain at least one zone")]
    NoZones,
    #[error("lighting period must be between 100 and 60000 ms; got {0}")]
    PeriodOutOfRange(u64),
    #[error("lighting phase must be from 0 through 359 degrees; got {0}")]
    PhaseOutOfRange(u16),
    #[error("confetti step must be between 50 and 5000 ms; got {0}")]
    ConfettiStepOutOfRange(u64),
    #[error("triggered fade must last between 50 and 10000 ms; got {0}")]
    FadeOutOfRange(u64),
    #[error("invalid effect for lighting zone {zone:?}: {source}")]
    InvalidZoneEffect {
        zone: String,
        source: Box<SoftwareLightingError>,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    fn effect_color(effect: SoftwareLightingEffect, elapsed_ms: u64) -> RgbColor {
        effect.color_at(elapsed_ms, None).unwrap()
    }

    #[test]
    fn spectrum_has_stable_primary_and_secondary_anchors() {
        assert_eq!(spectrum_color(0), RgbColor::new(255, 0, 0));
        assert_eq!(spectrum_color(256), RgbColor::new(255, 255, 0));
        assert_eq!(spectrum_color(512), RgbColor::new(0, 255, 0));
        assert_eq!(spectrum_color(768), RgbColor::new(0, 255, 255));
        assert_eq!(spectrum_color(1024), RgbColor::new(0, 0, 255));
        assert_eq!(spectrum_color(1280), RgbColor::new(255, 0, 255));
        assert_eq!(spectrum_color(SPECTRUM_STEPS), spectrum_color(0));
    }

    #[test]
    fn renders_distinct_timed_effect_curves_and_phase_offsets() {
        let red = RgbColor::new(255, 0, 0);
        let pulse = SoftwareLightingEffect::Pulse {
            color: red,
            period_ms: 1_000,
            phase_degrees: 0,
        };
        assert_eq!(effect_color(pulse.clone(), 0), red);
        assert_eq!(effect_color(pulse, 1_000), red);

        let breathing = SoftwareLightingEffect::Breathing {
            color: red,
            period_ms: 1_000,
            phase_degrees: 0,
        };
        assert_eq!(effect_color(breathing.clone(), 0), RgbColor::BLACK);
        assert_eq!(effect_color(breathing, 500), red);

        let shifted = SoftwareLightingEffect::Cycle {
            period_ms: 6_000,
            phase_degrees: 120,
        };
        assert_eq!(effect_color(shifted, 0), RgbColor::new(0, 255, 0));
    }

    #[test]
    fn triggered_fade_restarts_from_full_color_and_expires() {
        let color = RgbColor::new(200, 100, 50);
        let fade = SoftwareLightingEffect::TriggeredFade {
            color,
            fade_ms: 1_000,
        };
        assert_eq!(fade.color_at(500, None).unwrap(), RgbColor::BLACK);
        assert_eq!(fade.color_at(500, Some(500)).unwrap(), color);
        assert_eq!(fade.color_at(1_500, Some(500)).unwrap(), RgbColor::BLACK);
    }

    #[test]
    fn confetti_is_deterministic_and_zone_seeds_can_differ() {
        let first = SoftwareLightingEffect::Confetti {
            step_ms: 100,
            seed: 1,
        };
        let second = SoftwareLightingEffect::Confetti {
            step_ms: 100,
            seed: 2,
        };
        assert_eq!(effect_color(first.clone(), 250), effect_color(first, 250));
        assert_ne!(
            effect_color(second, 250),
            effect_color(
                SoftwareLightingEffect::Confetti {
                    step_ms: 100,
                    seed: 1,
                },
                250,
            )
        );
    }

    #[test]
    fn validates_program_and_effect_bounds() {
        let mut zones = BTreeMap::new();
        zones.insert(
            "wheel".to_owned(),
            SoftwareLightingEffect::Cycle {
                period_ms: 5_000,
                phase_degrees: 0,
            },
        );
        let program = SoftwareLightingProgram {
            duration_seconds: 30,
            frame_interval_ms: 50,
            zones,
        };
        assert_eq!(program.validate(), Ok(()));
        assert!(!program.uses_triggers());

        let invalid = SoftwareLightingEffect::Breathing {
            color: RgbColor::new(255, 0, 0),
            period_ms: 0,
            phase_degrees: 360,
        };
        assert_eq!(
            invalid.validate(),
            Err(SoftwareLightingError::PeriodOutOfRange(0))
        );
    }
}
