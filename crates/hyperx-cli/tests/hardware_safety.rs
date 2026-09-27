//! The actual executable must refuse hazardous runtime access even on a host
//! with a real mouse attached. These cases intentionally use valid input so
//! rejection is the safety gate, not a parser failure or absent-device error.
use std::{path::PathBuf, process::Command};

#[test]
fn every_runtime_command_stops_before_discovery_and_raw_reports() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let profile = root.join("examples/profiles/pulsefire-raid.toml");
    let macro_file = root.join("examples/macros/ab-20ms.toml");
    let profile = profile.to_str().unwrap();
    let macro_file = macro_file.to_str().unwrap();
    let commands = [
        vec!["dpi", "get"],
        vec!["dpi", "set", "800"],
        vec!["dpi", "active", "1"],
        vec!["dpi", "stage", "set", "1", "--dpi", "900"],
        vec!["dpi", "stage", "add", "6400", "FF0000"],
        vec!["dpi", "stage", "remove-last"],
        vec!["polling", "get"],
        vec!["polling", "set", "500"],
        vec!["buttons", "list"],
        vec!["buttons", "primary-layout", "swapped"],
        vec!["buttons", "set", "button4", "disabled"],
        vec!["buttons", "set", "button4", "mouse", "back"],
        vec!["buttons", "set", "button4", "multimedia", "volume-up"],
        vec!["buttons", "set", "button4", "windows-shortcut", "copy"],
        vec!["buttons", "set", "button4", "keyboard", "a"],
        vec!["buttons", "set", "button4", "macro", macro_file],
        vec!["profile", "apply", profile],
        vec!["profile", "apply", profile, "--dry-run"],
        vec!["profile", "check-save-ack"],
        vec!["profile", "check-save-ack", "--initialize-session"],
        vec![
            "profile",
            "save-to-mouse",
            "--wheel",
            "off",
            "--logo",
            "0000FF",
            "--confirm",
        ],
        vec![
            "profile",
            "save-to-mouse",
            "--wheel",
            "off",
            "--logo",
            "0000FF",
            "--macro-definition",
            macro_file,
            "--confirm",
        ],
    ];
    for args in commands {
        let output = Command::new(env!("CARGO_BIN_EXE_hyperx-cli"))
            .arg("--trace")
            .args(&args)
            .output()
            .unwrap();
        assert!(!output.status.success(), "allowed {args:?}");
        let stdout = String::from_utf8(output.stdout).unwrap();
        let stderr = String::from_utf8(output.stderr).unwrap();
        assert!(
            stderr.contains("temporarily blocked for device safety"),
            "{args:?}: {stderr}"
        );
        assert!(
            stderr.contains("No HID enumeration or opening was attempted"),
            "{args:?}: {stderr}"
        );
        for text in [stdout, stderr] {
            assert!(
                !text.contains("enumerated HID collection"),
                "{args:?}: {text}"
            );
            assert!(!text.contains("raw HID report"), "{args:?}: {text}");
        }
    }
}

#[test]
fn info_help_describes_descriptor_only_access_without_vendor_reports() {
    let output = Command::new(env!("CARGO_BIN_EXE_hyperx-cli"))
        .args(["info", "--help"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("send no vendor reports"), "{stdout}");
    assert!(
        !stdout.contains("read the current runtime profile"),
        "{stdout}"
    );
}

#[test]
fn lab_get_requires_consent_and_has_no_arbitrary_report_or_send_arguments() {
    for args in [
        vec!["lab", "raid-feature-get"],
        vec!["lab", "raid-feature-get", "--unsafe", "--report-id", "4"],
        vec!["lab", "raid-feature-get", "--unsafe", "--send", "07030464"],
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_hyperx-cli"))
            .arg("--trace")
            .args(&args)
            .output()
            .unwrap();
        assert!(!output.status.success());
        let stderr = String::from_utf8(output.stderr).unwrap();
        assert!(
            stderr.contains("required arguments") || stderr.contains("unexpected argument"),
            "{args:?}: {stderr}"
        );
        assert!(!stderr.contains("enumerated HID collection"));
        assert!(!stderr.contains("raw HID report"));
    }
    let output = Command::new(env!("CARGO_BIN_EXE_hyperx-cli"))
        .args(["lab", "raid-feature-get", "--help"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("without any preceding SET_REPORT"));
    assert!(stdout.contains("--unsafe"));
}

#[test]
fn lab_request_get_rejects_missing_consent_and_arbitrary_payload_selector_or_delay() {
    for extra in [
        vec![],
        vec!["--unsafe", "--send", "07030464"],
        vec!["--unsafe", "--section", "runtime"],
        vec!["--unsafe", "--delay", "0"],
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_hyperx-cli"))
            .args(["--trace", "lab", "raid-read-request-get"])
            .args(extra)
            .output()
            .unwrap();
        assert!(!output.status.success());
        let stderr = String::from_utf8(output.stderr).unwrap();
        assert!(
            stderr.contains("required arguments") || stderr.contains("unexpected argument"),
            "{stderr}"
        );
        assert!(!stderr.contains("enumerated HID collection"));
        assert!(!stderr.contains("raw HID report"));
    }
    let output = Command::new(env!("CARGO_BIN_EXE_hyperx-cli"))
        .args(["lab", "raid-read-request-get", "--help"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let help = String::from_utf8(output.stdout).unwrap();
    assert!(help.contains("no profile selector"));
    assert!(help.contains("--unsafe"));
}

#[test]
fn lab_selector_is_a_fixed_explicitly_disruptive_probe_not_a_runtime_override() {
    for extra in [
        vec![],
        vec!["--unsafe", "--send", "07030464"],
        vec!["--unsafe", "--section", "onboard"],
        vec!["--unsafe", "--get"],
        vec!["--unsafe", "--initialize-session"],
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_hyperx-cli"))
            .args(["--trace", "lab", "raid-runtime-select-only"])
            .args(extra)
            .output()
            .unwrap();
        assert!(!output.status.success());
        let stderr = String::from_utf8(output.stderr).unwrap();
        assert!(
            stderr.contains("required arguments") || stderr.contains("unexpected argument"),
            "{stderr}"
        );
        assert!(!stderr.contains("enumerated HID collection"));
        assert!(!stderr.contains("raw HID report"));
    }
    let output = Command::new(env!("CARGO_BIN_EXE_hyperx-cli"))
        .args(["lab", "raid-runtime-select-only", "--help"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let help = String::from_utf8(output.stdout).unwrap();
    assert!(help.contains("MAY disable cursor/clicks/lighting"));
    assert!(help.contains("No request or GET"));
    assert!(help.contains("--unsafe"));
}
