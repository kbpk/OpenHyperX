use std::time::Duration;

use hyperx_core::{
    Capability, CapabilitySet, DeviceDescriptor, DpiCapabilities, DpiProfile, DpiValidationError,
    InterfaceSelector, LightingZone, PollingRate, RgbColor, UsbId,
};
use hyperx_hid::{HidError, HidTransport};
use hyperx_protocol::pulsefire_raid::{
    encode_direct_rgb, encode_profile_read_request, encode_runtime_profile_read_prelude,
    PerformanceProfile, PerformanceProfileError, ProfileImageKind, ProfileSection,
    DIRECT_REPORT_ID, DIRECT_REPORT_LENGTH,
};
use thiserror::Error;

const PROFILE_PRELUDE_DELAY: Duration = Duration::from_millis(65);
const PROFILE_READ_DELAY: Duration = Duration::from_millis(110);

pub const PULSEFIRE_RAID_DPI: DpiCapabilities = DpiCapabilities {
    minimum: 200,
    maximum: 16_000,
    step: 50,
    max_stages: 5,
};

const POLLING_RATES: &[PollingRate] = &[
    PollingRate::Hz125,
    PollingRate::Hz250,
    PollingRate::Hz500,
    PollingRate::Hz1000,
];

const FEATURES: &[Capability] = &[
    Capability::Dpi,
    Capability::DpiStages,
    Capability::PollingRate,
    Capability::ButtonRemapping,
    Capability::Macros,
    Capability::Lighting,
    Capability::OnboardProfile,
];

const LIGHTING_ZONES: &[LightingZone] = &[
    LightingZone {
        id: "wheel",
        name: "Scroll Wheel",
    },
    LightingZone {
        id: "logo",
        name: "Logo",
    },
];

/// Pulsefire Raid identity and hardware-level capabilities.
///
/// Protocol support is tracked separately and must never be inferred from
/// this hardware declaration.
pub const PULSEFIRE_RAID: DeviceDescriptor = DeviceDescriptor {
    id: "pulsefire-raid",
    name: "HyperX Pulsefire Raid",
    usb_id: UsbId {
        vendor_id: 0x0951,
        product_id: 0x16E4,
    },
    capabilities: CapabilitySet {
        features: FEATURES,
        button_count: 11,
        lighting_zones: LIGHTING_ZONES,
        dpi: Some(PULSEFIRE_RAID_DPI),
        polling_rates: POLLING_RATES,
    },
    configuration_interface: InterfaceSelector {
        interface_number: 1,
        usage_page: 0xFF01,
        usage: 0x0001,
    },
};

