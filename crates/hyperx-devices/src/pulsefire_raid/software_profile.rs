//! Software-profile application composes existing captured runtime operations.
//! It never selects onboard memory and never installs a background service.
use std::{collections::BTreeSet, fmt, time::Duration};

use hyperx_core::{
    SoftwareButtonBinding, SoftwareDpiProfile, SoftwareLightingMode, SoftwareProfile,
};

use super::*;

// Conservative host-side pacing, NOT a decoded firmware timing requirement.
// The first native immediate-write/read experiment returned 07 81 04 with an
// empty body, unlike earlier independent-process setter/readback checks. Keep
// subsequent operations separated; the minimum safe interval is still unknown.
const RUNTIME_APPLY_SETTLE_DELAY: Duration = Duration::from_secs(1);

/// Validated settings, constructed offline. Private fields prevent clients from
/// bypassing evidence gates; source provenance is never converted to an action.
#[derive(Clone, Debug)]
pub struct PulsefireRaidSoftwareProfile {
    dpi: Option<SoftwareDpiProfile>,
    polling: Option<PollingRate>,
    primary: Option<PrimaryButtonLayout>,
    assignments: Vec<PulsefireRaidRuntimeAssignment>,
    lighting: Option<(RgbColor, RgbColor)>,
    warnings: Vec<String>,
}

