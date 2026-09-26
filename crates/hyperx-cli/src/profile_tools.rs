//! Offline profile conversion and rendering. No HID handles or replay paths.
use std::num::NonZeroUsize;

use hyperx_core::{MacroEvent, SoftwareButtonBinding};
use hyperx_devices::export_pulsefire_raid_capture;

use super::*;

pub(super) fn export_capture(input: &Path, output: &Path, report: NonZeroUsize) -> Result<()> {
    let log = load_report_log(input)?;
    let record = log.records.get(report.get() - 1).ok_or_else(|| {
        anyhow!(
            "report {} is outside this capture's {} report(s); use profile inspect-capture first",
            report,
            log.records.len()
        )
    })?;
    let exported = export_pulsefire_raid_capture(record)?;
    // Serialize before creating the destination. No partial TOML on a parse,
    // selection, device-layout or serialization error; never overwrite a file.
    // Capture paths may be private, so retain only report/line provenance here.
    let mut encoded = format!(
        "# OpenHyperX offline Pulsefire Raid runtime RX export.\n# Selected report {} (source line {}); interface {}.\n# Model identity is an input assumption, not established by raw bytes.\n",
        report, record.line,
        record.interface.map_or_else(|| "not supplied".into(), |value| value.to_string())
    );
    for warning in &exported.warnings {
        encoded.push_str(&format!("# Warning: {warning}\n"));
    }
    encoded.push('\n');
    encoded.push_str(&toml::to_string_pretty(&exported.profile)?);
    let mut destination = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(output)
        .with_context(|| {
            format!(
                "cannot create {}; destination must be a new file (existing files are never overwritten)",
                output.display()
            )
        })?;
    destination.write_all(encoded.as_bytes()).with_context(|| {
        format!(
            "failed to write {}; an incomplete output may remain, inspect it before use",
            output.display()
        )
    })?;
    println!(
        "Exported report {} (line {}) into {}.",
        report,
        record.line,
        output.display()
    );
    for warning in exported.warnings {
        println!("Warning: {warning}");
    }
    println!("Offline: no HID device was discovered or opened, no reports were replayed, and no settings were applied or saved onboard. Inspect/validate the partial TOML separately.");
    Ok(())
}