#[derive(Debug, Error)]
pub enum PulsefireRaidError {
    #[error("Pulsefire Raid configuration requires HID interface 1, got {0}")]
    WrongInterface(i32),
    #[error("feature report write was short: expected {expected} bytes, wrote {actual}")]
    ShortFeatureWrite { expected: usize, actual: usize },
    #[error("expected a 264-byte Pulsefire Raid profile response, got {0} bytes")]
    WrongProfileResponseLength(usize),
    #[error("expected a runtime profile read response, got {kind:?} {section:?}")]
    WrongProfileResponse {
        kind: ProfileImageKind,
        section: ProfileSection,
    },
    #[error(transparent)]
    InvalidDpi(#[from] DpiValidationError),
    #[error(transparent)]
    InvalidProfile(#[from] PerformanceProfileError),
    #[error(transparent)]
    Transport(#[from] HidError),
}

pub struct PulsefireRaid<T> {
    transport: T,
}

impl<T: HidTransport> PulsefireRaid<T> {
    pub fn new(transport: T) -> Result<Self, PulsefireRaidError> {
        let interface = transport.interface_number();
        if interface != PULSEFIRE_RAID.configuration_interface.interface_number {
            return Err(PulsefireRaidError::WrongInterface(interface));
        }
        Ok(Self { transport })
    }

    /// Set volatile direct colors for both physical LEDs.
    ///
    /// The device returns to its stored effect unless this is periodically
    /// refreshed. This operation does not save to onboard memory.
    pub fn set_volatile_direct_rgb(
        &mut self,
        wheel: RgbColor,
        logo: RgbColor,
    ) -> Result<(), PulsefireRaidError> {
        let report = encode_direct_rgb(wheel, logo);
        self.send_feature_report(&report)?;
        std::thread::sleep(std::time::Duration::from_millis(10));
        Ok(())
    }

    /// Read the current volatile performance/button profile.
    ///
    /// This replays the exact read sequence observed twice in one local
    /// capture and in earlier isolated setting captures. It performs no
    /// profile write and deliberately does not retry an ambiguous failure.
    pub fn runtime_profile(&mut self) -> Result<PerformanceProfile, PulsefireRaidError> {
        self.runtime_profile_with_wait(std::thread::sleep)
    }

    fn runtime_profile_with_wait(
        &mut self,
        mut wait: impl FnMut(Duration),
    ) -> Result<PerformanceProfile, PulsefireRaidError> {
        let prelude = encode_runtime_profile_read_prelude();
        self.send_feature_report(&prelude)?;
        wait(PROFILE_PRELUDE_DELAY);

        let request = encode_profile_read_request();
        self.send_feature_report(&request)?;
        wait(PROFILE_READ_DELAY);

        let mut response = [0_u8; DIRECT_REPORT_LENGTH];
        response[0] = DIRECT_REPORT_ID;
        let response_length = self.transport.get_feature_report(&mut response)?;
        if response_length != DIRECT_REPORT_LENGTH {
            return Err(PulsefireRaidError::WrongProfileResponseLength(
                response_length,
            ));
        }

        let profile = PerformanceProfile::parse(&response)?;
        if profile.kind() != ProfileImageKind::DeviceReadResponse
            || profile.section() != ProfileSection::Runtime
        {
            return Err(PulsefireRaidError::WrongProfileResponse {
                kind: profile.kind(),
                section: profile.section(),
            });
        }

        Ok(profile)
    }

    /// Change both axes of the active DPI stage in the runtime profile.
    ///
    /// Every other stage, color, binding and unknown profile byte is preserved.
    /// This does not write the onboard profile.
    pub fn set_runtime_active_dpi(&mut self, dpi: u32) -> Result<DpiProfile, PulsefireRaidError> {
        self.set_runtime_active_dpi_with_wait(dpi, std::thread::sleep)
    }

    fn set_runtime_active_dpi_with_wait(
        &mut self,
        dpi: u32,
        wait: impl FnMut(Duration),
    ) -> Result<DpiProfile, PulsefireRaidError> {
        let dpi = PULSEFIRE_RAID_DPI.validate(dpi)?;
        let mut profile = self.runtime_profile_with_wait(wait)?;
        let mut dpi_profile = profile.dpi_profile()?;
        let stage = &mut dpi_profile.stages[dpi_profile.active_stage];
        stage.x = dpi;
        stage.y = dpi;
        profile.set_dpi_profile(&dpi_profile)?;
        self.write_runtime_profile(&profile)?;
        Ok(dpi_profile)
    }

    /// Change the polling rate in the runtime profile only.
    pub fn set_runtime_polling_rate(
        &mut self,
        polling_rate: PollingRate,
    ) -> Result<PollingRate, PulsefireRaidError> {
        self.set_runtime_polling_rate_with_wait(polling_rate, std::thread::sleep)
    }

    fn set_runtime_polling_rate_with_wait(
        &mut self,
        polling_rate: PollingRate,
        wait: impl FnMut(Duration),
    ) -> Result<PollingRate, PulsefireRaidError> {
        let mut profile = self.runtime_profile_with_wait(wait)?;
        profile.set_polling_rate(polling_rate);
        self.write_runtime_profile(&profile)?;
        Ok(polling_rate)
    }

    fn write_runtime_profile(
        &mut self,
        profile: &PerformanceProfile,
    ) -> Result<(), PulsefireRaidError> {
        if profile.section() != ProfileSection::Runtime {
            return Err(PulsefireRaidError::WrongProfileResponse {
                kind: profile.kind(),
                section: profile.section(),
            });
        }
        self.send_feature_report(&profile.to_write_report())?;
        Ok(())
    }

    fn send_feature_report(&mut self, report: &[u8]) -> Result<(), PulsefireRaidError> {
        let actual = self.transport.send_feature_report(report)?;
        if actual != report.len() {
            return Err(PulsefireRaidError::ShortFeatureWrite {
                expected: report.len(),
                actual,
            });
        }
        Ok(())
    }

    pub fn into_transport(self) -> T {
        self.transport
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hyperx_hid::testing::MockHidTransport;

    #[test]
    fn configuration_collection_matches_openrgb_detector() {
        assert_eq!(PULSEFIRE_RAID.configuration_interface.interface_number, 1);
        assert_eq!(PULSEFIRE_RAID.configuration_interface.usage_page, 0xFF01);
        assert_eq!(PULSEFIRE_RAID.configuration_interface.usage, 0x0001);
        assert_eq!(PULSEFIRE_RAID.capabilities.dpi, Some(PULSEFIRE_RAID_DPI));
        assert_eq!(PULSEFIRE_RAID.capabilities.polling_rates, POLLING_RATES);
    }

    #[test]
    fn driver_sends_exact_direct_rgb_feature_report() {
        let mut expected = [0_u8; DIRECT_REPORT_LENGTH];
        expected[..9].copy_from_slice(&[0x07, 0x0A, 0xFF, 0x00, 0x00, 0xFF, 0x00, 0x00, 0xA0]);
        let mut transport = MockHidTransport::new(1);
        transport.expect_feature_report(expected);

        let mut device = PulsefireRaid::new(transport).unwrap();
        device
            .set_volatile_direct_rgb(RgbColor::new(0xFF, 0, 0), RgbColor::new(0xFF, 0, 0))
            .unwrap();
        device.into_transport().assert_drained();
    }

    #[test]
    fn driver_reads_runtime_profile_with_exact_sequence_and_timing() {
        let prelude = encode_runtime_profile_read_prelude();
        let request = encode_profile_read_request();
        let mut response = [0_u8; DIRECT_REPORT_LENGTH];
        response[..3].copy_from_slice(&[0x07, 0x81, 0x04]);

        let mut transport = MockHidTransport::new(1);
        transport.expect_feature_report(prelude);
        transport.expect_feature_report(request);
        transport.queue_feature_response(response);

        let mut device = PulsefireRaid::new(transport).unwrap();
        let mut waits = Vec::new();
        let profile = device
            .runtime_profile_with_wait(|duration| waits.push(duration))
            .unwrap();

        assert_eq!(profile.kind(), ProfileImageKind::DeviceReadResponse);
        assert_eq!(profile.section(), ProfileSection::Runtime);
        assert_eq!(waits, [PROFILE_PRELUDE_DELAY, PROFILE_READ_DELAY]);
        device.into_transport().assert_drained();
    }

    fn performance_profile_response() -> [u8; DIRECT_REPORT_LENGTH] {
        let mut response = [0_u8; DIRECT_REPORT_LENGTH];
        response[..3].copy_from_slice(&[0x07, 0x81, 0x04]);
        response[0x18] = 0x02;
        response[0x19..0x1B].copy_from_slice(&[0x00, 0x10]);
        response[0x25..0x27].copy_from_slice(&[0x00, 0x10]);
        response[0x32] = 0x01;
        response[0x69..0x6C].copy_from_slice(&[0x2B, 0x00, 0xFF]);
        response
    }

    #[test]
    fn driver_changes_only_active_dpi_fields_and_write_opcode() {
        let response = performance_profile_response();
        let mut expected_write = response;
        expected_write[1] = 0x01;
        expected_write[0x19..0x1B].copy_from_slice(&[0x00, 0x12]);
        expected_write[0x25..0x27].copy_from_slice(&[0x00, 0x12]);

        let mut transport = MockHidTransport::new(1);
        transport.expect_feature_report(encode_runtime_profile_read_prelude());
        transport.expect_feature_report(encode_profile_read_request());
        transport.queue_feature_response(response);
        transport.expect_feature_report(expected_write);

        let mut device = PulsefireRaid::new(transport).unwrap();
        let dpi_profile = device
            .set_runtime_active_dpi_with_wait(900, |_| {})
            .unwrap();

        assert_eq!(dpi_profile.active().unwrap().x, 900);
        assert_eq!(dpi_profile.active().unwrap().y, 900);
        device.into_transport().assert_drained();
    }

    #[test]
    fn driver_rejects_invalid_dpi_before_any_hid_io() {
        let transport = MockHidTransport::new(1);
        let mut device = PulsefireRaid::new(transport).unwrap();

        assert!(matches!(
            device.set_runtime_active_dpi_with_wait(225, |_| {}),
            Err(PulsefireRaidError::InvalidDpi(
                DpiValidationError::NotStepAligned { dpi: 225, .. }
            ))
        ));
        device.into_transport().assert_drained();
    }

    #[test]
    fn driver_changes_only_polling_field_and_write_opcode() {
        let response = performance_profile_response();
        let mut expected_write = response;
        expected_write[1] = 0x01;
        expected_write[0x18] = 0x01;

        let mut transport = MockHidTransport::new(1);
        transport.expect_feature_report(encode_runtime_profile_read_prelude());
        transport.expect_feature_report(encode_profile_read_request());
        transport.queue_feature_response(response);
        transport.expect_feature_report(expected_write);

        let mut device = PulsefireRaid::new(transport).unwrap();
        assert_eq!(
            device
                .set_runtime_polling_rate_with_wait(PollingRate::Hz1000, |_| {})
                .unwrap(),
            PollingRate::Hz1000
        );
        device.into_transport().assert_drained();
    }

    #[test]
    fn driver_rejects_a_truncated_profile_response() {
        let mut transport = MockHidTransport::new(1);
        transport.expect_feature_report(encode_runtime_profile_read_prelude());
        transport.expect_feature_report(encode_profile_read_request());
        transport.queue_feature_response([0x07, 0x81, 0x04]);

        let mut device = PulsefireRaid::new(transport).unwrap();
        let error = device
            .runtime_profile_with_wait(|_| {})
            .expect_err("a truncated profile must be rejected");

        assert!(matches!(
            error,
            PulsefireRaidError::WrongProfileResponseLength(3)
        ));
    }

    #[test]
    fn driver_rejects_the_standard_mouse_interface() {
        let transport = MockHidTransport::new(0);
        assert!(matches!(
            PulsefireRaid::new(transport),
            Err(PulsefireRaidError::WrongInterface(0))
        ));
    }
}
