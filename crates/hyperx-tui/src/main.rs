use std::{
    io::{self, IsTerminal},
    path::PathBuf,
};

use anyhow::{bail, Result};
use clap::Parser;
use crossterm::{
    event::{
        self, DisableBracketedPaste, DisableMouseCapture, EnableBracketedPaste, EnableMouseCapture,
        Event,
    },
    execute,
};
use hyperx_app::{parse_profile, DraftRecoveryStore, ProfileDocument, RecoveryClient};
use hyperx_core::SoftwareProfile;
use ratatui::{backend::TestBackend, Terminal};

mod app;
mod bindings;
mod color_palette;
mod diagnostics;
mod editor;
mod files;
mod local_recovery;
mod macro_view;
mod macros;
mod profiles;
mod recovery_picker;
mod render;
mod resolution;
mod resolution_view;
#[cfg(test)]
mod tests;
mod widgets;

#[derive(Clone, Copy, clap::ValueEnum)]
enum View {
    Performance,
    Buttons,
    Macros,
    Lighting,
    Profiles,
}
impl View {
    fn index(self) -> usize {
        match self {
            Self::Performance => 0,
            Self::Buttons => 1,
            Self::Macros => 2,
            Self::Lighting => 3,
            Self::Profiles => 4,
        }
    }
}

#[derive(Parser)]
#[command(version, about = "OpenHyperX offline profile editor; never opens HID")]
struct Cli {
    /// Open an existing OpenHyperX TOML profile, not a live device.
    #[arg(conflicts_with_all = ["demo", "recover", "list_recovery", "discard_recovery"])]
    file: Option<PathBuf>,
    /// Load example settings, clearly labeled as demo rather than device state.
    #[arg(long, conflicts_with_all = ["recover", "list_recovery", "discard_recovery"])]
    demo: bool,
    /// Restore exactly this previously listed TUI draft snapshot; no device access.
    #[arg(long, conflicts_with = "list_recovery")]
    recover: Option<PathBuf>,
    /// List TUI crash-recovery snapshots without restoring or deleting them.
    #[arg(long, conflicts_with_all = ["render", "check", "view"])]
    list_recovery: bool,
    /// Permanently remove one explicitly listed TUI snapshot, never a profile or mouse state.
    #[arg(long, conflicts_with_all = ["recover", "list_recovery", "render", "check", "view"])]
    discard_recovery: Option<PathBuf>,
    /// Required confirmation for --discard-recovery.
    #[arg(long, requires = "discard_recovery")]
    confirm_discard_recovery: bool,
    /// Private directory for offline draft snapshots (default: user app-state directory).
    #[arg(long)]
    recovery_dir: Option<PathBuf>,
    /// Render one frame as plain text without requiring an interactive terminal.
    #[arg(long, conflicts_with = "check")]
    render: bool,
    /// Validate the input offline and exit; no terminal initialization.
    #[arg(long)]
    check: bool,
    /// Start on an offline view; also selects the view for --render smoke tests.
    #[arg(long, value_enum, conflicts_with = "check")]
    view: Option<View>,
    #[arg(long, default_value = "100", requires = "render", value_parser = clap::value_parser!(u16).range(20..=300))]
    width: u16,
    #[arg(long, default_value = "30", requires = "render", value_parser = clap::value_parser!(u16).range(8..=100))]
    height: u16,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let store = || -> Result<DraftRecoveryStore> {
        let directory = cli
            .recovery_dir
            .clone()
            .map(Ok)
            .unwrap_or_else(DraftRecoveryStore::default_directory)?;
        DraftRecoveryStore::new(&directory, RecoveryClient::Tui)
    };
    let mut recovery =
        if cli.list_recovery || cli.recover.is_some() || cli.discard_recovery.is_some() {
            Some(store()?)
        } else {
            None
        };
    if cli.list_recovery {
        let recovery = recovery.as_ref().expect("created for listing");
        let paths = recovery.list()?;
        println!("Offline TUI recovery snapshots (no HID access):");
        for path in &paths {
            match recovery.load(path) {
                Ok(document) => println!(
                    "{:?} | profile {:?} | original FILE {:?}",
                    path,
                    document.profile().name,
                    document.path()
                ),
                Err(error) => println!("{:?} | UNREADABLE: {error:#}", path),
            }
        }
        if paths.is_empty() {
            println!("No TUI recovery snapshots.");
        }
        return Ok(());
    }
    if let Some(path) = &cli.discard_recovery {
        if !cli.confirm_discard_recovery {
            bail!("--discard-recovery requires --confirm-discard-recovery; this permanently removes only that offline snapshot");
        }
        recovery
            .as_ref()
            .expect("created for discard")
            .discard(path)?;
        println!(
            "Discarded only TUI recovery snapshot {:?}; no profile or HID device was opened.",
            path
        );
        return Ok(());
    }
    let document = if let Some(path) = &cli.recover {
        recovery
            .as_mut()
            .expect("created for restore")
            .adopt(path)?
    } else if let Some(path) = &cli.file {
        ProfileDocument::open(path)?
    } else if cli.demo {
        ProfileDocument::from_profile(parse_profile(include_str!(
            "../../../examples/profiles/pulsefire-raid.toml"
        ))?)
    } else {
        ProfileDocument::from_profile(SoftwareProfile {
            name: "Untitled offline profile".into(),
            device: "pulsefire-raid".into(),
            partial: true,
            ..SoftwareProfile::default()
        })
    };
    let mut app = app::App::new(document, cli.demo);
    if cli.recover.is_some() {
        app.status =
            "RECOVERED FILE draft; unsaved. Save NEW or confirm FILE overwrite; no HID access."
                .into();
    }
    app.tab = cli.view.map_or(0, View::index);
    if cli.check {
        let result = hyperx_app::validate_profile(app.document.profile());
        println!("Offline profile check; no HID device was discovered or opened.");
        if let Some(error) = result.error {
            bail!("NOT READY: {error}");
        }
        for warning in result.warnings {
            println!("Warning: {warning}");
        }
        println!("Supplied fields pass encoding validation, not hardware verification.");
        return Ok(());
    }
    if cli.render {
        let mut terminal = Terminal::new(TestBackend::new(cli.width, cli.height))?;
        terminal.draw(|frame| app.render(frame))?;
        for row in terminal
            .backend()
            .buffer()
            .content
            .chunks(usize::from(cli.width))
        {
            let text: String = row.iter().map(|cell| cell.symbol()).collect();
            println!("{}", text.trim_end());
        }
        return Ok(());
    }
    if !io::stdin().is_terminal() || !io::stdout().is_terminal() {
        bail!("interactive TUI needs a terminal; run in Windows Terminal or use --demo --render / --check for offline smoke tests");
    }
    let recovery = recovery.map_or_else(store, Ok)?;
    let local_recovery = local_recovery::LocalRecoveryStore::new(recovery.directory())?;
    if cli.recover.is_none() {
        let file_count = recovery.list()?.len();
        let local_count = local_recovery.list()?.len();
        if file_count + local_count > 0 {
            app.status = format!(
                "{file_count} FILE and {local_count} local editor recovery snapshot(s). Profiles → F7 to inspect; no HID access."
            );
        }
    }
    app.recovery = Some(recovery);
    app.local_recovery = Some(local_recovery);
    // Ratatui's run wrapper restores raw mode/alternate screen on success,
    // returned errors and panic. Our additional paste mode has its own guard.
    ratatui::run(|terminal| -> Result<()> {
        struct PasteGuard;
        impl Drop for PasteGuard {
            fn drop(&mut self) {
                let _ = execute!(io::stdout(), DisableMouseCapture, DisableBracketedPaste);
            }
        }
        let _paste = PasteGuard;
        execute!(io::stdout(), EnableBracketedPaste, EnableMouseCapture)?;
        while !app.quit {
            terminal.draw(|frame| app.render(frame))?;
            handle_event(&mut app, event::read()?);
        }
        Ok(())
    })
}

