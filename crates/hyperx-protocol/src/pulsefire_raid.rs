use hyperx_core::{PollingRate, RgbColor};
use thiserror::Error;

pub const DIRECT_REPORT_ID: u8 = 0x07;
pub const DIRECT_REPORT_LENGTH: usize = 264;
const DIRECT_START: u8 = 0x0A;
const DIRECT_END: u8 = 0xA0;

const PROFILE_WRITE_OPCODE: u8 = 0x01;
const PROFILE_READ_RESPONSE_OPCODE: u8 = 0x81;
const PERFORMANCE_PROFILE_SECTION: u8 = 0x04;
const POLLING_INTERVAL_OFFSET: usize = 0x18;
const FIRST_DPI_X_OFFSET: usize = 0x1A;
const FIRST_DPI_Y_OFFSET: usize = 0x26;
const DPI_UNIT: u32 = 50;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProfileImageKind {
    HostWrite,
    DeviceReadResponse,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PerformanceProfile {
    report: [u8; DIRECT_REPORT_LENGTH],
    kind: ProfileImageKind,
}

#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum PerformanceProfileError {
    #[error("expected a 264-byte Pulsefire Raid profile report, got {0} bytes")]
    WrongLength(usize),
    #[error("expected report ID 0x07, got 0x{0:02X}")]
    WrongReportId(u8),
    #[error("unsupported Pulsefire Raid profile opcode 0x{0:02X}")]
    UnsupportedOpcode(u8),
    #[error("expected performance profile section 0x04, got 0x{0:02X}")]
    WrongSection(u8),
    #[error("unknown Pulsefire Raid polling interval code 0x{0:02X}")]
    UnknownPollingInterval(u8),
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

        if report[2] != PERFORMANCE_PROFILE_SECTION {
            return Err(PerformanceProfileError::WrongSection(report[2]));
        }

        Ok(Self { report, kind })
    }

    pub const fn kind(&self) -> ProfileImageKind {
        self.kind
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

    /// Return the first captured DPI stage as separate X/Y values.
    ///
    /// NGENUITY changed both fields together. Independent axis control remains
    /// unconfirmed and is intentionally not exposed as a mutation.
    pub fn first_dpi_stage(&self) -> (u32, u32) {
        (
            read_dpi(&self.report, FIRST_DPI_X_OFFSET),
            read_dpi(&self.report, FIRST_DPI_Y_OFFSET),
        )
    }

    pub const fn as_bytes(&self) -> &[u8; DIRECT_REPORT_LENGTH] {
        &self.report
    }
}

fn read_dpi(report: &[u8; DIRECT_REPORT_LENGTH], offset: usize) -> u32 {
    u32::from(u16::from_le_bytes([report[offset], report[offset + 1]])) * DPI_UNIT
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

    fn profile_fixture(kind: ProfileImageKind) -> [u8; DIRECT_REPORT_LENGTH] {
        let mut report = [0_u8; DIRECT_REPORT_LENGTH];
        report[0] = DIRECT_REPORT_ID;
        report[1] = match kind {
            ProfileImageKind::HostWrite => PROFILE_WRITE_OPCODE,
            ProfileImageKind::DeviceReadResponse => PROFILE_READ_RESPONSE_OPCODE,
        };
        report[2] = PERFORMANCE_PROFILE_SECTION;
        report[POLLING_INTERVAL_OFFSET] = 0x01;
        report[FIRST_DPI_X_OFFSET..FIRST_DPI_X_OFFSET + 2].copy_from_slice(&0x14_u16.to_le_bytes());
        report[FIRST_DPI_Y_OFFSET..FIRST_DPI_Y_OFFSET + 2].copy_from_slice(&0x14_u16.to_le_bytes());
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
        let report = profile_fixture(ProfileImageKind::DeviceReadResponse);
        let profile = PerformanceProfile::parse(&report).unwrap();

        assert_eq!(profile.kind(), ProfileImageKind::DeviceReadResponse);
        assert_eq!(profile.polling_rate().unwrap(), PollingRate::Hz1000);
        assert_eq!(profile.first_dpi_stage(), (1000, 1000));
    }

    #[test]
    fn polling_codes_match_repeated_capture_matrix() {
        for (code, expected) in [
            (0x01, PollingRate::Hz1000),
            (0x02, PollingRate::Hz500),
            (0x04, PollingRate::Hz250),
            (0x08, PollingRate::Hz125),
        ] {
            let mut report = profile_fixture(ProfileImageKind::HostWrite);
            report[POLLING_INTERVAL_OFFSET] = code;
            let profile = PerformanceProfile::parse(&report).unwrap();
            assert_eq!(profile.polling_rate().unwrap(), expected);
        }
    }

    #[test]
    fn offline_polling_patch_changes_only_confirmed_offset() {
        let report = profile_fixture(ProfileImageKind::HostWrite);
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
    fn rejects_non_profile_reports_and_unknown_polling_codes() {
        assert_eq!(
            PerformanceProfile::parse(&[0; 10]),
            Err(PerformanceProfileError::WrongLength(10))
        );

        let mut report = profile_fixture(ProfileImageKind::HostWrite);
        report[0] = 0x06;
        assert_eq!(
            PerformanceProfile::parse(&report),
            Err(PerformanceProfileError::WrongReportId(0x06))
        );

        let mut report = profile_fixture(ProfileImageKind::HostWrite);
        report[1] = 0x10;
        assert_eq!(
            PerformanceProfile::parse(&report),
            Err(PerformanceProfileError::UnsupportedOpcode(0x10))
        );

        let mut report = profile_fixture(ProfileImageKind::HostWrite);
        report[2] = 0x03;
        assert_eq!(
            PerformanceProfile::parse(&report),
            Err(PerformanceProfileError::WrongSection(0x03))
        );

        let mut report = profile_fixture(ProfileImageKind::HostWrite);
        report[POLLING_INTERVAL_OFFSET] = 0x03;
        let profile = PerformanceProfile::parse(&report).unwrap();
        assert_eq!(
            profile.polling_rate(),
            Err(PerformanceProfileError::UnknownPollingInterval(0x03))
        );
    }
}
