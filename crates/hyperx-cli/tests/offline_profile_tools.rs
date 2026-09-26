//! New profile tools exercise the actual executable without HID or a mouse.
use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
    sync::atomic::{AtomicUsize, Ordering},
};

use hyperx_core::{PrimaryButtonLayout, SoftwareProfile};
use hyperx_devices::PulsefireRaidSoftwareProfile;
use hyperx_protocol::{format_hex, pulsefire_raid::*};

struct Files {
    directory: PathBuf,
    input: PathBuf,
    output: PathBuf,
}

impl Files {
    fn new(contents: &str) -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let directory = std::env::temp_dir().join(format!(
            "openhyperx-offline-tools-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&directory).unwrap();
        let input = directory.join("input.txt");
        let output = directory.join("output.toml");
        fs::write(&input, contents).unwrap();
        Self {
            directory,
            input,
            output,
        }
    }

    fn export(&self, report: &str) -> Output {
        cli(&[
            "profile",
            "export-capture",
            path(&self.input),
            path(&self.output),
            "--report",
            report,
        ])
    }
}

impl Drop for Files {
    fn drop(&mut self) {
        for file in [&self.input, &self.output] {
            if file.exists() {
                fs::remove_file(file).unwrap();
            }
        }
        fs::remove_dir(&self.directory).unwrap();
    }
}

fn path(path: &Path) -> &str {
    path.to_str().unwrap()
}

fn cli(arguments: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_hyperx-cli"))
        .args(arguments)
        .output()
        .unwrap()
}

fn text(output: Output) -> String {
    assert!(output.status.success(), "{output:?}");
    String::from_utf8(output.stdout).unwrap()
}

fn runtime_read() -> [u8; DIRECT_REPORT_LENGTH] {
    let mut bytes = [0; DIRECT_REPORT_LENGTH];
    bytes[..3].copy_from_slice(&[7, 0x81, 4]);
    bytes[0x18] = 1;
    bytes[0x19..0x1B].copy_from_slice(&[0, 16]); // 800 X
    bytes[0x25..0x27].copy_from_slice(&[0, 16]); // 800 Y
    bytes[0x32] = 1;
    bytes[0x69..0x6C].copy_from_slice(&[0x2B, 0, 0xFF]);
    let mut profile = PerformanceProfile::parse(&bytes).unwrap();
    profile.set_primary_button_layout(PrimaryButtonLayout::Standard);
    *profile.as_bytes()
}

fn trace(direction: &str, interface: i32, bytes: &[u8]) -> String {
    format!("TRACE raw HID report direction=\"{direction}\" interface={interface} report_id=0x{:02X} bytes={}\n", bytes[0], format_hex(bytes))
}

#[test]
fn export_selects_one_based_rx_report_and_round_trips_without_hardware() {
    let read = runtime_read();
    let mut write = read;
    write[1] = 1;
    write[0x18] = 2;
    let mut empty = [0; DIRECT_REPORT_LENGTH];
    empty[..3].copy_from_slice(&[7, 0x81, 4]);
    let log = format!(
        "# synthetic test capture\n{}{}{}{}",
        trace("TX", 1, &encode_runtime_profile_read_prelude()),
        trace("RX", 1, &read),
        trace("TX", 1, &write),
        trace("RX", 1, &empty)
    );
    let files = Files::new(&log);
    let stdout = text(files.export("2"));
    assert!(stdout.contains("Exported report 2 (line 3)"), "{stdout}");
    assert!(stdout.contains("no HID device was discovered or opened"));
    let encoded = fs::read_to_string(&files.output).unwrap();
    assert!(encoded.contains("# Selected report 2 (source line 3); interface 1"));
    assert!(!encoded.contains(path(&files.input))); // Do not leak private capture paths.
    assert!(!encoded.contains("format_version"));
    let profile: SoftwareProfile = toml::from_str(&encoded).unwrap();
    assert!(profile.partial);
    assert_eq!(profile.polling.unwrap().hz, 1000); // Never last TX or empty RX.
    assert_eq!(profile.dpi.as_ref().unwrap().stages[0].x, 800);
    assert_eq!(
        profile.dpi.as_ref().unwrap().stages[0].color.to_string(),
        "#2B00FF"
    );
    assert_eq!(profile.primary_buttons, Some(PrimaryButtonLayout::Standard));
    assert_eq!(profile.buttons.len(), 9);
    assert!(profile.lighting.is_none() && profile.macros.is_empty());
    PulsefireRaidSoftwareProfile::new(&profile).unwrap();
    let inspection = text(cli(&["profile", "inspect", path(&files.output)]));
    for expected in [
        "partial=true",
        "1000 Hz",
        "X=800 Y=800",
        "current colors/effects are unknown, not off",
        "passed for supplied fields only",
        "no HID device was discovered or opened",
    ] {
        assert!(inspection.contains(expected), "{expected}: {inspection}");
    }
    assert_eq!(fs::read_to_string(&files.input).unwrap(), log);
}

#[test]
fn export_does_not_infer_macro_timelines_or_lighting_from_adjacent_reports() {
    let mut read = runtime_read();
    read[0x88..0x8C].copy_from_slice(&[0x53, 0, 0, 3]);
    read[0x8C..0x90].copy_from_slice(&[0x53, 0, 0, 4]);
    let log = format!(
        "{}RX {}\nTX {}\n",
        include_str!("../../hyperx-protocol/tests/fixtures/button4-ab-toggle.hex"),
        format_hex(&read),
        format_hex(&encode_direct_rgb(
            hyperx_core::RgbColor::new(255, 0, 0),
            hyperx_core::RgbColor::new(0, 0, 255)
        ))
    );
    let files = Files::new(&log);
    // Golden macro TX and ACK are reports 1/2, the snapshot is report 3.
    text(files.export("3"));
    let profile: SoftwareProfile =
        toml::from_str(&fs::read_to_string(&files.output).unwrap()).unwrap();
    assert!(profile.macros.is_empty() && profile.lighting.is_none());
    assert!(!profile.buttons.contains_key("button4"));
    assert!(!profile.buttons.contains_key("button5"));
    assert_eq!(
        profile
            .unresolved_button_assignments
            .iter()
            .map(|entry| entry.source_id.as_str())
            .collect::<Vec<_>>(),
        ["runtime:button4", "runtime:button5"]
    );
    let inspected = text(cli(&["profile", "inspect", path(&files.output)]));
    assert!(inspected.contains("Unresolved assignments: 2"));
    assert!(inspected.contains("NOT READY"));
    assert!(inspected.contains("No timelines supplied"));
    for arguments in [
        vec!["profile", "validate", path(&files.output)],
        vec!["profile", "apply", path(&files.output), "--dry-run"],
    ] {
        let result = cli(&arguments);
        assert!(!result.status.success());
        let stderr = String::from_utf8(result.stderr).unwrap();
        assert!(stderr.contains("unresolved button assignments"), "{stderr}");
        assert!(!stderr.contains("no supported") && !stderr.contains("failed to open Pulsefire"));
    }
}

#[test]
fn export_rejects_invalid_selection_or_snapshot_before_creating_any_output() {
    let read = runtime_read();
    let mut empty = [0; DIRECT_REPORT_LENGTH];
    empty[..3].copy_from_slice(&[7, 0x81, 4]);
    let mut onboard = read;
    onboard[2] = 1;
    let mut write = read;
    write[1] = 1;
    let mut malformed = read;
    malformed[0x18] = 0;
    for source in [
        format!("{}{}", trace("TX", 1, &write), trace("RX", 1, &read)),
        format!("{}{}", trace("RX", 1, &empty), trace("RX", 1, &read)),
        format!("{}{}", trace("RX", 1, &onboard), trace("RX", 1, &read)),
        format!("{}{}", trace("RX", 0, &read), trace("RX", 1, &read)),
        format!("{}{}", trace("RX", 1, &malformed), trace("RX", 1, &read)),
        format!("{}\nRX {}\n", format_hex(&read), format_hex(&read)),
        "RX 07 81 04\n".into(),
        "RX 07 ZZ\n".into(),
        "# no reports\n".into(),
        " ".repeat(16 * 1024 * 1024 + 1),
    ] {
        let files = Files::new(&source);
        assert!(
            !files.export("1").status.success(),
            "accepted {source:.100}"
        );
        assert!(!files.output.exists());
        assert_eq!(fs::read_to_string(&files.input).unwrap(), source);
    }
    let files = Files::new(&format!("RX {}\n", format_hex(&read)));
    for number in ["0", "-1", "abc", "2", "184467440737095516160"] {
        assert!(!files.export(number).status.success());
        assert!(!files.output.exists());
    }
    let missing = cli(&[
        "profile",
        "export-capture",
        path(&files.input),
        path(&files.output),
    ]);
    assert!(!missing.status.success());
    assert!(String::from_utf8(missing.stderr)
        .unwrap()
        .contains("--report"));
    assert!(!files.output.exists());
}

#[test]
fn export_never_overwrites_destination_or_capture_even_when_they_are_the_same() {
    let log = format!("RX {}\n", format_hex(&runtime_read()));
    let files = Files::new(&log);
    fs::write(&files.output, "important existing profile").unwrap();
    let failed = files.export("1");
    assert!(!failed.status.success());
    assert!(String::from_utf8(failed.stderr)
        .unwrap()
        .contains("never overwritten"));
    assert_eq!(
        fs::read_to_string(&files.output).unwrap(),
        "important existing profile"
    );
    let failed = cli(&[
        "profile",
        "export-capture",
        path(&files.input),
        path(&files.input),
        "--report",
        "1",
    ]);
    assert!(!failed.status.success());
    assert_eq!(fs::read_to_string(&files.input).unwrap(), log);
}

#[test]
fn inspection_lists_chords_mouse_events_nonuniform_timings_and_omissions() {
    let files = Files::new("name = 'Timeline'\ndevice = 'pulsefire-raid'\n[buttons.button4]\ntype = 'macro'\nid = 'chord'\n[[macros]]\nid = 'chord'\nname = 'Shift+A and click'\nplayback = 'once'\n[[macros.events]]\ntype = 'key-down'\nkey = 'left-shift'\ndelay_ms = 0\n[[macros.events]]\ntype = 'key-down'\nkey = 'a'\ndelay_ms = 17\n[[macros.events]]\ntype = 'key-up'\nkey = 'a'\ndelay_ms = 5\n[[macros.events]]\ntype = 'key-up'\nkey = 'left-shift'\ndelay_ms = 31\n[[macros.events]]\ntype = 'mouse-button-down'\nbutton = 'left'\ndelay_ms = 41\n[[macros.events]]\ntype = 'mouse-button-up'\nbutton = 'left'\ndelay_ms = 3\n");
    let stdout = text(cli(&["profile", "inspect", path(&files.input)]));
    for expected in [
        "Polling: <not present>",
        "Primary buttons: <not present>",
        "DPI: <not present>",
        "Omitted controls:",
        "Macro reference \"chord\"",
        "playback=once, 6 event(s)",
        "t=0 ms key-down \"left-shift\"; delay after event 0 ms",
        "t=0 ms key-down \"a\"; delay after event 17 ms",
        "t=17 ms key-up \"a\"",
        "t=22 ms key-up \"left-shift\"",
        "t=53 ms mouse-button-down \"left\"",
        "t=94 ms mouse-button-up \"left\"",
        "including final delay: 97 ms",
        "no files were changed",
    ] {
        assert!(stdout.contains(expected), "{expected}: {stdout}");
    }
    assert!(!files.output.exists());
}

#[test]
fn inspection_distinguishes_parsing_success_from_device_support_without_hid() {
    for source in [
        "name = 'Future'\ndevice = 'future-device'\n[polling]\nhz = 125\n",
        "name = 'Bad polling'\ndevice = 'pulsefire-raid'\n[polling]\nhz = 2000\n",
        "name = 'Legacy'\ndevice = 'pulsefire-raid'\npartial = true\n[source]\nformat = 'ngenuity-legacy-hxp'\nformat_version = 40\n[dpi]\nsource_active_stage = 1\nstages = []\n[[unresolved_button_assignments]]\nsource_id = 'opaque'\nmacro_source_id = 'missing'\n",
        "name = 'No fields'\ndevice = 'pulsefire-raid'\n",
        "name = 'Duplicate IDs'\ndevice = 'pulsefire-raid'\n[[macros]]\nid = 'a'\nname = 'First'\nplayback = 'once'\nevents = []\n[[macros]]\nid = 'a'\nname = 'Second'\nplayback = 'once'\nevents = []\n",
    ] {
        let files = Files::new(source);
        let stdout = text(cli(&["profile", "inspect", path(&files.input)]));
        assert!(stdout.contains("NOT READY"), "{stdout}");
        assert!(stdout.contains("no HID device was discovered or opened"));
        assert!(!cli(&["profile", "validate", path(&files.input)]).status.success());
        assert_eq!(fs::read_to_string(&files.input).unwrap(), source);
    }
    for source in [
        "name = 'Broken'\nunknown = 1\n",
        "format_version = 1\n",
        "garbage",
        &" ".repeat(1024 * 1024 + 1),
    ] {
        let files = Files::new(source);
        assert!(!cli(&["profile", "inspect", path(&files.input)])
            .status
            .success());
    }
}

#[test]
fn explicit_macro_resolution_imports_a_timeline_into_a_new_offline_file() {
    let source = "name = 'Captured'\ndevice = 'pulsefire-raid'\npartial = true\n[polling]\nhz = 1000\n[[unresolved_button_assignments]]\nsource_id = 'runtime:button5'\n";
    let files = Files::new(source);
    let macro_file =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples/macros/ab-20ms.toml");
    let args = [
        "profile",
        "resolve-macro",
        path(&files.input),
        path(&files.output),
        "--source-id",
        "runtime:button5",
        "--control",
        "button5",
        "--macro-id",
        "ab",
        "--macro-file",
        path(&macro_file),
    ];
    let output = text(cli(&args));
    assert!(output.contains("no HID device was discovered or opened"));
    let profile: SoftwareProfile =
        toml::from_str(&fs::read_to_string(&files.output).unwrap()).unwrap();
    assert!(profile.unresolved_button_assignments.is_empty());
    assert_eq!(profile.macros[0].definition.events.len(), 4);
    assert_eq!(
        profile.buttons["button5"],
        hyperx_core::SoftwareButtonBinding::Macro { id: "ab".into() }
    );
    assert_eq!(fs::read_to_string(&files.input).unwrap(), source);
    assert!(cli(&["profile", "validate", path(&files.output)])
        .status
        .success());
    assert!(!cli(&args).status.success()); // Existing output never overwritten.
}

#[test]
fn resolution_rejects_unsupported_targets_and_omission_is_explicit_and_offline() {
    let source = "name = 'Captured'\ndevice = 'pulsefire-raid'\npartial = true\n[polling]\nhz = 1000\n[[unresolved_button_assignments]]\nsource_id = 'runtime:button5'\n";
    let files = Files::new(source);
    let macro_file =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples/macros/ab-toggle-20ms.toml");
    for target in ["button4", "button5", "dpi", "unknown"] {
        let output = cli(&[
            "profile",
            "resolve-macro",
            path(&files.input),
            path(&files.output),
            "--source-id",
            "runtime:button5",
            "--control",
            target,
            "--macro-id",
            "repeat",
            "--macro-file",
            path(&macro_file),
        ]);
        assert!(!output.status.success());
        assert!(!files.output.exists());
        let message = String::from_utf8(output.stderr).unwrap();
        assert!(!message.contains("no supported") && !message.contains("failed to open Pulsefire"));
    }
    let output = text(cli(&[
        "profile",
        "omit-unresolved",
        path(&files.input),
        path(&files.output),
        "--source-id",
        "runtime:button5",
    ]));
    assert!(output.contains("no HID device was discovered or opened"));
    let profile: SoftwareProfile =
        toml::from_str(&fs::read_to_string(&files.output).unwrap()).unwrap();
    assert!(profile.unresolved_button_assignments.is_empty());
    assert!(profile.buttons.is_empty() && profile.macros.is_empty());
    assert_eq!(fs::read_to_string(&files.input).unwrap(), source);
}
