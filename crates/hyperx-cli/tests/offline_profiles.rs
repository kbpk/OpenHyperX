//! Exercise the real executable without a mouse, including invalid apply input.
use std::{
    fs,
    io::Write,
    path::PathBuf,
    process::{Command, Output},
    sync::atomic::{AtomicUsize, Ordering},
};

fn cli(arguments: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_hyperx-cli"))
        .args(arguments)
        .output()
        .expect("CLI should start")
}

struct ProfileFile(PathBuf);
impl ProfileFile {
    fn new(source: &str) -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "openhyperx-profile-test-{}-{}.toml",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .unwrap()
            .write_all(source.as_bytes())
            .unwrap();
        Self(path)
    }
}
impl Drop for ProfileFile {
    fn drop(&mut self) {
        fs::remove_file(&self.0).unwrap();
    }
}

#[test]
fn example_profile_validates_without_hardware() {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/profiles/pulsefire-raid.toml");
    let output = cli(&["profile", "validate", path.to_str().unwrap()]);
    assert!(output.status.success(), "{output:?}");
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("no HID device was discovered or opened"));
    assert!(stdout.contains("RGB is volatile"));
}

#[test]
fn cold_legacy_startup_fixture_shows_empty_read_before_populated_write_offline() {
    let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../hyperx-protocol/tests/fixtures/cold-legacy-startup-images.hex");
    let output = cli(&["profile", "inspect-capture", fixture.to_str().unwrap()]);
    assert!(output.status.success(), "{output:?}");
    let stdout = String::from_utf8(output.stdout).unwrap();
    for fragment in [
        "2 report(s)",
        "Runtime DeviceReadResponse",
        "WARNING: empty body after the report header",
        "Runtime HostWrite",
        "Polling rate: 1000 Hz",
        "1: 800 DPI, color #2B00FF",
        "Button 5:         Macro reference",
        "no HID device was discovered or opened",
    ] {
        assert!(stdout.contains(fragment), "missing {fragment}: {stdout}");
    }
    assert!(!stdout.contains("Runtime macro for"));
}

#[test]
fn optional_settings_and_binding_families_validate_without_a_format_version() {
    for source in [
        "[polling]\nhz = 125\n",
        "primary_buttons = 'swapped'\n",
        "[buttons.button4]\ntype = 'keyboard'\nkey = 'left-shift'\n",
        "[buttons.button6]\ntype = 'multimedia'\naction = 'volume-up'\n",
        "[buttons.wheel-click]\ntype = 'disabled'\n",
        "[buttons.wheel-tilt-left]\ntype = 'windows-shortcut'\naction = 'copy'\n",
    ] {
        let file = ProfileFile::new(&format!(
            "name = 'Partial'\ndevice = 'pulsefire-raid'\n{source}"
        ));
        let output = cli(&["profile", "validate", file.0.to_str().unwrap()]);
        assert!(output.status.success(), "{source}: {output:?}");
    }
}

