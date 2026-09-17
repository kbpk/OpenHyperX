use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
    thread,
    time::{Duration, Instant},
};

use anyhow::{anyhow, Context, Result};
use clap::{ArgAction, Parser, Subcommand, ValueEnum};
use hyperx_core::{
    ButtonBinding, DeviceDescriptor, DpiProfile, HidInterfaceInfo, MacroDefinition, MacroEvent,
    MacroPlayback, MouseFunction, MultimediaFunction, PollingRate, RgbColor, UsbId,
    WindowsShortcut,
};
use hyperx_devices::{
    find_supported_device, PulsefireRaid, PulsefireRaidButton5Assignment, PULSEFIRE_RAID,
    PULSEFIRE_RAID_DPI,
};
use hyperx_hid::{HidApiDiscovery, HidApiTransport, HidDiscovery, HidTransport};
use hyperx_protocol::{
    capture::{diff_captures, parse_hex_capture},
    format_hex,
    hid_descriptor::parse_report_layouts,
    pulsefire_raid::{PerformanceProfile, PulsefireRaidControl},
};
use tracing_subscriber::EnvFilter;

#[derive(Debug, Parser)]
#[command(
    name = "hyperx-cli",
    version,
    about = "Experimental HyperX mouse control"
)]
struct Cli {
    /// Increase diagnostic verbosity (-v for info, -vv for debug).
    #[arg(short, long, action = ArgAction::Count, global = true)]
    verbose: u8,

    /// Log every enumerated collection and, in future commands, raw HID reports.
    #[arg(long, global = true)]
    trace: bool,

    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Enumerate supported HyperX mice and their HID collections.
    Devices {
        /// Also print HID collections belonging to unsupported devices.
        #[arg(long)]
        all: bool,
    },
    /// Show the HID descriptor and read the current runtime profile.
    Info {
        /// Print the complete raw HID report descriptor in hexadecimal.
        #[arg(long)]
        descriptor: bool,
    },
    /// Apply volatile direct RGB. No setting is written to onboard memory.
    Rgb {
        #[command(subcommand)]
        command: RgbCommand,
    },
    /// Read or change the active runtime DPI stage.
    Dpi {
        #[command(subcommand)]
        command: DpiCommand,
    },
    /// Read or change the runtime USB polling rate.
    Polling {
        #[command(subcommand)]
        command: PollingCommand,
    },
    /// Read button mappings or apply an exact capture-backed runtime mapping.
    Buttons {
        #[command(subcommand)]
        command: ButtonsCommand,
    },
    /// Compare two text files containing one raw hexadecimal report per line.
    DecodeCapture {
        /// Baseline capture exported as hex lines.
        before: PathBuf,
        /// Capture after one isolated setting change.
        after: PathBuf,
    },
}

#[derive(Debug, Subcommand)]
enum RgbCommand {
    /// Set both wheel and logo to one static direct color.
    Static {
        /// Six hexadecimal RGB digits, with optional leading '#'.
        color: RgbColor,
        /// Keep refreshing in the foreground for this many seconds (max 3600).
        #[arg(long, default_value_t = 0)]
        duration: u64,
    },
    /// Set both LEDs to black in volatile direct mode.
    Off {
        /// Keep refreshing in the foreground for this many seconds (max 3600).
        #[arg(long, default_value_t = 0)]
        duration: u64,
    },
}

#[derive(Debug, Subcommand)]
enum DpiCommand {
    /// Show all DPI stages and the active stage.
    Get,
    /// Change both axes of the active stage; does not save onboard.
    Set {
        /// DPI from 200 through 16000 in steps of 50.
        #[arg(value_parser = parse_dpi)]
        dpi: u32,
    },
    /// Change one DPI stage, append/remove the last stage, or select a stage.
    Stage {
        #[command(subcommand)]
        command: DpiStageCommand,
    },
    /// Select an existing DPI stage without changing its value or color.
    Active {
        /// One-based stage number from 1 through 5.
        #[arg(value_parser = parse_dpi_stage)]
        stage: usize,
    },
}

#[derive(Debug, Subcommand)]
enum DpiStageCommand {
    /// Change an existing runtime stage; omitted fields are preserved.
    #[command(group(
        clap::ArgGroup::new("change")
            .required(true)
            .multiple(true)
            .args(["dpi", "color", "active"])
    ))]
    Set {
        /// One-based stage number from 1 through 5.
        #[arg(value_parser = parse_dpi_stage)]
        stage: usize,
        /// DPI from 200 through 16000 in steps of 50.
        #[arg(long, value_parser = parse_dpi)]
        dpi: Option<u32>,
        /// Six hexadecimal RGB digits, with optional leading '#'.
        #[arg(long)]
        color: Option<RgbColor>,
        /// Also make this the active stage.
        #[arg(long)]
        active: bool,
    },
    /// Append one runtime stage; does not save onboard.
    Add {
        /// DPI from 200 through 16000 in steps of 50.
        #[arg(value_parser = parse_dpi)]
        dpi: u32,
        /// Six hexadecimal RGB digits, with optional leading '#'.
        color: RgbColor,
        /// Also make the new stage active.
        #[arg(long)]
        active: bool,
    },
    /// Remove and clear the last runtime stage; at least one is retained.
    RemoveLast,
}

