//! Platform-independent device model and supported-device registry.

mod capability;
mod color;
mod device;

pub use capability::{Capability, CapabilitySet, LightingZone, PollingRate};
pub use color::{ColorParseError, RgbColor};
pub use device::{DeviceDescriptor, HidInterfaceInfo, InterfaceSelector, UsbId};
