/// A high-level feature exposed by a device.
///
/// This says what the hardware is expected to support. It does not imply that
/// OpenHyperX has implemented or verified the corresponding vendor command.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Capability {
    Dpi,
    DpiStages,
    PollingRate,
    ButtonRemapping,
    Macros,
    Lighting,
    OnboardProfile,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LightingZone {
    pub id: &'static str,
    pub name: &'static str,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CapabilitySet {
    pub features: &'static [Capability],
    pub button_count: u8,
    pub lighting_zones: &'static [LightingZone],
}