#[derive(Debug, Subcommand)]
enum PollingCommand {
    /// Show the current runtime polling rate.
    Get,
    /// Change the runtime polling rate; does not save onboard.
    Set {
        /// Polling rate in Hz: 125, 250, 500 or 1000.
        rate: PollingRate,
    },
}

#[derive(Debug, Subcommand)]
enum ButtonsCommand {
    /// Show all 11 runtime button mappings.
    List,
    /// Change one runtime mapping; currently limited to captured Button 5 cases.
    Set {
        /// Physical control to update.
        control: ButtonControlArg,
        /// Assignment category and its parameters.
        #[command(subcommand)]
        assignment: ButtonAssignmentCommand,
    },
}

#[derive(Clone, Copy, Debug, ValueEnum)]
enum ButtonControlArg {
    Button5,
}

#[derive(Debug, Subcommand)]
enum ButtonAssignmentCommand {
    Disabled,
    Mouse {
        #[arg(value_enum)]
        function: ButtonMouseFunctionArg,
    },
    Multimedia {
        #[arg(value_enum)]
        function: ButtonMultimediaFunctionArg,
    },
    WindowsShortcut {
        #[arg(value_enum)]
        shortcut: ButtonWindowsShortcutArg,
    },
    Keyboard {
        #[arg(value_enum)]
        key: ButtonKeyboardKeyArg,
    },
    /// Assign a macro timeline from a TOML file.
    Macro {
        /// TOML file containing playback and an ordered event timeline.
        file: PathBuf,
    },
}

