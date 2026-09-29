use hyperx_app::{parse_profile, DraftRecoveryStore, ProfileDocument, RecoveryClient};
use std::{
    fs,
    path::PathBuf,
    process::{Command, Output},
    sync::atomic::{AtomicUsize, Ordering},
};

fn cli(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_hyperx-tui"))
        .args(args)
        .output()
        .unwrap()
}

#[test]
fn demo_renders_without_terminal_or_hardware_and_is_labeled_as_demo() {
    let output = cli(&["--demo", "--render"]);
    assert!(output.status.success(), "{output:?}");
    let text = String::from_utf8(output.stdout).unwrap();
    for expected in [
        "OpenHyperX",
        "OFFLINE",
        "DEMO DATA",
        "Performance",
        "Buttons",
        "Macros",
        "Lighting",
        "Profiles",
        "X=800 Y=800",
        "no HID access",
        "Save to mouse is unavailable",
    ] {
        assert!(text.contains(expected), "{expected}: {text}");
    }
    for dimensions in [("20", "8"), ("45", "12"), ("160", "50")] {
        assert!(cli(&[
            "--demo",
            "--render",
            "--width",
            dimensions.0,
            "--height",
            dimensions.1
        ])
        .status
        .success());
    }
}

#[test]
fn every_offline_view_renders_from_the_actual_executable_without_a_terminal_or_mouse() {
    for (view, expected) in [
        ("performance", "X=800 Y=800"),
        ("buttons", "wheel-tilt-right"),
        ("macros", "File timeline including last delay: 80 ms"),
        ("lighting", "logo: #0000FF"),
        ("profiles", "Save to mouse: UNAVAILABLE OFFLINE"),
    ] {
        let output = cli(&[
            "--demo", "--render", "--view", view, "--width", "120", "--height", "40",
        ]);
        assert!(output.status.success(), "{view}: {output:?}");
        let text = String::from_utf8(output.stdout).unwrap();
        assert!(
            text.contains(expected) && text.contains("OFFLINE"),
            "{view}: {text}"
        );
        if view == "profiles" {
            for label in ["Browse F2", "Name F3", "Copy NEW F4", "Unresolved F5"] {
                assert!(text.contains(label), "{label}: {text}");
            }
        }
    }
    assert!(!cli(&["--demo", "--render", "--view", "firmware"])
        .status
        .success());
    assert!(!cli(&["--demo", "--check", "--view", "macros"])
        .status
        .success());
}

#[test]
fn unresolved_example_is_visible_but_not_ready_and_headless_inspection_never_changes_it() {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/profiles/pulsefire-raid-unresolved.toml");
    let before = fs::read(&path).unwrap();
    let output = cli(&[
        path.to_str().unwrap(),
        "--render",
        "--view",
        "profiles",
        "--width",
        "120",
        "--height",
        "40",
    ]);
    assert!(output.status.success(), "{output:?}");
    let text = String::from_utf8(output.stdout).unwrap();
    for label in [
        "synthetic",
        "NOT READY",
        "runtime:button5",
        "opaque-legacy-example",
        "Unresolved F5",
        "no HID access",
    ] {
        assert!(text.contains(label), "{label}: {text}");
    }
    assert!(!cli(&[path.to_str().unwrap(), "--check"]).status.success());
    assert_eq!(fs::read(path).unwrap(), before);
}

#[test]
fn offline_checks_accept_supported_examples_but_do_not_open_interactive_terminal() {
    assert!(cli(&["--demo", "--check"]).status.success());
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/profiles/pulsefire-raid.toml");
    let output = cli(&[path.to_str().unwrap(), "--check"]);
    assert!(output.status.success(), "{output:?}");
    assert!(String::from_utf8(output.stdout)
        .unwrap()
        .contains("no HID device was discovered or opened"));
    let empty = cli(&["--check"]);
    assert!(!empty.status.success());
    assert!(String::from_utf8(empty.stderr)
        .unwrap()
        .contains("NOT READY"));
    let no_terminal = cli(&["--demo"]);
    assert!(!no_terminal.status.success());
    assert!(String::from_utf8(no_terminal.stderr)
        .unwrap()
        .contains("interactive TUI needs a terminal"));
}

