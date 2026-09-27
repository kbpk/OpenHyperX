//! Named software-library edits, independent of device execution/readiness.
use anyhow::{bail, Result};
use hyperx_core::{NamedMacro, SoftwareButtonBinding, SoftwareProfile};

/// Canonical named inputs for the currently implemented Raid macro encoder.
/// Opening a picker must not normalize imported aliases or unknown inputs.
pub fn macro_keyboard_names(device: &str) -> Vec<String> {
    if device == "pulsefire-raid" {
        crate::bindings::keyboard_names()
    } else {
        Vec::new()
    }
}

pub fn macro_mouse_button_names(device: &str) -> Vec<String> {
    if device == "pulsefire-raid" {
        ["left", "right", "middle"].map(String::from).into()
    } else {
        Vec::new()
    }
}

/// Include opaque import references, not just already resolved physical buttons.
/// These labels are presentation/provenance, never USB slots or inferred targets.
pub fn macro_references(profile: &SoftwareProfile, source_id: &str) -> Vec<String> {
    profile
        .buttons
        .iter()
        .filter(|(_, binding)| {
            matches!(binding, SoftwareButtonBinding::Macro { id } if id == source_id)
        })
        .map(|(control, _)| format!("control: {control}"))
        .chain(
            profile
                .unresolved_button_assignments
                .iter()
                .filter(|entry| entry.macro_source_id.as_deref() == Some(source_id))
                .map(|entry| format!("unresolved source: {}", entry.source_id)),
        )
        .collect()
}

fn identity(value: &str, field: &str, maximum: usize) -> Result<()> {
    if value.trim().is_empty() || value.len() > maximum || value.chars().any(char::is_control) {
        bail!("macro {field} must be 1–{maximum} UTF-8 bytes without control characters");
    }
    Ok(())
}

fn validate_identity(definition: &NamedMacro) -> Result<()> {
    identity(&definition.source_id, "ID", 256)?;
    identity(&definition.name, "name", 128)
}

fn unique_index(profile: &SoftwareProfile, source_id: &str) -> Result<usize> {
    let matches: Vec<_> = profile
        .macros
        .iter()
        .enumerate()
        .filter(|(_, definition)| definition.source_id == source_id)
        .map(|(index, _)| index)
        .collect();
    if matches.len() != 1 {
        bail!("macro ID {source_id:?} must identify exactly one definition in this file");
    }
    Ok(matches[0])
}

pub(crate) fn create(profile: &mut SoftwareProfile, definition: NamedMacro) -> Result<()> {
    validate_identity(&definition)?;
    if profile
        .macros
        .iter()
        .any(|entry| entry.source_id == definition.source_id)
        || !macro_references(profile, &definition.source_id).is_empty()
    {
        // Creating an ID that is referenced by an incomplete import would
        // silently resolve that import. It has a separate explicit workflow.
        bail!("choose a fresh macro ID; existing definitions and references are reserved");
    }
    // File drafts may have empty/unbalanced timelines or unsupported delays,
    // event counts and modes. Validation and assignment preflight stay separate
    // from editing: neither saving a draft nor confirming replacement grants
    // permission to execute it. The shared file-size gate bounds draft storage.
    profile.macros.push(definition);
    Ok(())
}

pub(crate) fn replace(
    profile: &mut SoftwareProfile,
    source_id: &str,
    definition: NamedMacro,
    confirm_references: bool,
) -> Result<()> {
    let index = unique_index(profile, source_id)?;
    validate_identity(&definition)?;
    if definition.source_id != source_id {
        bail!("macro identity cannot change during replacement; create a fresh entry instead");
    }
    if profile.macros[index] != definition
        && !macro_references(profile, source_id).is_empty()
        && !confirm_references
    {
        bail!("explicit confirmation is required to replace a referenced macro definition");
    }
    profile.macros[index] = definition;
    Ok(())
}

pub(crate) fn remove(profile: &mut SoftwareProfile, source_id: &str) -> Result<()> {
    let index = unique_index(profile, source_id)?;
    if !macro_references(profile, source_id).is_empty() {
        bail!("cannot delete a referenced macro; explicitly remove its assignments first");
    }
    profile.macros.remove(index);
    Ok(())
}