#[derive(Clone, Copy, Debug, ValueEnum)]
enum ButtonMouseFunctionArg {
    Forward,
    Back,
    DpiToggle,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
enum ButtonMultimediaFunctionArg {
    VolumeUp,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
enum ButtonWindowsShortcutArg {
    Copy,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
enum ButtonKeyboardKeyArg {
    A,
}

impl ButtonAssignmentCommand {
    fn into_assignment(self) -> Result<(PulsefireRaidButton5Assignment, String)> {
        let (assignment, description) = match self {
            Self::Disabled => (PulsefireRaidButton5Assignment::Disabled, "disabled"),
            Self::Mouse { function } => match function {
                ButtonMouseFunctionArg::Forward => {
                    (PulsefireRaidButton5Assignment::Forward, "mouse forward")
                }
                ButtonMouseFunctionArg::Back => {
                    (PulsefireRaidButton5Assignment::Back, "mouse back")
                }
                ButtonMouseFunctionArg::DpiToggle => (
                    PulsefireRaidButton5Assignment::DpiToggle,
                    "mouse DPI toggle",
                ),
            },
            Self::Multimedia {
                function: ButtonMultimediaFunctionArg::VolumeUp,
            } => (
                PulsefireRaidButton5Assignment::VolumeUp,
                "multimedia volume-up",
            ),
            Self::WindowsShortcut {
                shortcut: ButtonWindowsShortcutArg::Copy,
            } => (
                PulsefireRaidButton5Assignment::Copy,
                "Windows shortcut copy",
            ),
            Self::Keyboard {
                key: ButtonKeyboardKeyArg::A,
            } => (PulsefireRaidButton5Assignment::KeyboardA, "keyboard A"),
            Self::Macro { file } => return load_button5_macro(&file),
        };
        Ok((assignment, description.to_owned()))
    }
}

fn load_button5_macro(path: &Path) -> Result<(PulsefireRaidButton5Assignment, String)> {
    let source = fs::read_to_string(path)
        .with_context(|| format!("failed to read macro file {}", path.display()))?;
    let definition: MacroDefinition = toml::from_str(&source)
        .with_context(|| format!("failed to parse macro file {}", path.display()))?;
    let assignment = confirmed_button5_macro_assignment(&definition)?;
    Ok((assignment, format!("macro from {}", path.display())))
}

fn confirmed_button5_macro_assignment(
    definition: &MacroDefinition,
) -> Result<PulsefireRaidButton5Assignment> {
    if definition.playback != MacroPlayback::Once {
        return Err(anyhow!(
            "macro playback {:?} is not capture-backed; only once is currently supported",
            definition.playback
        ));
    }

    let events = definition.events.as_slice();
    if matches_key_sequence(events, &[(true, "a", 20), (false, "a", 20)]) {
        return Ok(PulsefireRaidButton5Assignment::MacroA20Ms);
    }
    if matches_key_sequence(events, &[(true, "a", 300), (false, "a", 300)]) {
        return Ok(PulsefireRaidButton5Assignment::MacroA300Ms);
    }
    if matches_key_sequence(
        events,
        &[
            (true, "a", 20),
            (false, "a", 20),
            (true, "b", 20),
            (false, "b", 20),
        ],
    ) {
        return Ok(PulsefireRaidButton5Assignment::MacroAb20Ms);
    }

    Err(anyhow!(
        "macro event timeline is valid but not capture-backed for Pulsefire Raid; currently supported timelines are A/20 ms, A/300 ms and A then B/20 ms"
    ))
}

fn matches_key_sequence(events: &[MacroEvent], expected: &[(bool, &str, u16)]) -> bool {
    events.len() == expected.len()
        && events
            .iter()
            .zip(expected)
            .all(
                |(event, &(pressed, expected_key, expected_delay))| match event {
                    MacroEvent::KeyDown { key, delay_ms } if pressed => {
                        key.eq_ignore_ascii_case(expected_key) && *delay_ms == expected_delay
                    }
                    MacroEvent::KeyUp { key, delay_ms } if !pressed => {
                        key.eq_ignore_ascii_case(expected_key) && *delay_ms == expected_delay
                    }
                    _ => false,
                },
            )
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    init_logging(cli.verbose, cli.trace)?;

    match cli.command {
        Command::Devices { all } => devices(&HidApiDiscovery, all),
        Command::Info { descriptor } => info(&HidApiDiscovery, descriptor),
        Command::Rgb { command } => match command {
            RgbCommand::Static { color, duration } => rgb(color, duration),
            RgbCommand::Off { duration } => rgb(RgbColor::BLACK, duration),
        },
        Command::Dpi { command } => dpi(command),
        Command::Polling { command } => polling(command),
        Command::Buttons { command } => buttons(command),
        Command::DecodeCapture { before, after } => decode_capture(&before, &after),
    }
}

fn parse_dpi(input: &str) -> Result<u32, String> {
    let dpi = input
        .parse::<u32>()
        .map_err(|_| format!("DPI must be an integer; got {input}"))?;
    PULSEFIRE_RAID_DPI
        .validate(dpi)
        .map_err(|error| error.to_string())
}

fn parse_dpi_stage(input: &str) -> Result<usize, String> {
    let stage = input
        .parse::<usize>()
        .map_err(|_| format!("DPI stage must be an integer; got {input}"))?;
    if (1..=usize::from(PULSEFIRE_RAID_DPI.max_stages)).contains(&stage) {
        Ok(stage)
    } else {
        Err(format!(
            "DPI stage must be between 1 and {}; got {stage}",
            PULSEFIRE_RAID_DPI.max_stages
        ))
    }
}

fn init_logging(verbosity: u8, trace: bool) -> Result<()> {
    let default_directive = if trace {
        "hyperx_cli=debug,hyperx_hid=trace,hyperx_protocol=trace"
    } else {
        match verbosity {
            0 => "warn",
            1 => "hyperx_cli=info,hyperx_hid=info",
            _ => "hyperx_cli=debug,hyperx_hid=debug",
        }
    };

    let filter = EnvFilter::try_from_default_env()
        .or_else(|_| EnvFilter::try_new(default_directive))
        .context("invalid logging filter")?;

    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(false)
        .without_time()
        .try_init()
        .map_err(|error| anyhow!("failed to initialize logging: {error}"))?;

    Ok(())
}

fn devices(discovery: &dyn HidDiscovery, show_all: bool) -> Result<()> {
    let interfaces = discovery.enumerate()?;
    tracing::debug!(collections = interfaces.len(), "HID enumeration completed");

    let mut supported: BTreeMap<(u16, u16), (&DeviceDescriptor, Vec<&HidInterfaceInfo>)> =
        BTreeMap::new();
    let mut unsupported = Vec::new();

    for interface in &interfaces {
        if let Some(descriptor) = find_supported_device(interface.usb_id()) {
            supported
                .entry((interface.vendor_id, interface.product_id))
                .or_insert_with(|| (descriptor, Vec::new()))
                .1
                .push(interface);
        } else if show_all {
            unsupported.push(interface);
        }
    }

    if supported.is_empty() {
        println!("No supported HyperX mice found.");
    }

    for (_, (descriptor, mut interfaces)) in supported {
        interfaces.sort_by_key(|info| (info.interface_number, info.usage_page, info.usage));
        print_supported_device(descriptor, &interfaces);
    }

    if show_all {
        print_unsupported(&unsupported);
    }

    Ok(())
}

fn info(discovery: &dyn HidDiscovery, show_descriptor: bool) -> Result<()> {
    let interfaces = discovery.enumerate()?;
    let mut model_interfaces: Vec<_> = interfaces
        .iter()
        .filter(|info| info.usb_id() == PULSEFIRE_RAID.usb_id)
        .collect();
    if model_interfaces.is_empty() {
        return Err(anyhow!("HyperX Pulsefire Raid was not found"));
    }
    model_interfaces.sort_by_key(|info| (info.interface_number, info.usage_page, info.usage));
    print_supported_device(&PULSEFIRE_RAID, &model_interfaces);

    let configuration = select_configuration_interface(&interfaces)?;
    let transport = HidApiTransport::open(configuration).with_context(|| {
        format!(
            "failed to open the Pulsefire Raid configuration collection: {}",
            configuration.path
        )
    })?;
    let descriptor = transport
        .report_descriptor()
        .context("failed to read the HID report descriptor")?;
    let layouts = parse_report_layouts(&descriptor).context("invalid HID report descriptor")?;

    println!("Configuration collection:");
    println!("  Interface: {}", configuration.interface_number);
    println!("  Usage Page: 0x{:04X}", configuration.usage_page);
    println!("  Usage: 0x{:04X}", configuration.usage);
    println!("  Report descriptor: {} bytes", descriptor.len());
    println!("Reports:");
    for layout in layouts {
        println!(
            "  ID 0x{:02X}: input={} B, output={} B, feature={} B (hidapi feature buffer={} B)",
            layout.report_id,
            layout.input_bytes(),
            layout.output_bytes(),
            layout.feature_bytes(),
            layout.hidapi_feature_bytes(),
        );
    }
    if show_descriptor {
        println!("Raw descriptor:");
        println!("  {}", format_hex(&descriptor));
    }

    let mut device = PulsefireRaid::new(transport)?;
    let profile = device
        .runtime_profile()
        .context("failed to read the Pulsefire Raid runtime profile; close NGENUITY and retry")?;
    print_runtime_profile(&profile);

    Ok(())
}

fn print_runtime_profile(profile: &PerformanceProfile) {
    println!("Runtime profile:");
    match profile.polling_rate() {
        Ok(polling_rate) => println!("  Polling rate: {} Hz", polling_rate.hz()),
        Err(error) => println!("  Polling rate: <decode error: {error}>"),
    }

    println!("  DPI stages:");
    match profile.dpi_profile() {
        Ok(dpi_profile) => print_dpi_profile(&dpi_profile),
        Err(error) => println!("    <decode error: {error}>"),
    }

    println!("  Buttons:");
    print_button_profile(profile, "    ");
}

fn print_button_profile(profile: &PerformanceProfile, indent: &str) {
    for control in PulsefireRaidControl::ALL {
        if profile.has_confirmed_macro_reference(control) {
            println!(
                "{indent}{:<17} Macro reference (definition is not present in the runtime profile)",
                format!("{}:", control.name())
            );
            continue;
        }
        match profile.button_binding(control) {
            Ok(binding) => println!(
                "{indent}{:<17} {}",
                format!("{}:", control.name()),
                format_button_binding(&binding)
            ),
            Err(error) => println!(
                "{indent}{:<17} <decode error: {error}>",
                format!("{}:", control.name())
            ),
        }
    }
}

fn print_dpi_profile(dpi_profile: &DpiProfile) {
    for (index, stage) in dpi_profile.stages.iter().enumerate() {
        let active = if index == dpi_profile.active_stage {
            "*"
        } else {
            " "
        };
        let resolution = if stage.x == stage.y {
            format!("{} DPI", stage.x)
        } else {
            format!("{}x{} DPI", stage.x, stage.y)
        };
        println!(
            "    {active} {}: {resolution}, color #{:02X}{:02X}{:02X}",
            index + 1,
            stage.color.red,
            stage.color.green,
            stage.color.blue,
        );
    }
}

fn format_button_binding(binding: &ButtonBinding) -> String {
    match binding {
        ButtonBinding::Mouse(function) => format!("Mouse: {}", mouse_function_name(*function)),
        ButtonBinding::Keyboard(usage) => format!("Keyboard HID usage 0x{:04X}", usage.0),
        ButtonBinding::Multimedia(function) => {
            format!("Multimedia: {}", multimedia_function_name(*function))
        }
        ButtonBinding::Macro(macro_binding) => {
            format!("Macro: {} ({:?})", macro_binding.id, macro_binding.playback)
        }
        ButtonBinding::WindowsShortcut(shortcut) => {
            format!("Windows shortcut: {}", windows_shortcut_name(*shortcut))
        }
        ButtonBinding::Disabled => "Disabled".to_owned(),
    }
}

const fn mouse_function_name(function: MouseFunction) -> &'static str {
    match function {
        MouseFunction::LeftClick => "left click",
        MouseFunction::RightClick => "right click",
        MouseFunction::MiddleClick => "middle click",
        MouseFunction::Back => "back",
        MouseFunction::Forward => "forward",
        MouseFunction::TiltLeft => "tilt left",
        MouseFunction::TiltRight => "tilt right",
        MouseFunction::DpiToggle => "DPI toggle",
        MouseFunction::ScrollUp => "scroll up",
        MouseFunction::ScrollDown => "scroll down",
    }
}

const fn multimedia_function_name(function: MultimediaFunction) -> &'static str {
    match function {
        MultimediaFunction::PlayPause => "play/pause",
        MultimediaFunction::Stop => "stop",
        MultimediaFunction::NextTrack => "next track",
        MultimediaFunction::PreviousTrack => "previous track",
        MultimediaFunction::MuteVolume => "mute volume",
        MultimediaFunction::VolumeUp => "volume up",
        MultimediaFunction::VolumeDown => "volume down",
    }
}

const fn windows_shortcut_name(shortcut: WindowsShortcut) -> &'static str {
    match shortcut {
        WindowsShortcut::CycleApps => "cycle apps",
        WindowsShortcut::SwitchApps => "switch apps",
        WindowsShortcut::Cut => "cut",
        WindowsShortcut::Copy => "copy",
        WindowsShortcut::Paste => "paste",
        WindowsShortcut::Undo => "undo",
    }
}