#[test]
fn invalid_profiles_fail_before_discovery_in_validate_apply_and_dry_run() {
    let header = "name = 'Invalid'\ndevice = 'pulsefire-raid'\n";
    for (source, message) in [
        ("[polling]\nhz = 2000\n", "polling rate must be one of"),
        ("[buttons.left-click]\ntype = 'disabled'\n", "primary control"),
        ("[buttons.button4]\ntype = 'macro'\nid = 'missing'\n", "missing macro"),
        ("[lighting]\nmode = 'solid'\n[lighting.zones]\nwheel = '#FF0000'\n", "requires exactly wheel and logo"),
        ("[dpi]\nactive_stage = 0\n[[dpi.stages]]\nx = 0\ny = 800\ncolor = '#000000'\n", "outside the supported range"),
        ("[dpi]\nactive_stage = 0\n[[dpi.stages]]\nx = 800\ny = 900\ncolor = '#000000'\n", "independent X/Y"),
        ("unknown_setting = 42\n", "unknown field"),
        ("format_version = 1\n[polling]\nhz = 1000\n", "unknown field"),
        ("[buttons.button4]\ntype = 'disabled'\nkey = 'a'\n", "unknown field"),
        ("[lighting]\nmode = 'rainbow'\n[lighting.zones]\nwheel = '#FF0000'\nlogo = '#000000'\n", "unknown variant"),
        ("[[unresolved_button_assignments]]\nsource_id = 'opaque'\n[polling]\nhz = 1000\n", "unresolved button assignments"),
        ("[polling]\nhz = 1000\n[[macros]]\nid = 'a'\nname = 'A'\nplayback = 'once'\nextra = 1\nevents = []\n", "unknown field"),
        ("[polling]\nhz = 1000\n[[macros]]\nid = 'a'\nname = 'A'\nplayback = 'once'\n[[macros.events]]\ntype = 'key-down'\nkey = 'a'\ndelay_ms = 20\nextra = 1\n", "unknown field"),
    ] {
        let file = ProfileFile::new(&format!("{header}{source}"));
        for command in ["validate", "apply", "dry-run"] {
            let mut args = vec!["profile", if command == "dry-run" { "apply" } else { command }, file.0.to_str().unwrap()];
            if command == "dry-run" { args.push("--dry-run"); }
            let output = cli(&args);
            assert!(!output.status.success(), "{command} {source} accepted");
            let stderr = String::from_utf8(output.stderr).unwrap();
            assert!(stderr.contains(message), "{command} {source}: {stderr}");
            assert!(!stderr.contains("failed to open Pulsefire") && !stderr.contains("no supported"), "{stderr}");
        }
    }
}

#[test]
fn lighting_duration_requires_lighting_before_discovery_and_conflicts_with_dry_run() {
    let file =
        ProfileFile::new("name = 'Polling'\ndevice = 'pulsefire-raid'\n[polling]\nhz = 1000\n");
    let path = file.0.to_str().unwrap();
    let output = cli(&["profile", "apply", path, "--lighting-duration", "5"]);
    assert!(!output.status.success());
    assert!(String::from_utf8(output.stderr)
        .unwrap()
        .contains("requires a [lighting] section"));
    for duration in ["-1", "86401", "not-a-number"] {
        assert!(
            !cli(&["profile", "apply", path, "--lighting-duration", duration])
                .status
                .success()
        );
    }
    let output = cli(&[
        "profile",
        "apply",
        path,
        "--dry-run",
        "--lighting-duration",
        "5",
    ]);
    assert!(!output.status.success());
    assert!(String::from_utf8(output.stderr)
        .unwrap()
        .contains("cannot be used with"));
}

#[test]
fn profile_reader_rejects_oversized_files_before_discovery() {
    let file = ProfileFile::new(&" ".repeat(1024 * 1024 + 1));
    let output = cli(&["profile", "apply", file.0.to_str().unwrap()]);
    assert!(!output.status.success());
    assert!(String::from_utf8(output.stderr)
        .unwrap()
        .contains("1 MiB limit"));
}

#[test]
fn software_diff_reports_settings_macro_timing_and_omissions_without_hardware() {
    let before = ProfileFile::new(include_str!(
        "../../../examples/profiles/pulsefire-raid.toml"
    ));
    let mut after_profile: hyperx_core::SoftwareProfile = toml::from_str(include_str!(
        "../../../examples/profiles/pulsefire-raid.toml"
    ))
    .unwrap();
    after_profile.name = "Changed".into();
    after_profile.polling = None;
    after_profile.dpi.as_mut().unwrap().stages[0].x = 900;
    after_profile.dpi.as_mut().unwrap().stages[0].color = hyperx_core::RgbColor::new(0, 255, 0);
    after_profile.primary_buttons = Some(hyperx_core::PrimaryButtonLayout::Swapped);
    after_profile.buttons.insert(
        "button4".into(),
        hyperx_core::SoftwareButtonBinding::Disabled {},
    );
    after_profile
        .lighting
        .as_mut()
        .unwrap()
        .zones
        .insert("logo".into(), hyperx_core::RgbColor::new(255, 0, 0));
    after_profile.macros[0].definition.playback = hyperx_core::MacroPlayback::ToggleRepeat;
    after_profile.macros[0].definition.events[0] = hyperx_core::MacroEvent::KeyDown {
        key: "a".into(),
        delay_ms: 37,
    };
    let after = ProfileFile::new(&toml::to_string(&after_profile).unwrap());
    let output = cli(&[
        "profile",
        "diff",
        before.0.to_str().unwrap(),
        after.0.to_str().unwrap(),
    ]);
    assert!(output.status.success(), "{output:?}");
    let text = String::from_utf8(output.stdout).unwrap();
    for fragment in [
        "polling.hz: 1000 -> <not present>",
        "dpi.stages[1].x: 800 -> 900",
        "dpi.stages[1].color:",
        "primary_buttons: Standard -> Swapped",
        "buttons[\"button4\"]",
        "lighting.zones[\"logo\"]",
        "macros[\"ab\"].events[1].delay_ms: 20 -> 37",
        "macros[\"ab\"].playback: once -> toggle-repeat",
        "Metadata / unresolved provenance:",
        "not device state or a validated apply plan",
        "no HID device was discovered or opened",
    ] {
        assert!(text.contains(fragment), "missing {fragment}: {text}");
    }
    let identical = cli(&[
        "profile",
        "diff",
        before.0.to_str().unwrap(),
        before.0.to_str().unwrap(),
    ]);
    assert!(identical.status.success());
    assert_eq!(
        String::from_utf8(identical.stdout)
            .unwrap()
            .matches("No differences.")
            .count(),
        2
    );
}