pub(super) fn inspect(path: &Path) -> Result<()> {
    // Inspection is deliberately available even when capability validation
    // fails (unresolved captures/Legacy imports, unimplemented device settings).
    // `profile validate`, not successful inspection, is the readiness gate.
    let profile = load_software_profile_file(path)?;
    println!(
        "Software profile {:?}: device={:?}; partial={}",
        profile.name, profile.device, profile.partial
    );
    if let Some(source) = &profile.source {
        println!(
            "Source provenance: {:?}, format version {} (not an OpenHyperX schema version).",
            source.format, source.format_version
        );
    } else {
        println!("Source provenance: not supplied in TOML fields; capture exports may retain source report/line in comments.");
    }
    if profile.partial {
        println!("Warning: partial profile, not a complete device backup.");
    }
    println!("Omitted settings mean preserve current device state, not reset/off/disabled.");
    println!(
        "Polling: {}",
        profile.polling.map_or_else(
            || "<not present>".into(),
            |value| format!("{} Hz", value.hz)
        )
    );
    println!(
        "Primary buttons: {}",
        profile
            .primary_buttons
            .map_or_else(|| "<not present>".into(), |value| format!("{value:?}"))
    );
    if let Some(dpi) = &profile.dpi {
        println!(
            "DPI: {} stage(s); active_stage={} (zero-based).",
            dpi.stages.len(),
            dpi.active_stage.map_or_else(
                || "<not present; preserve current index>".into(),
                |value| value.to_string()
            )
        );
        if let Some(value) = dpi.source_active_stage {
            println!("  source_active_stage={value}: unconfirmed source indexing, provenance only; not applied.");
        }
        for (index, stage) in dpi.stages.iter().enumerate() {
            println!(
                "  Stage index {index}: X={} Y={} color={}",
                stage.x, stage.y, stage.color
            );
        }
    } else {
        println!("DPI: <not present>");
    }
    println!("Buttons (only supplied bindings):");
    if profile.buttons.is_empty() {
        println!("  <not present>");
    }
    for (control, binding) in &profile.buttons {
        println!("  {control}: {}", binding_label(binding));
    }
    if profile.device == PULSEFIRE_RAID.id {
        let missing = PulsefireRaidControl::ALL
            .into_iter()
            .filter(|control| {
                !matches!(
                    control,
                    PulsefireRaidControl::LeftClick | PulsefireRaidControl::RightClick
                )
            })
            .filter(|control| !profile.buttons.contains_key(control.id()))
            .map(PulsefireRaidControl::id)
            .collect::<Vec<_>>();
        if !missing.is_empty() {
            println!(
                "  Omitted controls: {} (preserve; see unresolved entries below).",
                missing.join(", ")
            );
        }
    }
    if let Some(lighting) = &profile.lighting {
        println!("Lighting: {:?}", lighting.mode);
        for (zone, color) in &lighting.zones {
            println!("  {zone}: {color}");
        }
    } else {
        println!("Lighting: <not present>; current colors/effects are unknown, not off.");
    }
    println!("Macro library: {} definition(s).", profile.macros.len());
    if profile.macros.is_empty() {
        println!(
            "  No timelines supplied; existing device macro events/modes are unknown, not erased."
        );
    }
    for named in &profile.macros {
        println!(
            "  Macro {:?} ({:?}): playback={}, {} event(s)",
            named.source_id,
            named.name,
            named.definition.playback,
            named.definition.events.len()
        );
        let mut elapsed = 0u64;
        for (index, event) in named.definition.events.iter().enumerate() {
            let (action, input, delay) = match event {
                MacroEvent::KeyDown { key, delay_ms } => ("key-down", key, delay_ms),
                MacroEvent::KeyUp { key, delay_ms } => ("key-up", key, delay_ms),
                MacroEvent::MouseButtonDown { button, delay_ms } => {
                    ("mouse-button-down", button, delay_ms)
                }
                MacroEvent::MouseButtonUp { button, delay_ms } => {
                    ("mouse-button-up", button, delay_ms)
                }
            };
            println!(
                "    Event {}: t={elapsed} ms {action} {input:?}; delay after event {delay} ms",
                index + 1
            );
            elapsed += u64::from(*delay);
        }
        println!("    Timeline duration including final delay: {elapsed} ms (not measured physical playback).");
    }
    println!(
        "Unresolved assignments: {}",
        profile.unresolved_button_assignments.len()
    );
    for assignment in &profile.unresolved_button_assignments {
        println!(
            "  {:?}: macro_source_id={} (diagnostic provenance, not an executable binding)",
            assignment.source_id,
            assignment
                .macro_source_id
                .as_ref()
                .map_or_else(|| "<not available>".into(), |id| format!("{id:?}"))
        );
    }
    match PulsefireRaidSoftwareProfile::new(&profile) {
        Ok(validated) => {
            println!("Offline device validation: passed for supplied fields only; not a hardware check or an apply plan.");
            for warning in validated.warnings() { println!("Warning: {warning}"); }
            println!("Composed runtime apply remains experimentally unverified after an empty hardware readback; passing offline validation is not permission to resume hardware testing.");
        }
        Err(error) => println!("Offline device validation: NOT READY: {error}. Inspection still succeeded; use profile validate for a failing exit status."),
    }
    println!("Inspection was offline; no HID device was discovered or opened, no reports were replayed, and no files were changed.");
    Ok(())
}

fn binding_label(binding: &SoftwareButtonBinding) -> String {
    match binding {
        SoftwareButtonBinding::Mouse { action } => format!("Mouse {action:?}"),
        SoftwareButtonBinding::Keyboard { key } => format!("Keyboard {key:?}"),
        SoftwareButtonBinding::Multimedia { action } => format!("Multimedia {action:?}"),
        SoftwareButtonBinding::WindowsShortcut { action } => format!("Windows shortcut {action:?}"),
        SoftwareButtonBinding::Disabled {} => "Disabled".into(),
        SoftwareButtonBinding::Macro { id } => {
            format!("Macro reference {id:?} (resolves within this file, not a device read)")
        }
    }
}
