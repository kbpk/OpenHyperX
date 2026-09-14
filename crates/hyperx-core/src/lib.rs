//! Platform-independent device model and supported-device registry.

mod button;
mod capability;
mod color;
mod device;
mod performance;

pub use button::{
    ButtonBinding, KeyState, KeyboardMacroEvent, KeyboardUsage, MacroBinding, MacroPlayback,
    MouseFunction, MultimediaFunction, WindowsShortcut,
};
pub use capability::{Capability, CapabilitySet, LightingZone, PollingRate};
pub use color::{ColorParseError, RgbColor};
pub use device::{DeviceDescriptor, HidInterfaceInfo, InterfaceSelector, UsbId};
pub use performance::{DpiProfile, DpiStage};
