//! Explicit, offline comparison of two captured Raid profile images.
//! Selection is by report number, never by a guessed transaction or USB state.

use std::{num::NonZeroUsize, path::Path};

use anyhow::{anyhow, bail, Context, Result};
use hyperx_protocol::{
    capture::{diff_captures, CaptureRecord},
    pulsefire_raid::{
        inspect_captured_report, PerformanceProfile, PulsefireRaidControl, RaidReportInspection,
    },
};

pub(super) fn diff_images(
    before_path: &Path,
    after_path: &Path,
    before_number: NonZeroUsize,
    after_number: NonZeroUsize,
    all_raw: bool,
) -> Result<()> {
    let (before, before_record) = select_image(before_path, before_number)?;
    let (after, after_record) = select_image(after_path, after_number)?;
    print_source(
        "Before",
        before_path,
        before_number,
        &before_record,
        &before,
    );
    print_source("After", after_path, after_number, &after_record, &after);

    let raw_diff = diff_captures(
        std::slice::from_ref(&before_record.report),
        std::slice::from_ref(&after_record.report),
    );
    let changes = raw_diff
        .first()
        .map_or(&[][..], |diff| diff.changes.as_slice());
    let body_count = changes.iter().filter(|change| change.offset >= 3).count();
    println!(
        "Raw byte differences: {} total; {} profile-body byte(s) at offsets 0x0003+.",
        changes.len(),
        body_count
    );
    let limit = if all_raw { changes.len() } else { 32 };
    for change in changes.iter().take(limit) {
        println!(
            "  0x{:04X}: {} -> {}{}",
            change.offset,
            change
                .before
                .map_or_else(|| "--".into(), |byte| format!("{byte:02X}")),
            change
                .after
                .map_or_else(|| "--".into(), |byte| format!("{byte:02X}")),
            if change.offset < 3 {
                " (report envelope)"
            } else {
                ""
            }
        );
    }
    if changes.len() > limit {
        println!(
            "  {} more changed byte(s); use --all-raw.",
            changes.len() - limit
        );
    }

    match (known_fields(&before), known_fields(&after)) {
        (Ok(left), Ok(right)) => {
            let mut changed = 0;
            for ((name, old), (other_name, new)) in left.iter().zip(&right) {
                debug_assert_eq!(name, other_name);
                if old != new {
                    println!("Decoded {name}: {old} -> {new}");
                    changed += 1;
                }
            }
            if changed == 0 {
                println!("Decoded known settings: equal (polling, DPI, primary layout and 11 binding/reference slots).");
            } else {
                println!("Decoded known settings: {changed} field(s) differ.");
            }
        }
        (left, right) => {
            println!(
                "Decoded known settings: comparison unavailable (before: {}; after: {}).",
                left.err()
                    .map_or_else(|| "decoded".into(), |error| error.to_string()),
                right
                    .err()
                    .map_or_else(|| "decoded".into(), |error| error.to_string())
            );
        }
    }
    println!("Offline comparison only. Report numbers, direction and section do not prove the same physical device, causal transaction, valid write baseline or safe USB behavior. Unknown body bytes are never synthesized, replayed or discarded.");
    Ok(())
}

fn select_image(path: &Path, number: NonZeroUsize) -> Result<(PerformanceProfile, CaptureRecord)> {
    let log = super::load_report_log(path)?;
    let record = log.records.get(number.get() - 1).ok_or_else(|| {
        anyhow!(
            "report {} is outside {}'s {} report(s); use profile inspect-capture first",
            number,
            path.display(),
            log.records.len()
        )
    })?;
    match inspect_captured_report(record) {
        RaidReportInspection::Profile(profile) => Ok((*profile, record.clone())),
        RaidReportInspection::InvalidKnownReport(reason) => bail!(
            "report {} in {} is an invalid Raid packet: {reason}",
            number,
            path.display()
        ),
        _ => bail!(
            "report {} in {} is not a complete recognized Raid profile image",
            number,
            path.display()
        ),
    }
}

fn print_source(
    label: &str,
    path: &Path,
    number: NonZeroUsize,
    record: &CaptureRecord,
    profile: &PerformanceProfile,
) {
    println!(
        "{label}: {} report {} (line {}, direction {}, iface {}): {:?} {:?}",
        path.display(),
        number,
        record.line,
        record
            .report
            .direction
            .map_or_else(|| "unknown".into(), |direction| direction.to_string()),
        record
            .interface
            .map_or_else(|| "unknown".into(), |interface| interface.to_string()),
        profile.section(),
        profile.kind()
    );
}

fn known_fields(profile: &PerformanceProfile) -> Result<Vec<(String, String)>> {
    if profile.as_bytes()[3..].iter().all(|byte| *byte == 0) {
        bail!("empty profile body");
    }
    // These are decoded, independently capture-backed fields, not an encoder
    // authorization or evidence that any remaining byte is safe to overwrite.
    let mut fields = vec![
        (
            "polling".into(),
            format!("{} Hz", profile.polling_rate()?.hz()),
        ),
        ("DPI".into(), format!("{:?}", profile.dpi_profile()?)),
        (
            "primary layout".into(),
            format!("{:?}", profile.primary_button_layout()?),
        ),
    ];
    for control in PulsefireRaidControl::ALL {
        let binding = if profile.has_confirmed_macro_reference(control) {
            "macro reference (timeline not in image)".to_owned()
        } else {
            format!(
                "{:?}",
                profile
                    .button_binding(control)
                    .with_context(|| format!("{} binding", control.name()))?
            )
        };
        fields.push((control.name().to_owned(), binding));
    }
    Ok(fields)
}