fn dpi(command: DpiCommand) -> Result<()> {
    let mut device = open_pulsefire_raid()?;
    match command {
        DpiCommand::Get => {
            let profile = device.runtime_profile().context(
                "failed to read the Pulsefire Raid runtime profile; close NGENUITY and retry",
            )?;
            let dpi_profile = profile.dpi_profile()?;
            println!("DPI stages:");
            print_dpi_profile(&dpi_profile);
        }
        DpiCommand::Set { dpi } => {
            let dpi_profile = device.set_runtime_active_dpi(dpi).context(
                "failed to update the active runtime DPI stage; the setting may be unchanged",
            )?;
            println!(
                "Updated active runtime DPI stage {} to {dpi} DPI.",
                dpi_profile.active_stage + 1
            );
            println!("DPI stages:");
            print_dpi_profile(&dpi_profile);
            println!("The onboard profile was not written.");
        }
        DpiCommand::Stage { command } => {
            let (message, dpi_profile) = match command {
                DpiStageCommand::Set {
                    stage,
                    dpi,
                    color,
                    active,
                } => {
                    let profile = device
                        .set_runtime_dpi_stage(stage - 1, dpi, color, active)
                        .with_context(|| {
                            format!(
                                "failed to update runtime DPI stage {stage}; the setting may be unchanged"
                            )
                        })?;
                    (format!("Updated runtime DPI stage {stage}."), profile)
                }
                DpiStageCommand::Add { dpi, color, active } => {
                    let profile = device.add_runtime_dpi_stage(dpi, color, active).context(
                        "failed to append a runtime DPI stage; the setting may be unchanged",
                    )?;
                    let stage = profile.stages.len();
                    (format!("Added runtime DPI stage {stage}."), profile)
                }
                DpiStageCommand::RemoveLast => {
                    let profile = device.remove_runtime_last_dpi_stage().context(
                        "failed to remove the last runtime DPI stage; the setting may be unchanged",
                    )?;
                    ("Removed the last runtime DPI stage.".to_owned(), profile)
                }
            };
            println!("{message}");
            println!("DPI stages:");
            print_dpi_profile(&dpi_profile);
            println!("The onboard profile was not written.");
        }
        DpiCommand::Active { stage } => {
            let dpi_profile = device
                .set_runtime_active_dpi_stage(stage - 1)
                .with_context(|| {
                    format!(
                        "failed to select runtime DPI stage {stage}; the setting may be unchanged"
                    )
                })?;
            println!("Selected runtime DPI stage {stage}.");
            println!("DPI stages:");
            print_dpi_profile(&dpi_profile);
            println!("The onboard profile was not written.");
        }
    }
    Ok(())
}

