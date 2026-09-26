//! Convert one confirmed runtime read into portable settings, without transport.
use hyperx_core::{
    SoftwareButtonBinding, SoftwareDpiProfile, SoftwarePollingProfile, SoftwareProfile,
    UnresolvedButtonAssignment,
};
use hyperx_protocol::{
    capture::CaptureRecord,
    ngenuity_legacy::keyboard_usage_name,
    pulsefire_raid::{inspect_captured_report, RaidReportInspection},
    Direction,
};

use super::*;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PulsefireRaidCaptureExport {
    pub profile: SoftwareProfile,
    pub warnings: Vec<String>,
}

#[derive(Debug, Error)]
pub enum PulsefireRaidCaptureExportError {
    #[error("capture export requires an explicit RX direction; use RX hex lines or an OpenHyperX trace log")]
    DirectionRequired,
    #[error("selected report is not a complete Pulsefire Raid runtime device-read response; host writes, onboard images and other collections cannot be exported")]
    NotRuntimeRead,
    #[error("selected snapshot is not usable: {0}")]
    InvalidSnapshot(#[from] PerformanceProfileError),
    #[error(
        "cannot export {control:?}: keyboard usage 0x{usage:04X} has no supported portable name"
    )]
    UnknownKeyboardName {
        control: PulsefireRaidControl,
        usage: u16,
    },
}

/// Never searches for a more convenient snapshot, correlates adjacent macro/RGB
/// packets, or invents settings absent from this image. A parseable profile is
/// NOT necessarily writable by current capabilities; inspect/validate separately.
pub fn export_pulsefire_raid_capture(
    record: &CaptureRecord,
) -> Result<PulsefireRaidCaptureExport, PulsefireRaidCaptureExportError> {
    if record.report.direction != Some(Direction::Rx) {
        return Err(PulsefireRaidCaptureExportError::DirectionRequired);
    }
    let RaidReportInspection::Profile(snapshot) = inspect_captured_report(record) else {
        return Err(PulsefireRaidCaptureExportError::NotRuntimeRead);
    };
    if snapshot.kind() != ProfileImageKind::DeviceReadResponse
        || snapshot.section() != ProfileSection::Runtime
    {
        return Err(PulsefireRaidCaptureExportError::NotRuntimeRead);
    }
    // This rejects empty bodies, malformed layouts/ranges/primary pairs and
    // unknown binding records. Do not silently export their plausible subset.
    snapshot.validate_confirmed_runtime_settings()?;
    let dpi = snapshot.dpi_profile()?;
    let mut profile = SoftwareProfile {
        name: "Pulsefire Raid captured runtime snapshot".into(),
        device: PULSEFIRE_RAID.id.into(),
        partial: true,
        dpi: Some(SoftwareDpiProfile {
            stages: dpi.stages,
            active_stage: Some(dpi.active_stage),
            source_active_stage: None,
        }),
        polling: Some(SoftwarePollingProfile {
            hz: snapshot.polling_rate()?.hz(),
        }),
        primary_buttons: Some(snapshot.primary_button_layout()?),
        // No invented source format_version, firmware version or device serial.
        // The CLI records report/line provenance in comments beside the TOML.
        ..SoftwareProfile::default()
    };
    let mut warnings = vec![
        "Assumes isolated Pulsefire Raid reports; raw bytes/collection numbers do not establish physical-device identity.".into(),
        "Partial historical snapshot, not a live device read or a complete backup; opaque profile bytes are not exported.".into(),
        "Lighting is omitted: current wheel/logo colors and effects cannot be recovered from this profile image; no black/off default is inferred.".into(),
        "Existing macro events, playback and timings cannot be recovered from profile references; adjacent capture packets are not used to guess them.".into(),
    ];
    for control in PulsefireRaidControl::ALL {
        if matches!(
            control,
            PulsefireRaidControl::LeftClick | PulsefireRaidControl::RightClick
        ) {
            continue; // Represent the confirmed legal pair through primary_buttons.
        }
        if snapshot.has_confirmed_macro_reference(control) {
            // An exporter-generated diagnostic label, NOT a vendor slot or
            // an invented executable macro ID. Preserve the omission visibly
            // and use the existing unresolved-assignment gate to block apply.
            profile
                .unresolved_button_assignments
                .push(UnresolvedButtonAssignment {
                    source_id: format!("runtime:{}", control.id()),
                    macro_source_id: None,
                });
            warnings.push(format!("{}: macro reference only; binding omitted and retained as unresolved runtime:{}; supply a real definition or explicitly omit this control before apply.", control.name(), control.id()));
            continue;
        }
        let binding = match snapshot.button_binding(control)? {
            ButtonBinding::Mouse(action) => SoftwareButtonBinding::Mouse { action },
            ButtonBinding::Keyboard(usage) => SoftwareButtonBinding::Keyboard {
                key: keyboard_usage_name(usage.0).ok_or(
                    PulsefireRaidCaptureExportError::UnknownKeyboardName {
                        control,
                        usage: usage.0,
                    },
                )?,
            },
            ButtonBinding::Multimedia(action) => SoftwareButtonBinding::Multimedia { action },
            ButtonBinding::WindowsShortcut(action) => {
                SoftwareButtonBinding::WindowsShortcut { action }
            }
            ButtonBinding::Disabled => SoftwareButtonBinding::Disabled {},
            ButtonBinding::Macro(_) => {
                unreachable!("ordinary decoding never recovers macro timelines")
            }
        };
        profile.buttons.insert(control.id().into(), binding);
    }
    Ok(PulsefireRaidCaptureExport { profile, warnings })
}