#[test]
fn software_diff_accepts_unresolved_imports_but_rejects_ambiguous_ids_and_bad_toml() {
    let source = "name = 'Legacy'\ndevice = 'pulsefire-raid'\npartial = true\n[[unresolved_button_assignments]]\nsource_id = 'opaque'\nmacro_source_id = 'unknown'\n";
    let before = ProfileFile::new(source);
    let after = ProfileFile::new(&source.replace("opaque", "changed"));
    let output = cli(&[
        "profile",
        "diff",
        before.0.to_str().unwrap(),
        after.0.to_str().unwrap(),
    ]);
    assert!(output.status.success(), "{output:?}");
    assert!(String::from_utf8(output.stdout)
        .unwrap()
        .contains("unresolved_button_assignments[1].source_id"));
    for invalid in [
        "name = 'Bad'\ndevice = 'pulsefire-raid'\nformat_version = 1\n",
        "name = 'Bad'\ndevice = 'pulsefire-raid'\n[[macros]]\nid = 'duplicate'\nname = 'A'\nplayback = 'once'\nevents = []\n[[macros]]\nid = 'duplicate'\nname = 'B'\nplayback = 'once'\nevents = []\n",
    ] {
        let file = ProfileFile::new(invalid);
        assert!(!cli(&["profile", "diff", before.0.to_str().unwrap(), file.0.to_str().unwrap()]).status.success());
    }
    let large = ProfileFile::new(&" ".repeat(1024 * 1024 + 1));
    let error = cli(&[
        "profile",
        "diff",
        before.0.to_str().unwrap(),
        large.0.to_str().unwrap(),
    ]);
    assert!(!error.status.success());
    assert!(String::from_utf8(error.stderr)
        .unwrap()
        .contains("1 MiB limit"));
}

#[test]
fn capture_inspection_decodes_a_complete_read_write_empty_readback_trace_without_replay() {
    use hyperx_protocol::{format_hex, pulsefire_raid::*};
    let mut response = [0; DIRECT_REPORT_LENGTH];
    response[..3].copy_from_slice(&[7, 0x81, 4]);
    response[0x18] = 1;
    response[0x19..0x1B].copy_from_slice(&[0, 16]);
    response[0x25..0x27].copy_from_slice(&[0, 16]);
    response[0x32] = 1;
    let mut profile = PerformanceProfile::parse(&response).unwrap();
    profile.set_primary_button_layout(hyperx_core::PrimaryButtonLayout::Standard);
    response = *profile.as_bytes();
    let mut write = profile.to_write_report();
    write[0x18] = 2;
    let mut empty = [0; DIRECT_REPORT_LENGTH];
    empty[..3].copy_from_slice(&[7, 0x81, 4]);
    let mut log = "\u{feff}TRACE enumerated collection\r\n".to_owned();
    for (direction, bytes) in [
        ("TX", encode_runtime_profile_read_prelude()),
        ("TX", encode_profile_read_request()),
        ("RX", response),
        ("TX", write),
        ("TX", encode_runtime_profile_read_prelude()),
        ("TX", encode_profile_read_request()),
        ("RX", empty),
    ] {
        log.push_str(&format!("\u{1b}[35mTRACE\u{1b}[0m raw HID report direction=\"{direction}\" interface=1 report_id=0x07 bytes={}\r\n", format_hex(&bytes)));
    }
    log.push_str("Error: original write failed\r\n");
    let file = ProfileFile::new(&log);
    let output = cli(&[
        "profile",
        "inspect-capture",
        file.0.to_str().unwrap(),
        "--all-raw",
    ]);
    assert!(output.status.success(), "{output:?}");
    let stdout = String::from_utf8(output.stdout).unwrap();
    for fragment in [
        "7 report(s)",
        "2 ignored",
        "Confirmed Runtime profile selector",
        "1000 Hz",
        "500 Hz",
        "empty body after the report header",
        "0x0018: 0x02 -> 0x00",
        "no reports were replayed",
    ] {
        assert!(stdout.contains(fragment), "missing {fragment}: {stdout}");
    }
    assert_eq!(fs::read_to_string(&file.0).unwrap(), log);
}

