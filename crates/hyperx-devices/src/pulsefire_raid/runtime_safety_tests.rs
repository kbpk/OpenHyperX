//! Regressions for full-image writes. No test opens a real HID device.
use hyperx_hid::testing::MockHidTransport;

use super::*;

#[derive(Clone, Copy, Debug)]
enum Mutation {
    ActiveDpi,
    StageDpi,
    StageSelection,
    AddStage,
    RemoveStage,
    Polling,
    PrimaryButtons,
    OrdinaryButton,
    Button4Macro,
    Button5Macro,
}

impl Mutation {
    const ALL: [Self; 10] = [
        Self::ActiveDpi,
        Self::StageDpi,
        Self::StageSelection,
        Self::AddStage,
        Self::RemoveStage,
        Self::Polling,
        Self::PrimaryButtons,
        Self::OrdinaryButton,
        Self::Button4Macro,
        Self::Button5Macro,
    ];

    fn run(self, device: &mut PulsefireRaid<MockHidTransport>) -> Result<(), PulsefireRaidError> {
        match self {
            Self::ActiveDpi => device
                .set_runtime_active_dpi_with_wait(900, |_| {})
                .map(|_| ()),
            Self::StageDpi => device
                .set_runtime_dpi_stage_with_wait(0, Some(900), None, false, |_| {})
                .map(|_| ()),
            Self::StageSelection => device
                .set_runtime_dpi_stage_with_wait(0, None, None, true, |_| {})
                .map(|_| ()),
            Self::AddStage => device
                .add_runtime_dpi_stage_with_wait(1600, RgbColor::BLACK, false, |_| {})
                .map(|_| ()),
            Self::RemoveStage => device
                .remove_runtime_last_dpi_stage_with_wait(|_| {})
                .map(|_| ()),
            Self::Polling => device
                .set_runtime_polling_rate_with_wait(PollingRate::Hz1000, |_| {})
                .map(|_| ()),
            Self::PrimaryButtons => device
                .set_runtime_primary_button_layout_with_wait(PrimaryButtonLayout::Swapped, |_| {})
                .map(|_| ()),
            Self::OrdinaryButton => device
                .set_runtime_button_assignment_with_wait(
                    PulsefireRaidRuntimeAssignment::ordinary(
                        PulsefireRaidControl::Button4,
                        ButtonBinding::Mouse(MouseFunction::Back),
                    )?,
                    |_| {},
                )
                .map(|_| ()),
            Self::Button4Macro | Self::Button5Macro => device
                .set_runtime_button_assignment_with_wait(
                    PulsefireRaidRuntimeAssignment::macro_timeline(
                        if matches!(self, Self::Button4Macro) {
                            PulsefireRaidControl::Button4
                        } else {
                            PulsefireRaidControl::Button5
                        },
                        super::tests::captured_ab_macro(),
                    )?,
                    |_| {},
                )
                .map(|_| ()),
        }
    }
}

fn baseline() -> [u8; DIRECT_REPORT_LENGTH] {
    let raw = super::tests::save_profile_response(ProfileSection::Runtime, false);
    let mut profile = PerformanceProfile::parse(&raw).unwrap();
    // Two stages make remove/add/select valid operations in the failure matrix.
    profile
        .set_dpi_profile(&DpiProfile {
            stages: vec![
                hyperx_core::DpiStage::new(800, 800, RgbColor::BLACK),
                hyperx_core::DpiStage::new(1600, 1600, RgbColor::BLACK),
            ],
            active_stage: 0,
        })
        .unwrap();
    *profile.as_bytes()
}

fn read_script() -> MockHidTransport {
    let mut mock = MockHidTransport::new(1);
    mock.expect_feature_report(encode_runtime_profile_read_prelude());
    mock.expect_feature_report(encode_profile_read_request());
    mock
}

fn assert_only_one_read(mock: MockHidTransport) {
    assert_eq!(
        mock.feature_reports_sent(),
        &[
            encode_runtime_profile_read_prelude().to_vec(),
            encode_profile_read_request().to_vec(),
        ]
    );
    assert_eq!(mock.feature_read_attempts(), 1);
    mock.assert_drained();
}

