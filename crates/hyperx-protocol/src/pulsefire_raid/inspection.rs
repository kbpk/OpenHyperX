//! Offline recognition only. Unknown bytes never become a send operation.
use crate::{capture::CaptureRecord, Direction};

use super::*;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RaidReportInspection {
    Profile(Box<PerformanceProfile>),
    ProfileSelector(ProfileSection),
    ProfileReadRequest,
    VendorSessionPhase(usize),
    DirectRgb {
        wheel: RgbColor,
        logo: RgbColor,
    },
    OnboardSolidSnapshot {
        index: usize,
        colors: Option<(RgbColor, RgbColor)>,
    },
    RuntimeMacro(Box<PulsefireRaidMacro>),
    /// Only the observed pattern, not correlation with a pending transaction.
    Acknowledgement {
        opcode: u8,
    },
    InvalidKnownReport(String),
    Unknown,
}

/// Caller explicitly selects Raid interpretation. A raw packet alone cannot
/// establish VID/PID, model, transfer type or successful device execution.
pub fn inspect_captured_report(record: &CaptureRecord) -> RaidReportInspection {
    let bytes = &record.report.bytes;
    if matches!(record.interface, None | Some(2))
        && matches!(record.report.direction, None | Some(Direction::Rx))
        && bytes.len() == 8
        && matches!(bytes[3], 0x03 | 0x18 | 0x05 | 0x01 | 0x07 | 0x81)
    {
        // Captured 81 ACKs use FF; the other listed opcodes use 00. Neither
        // value is generalized to status semantics or evidence of persistence.
        let opcode = bytes[3];
        let expected = [
            0,
            0,
            DIRECT_REPORT_ID,
            opcode,
            if opcode == 0x81 { 0xFF } else { 0 },
            0,
            0,
            0,
        ];
        if bytes.as_slice() == expected {
            return RaidReportInspection::Acknowledgement { opcode };
        }
    }
    if !matches!(record.interface, None | Some(1)) || bytes.first() != Some(&DIRECT_REPORT_ID) {
        return RaidReportInspection::Unknown;
    }
    let inspection = inspect_configuration_bytes(bytes);
    let expected_direction = match &inspection {
        RaidReportInspection::Profile(profile)
            if profile.kind() == ProfileImageKind::DeviceReadResponse =>
        {
            Some(Direction::Rx)
        }
        RaidReportInspection::Unknown | RaidReportInspection::InvalidKnownReport(_) => None,
        _ => Some(Direction::Tx),
    };
    if let (Some(actual), Some(expected)) = (record.report.direction, expected_direction) {
        if actual != expected {
            return RaidReportInspection::InvalidKnownReport(format!(
                "packet shape expects {expected}, but capture says {actual}"
            ));
        }
    }
    inspection
}

