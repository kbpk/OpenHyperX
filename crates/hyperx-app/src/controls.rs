//! Typed file edits for UI controls, with no transport or device session.
use anyhow::{bail, Context, Result};
use hyperx_core::{
    DpiStage, NamedMacro, PrimaryButtonLayout, RgbColor, SoftwareButtonBinding, SoftwareDpiProfile,
    SoftwareLightingMode, SoftwareLightingProfile, SoftwarePollingProfile, SoftwareProfile,
};

use crate::{device_descriptor, encode_profile};

#[derive(Clone, Debug)]
pub enum ProfileValueEdit {
    /// An explicit linked-axis edit; other levels and provenance stay intact.
    StageDpi {
        index: usize,
        dpi: u32,
    },
    StageColor {
        index: usize,
        color: RgbColor,
    },
    AddStage {
        dpi: u32,
        color: RgbColor,
    },
    /// Only the last level has a capture-backed removal operation.
    RemoveLastStage,
    ActiveStage(Option<usize>),
    Polling(Option<u16>),
    PrimaryButtons(Option<PrimaryButtonLayout>),
    /// None omits one file assignment; it is not Disabled or a factory reset.
    ButtonBinding {
        control: String,
        binding: Option<SoftwareButtonBinding>,
    },
    /// Add a fresh library entry, never satisfy an existing unresolved reference.
    MacroCreate {
        definition: NamedMacro,
    },
    /// Keep identity stable; replacing a referenced timeline requires consent.
    MacroReplace {
        source_id: String,
        definition: NamedMacro,
        confirm_references: bool,
    },
    /// Referenced definitions cannot be deleted; first explicitly omit bindings.
    MacroRemove {
        source_id: String,
    },
    SolidZone {
        zone: String,
        color: RgbColor,
    },
}

/// Validate the edited field, not the entire draft: unrelated unresolved or
/// unsupported values must remain inspectable, not normalized or discarded.
/// None means omission in a FILE. These operations never apply device defaults.
pub fn edit_profile_value(
    profile: &SoftwareProfile,
    edit: ProfileValueEdit,
) -> Result<SoftwareProfile> {
    let descriptor = device_descriptor(&profile.device).context("unknown profile device")?;
    let mut edited = profile.clone();
    match edit {
        ProfileValueEdit::StageDpi { index, dpi } => {
            descriptor
                .capabilities
                .dpi
                .context("DPI is not supported")?
                .validate(dpi)?;
            let stage = edited
                .dpi
                .as_mut()
                .and_then(|value| value.stages.get_mut(index))
                .context("select an existing DPI stage")?;
            stage.x = dpi;
            stage.y = dpi;
        }
        ProfileValueEdit::StageColor { index, color } => {
            let stage = edited
                .dpi
                .as_mut()
                .and_then(|value| value.stages.get_mut(index))
                .context("select an existing DPI stage")?;
            stage.color = color;
        }
        ProfileValueEdit::AddStage { dpi, color } => {
            let caps = descriptor
                .capabilities
                .dpi
                .context("DPI stages are not supported")?;
            caps.validate(dpi)?;
            let levels = edited.dpi.get_or_insert_with(|| SoftwareDpiProfile {
                stages: Vec::new(),
                active_stage: None,
                source_active_stage: None,
            });
            if levels.stages.len() >= usize::from(caps.max_stages) {
                bail!("maximum of {} DPI stages reached", caps.max_stages);
            }
            levels.stages.push(DpiStage::new(dpi, dpi, color));
        }
        ProfileValueEdit::RemoveLastStage => {
            let levels = edited.dpi.as_mut().context("no DPI stages supplied")?;
            if levels.stages.len() <= 1 {
                bail!("cannot remove the only DPI stage; use advanced editing to omit the entire DPI section");
            }
            let removed = levels.stages.len() - 1;
            if levels.active_stage.is_some_and(|active| active >= removed) {
                // Do not invent a replacement active level. Require the user
                // to choose another level first, preserving unrelated fields.
                bail!("choose another active stage before removing this level");
            }
            levels.stages.pop();
        }
        ProfileValueEdit::ActiveStage(active) => {
            let levels = edited.dpi.as_mut().context("no DPI stages supplied")?;
            if active.is_some_and(|index| index >= levels.stages.len()) {
                bail!("active stage is outside the supplied levels");
            }
            levels.active_stage = active;
        }
        ProfileValueEdit::Polling(hz) => {
            if hz.is_some_and(|value| {
                !descriptor
                    .capabilities
                    .polling_rates
                    .iter()
                    .any(|rate| rate.hz() == value)
            }) {
                bail!("polling rate is not supported by this model");
            }
            edited.polling = hz.map(|hz| SoftwarePollingProfile { hz });
        }
        ProfileValueEdit::PrimaryButtons(layout) => edited.primary_buttons = layout,
        ProfileValueEdit::ButtonBinding { control, binding } => {
            let target = crate::profile_controls(&profile.device)
                .into_iter()
                .find(|target| target.id == control)
                .context("unknown physical control")?;
            if target.primary {
                bail!("use the coupled primary-button layout for left/right click");
            }
            if let Some(binding) = binding {
                crate::validate_button_binding(profile, &control, &binding)?;
                edited.buttons.insert(control, binding);
            } else {
                edited.buttons.remove(&control);
            }
            // Explicit binding edits do not resolve/erase source provenance or
            // unrelated invalid fields; those have their own explicit actions.
        }
        ProfileValueEdit::MacroCreate { definition } => {
            crate::macros::create(&mut edited, definition)?;
        }
        ProfileValueEdit::MacroReplace {
            source_id,
            definition,
            confirm_references,
        } => crate::macros::replace(&mut edited, &source_id, definition, confirm_references)?,
        ProfileValueEdit::MacroRemove { source_id } => {
            crate::macros::remove(&mut edited, &source_id)?;
        }
        ProfileValueEdit::SolidZone { zone, color } => {
            if !descriptor
                .capabilities
                .lighting_zones
                .iter()
                .any(|value| value.id == zone)
            {
                bail!("unknown lighting zone {zone:?}");
            }
            let lighting = edited
                .lighting
                .get_or_insert_with(|| SoftwareLightingProfile {
                    mode: SoftwareLightingMode::Solid,
                    zones: Default::default(),
                });
            lighting.zones.insert(zone, color);
            // Missing other zones remain missing and fail readiness as before;
            // never create a black/default color for an unedited zone.
        }
    }
    encode_profile(&edited)?;
    Ok(edited)
}