fn polling(command: PollingCommand) -> Result<()> {
    let mut device = open_pulsefire_raid()?;
    match command {
        PollingCommand::Get => {
            let profile = device.runtime_profile().context(
                "failed to read the Pulsefire Raid runtime profile; close NGENUITY and retry",
            )?;
            println!("Polling rate: {} Hz", profile.polling_rate()?.hz());
        }
        PollingCommand::Set { rate } => {
            device.set_runtime_polling_rate(rate).context(
                "failed to update the runtime polling rate; the setting may be unchanged",
            )?;
            println!("Updated runtime polling rate to {} Hz.", rate.hz());
            println!("The onboard profile was not written.");
        }
    }
    Ok(())
}

fn buttons(command: ButtonsCommand) -> Result<()> {
    match command {
        ButtonsCommand::List => {
            let mut device = open_pulsefire_raid()?;
            let profile = device.runtime_profile().context(
                "failed to read the Pulsefire Raid runtime profile; close NGENUITY and retry",
            )?;
            println!("Button mappings:");
            print_button_profile(&profile, "  ");
        }
        ButtonsCommand::Set {
            control: ButtonControlArg::Button5,
            assignment,
        } => {
            let (assignment, description) = assignment.into_assignment()?;
            let mut device = open_pulsefire_raid()?;
            device.set_runtime_button5_assignment(assignment).context(
                "failed to update Button 5's runtime mapping; the setting may be unchanged",
            )?;
            println!("Updated Button 5 runtime mapping to {description}.");
            println!("The onboard profile was not written.");
        }
    }
    Ok(())
}

