use hyperx_core::{ButtonBinding, DpiProfile, PollingRate, PrimaryButtonLayout};

use super::{PerformanceProfile, PerformanceProfileError, ProfileSection, PulsefireRaidControl};

/// A decoded assignment in one observed profile image.
///
/// A macro reference contains neither its event timeline nor its playback
/// mode. Those are carried by separate reports and must not be inferred here.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PulsefireRaidObservedBinding {
    Binding(ButtonBinding),
    MacroReference,
}

/// Known settings from one device read-response image, without raw write bytes.
///
/// The section comes from the response envelope. This does not establish when
/// the image was produced, which section is currently active, or whether a
/// later write would be safe. In particular, a selector-free request has been
/// observed to return the onboard section.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PulsefireRaidProfileObservation {
    section: ProfileSection,
    polling_rate: PollingRate,
    dpi_profile: DpiProfile,
    primary_button_layout: PrimaryButtonLayout,
    bindings: Vec<(PulsefireRaidControl, PulsefireRaidObservedBinding)>,
}

impl PulsefireRaidProfileObservation {
    /// Decode only capture-backed fields of a complete device read response.
    /// Opaque, empty, malformed and host-write images are rejected.
    pub fn parse(report: &[u8]) -> Result<Self, PerformanceProfileError> {
        let profile = PerformanceProfile::parse(report)?;
        profile.validate_confirmed_read_settings()?;

        let mut bindings = Vec::with_capacity(PulsefireRaidControl::ALL.len());
        for control in PulsefireRaidControl::ALL {
            let binding = if profile.has_confirmed_macro_reference(control) {
                PulsefireRaidObservedBinding::MacroReference
            } else {
                PulsefireRaidObservedBinding::Binding(profile.button_binding(control)?)
            };
            bindings.push((control, binding));
        }

        Ok(Self {
            section: profile.section(),
            polling_rate: profile.polling_rate()?,
            dpi_profile: profile.dpi_profile()?,
            primary_button_layout: profile.primary_button_layout()?,
            bindings,
        })
    }

    pub const fn section(&self) -> ProfileSection {
        self.section
    }

    pub const fn polling_rate(&self) -> PollingRate {
        self.polling_rate
    }

    pub fn dpi_profile(&self) -> &DpiProfile {
        &self.dpi_profile
    }

    pub const fn primary_button_layout(&self) -> PrimaryButtonLayout {
        self.primary_button_layout
    }

    pub fn bindings(
        &self,
    ) -> impl ExactSizeIterator<Item = (PulsefireRaidControl, &PulsefireRaidObservedBinding)> {
        self.bindings
            .iter()
            .map(|(control, binding)| (*control, binding))
    }

    pub fn binding(&self, control: PulsefireRaidControl) -> &PulsefireRaidObservedBinding {
        let index = PulsefireRaidControl::ALL
            .iter()
            .position(|candidate| *candidate == control)
            .expect("all Pulsefire Raid controls are represented");
        &self.bindings[index].1
    }
}
