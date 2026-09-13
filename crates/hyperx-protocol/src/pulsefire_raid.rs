use hyperx_core::{DpiProfile, DpiStage, PollingRate, RgbColor};
use thiserror::Error;

pub const DIRECT_REPORT_ID: u8 = 0x07;
pub const DIRECT_REPORT_LENGTH: usize = 264;
const DIRECT_START: u8 = 0x0A;
const DIRECT_END: u8 = 0xA0;

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
const DPI_UNIT: u32 = 50;

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

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PerformanceProfile {
    report: [u8; DIRECT_REPORT_LENGTH],
    kind: ProfileImageKind,
    section: ProfileSection,
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

    pub const fn as_bytes(&self) -> &[u8; DIRECT_REPORT_LENGTH] {
        &self.report
    }
}

fn read_dpi(report: &[u8; DIRECT_REPORT_LENGTH], offset: usize) -> u32 {
    u32::from(u16::from_be_bytes([report[offset], report[offset + 1]])) * DPI_UNIT
}

fn read_color(report: &[u8; DIRECT_REPORT_LENGTH], offset: usize) -> RgbColor {
    RgbColor::new(report[offset], report[offset + 1], report[offset + 2])
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
}