fn open_pulsefire_raid() -> Result<PulsefireRaid<HidApiTransport>> {
    let discovery = HidApiDiscovery;
    let interfaces = discovery.enumerate()?;
    let configuration = select_configuration_interface(&interfaces)?;
    let transport = HidApiTransport::open(configuration).with_context(|| {
        format!(
            "failed to open the Pulsefire Raid configuration collection; close NGENUITY and retry: {}",
            configuration.path
        )
    })?;
    Ok(PulsefireRaid::new(transport)?)
}

fn rgb(color: RgbColor, duration_seconds: u64) -> Result<()> {
    if duration_seconds > 3600 {
        return Err(anyhow!("--duration cannot exceed 3600 seconds"));
    }

    let mut device = open_pulsefire_raid()?;
    let deadline = Instant::now() + Duration::from_secs(duration_seconds);

    loop {
        device.set_volatile_direct_rgb(color, color)?;
        if duration_seconds == 0 || Instant::now() >= deadline {
            break;
        }
        thread::sleep(Duration::from_millis(750));
    }

    println!(
        "Applied volatile direct RGB #{:02X}{:02X}{:02X} to wheel and logo.",
        color.red, color.green, color.blue
    );
    if duration_seconds == 0 {
        println!("The mouse may return to its stored effect after about one second.");
    } else {
        println!(
            "Foreground keepalive ended after {duration_seconds} seconds; the stored effect may now return."
        );
    }
    println!("No onboard profile or firmware data was written.");
    Ok(())
}

fn decode_capture(before_path: &Path, after_path: &Path) -> Result<()> {
    let before_text = fs::read_to_string(before_path)
        .with_context(|| format!("failed to read {}", before_path.display()))?;
    let after_text = fs::read_to_string(after_path)
        .with_context(|| format!("failed to read {}", after_path.display()))?;
    let before = parse_hex_capture(&before_text)
        .with_context(|| format!("failed to parse {}", before_path.display()))?;
    let after = parse_hex_capture(&after_text)
        .with_context(|| format!("failed to parse {}", after_path.display()))?;
    let diffs = diff_captures(&before, &after);

    println!(
        "Compared {} baseline report(s) with {} changed report(s).",
        before.len(),
        after.len()
    );
    if diffs.is_empty() {
        println!("No differences found.");
        return Ok(());
    }

    for diff in diffs {
        println!(
            "Report {}: {} {} B -> {} {} B",
            diff.index + 1,
            format_direction(diff.before_direction),
            diff.before_len,
            format_direction(diff.after_direction),
            diff.after_len,
        );
        if diff.changes.is_empty() {
            println!("  direction changed; payload is identical");
        } else {
            for change in diff.changes {
                println!(
                    "  offset 0x{:04X}: {} -> {}",
                    change.offset,
                    format_optional_byte(change.before),
                    format_optional_byte(change.after),
                );
            }
        }
    }
    Ok(())
}

fn format_direction(direction: Option<hyperx_protocol::Direction>) -> &'static str {
    match direction {
        Some(hyperx_protocol::Direction::Tx) => "TX",
        Some(hyperx_protocol::Direction::Rx) => "RX",
        None => "--",
    }
}

fn format_optional_byte(byte: Option<u8>) -> String {
    byte.map_or_else(|| "--".to_owned(), |byte| format!("{byte:02X}"))
}