#[test]
fn capture_inspection_supports_golden_hex_unknown_packets_and_invalid_input() {
    let file = ProfileFile::new(include_str!(
        "../../hyperx-protocol/tests/fixtures/button4-ab-toggle.hex"
    ));
    let output = cli(&[
        "profile",
        "inspect-capture",
        file.0.to_str().unwrap(),
        "--raw",
    ]);
    assert!(output.status.success(), "{output:?}");
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("playback=toggle-repeat, 4 event(s)"));
    assert!(stdout.contains("delay 20 ms"));
    assert!(stdout.contains("Raw: 07 05 04 03"));
    let unknown = ProfileFile::new("TX 07 FE FF 12\n");
    let output = cli(&[
        "profile",
        "inspect-capture",
        unknown.0.to_str().unwrap(),
        "--raw",
    ]);
    assert!(output.status.success());
    assert!(String::from_utf8(output.stdout)
        .unwrap()
        .contains("Unknown/unhandled packet"));
    for invalid in [
        "# no reports\n",
        "TX 07 ZZ\n",
        "TRACE raw HID report direction=TX interface=1 report_id=0x08 bytes=07 81\n",
    ] {
        let file = ProfileFile::new(invalid);
        assert!(
            !cli(&["profile", "inspect-capture", file.0.to_str().unwrap()])
                .status
                .success()
        );
    }
    let large = ProfileFile::new(&" ".repeat(16 * 1024 * 1024 + 1));
    let output = cli(&["profile", "inspect-capture", large.0.to_str().unwrap()]);
    assert!(!output.status.success());
    assert!(String::from_utf8(output.stderr)
        .unwrap()
        .contains("16 MiB limit"));
}

#[test]
fn selector_free_request_fixture_is_onboard_not_an_assumed_runtime_read() {
    let source = include_str!("../../hyperx-protocol/tests/fixtures/read-request-get-onboard.hex");
    let file = ProfileFile::new(source);
    let output = cli(&[
        "--trace",
        "profile",
        "inspect-capture",
        file.0.to_str().unwrap(),
    ]);
    assert!(output.status.success(), "{output:?}");
    let stdout = String::from_utf8(output.stdout).unwrap();
    for expected in [
        "2 report(s)",
        "no section in this packet",
        "Profile image: Onboard DeviceReadResponse",
        "Known setting fields validate for inspection; opaque bytes, freshness and write safety are not established.",
        "1000 Hz",
        "800 DPI",
        "6400 DPI",
        "definition is not present in this profile image",
        "no HID device was discovered or opened",
        "no reports were replayed",
    ] {
        assert!(stdout.contains(expected), "missing {expected}: {stdout}");
    }
    for wrong in [
        "Profile image: Runtime",
        "Confirmed Runtime profile selector",
        "definition is not present in the runtime profile",
        "enumerated HID collection",
        "raw HID report",
    ] {
        assert!(!stdout.contains(wrong), "unexpected {wrong}: {stdout}");
    }
    assert_eq!(fs::read_to_string(&file.0).unwrap(), source);
}
