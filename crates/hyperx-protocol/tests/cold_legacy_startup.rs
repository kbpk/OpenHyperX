//! Offline evidence only. These captured images must never be replayed.

use hyperx_protocol::{
    capture::parse_report_log,
    pulsefire_raid::{
        classify_profile_image_byte, inspect_captured_report, vendor_profile_field_at,
        PerformanceProfile, ProfileImageByteClass, ProfileImageKind, ProfileSection,
        PulsefireRaidControl, RaidReportInspection, DIRECT_REPORT_LENGTH,
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
    // bytes. Legacy's serializer now names these fields, but their values are
    // still opaque to our codec; this is not permission to zero or copy them.
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
    assert!(differing_body_offsets.iter().all(|&offset| {
        classify_profile_image_byte(offset) == Some(ProfileImageByteClass::VendorNamedRegion)
    }));
}

#[test]
fn profile_byte_layout_separates_decoded_and_vendor_named_regions() {
    for offset in 0..3 {
        assert_eq!(
            classify_profile_image_byte(offset),
            Some(ProfileImageByteClass::Envelope)
        );
    }
    for offset in [
        0x18, 0x19, 0x22, 0x25, 0x2E, 0x31, 0x32, 0x36, 0x69, 0x77, 0x7C, 0xA7,
    ] {
        assert_eq!(
            classify_profile_image_byte(offset),
            Some(ProfileImageByteClass::MappedSettingRegion),
            "offset 0x{offset:02X}"
        );
    }
    for offset in [
        0x16, 0x23, 0x24, 0x2F, 0x30, 0x37, 0x38, 0x3A, 0x3E, 0x5B, 0x5D, 0x68, 0x78, 0x79, 0xA8,
        0xAF, 0xB0, 0xE3,
    ] {
        assert_eq!(
            classify_profile_image_byte(offset),
            Some(ProfileImageByteClass::VendorNamedRegion),
            "offset 0x{offset:02X}"
        );
    }
    for offset in [0x17, 0x39, 0x3B, 0x5C, 0x7A, 0x7B, 0xE4, 0x107] {
        assert_eq!(
            classify_profile_image_byte(offset),
            Some(ProfileImageByteClass::UninterpretedBody),
            "offset 0x{offset:02X}"
        );
    }
    for (offset, name) in [
        (0x16, "LED intensity"),
        (0x24, "DPI sniper value"),
        (0x30, "DPI sniper mirror (write-only in Legacy parser)"),
        (0x37, "logo LED mode"),
        (0x38, "wheel LED mode"),
        (0xA8, "extra vendor button records 12-13"),
        (0xAF, "extra vendor button records 12-13"),
        (0xB0, "vendor single-key records (13 slots)"),
        (0xE3, "vendor single-key records (13 slots)"),
    ] {
        assert_eq!(vendor_profile_field_at(offset), Some(name));
    }
    assert_eq!(vendor_profile_field_at(0xE4), None);
    assert_eq!(classify_profile_image_byte(DIRECT_REPORT_LENGTH), None);
}

#[test]
fn extra_records_after_the_eleven_controls_are_retained_but_not_mapped() {
    let (empty, cold_write, onboard) = captured_profiles();
    let warm = parse_report_log(WARM).unwrap();
    let expected = [0x02, 0xF3, 0x00, 0x00, 0x02, 0xF4, 0x00, 0x00];

    assert_eq!(&empty.as_bytes()[0xA8..0xB0], &[0; 8]);
    for profile in [cold_write, onboard] {
        assert_eq!(&profile.as_bytes()[0xA8..0xB0], &expected);
    }
    for record in &warm.records {
        let RaidReportInspection::Profile(profile) = inspect_captured_report(record) else {
            panic!("expected a captured warm profile image");
        };
        assert_eq!(&profile.as_bytes()[0xA8..0xB0], &expected);
    }
    for offset in 0xA8..0xB0 {
        assert_eq!(
            classify_profile_image_byte(offset),
            Some(ProfileImageByteClass::VendorNamedRegion)
        );
    }
}

#[test]
fn cold_legacy_overlay_is_not_a_complete_runtime_restore_template() {
    let (_, cold_write, onboard) = captured_profiles();

    // The installed Legacy InitProfile method overlays brightness, DPI,
    // polling and button records after a profile GET. It does not initialize
    // these effect/single-key regions. This captured cold GET was all zero,
    // so the following TX retained zeros here even though onboard data had
    // nonzero values. These assertions are evidence against replaying TX as
    // a general restore image, not a recipe for constructing another TX.
    for (range, name) in [
        (0x37..0x39, "LED modes"),
        (0x3E..0x69, "LED palettes and effect parameters"),
        (0x78..0x7A, "bitmap metadata"),
        (0xB0..0xE4, "single-key records"),
    ] {
        assert!(
            cold_write.as_bytes()[range].iter().all(|byte| *byte == 0),
            "cold TX unexpectedly populates {name}"
        );
    }
    assert_eq!(cold_write.as_bytes()[0x16], 0x64);
    assert_eq!(onboard.as_bytes()[0x37], 0x04);
    assert_eq!(onboard.as_bytes()[0x78], 0x01);
    assert_eq!(onboard.as_bytes()[0xE3], 0xD5);
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
