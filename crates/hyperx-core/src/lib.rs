//! Platform-independent device model and supported-device registry.

mod button;
mod capability;
mod color;
mod device;
mod performance;

pub use button::{
    ButtonBinding, KeyState, KeyboardMacroEvent, KeyboardUsage, KeyboardUsageParseError,
    MacroBinding, MacroDefinition, MacroEvent, MacroPlayback, MouseFunction, MultimediaFunction,
    WindowsShortcut,
};
pub use capability::{
    Capability, CapabilitySet, DpiCapabilities, DpiValidationError, LightingZone, PollingRate,
    PollingRateParseError,
};
pub use color::{ColorParseError, RgbColor};
pub use device::{DeviceDescriptor, HidInterfaceInfo, InterfaceSelector, UsbId};
pub use performance::{DpiProfile, DpiStage};