fn select_configuration_interface(interfaces: &[HidInterfaceInfo]) -> Result<&HidInterfaceInfo> {
    let matches: Vec<_> = interfaces
        .iter()
        .filter(|info| {
            info.usb_id() == PULSEFIRE_RAID.usb_id
                && info.matches(PULSEFIRE_RAID.configuration_interface)
        })
        .collect();

    match matches.as_slice() {
        [] => Err(anyhow!(
            "Pulsefire Raid configuration collection MI_01/FF01:0001 was not found"
        )),
        [configuration] => Ok(*configuration),
        _ => Err(anyhow!(
            "multiple Pulsefire Raid configuration collections found; explicit device selection is not implemented yet"
        )),
    }
}

fn print_supported_device(descriptor: &DeviceDescriptor, interfaces: &[&HidInterfaceInfo]) {
    println!("{}", descriptor.name);
    println!("VID: 0x{:04X}", descriptor.usb_id.vendor_id);
    println!("PID: 0x{:04X}", descriptor.usb_id.product_id);
    print_optional(
        "Manufacturer",
        first_value(interfaces, |info| &info.manufacturer),
    );
    print_optional("Product", first_value(interfaces, |info| &info.product));
    print_optional(
        "Serial",
        first_value(interfaces, |info| &info.serial_number),
    );
    if let Some(release) = interfaces.first().map(|info| info.release_number) {
        println!("Release: 0x{release:04X}");
    }
    println!("Interfaces:");

    for info in interfaces {
        let marker = if info.matches(descriptor.configuration_interface) {
            " [configuration]"
        } else {
            ""
        };
        println!("  Interface {}{marker}", info.interface_number);
        println!("    Usage Page: 0x{:04X}", info.usage_page);
        println!("    Usage: 0x{:04X}", info.usage);
        println!("    Path: {}", info.path);
    }
    println!();
}

fn print_optional(label: &str, value: Option<&str>) {
    println!("{label}: {}", value.unwrap_or("<not available>"));
}

fn first_value<'a>(
    interfaces: &'a [&HidInterfaceInfo],
    select: impl Fn(&'a HidInterfaceInfo) -> &'a Option<String>,
) -> Option<&'a str> {
    interfaces.iter().find_map(|info| select(info).as_deref())
}