#[test]
fn every_runtime_mutation_rejects_unusable_baseline_before_any_setting_or_macro_write() {
    let mut cases = Vec::new();
    let mut empty = [0_u8; DIRECT_REPORT_LENGTH];
    empty[..3].copy_from_slice(&[7, 0x81, 4]);
    cases.push(("empty body", empty));
    // Bad fields must block writes even when the caller intends to replace that
    // exact field. Never repair the read image by guessing the rest of its state.
    for (name, offset, value) in [
        ("unknown polling", 0x18, 0),
        ("invalid active stage", 0x31, 4),
        ("invalid stage flag", 0x33, 2),
        ("out-of-range active X", 0x1A, 1), // 1 * 50 = 50 DPI
        ("out-of-range inactive Y", 0x28, 1),
        ("invalid primary pair", 0x7D, 0xF2), // two right clicks
        ("unknown unrelated button", 0x97, 0xFF), // Button 6
        ("unknown macro reference", 0x8C, 0x53), // wrong slot trailer
    ] {
        let mut raw = baseline();
        raw[offset] = value;
        cases.push((name, raw));
    }
    let mut noncontiguous = baseline();
    noncontiguous[0x32] = 0;
    cases.push(("noncontiguous DPI", noncontiguous));
    let mut no_dpi = baseline();
    no_dpi[0x32..0x37].fill(0);
    cases.push(("no enabled DPI", no_dpi));

    for (name, response) in cases {
        for operation in Mutation::ALL {
            let mut mock = read_script();
            mock.queue_feature_response(response);
            let mut device = PulsefireRaid::new(mock).unwrap();
            let error = operation.run(&mut device).expect_err(name);
            assert!(
                matches!(error, PulsefireRaidError::InvalidProfile(_)),
                "{name}, {operation:?}: {error}"
            );
            assert_only_one_read(device.into_transport());
        }
    }
}

#[test]
fn every_runtime_mutation_stops_on_truncated_wrong_section_and_wrong_kind_responses() {
    let mut onboard = baseline();
    onboard[2] = 1;
    let mut write_kind = baseline();
    write_kind[1] = 1;
    for response in [vec![7, 0x81, 4], onboard.to_vec(), write_kind.to_vec()] {
        for operation in Mutation::ALL {
            let mut mock = read_script();
            mock.queue_feature_response(response.clone());
            let mut device = PulsefireRaid::new(mock).unwrap();
            assert!(matches!(
                operation.run(&mut device),
                Err(PulsefireRaidError::WrongProfileResponseLength(3))
                    | Err(PulsefireRaidError::WrongProfileResponse { .. })
            ));
            assert_only_one_read(device.into_transport());
        }
    }
}

#[test]
fn every_runtime_mutation_stops_on_read_error_without_retry_or_settings_writes() {
    for operation in Mutation::ALL {
        let mut mock = read_script();
        mock.queue_feature_error(HidError::Transport("injected GET_REPORT timeout".into()));
        let mut device = PulsefireRaid::new(mock).unwrap();
        let error = operation.run(&mut device).unwrap_err();
        assert!(error.to_string().contains("injected GET_REPORT timeout"));
        assert_only_one_read(device.into_transport());
    }
}

#[test]
fn every_runtime_mutation_stops_at_a_failed_read_selector_or_request() {
    for failing_packet in 0..2 {
        for short_write in [false, true] {
            for operation in Mutation::ALL {
                let mut mock = MockHidTransport::new(1);
                let packets = [
                    encode_runtime_profile_read_prelude(),
                    encode_profile_read_request(),
                ];
                for (index, packet) in packets.iter().enumerate().take(failing_packet + 1) {
                    if index == failing_packet {
                        mock.expect_feature_report_result(
                            packet.to_vec(),
                            if short_write {
                                Ok(DIRECT_REPORT_LENGTH - 1)
                            } else {
                                Err(HidError::Transport("injected read TX failure".into()))
                            },
                        );
                    } else {
                        mock.expect_feature_report(packet.to_vec());
                    }
                }
                let mut device = PulsefireRaid::new(mock).unwrap();
                assert!(matches!(
                    operation.run(&mut device),
                    Err(PulsefireRaidError::ShortFeatureWrite { .. })
                        | Err(PulsefireRaidError::Transport(_))
                ));
                let mock = device.into_transport();
                assert_eq!(
                    mock.feature_reports_sent(),
                    &packets[..=failing_packet]
                        .iter()
                        .map(|packet| packet.to_vec())
                        .collect::<Vec<_>>()
                );
                assert_eq!(mock.feature_read_attempts(), 0);
                mock.assert_drained();
            }
        }
    }
}

