//! Platform-independent device model and supported-device registry.

mod button;
mod capability;
mod color;
mod device;
mod lighting;
mod performance;
mod profile;

pub use button::{
    ButtonBinding, KeyState, KeyboardMacroEvent, KeyboardUsage, KeyboardUsageParseError,
    MacroBinding, MacroDefinition, MacroEvent, MacroPlayback, MouseFunction, MultimediaFunction,
    PrimaryButtonLayout, WindowsShortcut,
};
pub use capability::{
    Capability, CapabilitySet, DpiCapabilities, DpiValidationError, LightingZone, PollingRate,
    PollingRateParseError,
};
pub use color::{ColorParseError, RgbColor};
pub use device::{DeviceDescriptor, HidInterfaceInfo, InterfaceSelector, UsbId};
pub use lighting::{
    spectrum_color, SoftwareLightingEffect, SoftwareLightingError, SoftwareLightingProgram,
    SPECTRUM_STEPS,
};
pub use performance::{DpiProfile, DpiStage};
pub use profile::{
    NamedMacro, SoftwareDpiProfile, SoftwareProfile, SoftwareProfileSource,
    UnresolvedButtonAssignment,
};
