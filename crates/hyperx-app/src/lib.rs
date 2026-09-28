//! Shared offline application operations. No discovery, transport or hardware writes.
use std::{
    collections::BTreeMap,
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
};

use anyhow::{anyhow, bail, Context, Result};
use hyperx_core::{
    diff_software_profiles, DeviceDescriptor, MacroCapabilities, MacroDefinition, NamedMacro,
    PrimaryButtonLayout, SoftwareButtonBinding, SoftwareDpiProfile, SoftwareLightingProfile,
    SoftwarePollingProfile, SoftwareProfile, SoftwareProfileDiff,
};
use hyperx_devices::{
    PulsefireRaidRuntimeAssignment, PulsefireRaidSoftwareProfile, SUPPORTED_DEVICES,
};
use serde::{Deserialize, Serialize};

pub const MAX_PROFILE_BYTES: usize = 1024 * 1024;

mod controls;
pub use controls::{edit_profile_value, ProfileValueEdit};
mod bindings;
pub use bindings::{profile_binding_choices, validate_button_binding, ProfileBindingChoice};
mod macros;
pub use macros::{macro_keyboard_names, macro_mouse_button_names, macro_references};
mod resolution;
pub use resolution::{macro_resolution_targets, MacroResolutionTarget};

/// Validation is offline encoding support, never proof of device state or playback.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProfileReadiness {
    pub error: Option<String>,
    /// File field responsible for a typed offline-validation error, if known.
    /// This is not a device register or a claim about hardware state.
    pub field: Option<String>,
    pub warnings: Vec<String>,
}

pub fn validate_profile(profile: &SoftwareProfile) -> ProfileReadiness {
    match PulsefireRaidSoftwareProfile::new(profile) {
        Ok(value) => ProfileReadiness {
            error: None,
            field: None,
            warnings: value.warnings().to_vec(),
        },
        Err(error) => ProfileReadiness {
            field: error.field().map(str::to_owned),
            error: Some(error.to_string()),
            warnings: Vec::new(),
        },
    }
}

pub fn device_descriptor(id: &str) -> Option<&'static DeviceDescriptor> {
    SUPPORTED_DEVICES.iter().find(|device| device.id == id)
}

#[derive(Clone, Debug)]
pub struct ProfileControl {
    pub id: &'static str,
    pub name: &'static str,
    pub primary: bool,
    pub macros: Option<MacroCapabilities>,
}

/// Public model metadata, not enumeration. Clients need no vendor offsets/types.
pub fn profile_controls(device: &str) -> Vec<ProfileControl> {
    use hyperx_devices::pulsefire_raid::PulsefireRaidControl;
    if device != "pulsefire-raid" {
        return Vec::new();
    }
    PulsefireRaidControl::ALL
        .into_iter()
        .map(|control| ProfileControl {
            id: control.id(),
            name: control.name(),
            primary: matches!(
                control,
                PulsefireRaidControl::LeftClick | PulsefireRaidControl::RightClick
            ),
            macros: PulsefireRaidRuntimeAssignment::macro_capabilities(control),
        })
        .collect()
}

pub fn read_text(path: &Path) -> Result<String> {
    let file = fs::File::open(path)
        .with_context(|| format!("failed to open software profile {}", path.display()))?;
    let mut text = String::new();
    file.take((MAX_PROFILE_BYTES + 1) as u64)
        .read_to_string(&mut text)
        .with_context(|| format!("failed to read UTF-8 software profile {}", path.display()))?;
    check_size(&text)?;
    Ok(text.trim_start_matches('\u{feff}').to_owned())
}

fn check_size(text: &str) -> Result<()> {
    if text.len() > MAX_PROFILE_BYTES {
        bail!("software profile exceeds the 1 MiB limit");
    }
    Ok(())
}

pub fn parse_profile(text: &str) -> Result<SoftwareProfile> {
    check_size(text)?;
    toml::from_str(text.trim_start_matches('\u{feff}'))
        .context("failed to parse software profile TOML")
}

pub fn load_profile(path: &Path) -> Result<SoftwareProfile> {
    parse_profile(&read_text(path)?)
        .with_context(|| format!("failed to parse software profile {}", path.display()))
}

pub fn load_macro(path: &Path) -> Result<MacroDefinition> {
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct MacroFile {
        playback: hyperx_core::MacroPlayback,
        events: Vec<hyperx_core::MacroEvent>,
    }
    let value: MacroFile =
        toml::from_str(&read_text(path)?).context("failed to parse macro timeline TOML")?;
    Ok(MacroDefinition {
        playback: value.playback,
        events: value.events,
    })
}

