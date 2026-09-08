use std::fmt;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct UsbId {
    pub vendor_id: u16,
    pub product_id: u16,
}

impl fmt::Display for UsbId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:04X}:{:04X}", self.vendor_id, self.product_id)
    }
}

/// Static metadata for a supported device model.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DeviceDescriptor {
    pub id: &'static str,
    pub name: &'static str,
    pub usb_id: UsbId,
    pub capabilities: crate::CapabilitySet,
    /// Interface selected for known configuration traffic.
    pub configuration_interface: InterfaceSelector,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InterfaceSelector {
    pub interface_number: i32,
    pub usage_page: u16,
    pub usage: u16,
}

/// One top-level HID collection returned by hidapi.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HidInterfaceInfo {
    pub path: String,
    pub vendor_id: u16,
    pub product_id: u16,
    pub release_number: u16,
    pub interface_number: i32,
    pub usage_page: u16,
    pub usage: u16,
    pub manufacturer: Option<String>,
    pub product: Option<String>,
    pub serial_number: Option<String>,
}

impl HidInterfaceInfo {
    pub fn usb_id(&self) -> UsbId {
        UsbId {
            vendor_id: self.vendor_id,
            product_id: self.product_id,
        }
    }

    pub fn matches(&self, selector: InterfaceSelector) -> bool {
        self.interface_number == selector.interface_number
            && self.usage_page == selector.usage_page
            && self.usage == selector.usage
    }
}
