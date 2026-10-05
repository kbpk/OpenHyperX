//! Production driver API safety: no public profile path may transmit a report.
//! This integration test links the library without its private mock-only test
//! constructor, so it exercises the same gate as CLI, GUI and TUI clients.

use hyperx_core::{
    ButtonBinding, MouseFunction, PollingRate, PrimaryButtonLayout, RgbColor,
    SoftwarePollingProfile, SoftwareProfile,
};
use hyperx_devices::{
    pulsefire_raid::PulsefireRaidControl, PulsefireRaid, PulsefireRaidError,
    PulsefireRaidOnboardMacros, PulsefireRaidProfileError, PulsefireRaidRuntimeAssignment,
    PulsefireRaidSoftwareProfile,
};
use hyperx_hid::testing::MockHidTransport;
use hyperx_protocol::pulsefire_raid::encode_direct_rgb;

macro_rules! suspended {
    ($operation:expr) => {{
        let result = $operation;
        assert!(
            matches!(result, Err(PulsefireRaidError::RuntimeAccessSuspended)),
            "expected suspended profile access, got {result:?}"
        );
    }};
}

#[test]
fn every_public_runtime_and_save_entrypoint_stops_before_hid_io() {
    let configuration = MockHidTransport::new(1);
    let mut acknowledgements = MockHidTransport::new(2);
    let mut device = PulsefireRaid::new(configuration).unwrap();
    let black = RgbColor::BLACK;
    let macros = PulsefireRaidOnboardMacros::default();
    let assignment = PulsefireRaidRuntimeAssignment::ordinary(
        PulsefireRaidControl::Button4,
        ButtonBinding::Mouse(MouseFunction::Back),
    )
    .unwrap();
    let software = PulsefireRaidSoftwareProfile::new(&SoftwareProfile {
        device: "pulsefire-raid".into(),
        polling: Some(SoftwarePollingProfile { hz: 500 }),
        ..SoftwareProfile::default()
    })
    .unwrap();

    suspended!(device.runtime_profile());
    suspended!(device.set_runtime_active_dpi(900));
    suspended!(device.set_runtime_dpi_stage(0, Some(900), None, false));
    suspended!(device.add_runtime_dpi_stage(1600, black, false));
    suspended!(device.remove_runtime_last_dpi_stage());
    suspended!(device.set_runtime_active_dpi_stage(0));
    suspended!(device.set_runtime_polling_rate(PollingRate::Hz500));
    suspended!(device.set_runtime_primary_button_layout(PrimaryButtonLayout::Swapped));
    suspended!(device.set_runtime_button_assignment(assignment));
    suspended!(device.initialize_vendor_session(&mut acknowledgements));
    suspended!(device.check_save_ack_path(&mut acknowledgements));
    suspended!(device.save_runtime_to_onboard(&mut acknowledgements, black, black, None));
    suspended!(device.save_runtime_to_onboard_with_macros(
        &mut acknowledgements,
        black,
        black,
        &macros
    ));
    assert!(matches!(
        device.preview_software_profile(&software),
        Err(PulsefireRaidProfileError::Driver(
            PulsefireRaidError::RuntimeAccessSuspended
        ))
    ));
    assert!(matches!(
        device.apply_software_profile(&software),
        Err(PulsefireRaidProfileError::Driver(
            PulsefireRaidError::RuntimeAccessSuspended
        ))
    ));

    let configuration = device.into_transport();
    assert!(configuration.feature_reports_sent().is_empty());
    assert_eq!(configuration.feature_read_attempts(), 0);
    configuration.assert_drained();
    assert!(acknowledgements.feature_reports_sent().is_empty());
    assert_eq!(acknowledgements.feature_read_attempts(), 0);
    acknowledgements.assert_drained();
}

#[test]
fn suspended_profile_access_does_not_disable_independent_direct_rgb() {
    let wheel = RgbColor::BLACK;
    let logo = RgbColor::new(0, 0, 255);
    let mut configuration = MockHidTransport::new(1);
    configuration.expect_feature_report(encode_direct_rgb(wheel, logo));
    let mut device = PulsefireRaid::new(configuration).unwrap();

    device.set_volatile_direct_rgb(wheel, logo).unwrap();

    let configuration = device.into_transport();
    assert_eq!(configuration.feature_reports_sent().len(), 1);
    assert_eq!(configuration.feature_read_attempts(), 0);
    configuration.assert_drained();
}
