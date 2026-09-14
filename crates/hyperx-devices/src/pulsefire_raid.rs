use std::time::Duration;

use hyperx_core::{
    Capability, CapabilitySet, DeviceDescriptor, InterfaceSelector, LightingZone, RgbColor, UsbId,
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
/// Only discovery is implemented at milestone 1. Protocol support must be
/// tracked separately and must never be inferred from this declaration.
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
    #[error("expected a 264-byte Pulsefire Raid profile response, got {0} bytes")]
    WrongProfileResponseLength(usize),
    #[error("expected a runtime profile read response, got {kind:?} {section:?}")]
    WrongProfileResponse {
        kind: ProfileImageKind,
        section: ProfileSection,
    },
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
        self.transport.send_feature_report(&report)?;
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
        self.transport.send_feature_report(&prelude)?;
        wait(PROFILE_PRELUDE_DELAY);

        let request = encode_profile_read_request();
        self.transport.send_feature_report(&request)?;
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