fn handle_event(app: &mut app::App, event: Event) {
    let before_profile = app.document.profile().clone();
    let before_path = app.document.path().map(std::path::Path::to_path_buf);
    let before_dirty = app.document.dirty();
    match event {
        Event::Key(key) => app.handle_key(key),
        Event::Paste(text) => app.handle_paste(&text),
        Event::Mouse(mouse) => app.handle_mouse(mouse),
        Event::Resize(_, _) => app.clear_mouse_layout(),
        _ => {}
    }
    if app.document.profile() != &before_profile
        || app.document.path() != before_path.as_deref()
        || app.document.dirty() != before_dirty
    {
        if let Some(recovery) = &mut app.recovery {
            if let Err(error) = recovery.capture(&app.document) {
                app.status = format!(
                    "RECOVERY SNAPSHOT FAILED: {error:#}. FILE draft is still in memory; save NEW now."
                );
            }
        }
    }
    if let Some(local) = &mut app.local_recovery {
        if let Err(error) = local.capture(&app.document, app.modal.as_ref()) {
            app.status = format!(
                "LOCAL EDITOR RECOVERY FAILED: {error:#}. Edits remain in memory; accept or copy the text before closing."
            );
        }
    }
}

#[cfg(test)]
mod recovery_tests {
    use std::{
        fs,
        sync::atomic::{AtomicUsize, Ordering},
    };

    use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
    use hyperx_app::{parse_profile, DraftRecoveryStore, ProfileDocument, RecoveryClient};

    use super::{app, handle_event};

    #[test]
    fn committed_tui_edits_autosnapshot_and_reverting_to_baseline_clears_it() {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let directory = std::env::temp_dir().join(format!(
            "openhyperx-tui-autosnapshot-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let store = DraftRecoveryStore::new(&directory, RecoveryClient::Tui).unwrap();
        let profile = parse_profile(include_str!(
            "../../../examples/profiles/pulsefire-raid.toml"
        ))
        .unwrap();
        let mut app = app::App::new(ProfileDocument::from_profile(profile), true);
        app.recovery = Some(store);
        handle_event(
            &mut app,
            Event::Key(KeyEvent::new(KeyCode::Right, KeyModifiers::NONE)),
        );
        assert!(app.document.dirty());
        assert_eq!(app.recovery.as_ref().unwrap().list().unwrap().len(), 1);
        handle_event(
            &mut app,
            Event::Key(KeyEvent::new(KeyCode::Left, KeyModifiers::NONE)),
        );
        assert!(!app.document.dirty());
        assert!(app.recovery.as_ref().unwrap().list().unwrap().is_empty());
        fs::remove_dir_all(directory).unwrap();
    }
}
