use hyperx_core::{
    Capability, CapabilitySet, DeviceDescriptor, InterfaceSelector, LightingZone, RgbColor, UsbId,
};
use hyperx_hid::{HidError, HidTransport};
use hyperx_protocol::pulsefire_raid::encode_direct_rgb;
use thiserror::Error;

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

    pub fn into_transport(self) -> T {
        self.transport
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hyperx_hid::testing::MockHidTransport;
    use hyperx_protocol::pulsefire_raid::DIRECT_REPORT_LENGTH;

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
    fn driver_rejects_the_standard_mouse_interface() {
        let transport = MockHidTransport::new(0);
        assert!(matches!(
            PulsefireRaid::new(transport),
            Err(PulsefireRaidError::WrongInterface(0))
        ));
    }
}
