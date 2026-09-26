//! Device-driver registry.
//!
//! Each supported model owns its identity, capability declaration and, in
//! later milestones, its protocol implementation in this crate.

pub mod pulsefire_raid;

use hyperx_core::{DeviceDescriptor, UsbId};

pub use pulsefire_raid::{
    PulsefireRaid, PulsefireRaidError, PulsefireRaidOnboardMacros, PulsefireRaidRuntimeAssignment,
    PULSEFIRE_RAID, PULSEFIRE_RAID_ACKNOWLEDGEMENT_INTERFACE, PULSEFIRE_RAID_DPI,
};

pub static SUPPORTED_DEVICES: &[DeviceDescriptor] = &[PULSEFIRE_RAID];

pub fn find_supported_device(usb_id: UsbId) -> Option<&'static DeviceDescriptor> {
    SUPPORTED_DEVICES
        .iter()
        .find(|descriptor| descriptor.usb_id == usb_id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recognizes_pulsefire_raid() {
        let found = find_supported_device(UsbId {
            vendor_id: 0x0951,
            product_id: 0x16E4,
        });

        assert_eq!(found, Some(&PULSEFIRE_RAID));
    }

    #[test]
    fn does_not_guess_unknown_hyperx_device() {
        assert_eq!(
            find_supported_device(UsbId {
                vendor_id: 0x0951,
                product_id: 0xFFFF,
            }),
            None
        );
    }
}
