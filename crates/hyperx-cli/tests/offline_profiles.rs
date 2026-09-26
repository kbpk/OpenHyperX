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
        ("[[unresolved_button_assignments]]\nsource_id = 'opaque'\n[polling]\nhz = 1000\n", "unresolved Legacy"),
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