pub fn encode_profile(profile: &SoftwareProfile) -> Result<String> {
    let text = toml::to_string_pretty(profile)?;
    check_size(&text)?;
    Ok(text)
}

/// Rename only the offline document, even for an unknown/unsupported target.
/// Existing imported names remain inspectable; explicit new names are bounded.
pub fn rename_profile(profile: &SoftwareProfile, name: &str) -> Result<SoftwareProfile> {
    if name.trim().is_empty() || name.len() > 128 || name.chars().any(char::is_control) {
        bail!("profile name must be nonblank, at most 128 UTF-8 bytes, with no control characters");
    }
    let mut edited = profile.clone();
    edited.name = name.into();
    encode_profile(&edited)?;
    Ok(edited)
}

/// Even a semantically unsupported draft can be saved offline. No apply implied.
/// Serialize first, then create exclusively. I/O errors may leave an incomplete
/// NEW file; never delete or overwrite a preexisting file while handling them.
pub fn save_profile_new(path: &Path, profile: &SoftwareProfile, comments: &[String]) -> Result<()> {
    let mut text = String::new();
    for comment in comments {
        for line in comment.lines() {
            text.push_str(&format!("# {line}\n"));
        }
    }
    text.push_str(&encode_profile(profile)?);
    check_size(&text)?;
    let mut file = fs::OpenOptions::new().write(true).create_new(true).open(path)
        .with_context(|| format!("cannot create {}; destination must be a new file (existing files are never overwritten)", path.display()))?;
    file.write_all(text.as_bytes()).with_context(|| {
        format!(
            "failed to write {}; an incomplete output may remain, inspect it before use",
            path.display()
        )
    })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProfileSection {
    Performance,
    Buttons,
    Macros,
    Lighting,
    All,
}

#[derive(Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct PerformanceSection {
    dpi: Option<SoftwareDpiProfile>,
    polling: Option<SoftwarePollingProfile>,
    primary_buttons: Option<PrimaryButtonLayout>,
}
#[derive(Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ButtonsSection {
    #[serde(default)]
    buttons: BTreeMap<String, SoftwareButtonBinding>,
}
#[derive(Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct MacrosSection {
    #[serde(default)]
    macros: Vec<NamedMacro>,
}
#[derive(Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct LightingSection {
    lighting: Option<SoftwareLightingProfile>,
}

pub fn section_toml(profile: &SoftwareProfile, section: ProfileSection) -> Result<String> {
    Ok(match section {
        ProfileSection::Performance => toml::to_string_pretty(&PerformanceSection {
            dpi: profile.dpi.clone(),
            polling: profile.polling,
            primary_buttons: profile.primary_buttons,
        })?,
        ProfileSection::Buttons => toml::to_string_pretty(&ButtonsSection {
            buttons: profile.buttons.clone(),
        })?,
        ProfileSection::Macros => toml::to_string_pretty(&MacrosSection {
            macros: profile.macros.clone(),
        })?,
        ProfileSection::Lighting => toml::to_string_pretty(&LightingSection {
            lighting: profile.lighting.clone(),
        })?,
        ProfileSection::All => encode_profile(profile)?,
    })
}

/// Atomic draft edit: parse the whole section before changing any document field.
/// Missing values remove them FROM THE FILE, never disable/reset the hardware.
pub fn edit_section(
    profile: &SoftwareProfile,
    section: ProfileSection,
    text: &str,
) -> Result<SoftwareProfile> {
    check_size(text)?;
    let mut edited = profile.clone();
    match section {
        ProfileSection::Performance => {
            let value: PerformanceSection = toml::from_str(text)?;
            edited.dpi = value.dpi;
            edited.polling = value.polling;
            edited.primary_buttons = value.primary_buttons;
        }
        ProfileSection::Buttons => edited.buttons = toml::from_str::<ButtonsSection>(text)?.buttons,
        ProfileSection::Macros => edited.macros = toml::from_str::<MacrosSection>(text)?.macros,
        ProfileSection::Lighting => {
            edited.lighting = toml::from_str::<LightingSection>(text)?.lighting
        }
        ProfileSection::All => edited = parse_profile(text)?,
    }
    // Ensure the combined draft still fits the shared file limit.
    encode_profile(&edited)?;
    Ok(edited)
}

