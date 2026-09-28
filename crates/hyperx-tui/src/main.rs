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
use hyperx_app::{parse_profile, ProfileDocument};
use hyperx_core::SoftwareProfile;
use ratatui::{backend::TestBackend, Terminal};

mod app;
mod bindings;
mod color_palette;
mod diagnostics;
mod editor;
mod files;
mod macro_view;
mod macros;
mod profiles;
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
    #[arg(conflicts_with = "demo")]
    file: Option<PathBuf>,
    /// Load example settings, clearly labeled as demo rather than device state.
    #[arg(long)]
    demo: bool,
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
    let document = if let Some(path) = &cli.file {
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
            match event::read()? {
                Event::Key(key) => app.handle_key(key),
                Event::Paste(text) => app.handle_paste(&text),
                Event::Mouse(mouse) => app.handle_mouse(mouse),
                Event::Resize(_, _) => app.clear_mouse_layout(),
                _ => {}
            }
        }
        Ok(())
    })
}