fn print_unsupported(interfaces: &[&HidInterfaceInfo]) {
    println!("Other HID collections: {}", interfaces.len());
    for info in interfaces {
        println!(
            "  {} interface={} usage_page=0x{:04X} usage=0x{:04X} path={}",
            UsbId {
                vendor_id: info.vendor_id,
                product_id: info.product_id,
            },
            info.interface_number,
            info.usage_page,
            info.usage,
            info.path
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_value_skips_missing_strings() {
        let mut first = fixture();
        first.product = None;
        let second = fixture();
        let interfaces = [&first, &second];

        assert_eq!(
            first_value(&interfaces, |info| &info.product),
            Some("HyperX Pulsefire Raid")
        );
    }

    #[test]
    fn cli_validates_dpi_before_running_the_command() {
        for valid in ["200", "900", "16000"] {
            assert!(Cli::try_parse_from(["hyperx-cli", "dpi", "set", valid]).is_ok());
        }
        for invalid in ["199", "225", "16050", "not-a-number"] {
            assert!(Cli::try_parse_from(["hyperx-cli", "dpi", "set", invalid]).is_err());
        }
    }

    #[test]
    fn cli_validates_dpi_stage_commands_before_opening_the_device() {
        assert!(Cli::try_parse_from([
            "hyperx-cli",
            "dpi",
            "stage",
            "set",
            "2",
            "--dpi",
            "1700",
            "--color",
            "CD00FF",
            "--active",
        ])
        .is_ok());
        assert!(Cli::try_parse_from([
            "hyperx-cli",
            "dpi",
            "stage",
            "add",
            "16000",
            "FFFFFF",
            "--active",
        ])
        .is_ok());
        assert!(Cli::try_parse_from(["hyperx-cli", "dpi", "stage", "remove-last"]).is_ok());
        assert!(Cli::try_parse_from(["hyperx-cli", "dpi", "active", "5"]).is_ok());

        assert!(Cli::try_parse_from(["hyperx-cli", "dpi", "stage", "set", "2"]).is_err());
        assert!(
            Cli::try_parse_from(["hyperx-cli", "dpi", "stage", "set", "0", "--active",]).is_err()
        );
        assert!(
            Cli::try_parse_from(["hyperx-cli", "dpi", "stage", "add", "225", "FFFFFF",]).is_err()
        );
        assert!(Cli::try_parse_from(["hyperx-cli", "dpi", "active", "6"]).is_err());
    }

    #[test]
    fn cli_accepts_only_confirmed_polling_rates() {
        for valid in ["125", "250", "500", "1000"] {
            assert!(Cli::try_parse_from(["hyperx-cli", "polling", "set", valid]).is_ok());
        }
        assert!(Cli::try_parse_from(["hyperx-cli", "polling", "set", "2000"]).is_err());
    }

    #[test]
    fn cli_exposes_only_capture_backed_button5_assignments() {
        assert!(Cli::try_parse_from(["hyperx-cli", "buttons", "list"]).is_ok());
        assert!(
            Cli::try_parse_from(["hyperx-cli", "buttons", "set", "button5", "disabled",]).is_ok()
        );
        for function in ["forward", "back", "dpi-toggle"] {
            assert!(Cli::try_parse_from([
                "hyperx-cli",
                "buttons",
                "set",
                "button5",
                "mouse",
                function,
            ])
            .is_ok());
        }
        assert!(Cli::try_parse_from([
            "hyperx-cli",
            "buttons",
            "set",
            "button5",
            "multimedia",
            "volume-up",
        ])
        .is_ok());
        assert!(Cli::try_parse_from([
            "hyperx-cli",
            "buttons",
            "set",
            "button5",
            "windows-shortcut",
            "copy",
        ])
        .is_ok());
        assert!(
            Cli::try_parse_from(["hyperx-cli", "buttons", "set", "button5", "keyboard", "a",])
                .is_ok()
        );
        assert!(Cli::try_parse_from([
            "hyperx-cli",
            "buttons",
            "set",
            "button5",
            "macro",
            "macro.toml",
        ])
        .is_ok());

        assert!(
            Cli::try_parse_from(["hyperx-cli", "buttons", "set", "button5", "macro-ab-20ms",])
                .is_err()
        );
        assert!(
            Cli::try_parse_from(["hyperx-cli", "buttons", "set", "button4", "mouse", "back",])
                .is_err()
        );
        assert!(Cli::try_parse_from([
            "hyperx-cli",
            "buttons",
            "set",
            "button5",
            "multimedia",
            "volume-down",
        ])
        .is_err());
    }

    #[test]
    fn macro_toml_models_event_timelines_but_gates_device_encoding() {
        let example =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/macros/ab-20ms.toml");
        assert_eq!(
            load_button5_macro(&example).unwrap().0,
            PulsefireRaidButton5Assignment::MacroAb20Ms
        );

        let macro_ab: MacroDefinition = toml::from_str(
            r#"
                playback = "once"
                [[events]]
                type = "key-down"
                key = "a"
                delay_ms = 20
                [[events]]
                type = "key-up"
                key = "a"
                delay_ms = 20
                [[events]]
                type = "key-down"
                key = "b"
                delay_ms = 20
                [[events]]
                type = "key-up"
                key = "b"
                delay_ms = 20
            "#,
        )
        .unwrap();
        assert_eq!(
            confirmed_button5_macro_assignment(&macro_ab).unwrap(),
            PulsefireRaidButton5Assignment::MacroAb20Ms
        );

        let shift_a: MacroDefinition = toml::from_str(
            r#"
                playback = "once"
                [[events]]
                type = "key-down"
                key = "left-shift"
                delay_ms = 0
                [[events]]
                type = "key-down"
                key = "a"
                delay_ms = 20
                [[events]]
                type = "key-up"
                key = "a"
                delay_ms = 0
                [[events]]
                type = "key-up"
                key = "left-shift"
                delay_ms = 20
            "#,
        )
        .unwrap();
        assert_eq!(shift_a.events.len(), 4);
        assert!(confirmed_button5_macro_assignment(&shift_a).is_err());

        let nonuniform = MacroDefinition {
            playback: MacroPlayback::Once,
            events: vec![
                MacroEvent::KeyDown {
                    key: "a".to_owned(),
                    delay_ms: 20,
                },
                MacroEvent::KeyUp {
                    key: "a".to_owned(),
                    delay_ms: 40,
                },
            ],
        };
        assert!(confirmed_button5_macro_assignment(&nonuniform).is_err());
    }

    fn fixture() -> HidInterfaceInfo {
        HidInterfaceInfo {
            path: r"\\?\hid#vid_0951&pid_16e4&mi_01".to_owned(),
            vendor_id: 0x0951,
            product_id: 0x16E4,
            release_number: 0x1124,
            interface_number: 1,
            usage_page: 0xFF01,
            usage: 0x0001,
            manufacturer: Some("HyperX".to_owned()),
            product: Some("HyperX Pulsefire Raid".to_owned()),
            serial_number: None,
        }
    }
}