#[test]
fn unsupported_file_is_inspectable_in_render_but_fails_check_without_modification() {
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    let path = std::env::temp_dir().join(format!(
        "openhyperx-tui-input-{}-{}.toml",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    let contents = "name = 'Unsupported'\ndevice = 'pulsefire-raid'\n[polling]\nhz = 2000\n";
    fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&path)
        .unwrap();
    fs::write(&path, contents).unwrap();
    let render = cli(&[path.to_str().unwrap(), "--render"]);
    assert!(render.status.success(), "{render:?}");
    let text = String::from_utf8(render.stdout).unwrap();
    assert!(text.contains("FILE DRAFT") && text.contains("2000 Hz") && text.contains("NOT READY"));
    assert!(!cli(&[path.to_str().unwrap(), "--check"]).status.success());
    assert_eq!(fs::read_to_string(&path).unwrap(), contents);
    fs::remove_file(path).unwrap();
}

#[test]
fn arguments_reject_invalid_sizes_conflicts_and_missing_files() {
    for args in [
        vec!["--demo", "--render", "--check"],
        vec!["--render", "--width", "0"],
        vec!["--render", "--height", "101"],
        vec!["--width", "100"],
        vec!["missing-openhyperx-profile.toml", "--render"],
        vec!["missing-openhyperx-profile.toml", "--demo", "--render"],
    ] {
        assert!(!cli(&args).status.success(), "{args:?}");
    }
}

#[test]
fn actual_executable_lists_and_renders_an_explicit_recovered_file_draft() {
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    let root = std::env::temp_dir().join(format!(
        "openhyperx-tui-recovery-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    let directory = root.join("private");
    let mut store = DraftRecoveryStore::new(&directory, RecoveryClient::Tui).unwrap();
    let profile = parse_profile(include_str!(
        "../../../examples/profiles/pulsefire-raid.toml"
    ))
    .unwrap();
    let mut document = ProfileDocument::from_profile(profile);
    let mut edited = document.profile().clone();
    edited.name = "Recovered after restart".into();
    document.replace(edited);
    let snapshot = store.capture(&document).unwrap().unwrap();
    let listed = cli(&[
        "--list-recovery",
        "--recovery-dir",
        directory.to_str().unwrap(),
    ]);
    assert!(listed.status.success(), "{listed:?}");
    let listing = String::from_utf8(listed.stdout).unwrap();
    assert!(listing.contains("Recovered after restart") && listing.contains("no HID access"));
    let rendered = cli(&[
        "--recover",
        snapshot.to_str().unwrap(),
        "--recovery-dir",
        directory.to_str().unwrap(),
        "--render",
        "--view",
        "profiles",
    ]);
    assert!(rendered.status.success(), "{rendered:?}");
    let text = String::from_utf8(rendered.stdout).unwrap();
    for label in [
        "RECOVERED FILE draft",
        "UNSAVED",
        "Recovered after restart",
        "OFFLINE",
    ] {
        assert!(text.contains(label), "{label}: {text}");
    }
    assert!(
        snapshot.exists(),
        "read-only rendering must retain the snapshot"
    );
    assert!(!cli(&[
        "--recover",
        root.join("unrelated.toml").to_str().unwrap(),
        "--recovery-dir",
        directory.to_str().unwrap(),
        "--render",
    ])
    .status
    .success());
    assert!(!cli(&[
        "--discard-recovery",
        snapshot.to_str().unwrap(),
        "--recovery-dir",
        directory.to_str().unwrap(),
    ])
    .status
    .success());
    assert!(snapshot.exists());
    let discarded = cli(&[
        "--discard-recovery",
        snapshot.to_str().unwrap(),
        "--confirm-discard-recovery",
        "--recovery-dir",
        directory.to_str().unwrap(),
    ]);
    assert!(discarded.status.success(), "{discarded:?}");
    assert!(!snapshot.exists());
    fs::remove_dir_all(root).unwrap();
}
