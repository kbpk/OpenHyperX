//! Offline evidence only. These captured images must never be replayed.

use hyperx_protocol::{
    capture::parse_report_log,
    pulsefire_raid::{
        inspect_captured_report, PerformanceProfile, ProfileImageKind, ProfileSection,
        PulsefireRaidControl, RaidReportInspection,
    },
    Direction,
};

const COLD: &str = include_str!("fixtures/cold-legacy-startup-images.hex");
const WARM: &str = include_str!("fixtures/warm-legacy-startup-images.hex");
const ONBOARD: &str = include_str!("fixtures/read-request-get-onboard.hex");

fn differing_body_offsets(before: &PerformanceProfile, after: &PerformanceProfile) -> Vec<usize> {
    before
        .as_bytes()
        .iter()
        .zip(after.as_bytes())
        .enumerate()
        .skip(3)
        .filter_map(|(offset, (before, after))| (before != after).then_some(offset))
        .collect()
}

fn captured_profiles() -> (PerformanceProfile, PerformanceProfile, PerformanceProfile) {
    let cold = parse_report_log(COLD).unwrap();
    assert_eq!(cold.records.len(), 2);
    assert_eq!(cold.records[0].report.direction, Some(Direction::Rx));
    assert_eq!(cold.records[1].report.direction, Some(Direction::Tx));
    let image = |index| match inspect_captured_report(&cold.records[index]) {
        RaidReportInspection::Profile(profile) => *profile,
        other => panic!("expected a captured profile image, got {other:?}"),
    };

    let onboard = parse_report_log(ONBOARD).unwrap();
    assert_eq!(onboard.records.len(), 2);
    let onboard = match inspect_captured_report(&onboard.records[1]) {
        RaidReportInspection::Profile(profile) => *profile,
        other => panic!("expected captured onboard image, got {other:?}"),
    };
    (image(0), image(1), onboard)
}

#[test]
fn cold_runtime_read_is_empty_even_after_legacy_session_start() {
    let (empty, write, _) = captured_profiles();
    assert_eq!(empty.kind(), ProfileImageKind::DeviceReadResponse);
    assert_eq!(empty.section(), ProfileSection::Runtime);
    assert_eq!(&empty.as_bytes()[..3], &[0x07, 0x81, 0x04]);
    assert!(empty.as_bytes()[3..].iter().all(|byte| *byte == 0));
    assert!(empty.validate_confirmed_runtime_settings().is_err());

    // Flipping the read-response opcode cannot turn this empty image into a
    // usable runtime baseline. This asserts only offline validation behavior.
    let naive_write = PerformanceProfile::parse(&empty.to_write_report()).unwrap();
    assert!(naive_write.validate_confirmed_runtime_settings().is_err());
    assert_eq!(write.kind(), ProfileImageKind::HostWrite);
    assert_eq!(write.section(), ProfileSection::Runtime);
    assert!(write.validate_confirmed_runtime_settings().is_ok());
}

#[test]
fn cold_legacy_write_matches_known_onboard_settings_but_not_opaque_bytes() {
    let (_, write, onboard) = captured_profiles();
    assert_eq!(onboard.section(), ProfileSection::Onboard);
    assert_eq!(
        write.polling_rate().unwrap(),
        onboard.polling_rate().unwrap()
    );
    assert_eq!(write.dpi_profile().unwrap(), onboard.dpi_profile().unwrap());
    assert_eq!(
        write.primary_button_layout().unwrap(),
        onboard.primary_button_layout().unwrap()
    );
    for control in PulsefireRaidControl::ALL {
        assert_eq!(
            write.has_confirmed_macro_reference(control),
            onboard.has_confirmed_macro_reference(control),
            "{} macro reference differs",
            control.name()
        );
        if !write.has_confirmed_macro_reference(control) {
            assert_eq!(
                write.button_binding(control).unwrap(),
                onboard.button_binding(control).unwrap(),
                "{} binding differs",
                control.name()
            );
        }
    }
    assert!(write.has_confirmed_macro_reference(PulsefireRaidControl::Button5));

    // Ignore only the confirmed read/write opcode and onboard/runtime section
    // bytes. All remaining differences are opaque to the *current* codec;
    // this list is capture evidence, not permission to zero or copy them.
    let differing_body_offsets = differing_body_offsets(&onboard, &write);
    assert_eq!(
        differing_body_offsets,
        [
            0x24, 0x30, 0x37, 0x38, 0x3E, 0x41, 0x46, 0x48, 0x4A, 0x4D, 0x51, 0x53, 0x57, 0x5B,
            0x5D, 0x5E, 0x60, 0x61, 0x63, 0x64, 0x68, 0x78, 0x79, 0xE3
        ]
    );
    assert!(differing_body_offsets
        .iter()
        .all(|offset| write.as_bytes()[*offset] == 0));
}

#[test]
fn warm_legacy_read_write_only_changes_opcode_and_has_section_specific_opaque_bytes() {
    let warm = parse_report_log(WARM).unwrap();
    assert_eq!(warm.records.len(), 2);
    assert_eq!(warm.records[0].report.direction, Some(Direction::Rx));
    assert_eq!(warm.records[1].report.direction, Some(Direction::Tx));
    let image = |index| match inspect_captured_report(&warm.records[index]) {
        RaidReportInspection::Profile(profile) => *profile,
        other => panic!("expected warm runtime profile, got {other:?}"),
    };
    let warm_read = image(0);
    let warm_write = image(1);
    assert_eq!(warm_read.kind(), ProfileImageKind::DeviceReadResponse);
    assert_eq!(warm_write.kind(), ProfileImageKind::HostWrite);
    assert_eq!(warm_read.section(), ProfileSection::Runtime);
    assert_eq!(warm_read.to_write_report(), *warm_write.as_bytes());
    assert!(warm_read.validate_confirmed_runtime_settings().is_ok());

    let (_, cold_write, onboard) = captured_profiles();
    assert_eq!(
        warm_write.polling_rate().unwrap(),
        cold_write.polling_rate().unwrap()
    );
    assert_eq!(
        warm_write.dpi_profile().unwrap(),
        cold_write.dpi_profile().unwrap()
    );
    for control in PulsefireRaidControl::ALL {
        assert_eq!(
            warm_write.has_confirmed_macro_reference(control),
            cold_write.has_confirmed_macro_reference(control)
        );
        if !warm_write.has_confirmed_macro_reference(control) {
            assert_eq!(
                warm_write.button_binding(control).unwrap(),
                cold_write.button_binding(control).unwrap()
            );
        }
    }
    // Same known settings, different opaque bytes. Their values must not be
    // synthesized from this single warm/cold contrast.
    let changed = differing_body_offsets(&warm_write, &cold_write);
    assert_eq!(changed.len(), 24);
    assert!(changed
        .iter()
        .all(|offset| warm_write.as_bytes()[*offset] != 0 && cold_write.as_bytes()[*offset] == 0));
    assert_eq!(
        differing_body_offsets(&onboard, &warm_read),
        [0x37, 0x38, 0x78, 0x79, 0xE3]
    );
}
