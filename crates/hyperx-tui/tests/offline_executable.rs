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