#[test]
fn final_runtime_write_guard_rejects_invalid_images_without_io() {
    let mut invalid = baseline();
    invalid[0x18] = 0;
    let profile = PerformanceProfile::parse(&invalid).unwrap();
    let mut device = PulsefireRaid::new(MockHidTransport::new(1)).unwrap();
    assert!(matches!(
        device.write_runtime_profile(&profile),
        Err(PulsefireRaidError::InvalidProfile(_))
    ));
    let mock = device.into_transport();
    assert!(mock.feature_reports_sent().is_empty());
    assert_eq!(mock.feature_read_attempts(), 0);
    mock.assert_drained();
}

#[test]
fn macro_upload_short_write_or_error_stops_before_reference_and_without_retry() {
    let assignment = PulsefireRaidRuntimeAssignment::macro_timeline(
        PulsefireRaidControl::Button4,
        super::tests::captured_ab_macro(),
    )
    .unwrap();
    let macro_report = assignment.encoded_macro().unwrap().unwrap();
    for result in [
        Ok(DIRECT_REPORT_LENGTH - 1),
        Err(HidError::Transport("injected macro upload failure".into())),
    ] {
        let mut mock = read_script();
        mock.queue_feature_response(baseline());
        mock.expect_feature_report_result(macro_report.as_bytes().to_vec(), result);
        let mut device = PulsefireRaid::new(mock).unwrap();
        assert!(matches!(
            device.set_runtime_button_assignment_with_wait(assignment.clone(), |_| {}),
            Err(PulsefireRaidError::ShortFeatureWrite { .. })
                | Err(PulsefireRaidError::Transport(_))
        ));
        let mock = device.into_transport();
        assert_eq!(mock.feature_reports_sent().len(), 3);
        assert_eq!(mock.feature_reports_sent()[2], macro_report.as_bytes());
        assert_eq!(mock.feature_read_attempts(), 1);
        mock.assert_drained();
    }
}

#[test]
fn failed_macro_reference_write_does_not_retry_or_claim_to_undo_uploaded_definition() {
    let assignment = PulsefireRaidRuntimeAssignment::macro_timeline(
        PulsefireRaidControl::Button4,
        super::tests::captured_ab_macro(),
    )
    .unwrap();
    let definition = assignment.encoded_macro().unwrap().unwrap();
    let mut profile = PerformanceProfile::parse(&baseline()).unwrap();
    definition.apply_to_profile(&mut profile).unwrap();
    for short_write in [false, true] {
        let mut mock = read_script();
        mock.queue_feature_response(baseline());
        mock.expect_feature_report(definition.as_bytes().to_vec());
        mock.expect_feature_report_result(
            profile.to_write_report(),
            if short_write {
                Ok(DIRECT_REPORT_LENGTH - 1)
            } else {
                Err(HidError::Transport(
                    "injected reference write failure".into(),
                ))
            },
        );
        let mut device = PulsefireRaid::new(mock).unwrap();
        assert!(device
            .set_runtime_button_assignment_with_wait(assignment.clone(), |_| {})
            .is_err());
        let mock = device.into_transport();
        assert_eq!(mock.feature_reports_sent().len(), 4);
        assert_eq!(mock.feature_reports_sent()[3], profile.to_write_report());
        assert_eq!(mock.feature_read_attempts(), 1);
        mock.assert_drained();
    }
}
