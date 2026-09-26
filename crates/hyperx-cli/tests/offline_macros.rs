//! Executable smoke tests: macro support and validation must work without USB.
use std::{
    path::PathBuf,
    process::{Command, Output},
};

fn cli(arguments: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_hyperx-cli"))
        .args(arguments)
        .output()
        .expect("CLI executable should start")
}

fn macro_file(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/macros")
        .join(name)
}

#[test]
fn capabilities_are_available_without_hardware() {
    let output = cli(&["buttons", "capabilities"]);
    assert!(output.status.success(), "{output:?}");
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout
        .contains("Button 4: runtime [once, toggle-repeat, repeat-while-held]; onboard [once]"));
    assert!(stdout.contains("Button 5: runtime [once]; onboard [once]"));
    assert!(stdout.contains("DPI button: macro encoding not implemented"));
    assert!(stdout.contains("max 14 events, delay 0..9999 ms"));
}

#[test]
fn validates_supported_runtime_and_onboard_macros_without_hardware() {
    for (control, file, onboard) in [
        ("button4", "ab-20ms.toml", false),
        ("button5", "ab-20ms.toml", false),
        ("button4", "ab-20ms.toml", true),
        ("button5", "coverage-recorded-timing.toml", true),
        ("button4", "ab-toggle-20ms.toml", false),
        ("button4", "ab-hold-20ms.toml", false),
    ] {
        let path = macro_file(file);
        let mut args = vec!["buttons", "validate-macro", control, path.to_str().unwrap()];
        if onboard {
            args.push("--onboard");
        }
        let output = cli(&args);
        assert!(
            output.status.success(),
            "{control} {file} {onboard}: {output:?}"
        );
        assert!(String::from_utf8(output.stdout)
            .unwrap()
            .contains("no HID device was discovered or opened"));
    }
}

#[test]
fn rejects_unconfirmed_targets_and_persistence_before_discovery() {
    for (control, file, onboard, error) in [
        (
            "dpi",
            "ab-20ms.toml",
            false,
            "has no confirmed Pulsefire Raid encoding",
        ),
        (
            "wheel-click",
            "ab-20ms.toml",
            true,
            "has no confirmed Pulsefire Raid encoding",
        ),
        (
            "button5",
            "ab-toggle-20ms.toml",
            false,
            "is not captured for Button5",
        ),
        (
            "button4",
            "ab-toggle-20ms.toml",
            true,
            "onboard macro playback",
        ),
        (
            "button4",
            "ab-hold-20ms.toml",
            true,
            "onboard macro playback",
        ),
    ] {
        let path = macro_file(file);
        let mut args = vec!["buttons", "validate-macro", control, path.to_str().unwrap()];
        if onboard {
            args.push("--onboard");
        }
        let output = cli(&args);
        assert!(
            !output.status.success(),
            "{control} {file} unexpectedly accepted"
        );
        let stderr = String::from_utf8(output.stderr).unwrap();
        assert!(stderr.contains(error), "{stderr}");
        assert!(!stderr.contains("failed to open"), "{stderr}");
        assert!(!stderr.contains("no supported"), "{stderr}");
    }
}