#[derive(Debug, Error)]
pub enum PulsefireRaidProfileError {
    #[error("invalid software profile: {message}")]
    InvalidInput { field: String, message: String },
    #[error("{source}")]
    InvalidField {
        field: String,
        #[source]
        source: PulsefireRaidError,
    },
    #[error(transparent)]
    Driver(#[from] PulsefireRaidError),
    #[error("runtime apply stopped at {step}: {source}; earlier changes may remain; no automatic retry or rollback was attempted")]
    ApplyStep {
        step: String,
        #[source]
        source: PulsefireRaidError,
    },
    #[error("runtime readback differs from the planned image: {0}; no RGB was sent and no automatic retry or rollback was attempted")]
    ReadbackMismatch(PulsefireRaidProfileReadbackDiff),
}

impl PulsefireRaidProfileError {
    /// Exact file field when offline validation can identify one. Runtime I/O
    /// failures intentionally have no file location and must not be guessed.
    pub fn field(&self) -> Option<&str> {
        match self {
            Self::InvalidInput { field, .. } => Some(field),
            Self::InvalidField { field, .. } => Some(field),
            Self::Driver(_) | Self::ApplyStep { .. } | Self::ReadbackMismatch(_) => None,
        }
    }
}

/// Raw offsets include the report ID at offset zero. Differences preserve the
/// actual read without normalizing aliases, empty settings or opaque fields.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PulsefireRaidProfileByteDiff {
    pub offset: usize,
    pub expected: u8,
    pub actual: u8,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PulsefireRaidProfileReadbackDiff {
    pub empty_body: bool,
    pub bytes: Vec<PulsefireRaidProfileByteDiff>,
}

impl PulsefireRaidProfileReadbackDiff {
    fn between(expected: &PerformanceProfile, actual: &PerformanceProfile) -> Self {
        Self {
            // The envelope was already parsed. This flag describes bytes, not
            // the cause of the failed read or the device's physical state.
            empty_body: actual.as_bytes()[3..].iter().all(|byte| *byte == 0),
            bytes: expected
                .as_bytes()
                .iter()
                .zip(actual.as_bytes())
                .enumerate()
                .filter_map(|(offset, (&expected, &actual))| {
                    (expected != actual).then_some(PulsefireRaidProfileByteDiff {
                        offset,
                        expected,
                        actual,
                    })
                })
                .collect(),
        }
    }
}

impl fmt::Display for PulsefireRaidProfileReadbackDiff {
    fn fmt(&self, output: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.empty_body {
            output.write_str("empty body after the report header; ")?;
        }
        write!(output, "{} differing byte(s)", self.bytes.len())?;
        for byte in &self.bytes {
            write!(
                output,
                "; 0x{:04X} expected=0x{:02X} actual=0x{:02X}",
                byte.offset, byte.expected, byte.actual
            )?;
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PulsefireRaidProfileChange {
    pub setting: String,
    pub before: String,
    pub after: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PulsefireRaidProfilePreview {
    pub changes: Vec<PulsefireRaidProfileChange>,
    pub warnings: Vec<String>,
}

fn invalid_at(field: impl Into<String>, message: impl Into<String>) -> PulsefireRaidProfileError {
    PulsefireRaidProfileError::InvalidInput {
        field: field.into(),
        message: message.into(),
    }
}

fn invalid_source(
    field: impl Into<String>,
    source: PulsefireRaidError,
) -> PulsefireRaidProfileError {
    PulsefireRaidProfileError::InvalidField {
        field: field.into(),
        source,
    }
}

impl PulsefireRaidSoftwareProfile {
    /// Validate every supplied value, timeline and reference before HID I/O.
    /// Unresolved Legacy/capture assignments require explicit human resolution, rather
    /// than silently dropping requested bindings during an otherwise valid apply.
    pub fn new(profile: &SoftwareProfile) -> Result<Self, PulsefireRaidProfileError> {
        if profile.device != PULSEFIRE_RAID.id {
            return Err(invalid_at(
                "device",
                format!("unsupported device {:?}", profile.device),
            ));
        }
        if !profile.unresolved_button_assignments.is_empty() {
            return Err(invalid_at("unresolved_button_assignments", "unresolved button assignments remain (Legacy source targets or unreadable capture macros); resolve them into [buttons] or explicitly remove them to preserve omitted controls before applying"));
        }
        let mut warnings = Vec::new();
        if profile.partial {
            warnings
                .push("Partial profile: only explicitly supplied settings will be applied.".into());
        }
        if let Some(dpi) = &profile.dpi {
            if !(1..=usize::from(PULSEFIRE_RAID_DPI.max_stages)).contains(&dpi.stages.len()) {
                return Err(invalid_at("dpi.stages", "DPI must contain 1..=5 stages"));
            }
            if dpi
                .active_stage
                .is_some_and(|active| active >= dpi.stages.len())
            {
                return Err(invalid_at(
                    "dpi.active_stage",
                    "DPI active_stage is a zero-based index and must reference a supplied stage",
                ));
            }
            for (index, stage) in dpi.stages.iter().enumerate() {
                PULSEFIRE_RAID_DPI
                    .validate(stage.x)
                    .map_err(PulsefireRaidError::from)
                    .map_err(|source| invalid_source(format!("dpi.stages[{index}].x"), source))?;
                PULSEFIRE_RAID_DPI
                    .validate(stage.y)
                    .map_err(PulsefireRaidError::from)
                    .map_err(|source| invalid_source(format!("dpi.stages[{index}].y"), source))?;
                if stage.x != stage.y {
                    return Err(invalid_at(
                        format!("dpi.stages[{index}]"),
                        "independent X/Y DPI writes are not hardware-confirmed; use equal axes",
                    ));
                }
            }
            if dpi.source_active_stage.is_some() {
                warnings.push(
                    "dpi.source_active_stage is provenance only and will not select a stage."
                        .into(),
                );
            }
        }
        let polling = profile
            .polling
            .map(|polling| {
                polling
                    .hz
                    .to_string()
                    .parse::<PollingRate>()
                    .map_err(|error| invalid_at("polling.hz", error.to_string()))
            })
            .transpose()?;

        let mut ids = BTreeSet::new();
        for (index, named) in profile.macros.iter().enumerate() {
            if named.source_id.trim().is_empty() || !ids.insert(named.source_id.as_str()) {
                return Err(invalid_at(
                    format!("macros[{index}].source_id"),
                    "macro IDs must be nonempty and unique",
                ));
            }
            // Button 4 is the captured superset of runtime playback modes.
            // This validates unused library definitions too, not an upload or
            // permission to assign them to another control. Each actual target
            // is independently gated below by the existing assignment API.
            PulsefireRaidRuntimeAssignment::macro_timeline(
                PulsefireRaidControl::Button4,
                named.definition.clone(),
            )
            .map_err(|source| invalid_source(format!("macros[{index}]"), source))?;
        }
        let mut used_ids = BTreeSet::new();
        let assignments = profile.buttons.iter().map(|(name, binding)| {
            let control = control_by_id(name).ok_or_else(|| invalid_at(format!("buttons.{name}"), format!(
                "unknown or primary control {name:?}; use primary_buttons for the coupled left/right pair"
            )))?;
            let ordinary = match binding {
                SoftwareButtonBinding::Mouse { action } => ButtonBinding::Mouse(*action),
                SoftwareButtonBinding::Keyboard { key } => ButtonBinding::Keyboard(
                    key.parse().map_err(|error: hyperx_core::KeyboardUsageParseError| invalid_at(format!("buttons.{name}.key"), error.to_string()))?
                ),
                SoftwareButtonBinding::Multimedia { action } => ButtonBinding::Multimedia(*action),
                SoftwareButtonBinding::WindowsShortcut { action } => ButtonBinding::WindowsShortcut(*action),
                SoftwareButtonBinding::Disabled {} => ButtonBinding::Disabled,
                SoftwareButtonBinding::Macro { id } => {
                    let named = profile.macros.iter().find(|named| named.source_id == *id)
                        .ok_or_else(|| invalid_at(format!("buttons.{name}.id"), format!("{name} references missing macro {id:?}")))?;
                    used_ids.insert(id.as_str());
                    return PulsefireRaidRuntimeAssignment::macro_timeline(control, named.definition.clone())
                        .map_err(|source| invalid_source(format!("buttons.{name}"), source));
                }
            };
            PulsefireRaidRuntimeAssignment::ordinary(control, ordinary)
                .map_err(|source| invalid_source(format!("buttons.{name}"), source))
        }).collect::<Result<Vec<_>, PulsefireRaidProfileError>>()?;
        for named in &profile.macros {
            if !used_ids.contains(named.source_id.as_str()) {
                warnings.push(format!(
                    "Macro {:?} is unassigned and will not be uploaded.",
                    named.source_id
                ));
            }
        }

        let lighting = profile.lighting.as_ref().map(|lighting| {
            match lighting.mode {
                SoftwareLightingMode::Solid => {}
            }
            // There is no confirmed read of current direct LED colors. A packet
            // writes both zones, so accepting only one would overwrite an omitted
            // setting with a guessed value. Require both, never default to black.
            if lighting.zones.len() != 2 || !lighting.zones.contains_key("wheel") || !lighting.zones.contains_key("logo") {
                return Err(invalid_at("lighting.zones", "Solid lighting requires exactly wheel and logo zones; current direct colors cannot be read to preserve an omitted zone"));
            }
            warnings.push("Solid RGB is volatile direct lighting; it reverts after keepalive ends. Persist separately with save-to-mouse.".into());
            Ok((lighting.zones["wheel"], lighting.zones["logo"]))
        }).transpose()?;
        if profile.dpi.is_none()
            && polling.is_none()
            && profile.primary_buttons.is_none()
            && assignments.is_empty()
            && lighting.is_none()
        {
            return Err(invalid_at(
                "profile",
                "no applicable settings were supplied",
            ));
        }
        Ok(Self {
            dpi: profile.dpi.clone(),
            polling,
            primary: profile.primary_buttons,
            assignments,
            lighting,
            warnings,
        })
    }

    pub const fn direct_colors(&self) -> Option<(RgbColor, RgbColor)> {
        self.lighting
    }

    pub fn warnings(&self) -> &[String] {
        &self.warnings
    }

    fn plan(&self, baseline: PerformanceProfile) -> Result<ApplyPlan, PulsefireRaidProfileError> {
        // A valid report envelope alone does not establish a usable snapshot:
        // the failed native run returned 07 81 04 followed by all zeroes. Gate
        // EVERY path, including RGB-only/no-op, before any mutable operation.
        baseline
            .validate_confirmed_runtime_settings()
            .map_err(PulsefireRaidError::from)?;
        let mut profile = baseline;
        let mut writes = Vec::new();
        let mut changes = Vec::new();
        if let Some(polling) = self.polling {
            let old = profile.polling_rate().map_err(PulsefireRaidError::from)?;
            if old != polling {
                profile.set_polling_rate(polling);
                push_step(
                    &mut writes,
                    &mut changes,
                    &profile,
                    "polling",
                    format!("{} Hz", old.hz()),
                    format!("{} Hz", polling.hz()),
                    None,
                );
            }
        }
        if let Some(dpi) = &self.dpi {
            let old = profile.dpi_profile().map_err(PulsefireRaidError::from)?;
            let desired = DpiProfile {
                stages: dpi.stages.clone(),
                active_stage: dpi.active_stage.unwrap_or(old.active_stage),
            };
            // Removing the currently active stage without specifying a new one
            // is an error, not an implicit activation or clamp to another stage.
            let mut patched = profile.clone();
            patched
                .set_dpi_profile(&desired)
                .map_err(PulsefireRaidError::from)?;
            if old != desired {
                profile = patched;
                push_step(
                    &mut writes,
                    &mut changes,
                    &profile,
                    "dpi",
                    describe_dpi(&old),
                    describe_dpi(&desired),
                    None,
                );
            }
        }
        if let Some(primary) = self.primary {
            let old = profile
                .primary_button_layout()
                .map_err(PulsefireRaidError::from)?;
            if old != primary {
                profile.set_primary_button_layout(primary);
                push_step(
                    &mut writes,
                    &mut changes,
                    &profile,
                    "primary_buttons",
                    format!("{old:?}"),
                    format!("{primary:?}"),
                    None,
                );
            }
        }
        for assignment in &self.assignments {
            let control = assignment.control();
            // Ordinary decoding intentionally excludes macro references: they
            // contain neither playback nor events. Recognize the confirmed
            // slot separately instead of inventing a complete MacroBinding.
            let old = if profile.has_confirmed_macro_reference(control) {
                None
            } else {
                Some(
                    profile
                        .button_binding(control)
                        .map_err(PulsefireRaidError::from)?,
                )
            };
            let before = old.as_ref().map_or_else(
                || "Macro reference (existing timeline unreadable)".into(),
                |binding| format!("{binding:?}"),
            );
            if let Some(definition) = assignment.encoded_macro()? {
                definition
                    .apply_to_profile(&mut profile)
                    .map_err(PulsefireRaidError::from)?;
                // A profile contains only a macro reference, never its events or
                // mode. Even an identical reference MUST upload the supplied
                // definition; a readback cannot prove the timeline is unchanged.
                let timeline = assignment.macro_definition().expect("macro assignment");
                push_step(
                    &mut writes,
                    &mut changes,
                    &profile,
                    control.name(),
                    format!("{before}; existing timeline unreadable"),
                    format!(
                        "macro {} ({} events); definition upload required",
                        timeline.playback,
                        timeline.events.len()
                    ),
                    Some(definition),
                );
            } else {
                let desired = assignment.ordinary_binding().expect("ordinary assignment");
                if old.as_ref() != Some(desired) {
                    profile
                        .set_button_binding(control, desired)
                        .map_err(PulsefireRaidError::from)?;
                    push_step(
                        &mut writes,
                        &mut changes,
                        &profile,
                        control.name(),
                        before,
                        format!("{desired:?}"),
                        None,
                    );
                }
            }
        }
        if let Some((wheel, logo)) = self.lighting {
            changes.push(PulsefireRaidProfileChange {
                setting: "lighting (both zones)".into(),
                before: "unreadable; not compared".into(),
                after: format!("Solid wheel={wheel}, logo={logo}; volatile keepalive"),
            });
        }
        Ok(ApplyPlan {
            preview: PulsefireRaidProfilePreview {
                changes,
                warnings: self.warnings.clone(),
            },
            writes,
            expected: profile,
        })
    }
}

fn control_by_id(id: &str) -> Option<PulsefireRaidControl> {
    PulsefireRaidControl::ALL.into_iter().find(|control| {
        control.id() == id
            && !matches!(
                control,
                PulsefireRaidControl::LeftClick | PulsefireRaidControl::RightClick
            )
    })
}

fn describe_dpi(profile: &DpiProfile) -> String {
    format!(
        "[{}], active_stage={}",
        profile
            .stages
            .iter()
            .map(|stage| { format!("{}/{} {}", stage.x, stage.y, stage.color) })
            .collect::<Vec<_>>()
            .join(", "),
        profile.active_stage
    )
}

struct ApplyStep {
    label: String,
    profile: PerformanceProfile,
    macro_report: Option<PulsefireRaidMacro>,
}

struct ApplyPlan {
    preview: PulsefireRaidProfilePreview,
    writes: Vec<ApplyStep>,
    expected: PerformanceProfile,
}

fn push_step(
    writes: &mut Vec<ApplyStep>,
    changes: &mut Vec<PulsefireRaidProfileChange>,
    profile: &PerformanceProfile,
    setting: &str,
    before: String,
    after: String,
    macro_report: Option<PulsefireRaidMacro>,
) {
    changes.push(PulsefireRaidProfileChange {
        setting: setting.into(),
        before,
        after,
    });
    writes.push(ApplyStep {
        label: setting.into(),
        profile: profile.clone(),
        macro_report,
    });
}

impl<T: HidTransport> PulsefireRaid<T> {
    /// Reads runtime state but sends no setting writes, RGB, startup or onboard
    /// selectors. Its selector/request reports are the confirmed read sequence.
    pub fn preview_software_profile(
        &mut self,
        settings: &PulsefireRaidSoftwareProfile,
    ) -> Result<PulsefireRaidProfilePreview, PulsefireRaidProfileError> {
        self.preview_software_profile_with_wait(settings, std::thread::sleep)
    }

    fn preview_software_profile_with_wait(
        &mut self,
        settings: &PulsefireRaidSoftwareProfile,
        wait: impl FnMut(Duration),
    ) -> Result<PulsefireRaidProfilePreview, PulsefireRaidProfileError> {
        Ok(settings
            .plan(self.runtime_profile_with_wait(wait)?)?
            .preview)
    }

    /// Precompute and validate ALL steps against a fresh snapshot before the
    /// first mutation. Each step uses an existing captured operation, not a new
    /// combined transaction. Stop on the first failure; no rollback, retries or
    /// onboard save. RGB is sent only after full profile-image readback matches.
    pub fn apply_software_profile(
        &mut self,
        settings: &PulsefireRaidSoftwareProfile,
    ) -> Result<PulsefireRaidProfilePreview, PulsefireRaidProfileError> {
        self.apply_software_profile_with_wait(settings, std::thread::sleep)
    }

    fn apply_software_profile_with_wait(
        &mut self,
        settings: &PulsefireRaidSoftwareProfile,
        mut wait: impl FnMut(Duration),
    ) -> Result<PulsefireRaidProfilePreview, PulsefireRaidProfileError> {
        let plan = settings.plan(self.runtime_profile_with_wait(&mut wait)?)?;
        for step in &plan.writes {
            let result = (|| {
                if let Some(definition) = &step.macro_report {
                    self.send_feature_report(definition.as_bytes())?;
                }
                self.write_runtime_profile(&step.profile)
            })();
            result.map_err(|source| PulsefireRaidProfileError::ApplyStep {
                step: step.label.clone(),
                source,
            })?;
            wait(RUNTIME_APPLY_SETTLE_DELAY);
        }
        if !plan.writes.is_empty() {
            let actual = self
                .runtime_profile_with_wait(&mut wait)
                .map_err(|source| PulsefireRaidProfileError::ApplyStep {
                    step: "readback".into(),
                    source,
                })?;
            if actual != plan.expected {
                return Err(PulsefireRaidProfileError::ReadbackMismatch(
                    PulsefireRaidProfileReadbackDiff::between(&plan.expected, &actual),
                ));
            }
        }
        if let Some((wheel, logo)) = settings.lighting {
            self.set_volatile_direct_rgb(wheel, logo)
                .map_err(|source| PulsefireRaidProfileError::ApplyStep {
                    step: "volatile lighting".into(),
                    source,
                })?;
        }
        Ok(plan.preview)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hyperx_core::{
        NamedMacro, SoftwareLightingMode, SoftwareLightingProfile, SoftwarePollingProfile,
        UnresolvedButtonAssignment,
    };
    use hyperx_hid::testing::MockHidTransport;

    fn input() -> SoftwareProfile {
        SoftwareProfile {
            name: "Test".into(),
            device: PULSEFIRE_RAID.id.into(),
            polling: Some(SoftwarePollingProfile { hz: 1000 }),
            ..SoftwareProfile::default()
        }
    }

    fn baseline() -> [u8; DIRECT_REPORT_LENGTH] {
        let mut report = super::super::tests::save_profile_response(ProfileSection::Runtime, false);
        report[0x10] = 0xAC; // Unknown fields must survive EVERY intermediate write.
        report[0x60] = 0xD3;
        report[0xB0] = 0xED;
        report[263] = 0xEF;
        report
    }

    fn expect_read(mock: &mut MockHidTransport, report: [u8; DIRECT_REPORT_LENGTH]) {
        mock.expect_feature_report(encode_runtime_profile_read_prelude());
        mock.expect_feature_report(encode_profile_read_request());
        mock.queue_feature_response(report);
    }

    fn named_macro() -> NamedMacro {
        NamedMacro {
            source_id: "ab".into(),
            name: "AB".into(),
            definition: super::super::tests::captured_ab_macro(),
        }
    }

    #[test]
    fn offline_validation_rejects_invalid_dpi_on_every_axis_and_stage() {
        for count in 1..=5 {
            for index in 0..count {
                for x_axis in [true, false] {
                    for value in [0, 199, 225, 16_001, 16_050] {
                        let mut stages =
                            vec![hyperx_core::DpiStage::new(800, 800, RgbColor::BLACK); count];
                        if x_axis {
                            stages[index].x = value;
                        } else {
                            stages[index].y = value;
                        }
                        let profile = SoftwareProfile {
                            dpi: Some(SoftwareDpiProfile {
                                stages,
                                active_stage: Some(0),
                                source_active_stage: None,
                            }),
                            ..input()
                        };
                        assert!(
                            PulsefireRaidSoftwareProfile::new(&profile).is_err(),
                            "{count} {index} {x_axis} {value}"
                        );
                    }
                }
            }
        }
        let mut profile = input();
        profile.dpi = Some(SoftwareDpiProfile {
            stages: vec![hyperx_core::DpiStage::new(800, 900, RgbColor::BLACK)],
            active_stage: Some(0),
            source_active_stage: None,
        });
        assert!(PulsefireRaidSoftwareProfile::new(&profile)
            .unwrap_err()
            .to_string()
            .contains("independent X/Y"));
        for (count, active) in [(0, 0), (6, 0), (1, 1)] {
            profile.dpi = Some(SoftwareDpiProfile {
                stages: vec![hyperx_core::DpiStage::new(800, 800, RgbColor::BLACK); count],
                active_stage: Some(active),
                source_active_stage: None,
            });
            assert!(PulsefireRaidSoftwareProfile::new(&profile).is_err());
        }
    }

    #[test]
    fn offline_validation_rejects_unknown_device_polling_primary_targets_and_unresolved_imports() {
        let mut profile = input();
        profile.device = "other-device".into();
        assert!(PulsefireRaidSoftwareProfile::new(&profile).is_err());
        profile = input();
        profile.polling.as_mut().unwrap().hz = 2000;
        assert!(PulsefireRaidSoftwareProfile::new(&profile).is_err());
        for control in ["left", "right", "left-click", "unknown", "button9"] {
            profile = input();
            profile
                .buttons
                .insert(control.into(), SoftwareButtonBinding::Disabled {});
            assert!(PulsefireRaidSoftwareProfile::new(&profile).is_err());
        }
        profile = input();
        profile
            .unresolved_button_assignments
            .push(UnresolvedButtonAssignment {
                source_id: "unknown".into(),
                macro_source_id: None,
            });
        assert!(PulsefireRaidSoftwareProfile::new(&profile)
            .unwrap_err()
            .to_string()
            .contains("unresolved button assignments"));
        profile = SoftwareProfile {
            polling: None,
            ..input()
        };
        assert!(PulsefireRaidSoftwareProfile::new(&profile).is_err());
    }

    #[test]
    fn offline_validation_rejects_ambiguous_and_invalid_macro_references_and_modes() {
        let mut profile = input();
        profile.buttons.insert(
            "button4".into(),
            SoftwareButtonBinding::Macro { id: "ab".into() },
        );
        assert!(PulsefireRaidSoftwareProfile::new(&profile)
            .unwrap_err()
            .to_string()
            .contains("missing macro"));
        profile.macros.push(named_macro());
        assert!(PulsefireRaidSoftwareProfile::new(&profile).is_ok());
        profile.macros.push(named_macro());
        assert!(PulsefireRaidSoftwareProfile::new(&profile).is_err());
        profile.macros.pop();
        profile.macros[0].source_id.clear();
        assert!(PulsefireRaidSoftwareProfile::new(&profile).is_err());
        profile.macros[0] = named_macro();
        profile.macros[0].definition.events.pop();
        assert!(PulsefireRaidSoftwareProfile::new(&profile).is_err());
        profile.macros[0] = named_macro();
        profile.macros[0].definition.playback = hyperx_core::MacroPlayback::ToggleRepeat;
        assert!(PulsefireRaidSoftwareProfile::new(&profile).is_ok());
        profile.buttons.clear();
        profile.buttons.insert(
            "button5".into(),
            SoftwareButtonBinding::Macro { id: "ab".into() },
        );
        assert!(PulsefireRaidSoftwareProfile::new(&profile).is_err());
        profile.buttons.clear();
        profile.buttons.insert(
            "dpi".into(),
            SoftwareButtonBinding::Macro { id: "ab".into() },
        );
        assert!(PulsefireRaidSoftwareProfile::new(&profile).is_err());
        profile.buttons.clear();
        profile.macros[0].definition.events[0] = MacroEvent::KeyDown {
            key: "unknown".into(),
            delay_ms: 20,
        };
        assert!(
            PulsefireRaidSoftwareProfile::new(&profile).is_err(),
            "unused invalid definition must not pass"
        );
    }

    #[test]
    fn offline_validation_requires_both_known_rgb_zones_without_defaulting_missing_colors() {
        for zones in [
            vec!["wheel"],
            vec!["logo"],
            vec!["wheel", "logo", "other"],
            vec!["wheel", "unknown"],
        ] {
            let profile = SoftwareProfile {
                lighting: Some(SoftwareLightingProfile {
                    mode: SoftwareLightingMode::Solid,
                    zones: zones
                        .into_iter()
                        .map(|zone| (zone.into(), RgbColor::BLACK))
                        .collect(),
                }),
                ..input()
            };
            assert!(PulsefireRaidSoftwareProfile::new(&profile).is_err());
        }
        let profile = SoftwareProfile {
            lighting: Some(SoftwareLightingProfile {
                mode: SoftwareLightingMode::Solid,
                zones: [
                    ("wheel".into(), RgbColor::BLACK),
                    ("logo".into(), RgbColor::new(0, 0, 255)),
                ]
                .into(),
            }),
            ..input()
        };
        assert_eq!(
            PulsefireRaidSoftwareProfile::new(&profile)
                .unwrap()
                .direct_colors(),
            Some((RgbColor::BLACK, RgbColor::new(0, 0, 255)))
        );
    }

    #[test]
    fn preview_performs_only_confirmed_runtime_reads_and_reports_unknown_rgb_and_macro_contents() {
        let mut profile = input();
        profile.buttons.insert(
            "button4".into(),
            SoftwareButtonBinding::Macro { id: "ab".into() },
        );
        profile.macros.push(named_macro());
        profile.lighting = Some(SoftwareLightingProfile {
            mode: SoftwareLightingMode::Solid,
            zones: [
                ("wheel".into(), RgbColor::BLACK),
                ("logo".into(), RgbColor::new(0, 0, 255)),
            ]
            .into(),
        });
        let settings = PulsefireRaidSoftwareProfile::new(&profile).unwrap();
        let mut mock = MockHidTransport::new(1);
        expect_read(&mut mock, baseline());
        let mut device = PulsefireRaid::new(mock).unwrap();
        let preview = device
            .preview_software_profile_with_wait(&settings, |_| {})
            .unwrap();
        assert_eq!(preview.changes.len(), 3);
        assert!(preview.changes[1].before.contains("unreadable"));
        assert!(preview.changes[2].before.contains("not compared"));
        device.into_transport().assert_drained();
    }

    #[test]
    fn late_state_dependent_error_aborts_before_earlier_planned_polling_write() {
        let mut source = PerformanceProfile::parse(&baseline()).unwrap();
        let mut current_dpi = source.dpi_profile().unwrap();
        current_dpi
            .stages
            .push(hyperx_core::DpiStage::new(1600, 1600, RgbColor::BLACK));
        current_dpi.active_stage = 1;
        source.set_dpi_profile(&current_dpi).unwrap();
        let settings = PulsefireRaidSoftwareProfile::new(&SoftwareProfile {
            dpi: Some(SoftwareDpiProfile {
                stages: vec![current_dpi.stages[0]],
                active_stage: None,
                source_active_stage: Some(999),
            }),
            ..input()
        })
        .unwrap();
        assert!(settings
            .warnings()
            .iter()
            .any(|warning| warning.contains("provenance")));
        let mut mock = MockHidTransport::new(1);
        expect_read(&mut mock, *source.as_bytes());
        let mut device = PulsefireRaid::new(mock).unwrap();
        assert!(device
            .apply_software_profile_with_wait(&settings, |_| {})
            .is_err());
        device.into_transport().assert_drained();
    }

    #[test]
    fn apply_sends_existing_operations_in_order_preserves_opaque_bytes_and_verifies_final_image() {
        let mut profile = input();
        profile.dpi = Some(SoftwareDpiProfile {
            stages: vec![hyperx_core::DpiStage::new(900, 900, RgbColor::new(1, 2, 3))],
            active_stage: None,
            source_active_stage: None,
        });
        profile.primary_buttons = Some(PrimaryButtonLayout::Swapped);
        profile.buttons.insert(
            "button4".into(),
            SoftwareButtonBinding::Macro { id: "ab".into() },
        );
        profile
            .buttons
            .insert("button6".into(), SoftwareButtonBinding::Disabled {});
        profile.macros.push(named_macro());
        profile.lighting = Some(SoftwareLightingProfile {
            mode: SoftwareLightingMode::Solid,
            zones: [
                ("wheel".into(), RgbColor::BLACK),
                ("logo".into(), RgbColor::new(0, 0, 255)),
            ]
            .into(),
        });
        let settings = PulsefireRaidSoftwareProfile::new(&profile).unwrap();
        let source = baseline();
        let mut mock = MockHidTransport::new(1);
        expect_read(&mut mock, source);
        // Independent byte-level expectations: do not derive TX from the plan
        // being tested. Old binding aliases and unrelated opaque fields survive.
        let mut expected = source;
        expected[1] = 0x01;
        expected[0x18] = 0x01; // 1 ms / 1000 Hz
        mock.expect_feature_report(expected);
        expected[0x19..0x1B].copy_from_slice(&[0, 18]);
        expected[0x25..0x27].copy_from_slice(&[0, 18]);
        expected[0x69..0x6C].copy_from_slice(&[1, 2, 3]);
        mock.expect_feature_report(expected);
        expected[0x7C..0x84].copy_from_slice(&[2, 0xF2, 0, 2, 2, 0xF0, 0, 0]);
        mock.expect_feature_report(expected);
        let golden_macro = hyperx_protocol::capture::parse_hex_capture(include_str!(
            "../../../hyperx-protocol/tests/fixtures/button4-ab-once.hex"
        ))
        .unwrap();
        mock.expect_feature_report(golden_macro[0].bytes.clone());
        expected[0x88..0x8C].copy_from_slice(&[0x53, 0, 0, 3]);
        mock.expect_feature_report(expected);
        expected[0x94..0x98].fill(0);
        mock.expect_feature_report(expected);
        expected[1] = 0x81;
        expect_read(&mut mock, expected);
        let mut rgb = [0_u8; DIRECT_REPORT_LENGTH];
        rgb[..8].copy_from_slice(&[7, 0x0A, 0, 0, 0, 0, 0, 255]);
        rgb[8] = 0xA0; // Captured direct-mode selector, not part of either color.
        mock.expect_feature_report(rgb);
        let mut device = PulsefireRaid::new(mock).unwrap();
        let mut waits = Vec::new();
        let preview = device
            .apply_software_profile_with_wait(&settings, |delay| waits.push(delay))
            .unwrap();
        assert_eq!(preview.changes.len(), 6);
        assert_eq!(
            waits,
            vec![
                PROFILE_PRELUDE_DELAY,
                PROFILE_READ_DELAY,
                RUNTIME_APPLY_SETTLE_DELAY,
                RUNTIME_APPLY_SETTLE_DELAY,
                RUNTIME_APPLY_SETTLE_DELAY,
                RUNTIME_APPLY_SETTLE_DELAY,
                RUNTIME_APPLY_SETTLE_DELAY,
                PROFILE_PRELUDE_DELAY,
                PROFILE_READ_DELAY,
            ]
        );
        device.into_transport().assert_drained();
    }

    #[test]
    fn unchanged_ordinary_aliases_are_not_reencoded_and_noop_sends_no_setting_writes() {
        let mut profile = input();
        profile.polling.as_mut().unwrap().hz = 500;
        profile.buttons.insert(
            "wheel-click".into(),
            SoftwareButtonBinding::Mouse {
                action: MouseFunction::MiddleClick,
            },
        );
        let settings = PulsefireRaidSoftwareProfile::new(&profile).unwrap();
        let mut mock = MockHidTransport::new(1);
        expect_read(&mut mock, baseline());
        let mut device = PulsefireRaid::new(mock).unwrap();
        let preview = device
            .apply_software_profile_with_wait(&settings, |_| {})
            .unwrap();
        assert!(preview.changes.is_empty());
        device.into_transport().assert_drained();
    }

    #[test]
    fn same_macro_reference_still_uploads_definition_and_commits_its_reference() {
        let mut profile = input();
        profile.polling = None;
        profile.macros.push(named_macro());
        profile.buttons.insert(
            "button4".into(),
            SoftwareButtonBinding::Macro { id: "ab".into() },
        );
        let settings = PulsefireRaidSoftwareProfile::new(&profile).unwrap();
        let mut response = baseline();
        response[0x88..0x8C].copy_from_slice(&[0x53, 0, 0, 3]);
        let mut mock = MockHidTransport::new(1);
        expect_read(&mut mock, response);
        let golden_macro = hyperx_protocol::capture::parse_hex_capture(include_str!(
            "../../../hyperx-protocol/tests/fixtures/button4-ab-once.hex"
        ))
        .unwrap();
        mock.expect_feature_report(golden_macro[0].bytes.clone());
        let mut write = response;
        write[1] = 1;
        mock.expect_feature_report(write);
        expect_read(&mut mock, response);
        let mut device = PulsefireRaid::new(mock).unwrap();
        assert_eq!(
            device
                .apply_software_profile_with_wait(&settings, |_| {})
                .unwrap()
                .changes
                .len(),
            1
        );
        device.into_transport().assert_drained();
    }

    #[test]
    fn readback_mismatch_aborts_without_rgb_or_retry() {
        let mut profile = input();
        profile.lighting = Some(SoftwareLightingProfile {
            mode: SoftwareLightingMode::Solid,
            zones: [
                ("wheel".into(), RgbColor::BLACK),
                ("logo".into(), RgbColor::new(0, 0, 255)),
            ]
            .into(),
        });
        let settings = PulsefireRaidSoftwareProfile::new(&profile).unwrap();
        let response = baseline();
        let mut mock = MockHidTransport::new(1);
        expect_read(&mut mock, response);
        let mut write = response;
        write[1] = 1;
        write[0x18] = 1;
        mock.expect_feature_report(write);
        // The failed native run returned a valid header with a completely
        // empty body. Never accept this as success, normalize it, or retry.
        let mut empty_readback = [0_u8; DIRECT_REPORT_LENGTH];
        empty_readback[..3].copy_from_slice(&[7, 0x81, 4]);
        expect_read(&mut mock, empty_readback);
        let mut device = PulsefireRaid::new(mock).unwrap();
        let error = device
            .apply_software_profile_with_wait(&settings, |_| {})
            .unwrap_err();
        let PulsefireRaidProfileError::ReadbackMismatch(ref diff) = error else {
            panic!("expected readback mismatch, got {error}");
        };
        assert!(diff.empty_body);
        assert!(diff.bytes.contains(&PulsefireRaidProfileByteDiff {
            offset: 0x18,
            expected: 1,
            actual: 0,
        }));
        assert!(error
            .to_string()
            .contains("empty body after the report header"));
        let mock = device.into_transport();
        assert_eq!(mock.feature_reports_sent().len(), 5);
        assert_eq!(mock.feature_read_attempts(), 2);
        mock.assert_drained();
    }

    #[test]
    fn empty_baseline_blocks_every_apply_path_including_rgb_only_and_preview() {
        let mut empty = [0_u8; DIRECT_REPORT_LENGTH];
        empty[..3].copy_from_slice(&[7, 0x81, 4]);
        for rgb_only in [false, true] {
            let mut profile = input();
            if rgb_only {
                profile.polling = None;
                profile.lighting = Some(SoftwareLightingProfile {
                    mode: SoftwareLightingMode::Solid,
                    zones: [
                        ("wheel".into(), RgbColor::BLACK),
                        ("logo".into(), RgbColor::BLACK),
                    ]
                    .into(),
                });
            }
            let settings = PulsefireRaidSoftwareProfile::new(&profile).unwrap();
            for preview in [false, true] {
                let mut mock = MockHidTransport::new(1);
                expect_read(&mut mock, empty);
                let mut device = PulsefireRaid::new(mock).unwrap();
                let result = if preview {
                    device.preview_software_profile_with_wait(&settings, |_| {})
                } else {
                    device.apply_software_profile_with_wait(&settings, |_| {})
                };
                assert!(result.is_err());
                device.into_transport().assert_drained();
            }
        }
    }

    #[test]
    fn failed_macro_upload_aborts_before_reference_write_and_following_steps() {
        let mut profile = input();
        profile.polling = None;
        profile.macros.push(named_macro());
        profile.buttons.insert(
            "button4".into(),
            SoftwareButtonBinding::Macro { id: "ab".into() },
        );
        profile
            .buttons
            .insert("button6".into(), SoftwareButtonBinding::Disabled {});
        let settings = PulsefireRaidSoftwareProfile::new(&profile).unwrap();
        let mut mock = MockHidTransport::new(1);
        expect_read(&mut mock, baseline());
        let definition = settings.assignments[0].encoded_macro().unwrap().unwrap();
        mock.expect_feature_report_result(
            definition.as_bytes().to_vec(),
            Err(HidError::Transport("injected macro upload failure".into())),
        );
        let mut device = PulsefireRaid::new(mock).unwrap();
        let error = device
            .apply_software_profile_with_wait(&settings, |_| {})
            .unwrap_err();
        assert!(
            matches!(error, PulsefireRaidProfileError::ApplyStep { ref step, .. } if step == "Button 4")
        );
        let mock = device.into_transport();
        assert_eq!(mock.feature_reports_sent().len(), 3);
        assert_eq!(mock.feature_read_attempts(), 1);
        mock.assert_drained();
    }

    #[test]
    fn readback_diff_keeps_exact_offsets_and_expected_actual_values_including_opaque_bytes() {
        let expected = PerformanceProfile::parse(&baseline()).unwrap();
        let mut raw = baseline();
        raw[0x18] = 1;
        raw[0xB0] = 0xAA; // Unknown bytes must be compared too.
        let actual = PerformanceProfile::parse(&raw).unwrap();
        let diff = PulsefireRaidProfileReadbackDiff::between(&expected, &actual);
        assert!(!diff.empty_body);
        assert_eq!(
            diff.bytes,
            [
                PulsefireRaidProfileByteDiff {
                    offset: 0x18,
                    expected: 2,
                    actual: 1
                },
                PulsefireRaidProfileByteDiff {
                    offset: 0xB0,
                    expected: 0xED,
                    actual: 0xAA
                },
            ]
        );
        assert_eq!(
            diff.to_string(),
            "2 differing byte(s); 0x0018 expected=0x02 actual=0x01; 0x00B0 expected=0xED actual=0xAA"
        );
        assert!(
            PulsefireRaidProfileReadbackDiff::between(&expected, &expected)
                .bytes
                .is_empty()
        );
    }

    #[test]
    fn late_profile_write_failure_stops_remaining_steps_readback_and_rgb() {
        let mut profile = input();
        profile
            .buttons
            .insert("button6".into(), SoftwareButtonBinding::Disabled {});
        profile
            .buttons
            .insert("button7".into(), SoftwareButtonBinding::Disabled {});
        profile.lighting = Some(SoftwareLightingProfile {
            mode: SoftwareLightingMode::Solid,
            zones: [
                ("wheel".into(), RgbColor::BLACK),
                ("logo".into(), RgbColor::BLACK),
            ]
            .into(),
        });
        let settings = PulsefireRaidSoftwareProfile::new(&profile).unwrap();
        let mut polling_write = baseline();
        polling_write[1] = 1;
        polling_write[0x18] = 1;
        let mut button_write = polling_write;
        button_write[0x94..0x98].fill(0); // Button 6 Disabled
        for short_write in [false, true] {
            let mut mock = MockHidTransport::new(1);
            expect_read(&mut mock, baseline());
            mock.expect_feature_report(polling_write);
            mock.expect_feature_report_result(
                button_write,
                if short_write {
                    Ok(DIRECT_REPORT_LENGTH - 1)
                } else {
                    Err(HidError::Transport(
                        "injected late profile write failure".into(),
                    ))
                },
            );
            let mut device = PulsefireRaid::new(mock).unwrap();
            let error = device
                .apply_software_profile_with_wait(&settings, |_| {})
                .unwrap_err();
            assert!(
                matches!(error, PulsefireRaidProfileError::ApplyStep { ref step, .. } if step == "Button 6")
            );
            assert!(error.to_string().contains("earlier changes may remain"));
            let mock = device.into_transport();
            assert_eq!(mock.feature_reports_sent().len(), 4);
            assert_eq!(mock.feature_read_attempts(), 1);
            mock.assert_drained();
        }
    }

    #[test]
    fn failed_or_invalid_readback_stops_before_rgb_without_retry() {
        let mut profile = input();
        profile.lighting = Some(SoftwareLightingProfile {
            mode: SoftwareLightingMode::Solid,
            zones: [
                ("wheel".into(), RgbColor::BLACK),
                ("logo".into(), RgbColor::BLACK),
            ]
            .into(),
        });
        let settings = PulsefireRaidSoftwareProfile::new(&profile).unwrap();
        let mut onboard = baseline();
        onboard[2] = 1;
        let mut wrong_kind = baseline();
        wrong_kind[1] = 1;
        for response in [
            Err(HidError::Transport("injected readback timeout".into())),
            Ok(vec![7, 0x81, 4]),
            Ok(onboard.to_vec()),
            Ok(wrong_kind.to_vec()),
        ] {
            let mut mock = MockHidTransport::new(1);
            expect_read(&mut mock, baseline());
            let mut write = baseline();
            write[1] = 1;
            write[0x18] = 1;
            mock.expect_feature_report(write);
            mock.expect_feature_report(encode_runtime_profile_read_prelude());
            mock.expect_feature_report(encode_profile_read_request());
            match response {
                Ok(report) => mock.queue_feature_response(report),
                Err(error) => mock.queue_feature_error(error),
            }
            let mut device = PulsefireRaid::new(mock).unwrap();
            let error = device
                .apply_software_profile_with_wait(&settings, |_| {})
                .unwrap_err();
            assert!(
                matches!(error, PulsefireRaidProfileError::ApplyStep { ref step, .. } if step == "readback")
            );
            let mock = device.into_transport();
            assert_eq!(mock.feature_reports_sent().len(), 5);
            assert_eq!(mock.feature_read_attempts(), 2);
            mock.assert_drained();
        }
    }
}