fn unresolved_index(profile: &SoftwareProfile, source_id: &str) -> Result<usize> {
    let matches = profile
        .unresolved_button_assignments
        .iter()
        .enumerate()
        .filter(|(_, assignment)| assignment.source_id == source_id)
        .map(|(index, _)| index)
        .collect::<Vec<_>>();
    if matches.len() != 1 {
        bail!(
            "unresolved source ID {source_id:?} must identify exactly one entry (found {})",
            matches.len()
        );
    }
    Ok(matches[0])
}

/// Explicit caller choice, never infer a physical target from a Legacy source ID.
/// Optional imported definition must have a fresh ID; never replace a library
/// definition or an existing physical assignment implicitly.
pub fn resolve_macro_assignment(
    profile: &SoftwareProfile,
    source_id: &str,
    control: &str,
    macro_id: &str,
    imported: Option<MacroDefinition>,
) -> Result<SoftwareProfile> {
    let index = resolution::check_target(profile, source_id, control)?;
    let mut edited = profile.clone();
    if let Some(definition) = imported {
        edited = import_macro(&edited, macro_id, definition)?;
    }
    let definitions = edited
        .macros
        .iter()
        .filter(|value| value.source_id == macro_id)
        .collect::<Vec<_>>();
    if definitions.len() != 1 {
        bail!("macro ID {macro_id:?} must identify exactly one definition in this file");
    }
    // Validate only the requested target/timeline so OTHER unresolved entries
    // or unsupported draft fields can remain inspectable and be resolved later.
    let isolated = SoftwareProfile {
        name: "Macro resolution preflight".into(),
        device: edited.device.clone(),
        buttons: [(
            control.into(),
            SoftwareButtonBinding::Macro {
                id: macro_id.into(),
            },
        )]
        .into(),
        macros: vec![definitions[0].clone()],
        ..SoftwareProfile::default()
    };
    let readiness = validate_profile(&isolated);
    if let Some(error) = readiness.error {
        bail!("macro resolution rejected: {error}");
    }
    edited.buttons.extend(isolated.buttons);
    edited.unresolved_button_assignments.remove(index);
    encode_profile(&edited)?;
    Ok(edited)
}

pub fn import_macro(
    profile: &SoftwareProfile,
    id: &str,
    definition: MacroDefinition,
) -> Result<SoftwareProfile> {
    if id.trim().is_empty() || profile.macros.iter().any(|value| value.source_id == id) {
        bail!("imported macro ID must be nonempty and new");
    }
    let mut edited = profile.clone();
    edited.macros.push(NamedMacro {
        source_id: id.into(),
        name: id.into(),
        definition,
    });
    encode_profile(&edited)?;
    Ok(edited)
}

/// Remove provenance only by explicit choice. An absent binding means preserve.
pub fn omit_unresolved_assignment(
    profile: &SoftwareProfile,
    source_id: &str,
) -> Result<SoftwareProfile> {
    let index = unresolved_index(profile, source_id)?;
    let mut edited = profile.clone();
    edited.unresolved_button_assignments.remove(index);
    Ok(edited)
}

/// Session baseline tracks FILE edits, not a measured runtime device snapshot.
pub struct ProfileDocument {
    profile: SoftwareProfile,
    baseline: SoftwareProfile,
    path: Option<PathBuf>,
}

#[cfg(test)]
mod tests;

impl ProfileDocument {
    pub fn from_profile(profile: SoftwareProfile) -> Self {
        Self {
            baseline: profile.clone(),
            profile,
            path: None,
        }
    }
    pub fn open(path: &Path) -> Result<Self> {
        let mut document = Self::from_profile(load_profile(path)?);
        document.path = Some(path.into());
        Ok(document)
    }
    pub fn profile(&self) -> &SoftwareProfile {
        &self.profile
    }
    pub fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }
    pub fn dirty(&self) -> bool {
        self.profile != self.baseline
    }
    pub fn replace(&mut self, profile: SoftwareProfile) {
        self.profile = profile;
    }
    pub fn diff(&self) -> Result<SoftwareProfileDiff> {
        diff_software_profiles(&self.baseline, &self.profile).map_err(|error| anyhow!(error))
    }
    pub fn save_as(&mut self, path: &Path) -> Result<()> {
        save_profile_new(
            path,
            &self.profile,
            &["Offline application profile; not a device backup or an onboard save.".into()],
        )?;
        self.baseline = self.profile.clone();
        self.path = Some(path.into());
        Ok(())
    }
}