fn inspect_configuration_bytes(bytes: &[u8]) -> RaidReportInspection {
    if bytes == encode_runtime_profile_read_prelude() {
        return RaidReportInspection::ProfileSelector(ProfileSection::Runtime);
    }
    if bytes == encode_onboard_profile_read_prelude() {
        return RaidReportInspection::ProfileSelector(ProfileSection::Onboard);
    }
    if bytes == encode_profile_read_request() {
        return RaidReportInspection::ProfileReadRequest;
    }
    for (phase, report) in encode_vendor_session_start_reports().iter().enumerate() {
        if bytes == report {
            return RaidReportInspection::VendorSessionPhase(phase);
        }
    }
    match bytes.get(1) {
        Some(&PROFILE_WRITE_OPCODE) | Some(&PROFILE_READ_RESPONSE_OPCODE) => {
            PerformanceProfile::parse(bytes).map_or_else(
                |error| RaidReportInspection::InvalidKnownReport(error.to_string()),
                |profile| RaidReportInspection::Profile(Box::new(profile)),
            )
        }
        Some(&DIRECT_START) if bytes.len() == DIRECT_REPORT_LENGTH => {
            let wheel = read_color(bytes.try_into().unwrap(), 2);
            let logo = read_color(bytes.try_into().unwrap(), 5);
            if bytes == encode_direct_rgb(wheel, logo) {
                RaidReportInspection::DirectRgb { wheel, logo }
            } else {
                RaidReportInspection::InvalidKnownReport(
                    "direct RGB markers/padding differ from confirmed layout".into(),
                )
            }
        }
        Some(&ONBOARD_AUXILIARY_OPCODE) if bytes.len() == DIRECT_REPORT_LENGTH => {
            let index = usize::from(bytes[ONBOARD_AUXILIARY_INDEX_OFFSET]);
            let report: &[u8; DIRECT_REPORT_LENGTH] = bytes.try_into().unwrap();
            let wheel = read_color(report, ONBOARD_AUXILIARY_WHEEL_COLOR_OFFSET);
            let logo = read_color(report, ONBOARD_AUXILIARY_LOGO_COLOR_OFFSET);
            let known = encode_onboard_static_lighting_reports(wheel, logo);
            if known.get(index).is_some_and(|expected| bytes == expected) {
                RaidReportInspection::OnboardSolidSnapshot {
                    index,
                    colors: (index == 0).then_some((wheel, logo)),
                }
            } else {
                RaidReportInspection::InvalidKnownReport(
                    "auxiliary report is not a confirmed indexed Solid snapshot".into(),
                )
            }
        }
        Some(&0x05) if bytes.get(2) == Some(&RUNTIME_PROFILE_SECTION) => {
            PulsefireRaidMacro::parse(bytes).map_or_else(
                |error| RaidReportInspection::InvalidKnownReport(error.to_string()),
                |definition| RaidReportInspection::RuntimeMacro(Box::new(definition)),
            )
        }
        // A known opcode with arbitrary constants is NOT a decoded selector or
        // session phase. Keep it diagnostic, never infer alternate meanings.
        Some(&PROFILE_ACCESS_OPCODE)
        | Some(&0x07)
        | Some(&DIRECT_START)
        | Some(&ONBOARD_AUXILIARY_OPCODE) => RaidReportInspection::InvalidKnownReport(
            "length/constants differ from a confirmed packet".into(),
        ),
        _ => RaidReportInspection::Unknown,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::capture::{parse_report_log, CapturedReport};

    fn record(bytes: Vec<u8>) -> CaptureRecord {
        CaptureRecord {
            line: 1,
            interface: Some(1),
            report: CapturedReport {
                direction: Some(Direction::Tx),
                bytes,
            },
        }
    }

    #[test]
    fn recognizes_only_complete_confirmed_selector_session_rgb_and_solid_packets() {
        assert_eq!(
            inspect_captured_report(&record(encode_runtime_profile_read_prelude().to_vec())),
            RaidReportInspection::ProfileSelector(ProfileSection::Runtime)
        );
        assert_eq!(
            inspect_captured_report(&record(encode_profile_read_request().to_vec())),
            RaidReportInspection::ProfileReadRequest
        );
        for (phase, bytes) in encode_vendor_session_start_reports().iter().enumerate() {
            assert_eq!(
                inspect_captured_report(&record(bytes.to_vec())),
                RaidReportInspection::VendorSessionPhase(phase)
            );
        }
        let wheel = RgbColor::new(255, 0, 0);
        let logo = RgbColor::new(0, 0, 255);
        assert_eq!(
            inspect_captured_report(&record(encode_direct_rgb(wheel, logo).to_vec())),
            RaidReportInspection::DirectRgb { wheel, logo }
        );
        for (index, bytes) in encode_onboard_static_lighting_reports(wheel, logo)
            .iter()
            .enumerate()
        {
            assert_eq!(
                inspect_captured_report(&record(bytes.to_vec())),
                RaidReportInspection::OnboardSolidSnapshot {
                    index,
                    colors: (index == 0).then_some((wheel, logo))
                }
            );
        }
        let mut bad = encode_direct_rgb(wheel, logo);
        bad[263] = 1;
        assert!(matches!(
            inspect_captured_report(&record(bad.to_vec())),
            RaidReportInspection::InvalidKnownReport(_)
        ));
    }

    #[test]
    fn decodes_golden_runtime_macro_fixture_without_io() {
        let log =
            parse_report_log(include_str!("../../tests/fixtures/button4-ab-toggle.hex")).unwrap();
        let RaidReportInspection::RuntimeMacro(definition) =
            inspect_captured_report(&log.records[0])
        else {
            panic!("expected macro");
        };
        assert_eq!(definition.control(), PulsefireRaidControl::Button4);
        assert_eq!(definition.playback(), MacroPlayback::ToggleRepeat);
        assert_eq!(definition.events().len(), 4);
    }

    #[test]
    fn empty_profile_is_inspectable_but_not_a_usable_snapshot() {
        let mut bytes = vec![0; DIRECT_REPORT_LENGTH];
        bytes[..3].copy_from_slice(&[7, 0x81, 4]);
        let mut captured = record(bytes);
        captured.report.direction = Some(Direction::Rx);
        let RaidReportInspection::Profile(profile) = inspect_captured_report(&captured) else {
            panic!("expected profile");
        };
        assert!(profile.validate_confirmed_runtime_settings().is_err());
        assert!(matches!(
            inspect_captured_report(&record(vec![7, 0x81, 4])),
            RaidReportInspection::InvalidKnownReport(_)
        ));
    }

    #[test]
    fn refuses_wrong_interface_direction_and_unknown_ack_patterns() {
        let mut captured = record(encode_runtime_profile_read_prelude().to_vec());
        captured.interface = Some(0);
        assert_eq!(
            inspect_captured_report(&captured),
            RaidReportInspection::Unknown
        );
        captured.interface = Some(1);
        captured.report.direction = Some(Direction::Rx);
        assert!(matches!(
            inspect_captured_report(&captured),
            RaidReportInspection::InvalidKnownReport(_)
        ));
        captured.interface = Some(2);
        captured.report.bytes = vec![0, 0, 7, 0x81, 0xFF, 0, 0, 0];
        assert_eq!(
            inspect_captured_report(&captured),
            RaidReportInspection::Acknowledgement { opcode: 0x81 }
        );
        captured.report.bytes[4] = 0;
        assert_eq!(
            inspect_captured_report(&captured),
            RaidReportInspection::Unknown
        );
    }
}