#[cfg(test)]
mod tests {
    use super::*;
    use hyperx_protocol::capture::CapturedReport;

    fn read_record() -> CaptureRecord {
        CaptureRecord {
            line: 20,
            interface: Some(1),
            report: CapturedReport {
                direction: Some(Direction::Rx),
                bytes: super::super::tests::save_profile_response(ProfileSection::Runtime, false)
                    .to_vec(),
            },
        }
    }

    #[test]
    fn exports_confirmed_fields_all_binding_families_and_atomic_primary_layout() {
        let mut captured = read_record();
        let mut snapshot = PerformanceProfile::parse(&captured.report.bytes).unwrap();
        snapshot.set_primary_button_layout(PrimaryButtonLayout::Swapped);
        for (control, binding) in [
            (
                PulsefireRaidControl::MiddleClick,
                ButtonBinding::Keyboard(KeyboardUsage(4)),
            ),
            (
                PulsefireRaidControl::Button4,
                ButtonBinding::WindowsShortcut(WindowsShortcut::Copy),
            ),
            (PulsefireRaidControl::Button5, ButtonBinding::Disabled),
        ] {
            snapshot.set_button_binding(control, &binding).unwrap();
        }
        captured.report.bytes = snapshot.as_bytes().to_vec();
        let exported = export_pulsefire_raid_capture(&captured).unwrap();
        let profile = exported.profile;
        assert!(profile.partial);
        assert!(profile.source.is_none());
        assert!(profile.lighting.is_none());
        assert!(profile.macros.is_empty());
        assert_eq!(profile.polling.unwrap().hz, 500);
        assert_eq!(profile.primary_buttons, Some(PrimaryButtonLayout::Swapped));
        assert_eq!(profile.dpi.as_ref().unwrap().active_stage, Some(0));
        assert_eq!(profile.dpi.as_ref().unwrap().stages[0].x, 800);
        assert_eq!(
            profile.dpi.as_ref().unwrap().stages[0].color,
            RgbColor::new(0x2B, 0, 0xFF)
        );
        assert_eq!(profile.buttons.len(), 9);
        assert_eq!(
            profile.buttons["wheel-click"],
            SoftwareButtonBinding::Keyboard { key: "a".into() }
        );
        assert_eq!(
            profile.buttons["button4"],
            SoftwareButtonBinding::WindowsShortcut {
                action: WindowsShortcut::Copy
            }
        );
        assert_eq!(
            profile.buttons["button5"],
            SoftwareButtonBinding::Disabled {}
        );
        assert!(profile.unresolved_button_assignments.is_empty());
        PulsefireRaidSoftwareProfile::new(&profile).unwrap();
    }

    #[test]
    fn macro_references_remain_unresolved_and_block_apply_without_fake_timelines() {
        let mut captured = read_record();
        captured.report.bytes[0x88..0x8C].copy_from_slice(&[0x53, 0, 0, 3]);
        captured.report.bytes[0x8C..0x90].copy_from_slice(&[0x53, 0, 0, 4]);
        let exported = export_pulsefire_raid_capture(&captured).unwrap();
        assert!(exported.profile.macros.is_empty());
        assert!(!exported.profile.buttons.contains_key("button4"));
        assert!(!exported.profile.buttons.contains_key("button5"));
        assert_eq!(
            exported
                .profile
                .unresolved_button_assignments
                .iter()
                .map(|assignment| assignment.source_id.as_str())
                .collect::<Vec<_>>(),
            ["runtime:button4", "runtime:button5"]
        );
        assert!(PulsefireRaidSoftwareProfile::new(&exported.profile).is_err());
    }

    #[test]
    fn rejects_wrong_direction_collection_section_write_empty_and_unknown_images() {
        for direction in [None, Some(Direction::Tx)] {
            let mut captured = read_record();
            captured.report.direction = direction;
            assert!(matches!(
                export_pulsefire_raid_capture(&captured),
                Err(PulsefireRaidCaptureExportError::DirectionRequired)
            ));
        }
        for (offset, value) in [(1, 1), (2, 1), (0x18, 0), (0x1A, 1), (0x97, 0xFF)] {
            let mut captured = read_record();
            captured.report.bytes[offset] = value;
            assert!(export_pulsefire_raid_capture(&captured).is_err());
        }
        let mut captured = read_record();
        captured.report.bytes[3..].fill(0);
        assert!(export_pulsefire_raid_capture(&captured).is_err());
        captured = read_record();
        captured.interface = Some(0);
        assert!(export_pulsefire_raid_capture(&captured).is_err());
        captured = read_record();
        captured.report.bytes.truncate(3);
        assert!(export_pulsefire_raid_capture(&captured).is_err());
    }

    #[test]
    fn preserves_independent_axes_instead_of_claiming_supported_writes() {
        let mut captured = read_record();
        captured.interface = None; // An isolated normalized RX export has no interface metadata.
        captured.report.bytes[0x26] = 18; // 900 Y versus 800 X
        let exported = export_pulsefire_raid_capture(&captured).unwrap();
        let stage = &exported.profile.dpi.as_ref().unwrap().stages[0];
        assert_eq!((stage.x, stage.y), (800, 900));
        assert!(PulsefireRaidSoftwareProfile::new(&exported.profile).is_err());
    }
}
