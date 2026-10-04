//! Captured responses are parsed offline; none of these bytes are transmitted.

use hyperx_core::{ButtonBinding, MouseFunction, PollingRate, PrimaryButtonLayout};
use hyperx_protocol::{
    capture::parse_report_log,
    pulsefire_raid::{
        PerformanceProfileError, ProfileImageKind, ProfileSection, PulsefireRaidControl,
        PulsefireRaidObservedBinding, PulsefireRaidProfileObservation, DIRECT_REPORT_LENGTH,
    },
};

fn captured_response(source: &str, index: usize) -> [u8; DIRECT_REPORT_LENGTH] {
    parse_report_log(source).unwrap().records[index]
        .report
        .bytes
        .as_slice()
        .try_into()
        .unwrap()
}

#[test]
fn selector_free_capture_is_an_onboard_observation_with_typed_known_fields() {
    let report = captured_response(include_str!("fixtures/read-request-get-onboard.hex"), 1);
    let observed = PulsefireRaidProfileObservation::parse(&report).unwrap();

    assert_eq!(observed.section(), ProfileSection::Onboard);
    assert_eq!(observed.polling_rate(), PollingRate::Hz1000);
    assert_eq!(
        observed.primary_button_layout(),
        PrimaryButtonLayout::Standard
    );
    assert_eq!(observed.dpi_profile().active_stage, 0);
    assert_eq!(
        observed
            .dpi_profile()
            .stages
            .iter()
            .map(|stage| stage.x)
            .collect::<Vec<_>>(),
        [800, 1600, 3200, 6400]
    );
    assert_eq!(observed.bindings().len(), 11);
    assert_eq!(
        observed.binding(PulsefireRaidControl::Button4),
        &PulsefireRaidObservedBinding::Binding(ButtonBinding::Mouse(MouseFunction::Back))
    );
    assert_eq!(
        observed.binding(PulsefireRaidControl::Button5),
        &PulsefireRaidObservedBinding::MacroReference
    );
}

#[test]
fn warm_runtime_response_is_only_an_observation() {
    let report = captured_response(include_str!("fixtures/warm-legacy-startup-images.hex"), 0);
    let observed = PulsefireRaidProfileObservation::parse(&report).unwrap();
    assert_eq!(observed.section(), ProfileSection::Runtime);
    assert_eq!(observed.bindings().len(), 11);
}

#[test]
fn empty_opaque_host_write_and_invalid_known_settings_are_rejected() {
    let empty = captured_response(include_str!("fixtures/cold-legacy-startup-images.hex"), 0);
    assert!(PulsefireRaidProfileObservation::parse(&empty).is_err());

    let mut opaque = [0; DIRECT_REPORT_LENGTH];
    opaque[..4].copy_from_slice(&[7, 0xFF, 0xFF, 0xFF]);
    assert!(PulsefireRaidProfileObservation::parse(&opaque).is_err());

    let onboard = captured_response(include_str!("fixtures/read-request-get-onboard.hex"), 1);
    let mut host_write = onboard;
    host_write[1] = 1;
    assert_eq!(
        PulsefireRaidProfileObservation::parse(&host_write),
        Err(PerformanceProfileError::ExpectedDeviceReadResponse(
            ProfileImageKind::HostWrite
        ))
    );

    let mut invalid_polling = onboard;
    invalid_polling[0x18] = 3;
    assert_eq!(
        PulsefireRaidProfileObservation::parse(&invalid_polling),
        Err(PerformanceProfileError::UnknownPollingInterval(3))
    );

    let mut invalid_binding = onboard;
    invalid_binding[0x88..0x8C].copy_from_slice(&[0xFF; 4]);
    assert!(PulsefireRaidProfileObservation::parse(&invalid_binding).is_err());
}
