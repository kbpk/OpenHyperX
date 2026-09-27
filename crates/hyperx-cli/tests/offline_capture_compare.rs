//! The executable compares explicitly chosen evidence without opening HID.

use std::{path::PathBuf, process::Command};

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../hyperx-protocol/tests/fixtures")
        .join(name)
}

#[test]
fn compares_cross_section_images_without_inventing_opaque_field_semantics() {
    let onboard = fixture("read-request-get-onboard.hex");
    let cold = fixture("cold-legacy-startup-images.hex");
    let output = Command::new(env!("CARGO_BIN_EXE_hyperx-cli"))
        .args([
            "profile",
            "diff-capture-images",
            onboard.to_str().unwrap(),
            cold.to_str().unwrap(),
            "--before-report",
            "2",
            "--after-report",
            "2",
        ])
        .output()
        .unwrap();
    assert!(output.status.success(), "{output:?}");
    let stdout = String::from_utf8(output.stdout).unwrap();
    for fragment in [
        "Onboard DeviceReadResponse",
        "Runtime HostWrite",
        "26 total; 24 profile-body byte(s)",
        "0x0001: 81 -> 01 (report envelope)",
        "0x0024:",
        "Decoded known settings: equal",
        "Offline comparison only",
    ] {
        assert!(stdout.contains(fragment), "missing {fragment}: {stdout}");
    }
    assert!(!stdout.contains("macro event"));
}

#[test]
fn empty_cold_runtime_is_not_presented_as_a_valid_known_settings_baseline() {
    let cold = fixture("cold-legacy-startup-images.hex");
    let output = Command::new(env!("CARGO_BIN_EXE_hyperx-cli"))
        .args([
            "profile",
            "diff-capture-images",
            cold.to_str().unwrap(),
            cold.to_str().unwrap(),
            "--before-report",
            "1",
            "--after-report",
            "2",
        ])
        .output()
        .unwrap();
    assert!(output.status.success(), "{output:?}");
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(
        stdout.contains("comparison unavailable (before: empty profile body; after: decoded)"),
        "{stdout}"
    );
    assert!(!stdout.contains("Decoded known settings: equal"));
}

#[test]
fn report_selection_rejects_nonprofiles_and_out_of_range_numbers() {
    let onboard = fixture("read-request-get-onboard.hex");
    for (before_number, message) in [
        ("1", "not a complete recognized Raid profile image"),
        ("3", "outside"),
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_hyperx-cli"))
            .args([
                "--trace",
                "profile",
                "diff-capture-images",
                onboard.to_str().unwrap(),
                onboard.to_str().unwrap(),
                "--before-report",
                before_number,
                "--after-report",
                "2",
            ])
            .output()
            .unwrap();
        assert!(!output.status.success());
        let stderr = String::from_utf8(output.stderr).unwrap();
        assert!(stderr.contains(message), "{stderr}");
        assert!(!stderr.contains("enumerated HID collection"));
        assert!(!stderr.contains("raw HID report"));
    }
}
