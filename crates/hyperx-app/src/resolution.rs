//! Target selection for explicit offline provenance resolution, not discovery.
use anyhow::{bail, Context, Result};
use hyperx_core::SoftwareProfile;

use crate::{profile_controls, unresolved_index, ProfileControl};

#[derive(Clone, Debug)]
pub struct MacroResolutionTarget {
    pub control: ProfileControl,
    /// Rejected choices remain inspectable; absence means only the target is legal.
    /// The caller must separately validate the chosen real macro definition.
    pub error: Option<String>,
}

pub(crate) fn check_target(
    profile: &SoftwareProfile,
    source_id: &str,
    control: &str,
) -> Result<usize> {
    let index = unresolved_index(profile, source_id)?;
    // Only normalized capture diagnostics identify a physical target. Legacy
    // IDs and their macro-source hints are provenance, never control numbers.
    if let Some(known) = source_id.strip_prefix("runtime:") {
        if known != control {
            bail!("captured source {source_id:?} belongs to {known}, not {control}");
        }
    }
    if profile.buttons.contains_key(control) {
        bail!("control {control:?} already has a supplied binding; edit it explicitly rather than overwriting it during resolution");
    }
    let target = profile_controls(&profile.device)
        .into_iter()
        .find(|target| target.id == control)
        .context("target is not declared by this profile's device model")?;
    if target.macros.is_none() {
        bail!("macro encoding is not implemented for {}", target.name);
    }
    Ok(index)
}

/// Selection metadata shared by file clients. Does not assume a timeline from a
/// source hint or hide a forbidden target. No transport or settings mutation.
pub fn macro_resolution_targets(
    profile: &SoftwareProfile,
    source_id: &str,
) -> Result<Vec<MacroResolutionTarget>> {
    unresolved_index(profile, source_id)?;
    Ok(profile_controls(&profile.device)
        .into_iter()
        .map(|control| {
            let error = check_target(profile, source_id, control.id)
                .err()
                .map(|error| error.to_string());
            MacroResolutionTarget { control, error }
        })
        .collect())
}
