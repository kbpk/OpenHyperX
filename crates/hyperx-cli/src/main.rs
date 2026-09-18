use std::{
    collections::BTreeMap,
    fs,
    io::Write,
    path::{Path, PathBuf},
    thread,
    time::{Duration, Instant},
};

use anyhow::{anyhow, Context, Result};
use clap::{ArgAction, Parser, Subcommand, ValueEnum};
use hyperx_core::{
    ButtonBinding, DeviceDescriptor, DpiProfile, HidInterfaceInfo, KeyboardUsage, MacroDefinition,
    MouseFunction, MultimediaFunction, PollingRate, RgbColor, SoftwareLightingEffect,
    SoftwareLightingProgram, UsbId, WindowsShortcut,
};
use hyperx_devices::{
    find_supported_device, PulsefireRaid, PulsefireRaidRuntimeAssignment, PULSEFIRE_RAID,
    PULSEFIRE_RAID_DPI,
};
use hyperx_hid::{HidApiDiscovery, HidApiTransport, HidDiscovery, HidTransport};
use hyperx_protocol::{
    capture::{diff_captures, parse_hex_capture},
    format_hex,
    hid_descriptor::parse_report_layouts,
    ngenuity_legacy::{
        format_ngenuity_legacy_id, keyboard_usage_name, ngenuity_legacy_embedded_payload,
        parse_ngenuity_legacy_preset, NgenuityContainer, NgenuityInputAction, NgenuityMacro,
        NgenuityMacroItem, NgenuityMouseButton, NgenuityPreset,
    },
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
    /// Inspect or import an application profile without accessing hardware.
    Profile {
        #[command(subcommand)]
        command: ProfileCommand,
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
enum ProfileCommand {
    /// Display confirmed fields from an NGENUITY Legacy version-40 .hxp preset.
    #[command(name = "inspect-ngenuity-legacy")]
    Inspect {
        /// Exported .hxp or internal NGENUITY Legacy Master.hxp/Preset file.
        file: PathBuf,
    },
    /// Compare two NGENUITY Legacy presets semantically and byte by byte.
    #[command(name = "diff-ngenuity-legacy")]
    Diff {
        /// Baseline exported .hxp or internal preset file.
        before: PathBuf,
        /// Preset exported after one isolated setting change.
        after: PathBuf,
        /// Print every raw byte change instead of the first 256.
        #[arg(long)]
        all_raw: bool,
    },
    /// Convert confirmed .hxp fields to a partial OpenHyperX TOML profile.
    #[command(name = "import-ngenuity-legacy")]
    Import {
        /// Exported .hxp or internal NGENUITY Legacy Master.hxp/Preset file.
        source: PathBuf,
        /// New TOML file. Existing files are never overwritten.
        output: PathBuf,
    },
}

#[derive(Debug, Subcommand)]
enum RgbCommand {
    /// Set wheel and logo to static direct colors.
    Static {
        /// Default for both zones: six hexadecimal RGB digits, with optional leading '#'.
        #[arg(required_unless_present_any = ["wheel", "logo"])]
        color: Option<RgbColor>,
        /// Scroll-wheel color, or "off". Overrides COLOR; black if COLOR is omitted.
        #[arg(long, value_parser = parse_rgb_or_off)]
        wheel: Option<RgbColor>,
        /// HyperX-logo color, or "off". Overrides COLOR; black if COLOR is omitted.
        #[arg(long, value_parser = parse_rgb_or_off)]
        logo: Option<RgbColor>,
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
    /// Render a software RGB spectrum in the foreground; never saves onboard.
    Cycle {
        /// Physical LEDs receiving the effect.
        #[arg(long, value_enum, default_value_t = RgbTargetArg::All)]
        target: RgbTargetArg,
        /// Foreground runtime in seconds, from 1 through 3600.
        #[arg(long, default_value_t = 30, value_parser = parse_effect_duration)]
        duration: u64,
        /// Seconds per complete color cycle, from 1 through 60.
        #[arg(long, default_value_t = 5, value_parser = parse_cycle_period)]
        period: u64,
        /// Additional logo phase from 0 through 359 degrees.
        #[arg(long, default_value_t = 0, value_parser = parse_phase_degrees)]
        logo_phase: u16,
    },
    /// Play a validated per-zone software-lighting TOML program in foreground.
    Play {
        /// Program containing independent wheel and logo effects.
        program: PathBuf,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
enum RgbTargetArg {
    All,
    Wheel,
    Logo,
}

impl RgbTargetArg {
    const fn colors(self, wheel: RgbColor, logo: RgbColor) -> (RgbColor, RgbColor) {
        match self {
            Self::All => (wheel, logo),
            Self::Wheel => (wheel, RgbColor::BLACK),
            Self::Logo => (RgbColor::BLACK, logo),
        }
    }

    const fn name(self) -> &'static str {
        match self {
            Self::All => "all LEDs",
            Self::Wheel => "wheel",
            Self::Logo => "logo",
        }
    }
}

#[cfg(windows)]
struct SystemMouseTrigger {
    previous: u8,
}

#[cfg(windows)]
impl SystemMouseTrigger {
    const SUPPORTED: bool = true;
    const BUTTON_KEYS: [i32; 5] = [0x01, 0x02, 0x04, 0x05, 0x06];

    fn new() -> Self {
        Self {
            previous: Self::current_buttons(),
        }
    }

    fn poll_down_edge(&mut self) -> bool {
        let current = Self::current_buttons();
        let down_edge = current & !self.previous != 0;
        self.previous = current;
        down_edge
    }

    fn current_buttons() -> u8 {
        Self::BUTTON_KEYS
            .iter()
            .enumerate()
            .fold(0_u8, |state, (index, &key)| {
                // SAFETY: GetAsyncKeyState has no pointer arguments and is
                // called with documented virtual-key constants only.
                let pressed = unsafe { GetAsyncKeyState(key) } as u16 & 0x8000 != 0;
                state | (u8::from(pressed) << index)
            })
    }
}

#[cfg(windows)]
#[link(name = "user32")]
extern "system" {
    fn GetAsyncKeyState(virtual_key: i32) -> i16;
}

#[cfg(not(windows))]
struct SystemMouseTrigger;

#[cfg(not(windows))]
impl SystemMouseTrigger {
    const SUPPORTED: bool = false;

    const fn new() -> Self {
        Self
    }

    const fn poll_down_edge(&mut self) -> bool {
        false
    }
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
    /// Change one target-specific, capture-backed runtime mapping.
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
    WheelClick,
    Button4,
    Button5,
    Button7,
    Button6,
    Button8,
    Dpi,
    WheelTiltLeft,
    WheelTiltRight,
}

impl ButtonControlArg {
    const fn protocol_control(self) -> PulsefireRaidControl {
        match self {
            Self::WheelClick => PulsefireRaidControl::MiddleClick,
            Self::Button4 => PulsefireRaidControl::Button4,
            Self::Button5 => PulsefireRaidControl::Button5,
            Self::Button7 => PulsefireRaidControl::Button7,
            Self::Button6 => PulsefireRaidControl::Button6,
            Self::Button8 => PulsefireRaidControl::Button8,
            Self::Dpi => PulsefireRaidControl::Dpi,
            Self::WheelTiltLeft => PulsefireRaidControl::WheelTiltLeft,
            Self::WheelTiltRight => PulsefireRaidControl::WheelTiltRight,
        }
    }
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
        /// Named USB HID key, for example: a, space, f12, left-shift.
        key: KeyboardUsage,
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
    VolumeDown,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
enum ButtonWindowsShortcutArg {
    Copy,
}

impl ButtonAssignmentCommand {
    fn into_assignment(
        self,
        control: PulsefireRaidControl,
    ) -> Result<(PulsefireRaidRuntimeAssignment, String)> {
        let (binding, description) = match self {
            Self::Disabled => (ButtonBinding::Disabled, "disabled"),
            Self::Mouse { function } => match function {
                ButtonMouseFunctionArg::Forward => (
                    ButtonBinding::Mouse(MouseFunction::Forward),
                    "mouse forward",
                ),
                ButtonMouseFunctionArg::Back => {
                    (ButtonBinding::Mouse(MouseFunction::Back), "mouse back")
                }
                ButtonMouseFunctionArg::DpiToggle => (
                    ButtonBinding::Mouse(MouseFunction::DpiToggle),
                    "mouse DPI toggle",
                ),
            },
            Self::Multimedia { function } => match function {
                ButtonMultimediaFunctionArg::VolumeUp => (
                    ButtonBinding::Multimedia(MultimediaFunction::VolumeUp),
                    "multimedia volume-up",
                ),
                ButtonMultimediaFunctionArg::VolumeDown => (
                    ButtonBinding::Multimedia(MultimediaFunction::VolumeDown),
                    "multimedia volume-down",
                ),
            },
            Self::WindowsShortcut {
                shortcut: ButtonWindowsShortcutArg::Copy,
            } => (
                ButtonBinding::WindowsShortcut(WindowsShortcut::Copy),
                "Windows shortcut copy",
            ),
            Self::Keyboard { key } => {
                let assignment = PulsefireRaidRuntimeAssignment::ordinary(
                    control,
                    ButtonBinding::Keyboard(key),
                )?;
                return Ok((assignment, format!("keyboard HID usage 0x{:02X}", key.0)));
            }
            Self::Macro { file } => return load_button_macro(control, &file),
        };
        let assignment = PulsefireRaidRuntimeAssignment::ordinary(control, binding)?;
        Ok((assignment, description.to_owned()))
    }
}

fn load_button_macro(
    control: PulsefireRaidControl,
    path: &Path,
) -> Result<(PulsefireRaidRuntimeAssignment, String)> {
    let source = fs::read_to_string(path)
        .with_context(|| format!("failed to read macro file {}", path.display()))?;
    let definition: MacroDefinition = toml::from_str(&source)
        .with_context(|| format!("failed to parse macro file {}", path.display()))?;
    let assignment = PulsefireRaidRuntimeAssignment::macro_timeline(control, definition)
        .with_context(|| format!("unsupported Pulsefire Raid macro in {}", path.display()))?;
    Ok((assignment, format!("macro from {}", path.display())))
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    init_logging(cli.verbose, cli.trace)?;

    match cli.command {
        Command::Devices { all } => devices(&HidApiDiscovery, all),
        Command::Info { descriptor } => info(&HidApiDiscovery, descriptor),
        Command::Rgb { command } => match command {
            RgbCommand::Static {
                color,
                wheel,
                logo,
                duration,
            } => {
                let (wheel, logo) = resolve_static_rgb(color, wheel, logo)?;
                rgb(wheel, logo, duration)
            }
            RgbCommand::Off { duration } => rgb(RgbColor::BLACK, RgbColor::BLACK, duration),
            RgbCommand::Cycle {
                target,
                duration,
                period,
                logo_phase,
            } => rgb_cycle(target, duration, period, logo_phase),
            RgbCommand::Play { program } => {
                let program = load_lighting_program(&program)?;
                rgb_play(program)
            }
        },
        Command::Dpi { command } => dpi(command),
        Command::Polling { command } => polling(command),
        Command::Buttons { command } => buttons(command),
        Command::Profile { command } => profile(command),
        Command::DecodeCapture { before, after } => decode_capture(&before, &after),
    }
}

const MAX_NGENUITY_LEGACY_PRESET_BYTES: u64 = 16 * 1024 * 1024;

fn profile(command: ProfileCommand) -> Result<()> {
    match command {
        ProfileCommand::Inspect { file } => {
            let preset = load_ngenuity_legacy_preset(&file)?;
            print_ngenuity_legacy_preset(&preset);
            Ok(())
        }
        ProfileCommand::Diff {
            before,
            after,
            all_raw,
        } => diff_ngenuity_legacy_profiles(&before, &after, all_raw),
        ProfileCommand::Import { source, output } => {
            let preset = load_ngenuity_legacy_preset(&source)?;
            let profile = preset
                .to_software_profile("pulsefire-raid")
                .with_context(|| format!("failed to import {}", source.display()))?;
            let encoded = toml::to_string_pretty(&profile)
                .context("failed to serialize the imported OpenHyperX profile")?;

            let mut destination = fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&output)
                .with_context(|| {
                    format!(
                        "failed to create {}; the importer never overwrites an existing file",
                        output.display()
                    )
                })?;
            destination
                .write_all(encoded.as_bytes())
                .with_context(|| format!("failed to write {}", output.display()))?;

            println!(
                "Imported NGENUITY Legacy preset {:?} to {}.",
                preset.name,
                output.display()
            );
            println!(
                "Imported {} DPI stage(s), {} macro(s), and retained {} unresolved button assignment(s).",
                preset.dpi_stages.len(),
                preset.macros.len(),
                preset.key_assignments.len(),
            );
            println!(
                "The profile is marked partial: polling, lighting and physical assignment targets are not decoded from .hxp yet."
            );
            println!("No HID device was opened and no onboard profile was written.");
            Ok(())
        }
    }
}

fn load_ngenuity_legacy_preset(path: &Path) -> Result<NgenuityPreset> {
    let bytes = load_ngenuity_legacy_bytes(path)?;
    parse_ngenuity_legacy_preset(&bytes)
        .with_context(|| format!("failed to parse NGENUITY Legacy preset {}", path.display()))
}

fn load_ngenuity_legacy_bytes(path: &Path) -> Result<Vec<u8>> {
    let metadata = fs::metadata(path).with_context(|| {
        format!(
            "failed to inspect NGENUITY Legacy preset {}",
            path.display()
        )
    })?;
    if metadata.len() > MAX_NGENUITY_LEGACY_PRESET_BYTES {
        return Err(anyhow!(
            "NGENUITY Legacy preset {} is {} bytes; the offline parser limit is {} bytes",
            path.display(),
            metadata.len(),
            MAX_NGENUITY_LEGACY_PRESET_BYTES,
        ));
    }
    fs::read(path)
        .with_context(|| format!("failed to read NGENUITY Legacy preset {}", path.display()))
}

const DEFAULT_RAW_NGENUITY_LEGACY_CHANGE_LIMIT: usize = 256;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct RawByteChange {
    offset: usize,
    before: Option<u8>,
    after: Option<u8>,
}

fn diff_ngenuity_legacy_profiles(
    before_path: &Path,
    after_path: &Path,
    all_raw: bool,
) -> Result<()> {
    let before_bytes = load_ngenuity_legacy_bytes(before_path)?;
    let after_bytes = load_ngenuity_legacy_bytes(after_path)?;
    let before = parse_ngenuity_legacy_preset(&before_bytes)
        .with_context(|| format!("failed to parse {}", before_path.display()))?;
    let after = parse_ngenuity_legacy_preset(&after_bytes)
        .with_context(|| format!("failed to parse {}", after_path.display()))?;
    let before_payload = ngenuity_legacy_embedded_payload(&before_bytes)?;
    let after_payload = ngenuity_legacy_embedded_payload(&after_bytes)?;

    println!(
        "Compared NGENUITY Legacy presets {:?} and {:?}.",
        before.name, after.name
    );
    println!("Decoded changes:");
    let semantic = semantic_ngenuity_legacy_changes(&before, &after);
    if semantic.is_empty() {
        println!("  <none>");
    } else {
        for change in semantic {
            println!("  {change}");
        }
    }

    let raw = raw_byte_changes(before_payload, after_payload);
    println!(
        "Raw embedded changes: {} byte(s); lengths {} -> {} bytes.",
        raw.len(),
        before_payload.len(),
        after_payload.len(),
    );
    if before_payload.len() != after_payload.len() {
        println!(
            "  Lengths differ; raw offsets after an insertion or removal may no longer align."
        );
    }
    let limit = if all_raw {
        raw.len()
    } else {
        DEFAULT_RAW_NGENUITY_LEGACY_CHANGE_LIMIT
    };
    for change in raw.iter().take(limit) {
        println!(
            "  offset 0x{:04X}: {} -> {}",
            change.offset,
            format_optional_byte(change.before),
            format_optional_byte(change.after),
        );
    }
    if raw.len() > limit {
        println!(
            "  ... {} more changed byte(s); rerun with --all-raw to print them.",
            raw.len() - limit
        );
    }
    if !raw.is_empty() {
        println!(
            "  Raw differences can include regenerated source identifiers; decoded comparisons ignore identifier values."
        );
    }
    println!("Comparison was offline; no HID device was opened.");
    Ok(())
}

fn semantic_ngenuity_legacy_changes(
    before: &NgenuityPreset,
    after: &NgenuityPreset,
) -> Vec<String> {
    let mut changes = Vec::new();
    if before.name != after.name {
        changes.push(format!("name: {:?} -> {:?}", before.name, after.name));
    }
    if before.dpi_stages.len() != after.dpi_stages.len() {
        changes.push(format!(
            "DPI stage count: {} -> {}",
            before.dpi_stages.len(),
            after.dpi_stages.len()
        ));
    }
    for index in 0..before.dpi_stages.len().max(after.dpi_stages.len()) {
        match (before.dpi_stages.get(index), after.dpi_stages.get(index)) {
            (Some(left), Some(right))
                if left.dpi != right.dpi
                    || left.alpha != right.alpha
                    || left.color != right.color =>
            {
                changes.push(format!(
                    "DPI stage {}: {} DPI {}/alpha {} -> {} DPI {}/alpha {}",
                    index + 1,
                    left.dpi,
                    left.color,
                    left.alpha,
                    right.dpi,
                    right.color,
                    right.alpha,
                ));
            }
            (None, Some(stage)) => changes.push(format!(
                "DPI stage {} added: {} DPI {}/alpha {}",
                index + 1,
                stage.dpi,
                stage.color,
                stage.alpha,
            )),
            (Some(stage), None) => changes.push(format!(
                "DPI stage {} removed: {} DPI {}/alpha {}",
                index + 1,
                stage.dpi,
                stage.color,
                stage.alpha,
            )),
            _ => {}
        }
    }
    if before.active_dpi_stage != after.active_dpi_stage {
        changes.push(format!(
            "stored active-stage value: {} -> {}",
            before.active_dpi_stage, after.active_dpi_stage
        ));
    }

    if before.macros.len() != after.macros.len() {
        changes.push(format!(
            "macro count: {} -> {}",
            before.macros.len(),
            after.macros.len()
        ));
    }
    for index in 0..before.macros.len().max(after.macros.len()) {
        match (before.macros.get(index), after.macros.get(index)) {
            (Some(left), Some(right)) => {
                compare_ngenuity_legacy_macro(index, left, right, &mut changes)
            }
            (None, Some(source_macro)) => changes.push(format!(
                "macro {} added: {:?}",
                index + 1,
                source_macro.name
            )),
            (Some(source_macro), None) => changes.push(format!(
                "macro {} removed: {:?}",
                index + 1,
                source_macro.name
            )),
            _ => {}
        }
    }

    if before.key_assignments.len() != after.key_assignments.len() {
        changes.push(format!(
            "assignment count: {} -> {}",
            before.key_assignments.len(),
            after.key_assignments.len()
        ));
    }
    for index in 0..before
        .key_assignments
        .len()
        .max(after.key_assignments.len())
    {
        let left = before
            .key_assignments
            .get(index)
            .and_then(|assignment| macro_reference_index(before, assignment.macro_source_id));
        let right = after
            .key_assignments
            .get(index)
            .and_then(|assignment| macro_reference_index(after, assignment.macro_source_id));
        if left != right {
            changes.push(format!(
                "assignment {} macro reference: {} -> {}",
                index + 1,
                format_macro_reference(left),
                format_macro_reference(right),
            ));
        }
    }
    changes
}

fn compare_ngenuity_legacy_macro(
    index: usize,
    before: &NgenuityMacro,
    after: &NgenuityMacro,
    changes: &mut Vec<String>,
) {
    let number = index + 1;
    if before.name != after.name {
        changes.push(format!(
            "macro {number} name: {:?} -> {:?}",
            before.name, after.name
        ));
    }
    for (field, left, right) in [
        (
            "use-standard-timing",
            u32::from(before.use_standard_timing),
            u32::from(after.use_standard_timing),
        ),
        (
            "standard timing ms",
            before.standard_timing_ms,
            after.standard_timing_ms,
        ),
        (
            "expanded",
            u32::from(before.expanded),
            u32::from(after.expanded),
        ),
        ("playback mode", before.playback_mode, after.playback_mode),
        (
            "play times",
            u32::from(before.play_times),
            u32::from(after.play_times),
        ),
    ] {
        if left != right {
            changes.push(format!("macro {number} {field}: {left} -> {right}"));
        }
    }
    if before.items.len() != after.items.len() {
        changes.push(format!(
            "macro {number} event count: {} -> {}",
            before.items.len(),
            after.items.len()
        ));
    }
    for event in 0..before.items.len().max(after.items.len()) {
        match (before.items.get(event), after.items.get(event)) {
            (Some(left), Some(right)) if !same_ngenuity_legacy_macro_item(left, right) => {
                changes.push(format!(
                    "macro {number} event {}: {} (stored {} ms) -> {} (stored {} ms)",
                    event + 1,
                    describe_ngenuity_legacy_macro_item(left),
                    left.timing_ms,
                    describe_ngenuity_legacy_macro_item(right),
                    right.timing_ms,
                ));
            }
            (None, Some(item)) => changes.push(format!(
                "macro {number} event {} added: {} (stored {} ms)",
                event + 1,
                describe_ngenuity_legacy_macro_item(item),
                item.timing_ms,
            )),
            (Some(item), None) => changes.push(format!(
                "macro {number} event {} removed: {} (stored {} ms)",
                event + 1,
                describe_ngenuity_legacy_macro_item(item),
                item.timing_ms,
            )),
            _ => {}
        }
    }
}

fn same_ngenuity_legacy_macro_item(before: &NgenuityMacroItem, after: &NgenuityMacroItem) -> bool {
    before.item_type == after.item_type
        && before.action == after.action
        && before.state == after.state
        && before.data == after.data
        && before.timing_ms == after.timing_ms
        && before.key_data == after.key_data
}

fn macro_reference_index(preset: &NgenuityPreset, id: Option<[u8; 16]>) -> Option<usize> {
    id.and_then(|id| {
        preset
            .macros
            .iter()
            .position(|source_macro| source_macro.source_id == id)
            .map(|index| index + 1)
    })
}

fn format_macro_reference(reference: Option<usize>) -> String {
    reference.map_or_else(
        || "none/unresolved".to_owned(),
        |index| format!("macro {index}"),
    )
}

fn raw_byte_changes(before: &[u8], after: &[u8]) -> Vec<RawByteChange> {
    let mut changes = Vec::new();
    for offset in 0..before.len().max(after.len()) {
        let left = before.get(offset).copied();
        let right = after.get(offset).copied();
        if left != right {
            changes.push(RawByteChange {
                offset,
                before: left,
                after: right,
            });
        }
    }
    changes
}

fn print_ngenuity_legacy_preset(preset: &NgenuityPreset) {
    let container = match preset.container {
        NgenuityContainer::Export => "exported .hxp wrapper",
        NgenuityContainer::InternalPreset => "internal preset",
    };
    println!("NGENUITY Legacy preset: {}", preset.name);
    println!("Container: {container}");
    println!("Format version: {}", preset.version);
    println!("Embedded preset: {} bytes", preset.embedded_length);
    println!("DPI stages:");
    for (index, stage) in preset.dpi_stages.iter().enumerate() {
        let active = if u32::try_from(index + 1).ok() == Some(preset.active_dpi_stage) {
            " [active candidate]"
        } else {
            ""
        };
        println!(
            "  {}: {} DPI, {}, alpha={}{}",
            index + 1,
            stage.dpi,
            stage.color,
            stage.alpha,
            active,
        );
    }
    println!(
        "  Stored active-stage value: {} (indexing semantics are not confirmed).",
        preset.active_dpi_stage
    );

    println!("Macros:");
    if preset.macros.is_empty() {
        println!("  <none>");
    }
    for (index, source_macro) in preset.macros.iter().enumerate() {
        print_ngenuity_legacy_macro(index + 1, source_macro);
    }

    println!("Button assignments: {}", preset.key_assignments.len());
    for (index, assignment) in preset.key_assignments.iter().enumerate() {
        let reference = assignment.macro_source_id.as_ref().map_or_else(
            || "no decoded macro reference".to_owned(),
            |id| format!("macro {}", format_ngenuity_legacy_id(id)),
        );
        println!(
            "  {}: source_id={}, {reference}; physical control unresolved",
            index + 1,
            format_ngenuity_legacy_id(&assignment.source_id),
        );
    }
    println!(
        "Not decoded from .hxp yet: polling rate, lighting fields, and physical targets for assignments."
    );
    println!("Inspection was offline; no HID device was opened.");
}

fn print_ngenuity_legacy_macro(index: usize, source_macro: &NgenuityMacro) {
    let timing = if source_macro.use_standard_timing {
        format!("standard {} ms", source_macro.standard_timing_ms)
    } else {
        "recorded per event".to_owned()
    };
    let playback = if source_macro.playback_mode == 1 {
        "play once"
    } else {
        "unconfirmed"
    };
    println!(
        "  {index}: {:?} id={} ({timing}, {playback}, raw mode={}, play-times={}, events={})",
        source_macro.name,
        format_ngenuity_legacy_id(&source_macro.source_id),
        source_macro.playback_mode,
        source_macro.play_times,
        source_macro.items.len(),
    );
    for (event, item) in source_macro.items.iter().enumerate() {
        let effective = source_macro.effective_timing_ms(item);
        let stored = if effective != item.timing_ms {
            format!(", stored={} ms", item.timing_ms)
        } else {
            String::new()
        };
        println!(
            "    {}: {}, delay={} ms{}",
            event + 1,
            describe_ngenuity_legacy_macro_item(item),
            effective,
            stored,
        );
    }
}

fn describe_ngenuity_legacy_macro_item(item: &NgenuityMacroItem) -> String {
    match item.decoded_action() {
        Some(NgenuityInputAction::Keyboard { usage, pressed }) => {
            let key = keyboard_usage_name(usage).unwrap_or_else(|| format!("usage-0x{usage:04X}"));
            format!(
                "key-{} {key} (HID 0x{usage:04X})",
                if pressed { "down" } else { "up" }
            )
        }
        Some(NgenuityInputAction::MouseButton { button, pressed }) => {
            let button = match button {
                NgenuityMouseButton::Left => "left",
                NgenuityMouseButton::Right => "right",
                NgenuityMouseButton::Middle => "middle",
            };
            format!("mouse-{} {button}", if pressed { "down" } else { "up" })
        }
        None => format!(
            "unknown type={} action={} state={} data={} key-data=0x{:04X}",
            item.item_type, item.action, item.state, item.data, item.key_data
        ),
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

fn parse_rgb_or_off(input: &str) -> Result<RgbColor, String> {
    if input.eq_ignore_ascii_case("off") {
        Ok(RgbColor::BLACK)
    } else {
        input.parse::<RgbColor>().map_err(|error| error.to_string())
    }
}

fn parse_effect_duration(input: &str) -> Result<u64, String> {
    let seconds = input
        .parse::<u64>()
        .map_err(|_| format!("effect duration must be an integer; got {input}"))?;
    if (1..=3600).contains(&seconds) {
        Ok(seconds)
    } else {
        Err(format!(
            "effect duration must be between 1 and 3600 seconds; got {seconds}"
        ))
    }
}

fn parse_cycle_period(input: &str) -> Result<u64, String> {
    let seconds = input
        .parse::<u64>()
        .map_err(|_| format!("cycle period must be an integer; got {input}"))?;
    if (1..=60).contains(&seconds) {
        Ok(seconds)
    } else {
        Err(format!(
            "cycle period must be between 1 and 60 seconds; got {seconds}"
        ))
    }
}

fn parse_phase_degrees(input: &str) -> Result<u16, String> {
    let phase = input
        .parse::<u16>()
        .map_err(|_| format!("phase must be an integer; got {input}"))?;
    if phase < 360 {
        Ok(phase)
    } else {
        Err(format!(
            "phase must be from 0 through 359 degrees; got {phase}"
        ))
    }
}

fn load_lighting_program(path: &Path) -> Result<SoftwareLightingProgram> {
    let source = fs::read_to_string(path)
        .with_context(|| format!("failed to read lighting program {}", path.display()))?;
    let program: SoftwareLightingProgram = toml::from_str(&source)
        .with_context(|| format!("failed to parse lighting program {}", path.display()))?;
    validate_pulsefire_raid_lighting_program(&program)
        .with_context(|| format!("invalid lighting program {}", path.display()))?;
    Ok(program)
}

fn validate_pulsefire_raid_lighting_program(program: &SoftwareLightingProgram) -> Result<()> {
    program.validate()?;
    for zone in program.zones.keys() {
        if zone != "wheel" && zone != "logo" {
            return Err(anyhow!(
                "Pulsefire Raid lighting program contains unknown zone {zone:?}; use wheel or logo"
            ));
        }
    }
    if program.uses_triggers() && !SystemMouseTrigger::SUPPORTED {
        return Err(anyhow!(
            "triggered-fade currently requires the Windows system mouse-event adapter"
        ));
    }
    Ok(())
}

fn resolve_static_rgb(
    color: Option<RgbColor>,
    wheel: Option<RgbColor>,
    logo: Option<RgbColor>,
) -> Result<(RgbColor, RgbColor)> {
    if color.is_none() && wheel.is_none() && logo.is_none() {
        return Err(anyhow!(
            "provide COLOR, --wheel COLOR|off, or --logo COLOR|off"
        ));
    }
    let fallback = color.unwrap_or(RgbColor::BLACK);
    Ok((wheel.unwrap_or(fallback), logo.unwrap_or(fallback)))
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
    let profile = device.runtime_profile().context(
        "failed to read the Pulsefire Raid runtime profile; close every NGENUITY variant and retry",
    )?;
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
                "failed to read the Pulsefire Raid runtime profile; close every NGENUITY variant and retry",
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
                "failed to read the Pulsefire Raid runtime profile; close every NGENUITY variant and retry",
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
                "failed to read the Pulsefire Raid runtime profile; close every NGENUITY variant and retry",
            )?;
            println!("Button mappings:");
            print_button_profile(&profile, "  ");
        }
        ButtonsCommand::Set {
            control,
            assignment,
        } => {
            let control = control.protocol_control();
            let (assignment, description) = assignment.into_assignment(control)?;
            let mut device = open_pulsefire_raid()?;
            device
                .set_runtime_button_assignment(assignment)
                .with_context(|| {
                    format!(
                        "failed to update {}'s runtime mapping; the setting may be unchanged",
                        control.name()
                    )
                })?;
            println!(
                "Updated {} runtime mapping to {description}.",
                control.name()
            );
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
            "failed to open the Pulsefire Raid configuration collection; close every NGENUITY variant and retry: {}",
            configuration.path
        )
    })?;
    Ok(PulsefireRaid::new(transport)?)
}

fn rgb(wheel: RgbColor, logo: RgbColor, duration_seconds: u64) -> Result<()> {
    if duration_seconds > 3600 {
        return Err(anyhow!("--duration cannot exceed 3600 seconds"));
    }

    let mut device = open_pulsefire_raid()?;
    let deadline = Instant::now() + Duration::from_secs(duration_seconds);

    loop {
        device.set_volatile_direct_rgb(wheel, logo)?;
        if duration_seconds == 0 || Instant::now() >= deadline {
            break;
        }
        thread::sleep(Duration::from_millis(750));
    }

    println!(
        "Applied volatile direct RGB: wheel=#{:02X}{:02X}{:02X}, logo=#{:02X}{:02X}{:02X}.",
        wheel.red, wheel.green, wheel.blue, logo.red, logo.green, logo.blue
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

fn rgb_cycle(
    target: RgbTargetArg,
    duration_seconds: u64,
    period_seconds: u64,
    logo_phase: u16,
) -> Result<()> {
    let mut device = open_pulsefire_raid()?;
    let started = Instant::now();
    let duration = Duration::from_secs(duration_seconds);
    let wheel_effect = SoftwareLightingEffect::Cycle {
        period_ms: period_seconds * 1_000,
        phase_degrees: 0,
    };
    let logo_effect = SoftwareLightingEffect::Cycle {
        period_ms: period_seconds * 1_000,
        phase_degrees: logo_phase,
    };

    while started.elapsed() < duration {
        let elapsed_ms = started.elapsed().as_millis() as u64;
        let wheel = wheel_effect.color_at(elapsed_ms, None)?;
        let logo = logo_effect.color_at(elapsed_ms, None)?;
        let (wheel, logo) = target.colors(wheel, logo);
        device.set_volatile_direct_rgb(wheel, logo)?;
        thread::sleep(Duration::from_millis(50));
    }

    println!(
        "Rendered volatile RGB cycle on {} for {duration_seconds} seconds with a {period_seconds}-second period and {logo_phase}-degree logo phase.",
        target.name(),
    );
    println!("The previous lighting state may now return.");
    println!("No onboard profile or firmware data was written.");
    Ok(())
}

fn rgb_play(program: SoftwareLightingProgram) -> Result<()> {
    let wheel_effect = program.zones.get("wheel");
    let logo_effect = program.zones.get("logo");
    let mut trigger = SystemMouseTrigger::new();
    let mut last_trigger_ms = None;
    let mut device = open_pulsefire_raid()?;
    let started = Instant::now();
    let duration = Duration::from_secs(program.duration_seconds);

    while started.elapsed() < duration {
        let elapsed_ms = started.elapsed().as_millis() as u64;
        if trigger.poll_down_edge() {
            last_trigger_ms = Some(elapsed_ms);
        }
        let wheel = match wheel_effect {
            Some(effect) => effect.color_at(elapsed_ms, last_trigger_ms)?,
            None => RgbColor::BLACK,
        };
        let logo = match logo_effect {
            Some(effect) => effect.color_at(elapsed_ms, last_trigger_ms)?,
            None => RgbColor::BLACK,
        };
        device.set_volatile_direct_rgb(wheel, logo)?;
        thread::sleep(Duration::from_millis(program.frame_interval_ms));
    }

    println!(
        "Rendered volatile per-zone lighting program for {} seconds.",
        program.duration_seconds
    );
    println!("The previous lighting state may now return.");
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
    use hyperx_core::{MacroEvent, MacroPlayback};

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
    fn cli_resolves_legacy_and_per_zone_static_rgb() {
        let orange = RgbColor::new(0xFF, 0x80, 0x00);
        assert_eq!(
            resolve_static_rgb(Some(orange), None, None).unwrap(),
            (orange, orange)
        );
        assert_eq!(
            resolve_static_rgb(
                None,
                Some(RgbColor::new(0xFF, 0, 0)),
                Some(RgbColor::new(0, 0, 0xFF)),
            )
            .unwrap(),
            (RgbColor::new(0xFF, 0, 0), RgbColor::new(0, 0, 0xFF))
        );
        assert_eq!(
            resolve_static_rgb(None, None, Some(RgbColor::new(0, 0xFF, 0))).unwrap(),
            (RgbColor::BLACK, RgbColor::new(0, 0xFF, 0))
        );
        assert!(resolve_static_rgb(None, None, None).is_err());

        assert!(Cli::try_parse_from(["hyperx-cli", "rgb", "static", "FF8000"]).is_ok());
        assert!(Cli::try_parse_from([
            "hyperx-cli",
            "rgb",
            "static",
            "--wheel",
            "FF0000",
            "--logo",
            "0000FF",
            "--duration",
            "30",
        ])
        .is_ok());
        assert!(
            Cli::try_parse_from(["hyperx-cli", "rgb", "static", "FFFFFF", "--wheel", "off",])
                .is_ok()
        );
        assert!(Cli::try_parse_from(["hyperx-cli", "rgb", "static"]).is_err());
        assert!(
            Cli::try_parse_from(["hyperx-cli", "rgb", "static", "--logo", "not-a-color",]).is_err()
        );
    }

    #[test]
    fn cli_validates_foreground_rgb_cycle() {
        let red = RgbColor::new(255, 0, 0);
        let blue = RgbColor::new(0, 0, 255);
        assert_eq!(RgbTargetArg::All.colors(red, blue), (red, blue));
        assert_eq!(
            RgbTargetArg::Wheel.colors(red, blue),
            (red, RgbColor::BLACK)
        );
        assert_eq!(
            RgbTargetArg::Logo.colors(red, blue),
            (RgbColor::BLACK, blue)
        );

        assert!(Cli::try_parse_from(["hyperx-cli", "rgb", "cycle"]).is_ok());
        assert!(Cli::try_parse_from([
            "hyperx-cli",
            "rgb",
            "cycle",
            "--target",
            "wheel",
            "--duration",
            "5",
            "--period",
            "2",
            "--logo-phase",
            "180",
        ])
        .is_ok());

        for invalid in [
            ["--duration", "0"],
            ["--duration", "3601"],
            ["--period", "0"],
            ["--period", "61"],
            ["--logo-phase", "360"],
            ["--target", "not-a-zone"],
        ] {
            assert!(
                Cli::try_parse_from(["hyperx-cli", "rgb", "cycle", invalid[0], invalid[1],])
                    .is_err()
            );
        }
    }

    #[test]
    fn lighting_toml_parses_every_effect_and_rejects_unknown_zones() {
        let effects = [
            "effect = \"off\"",
            "effect = \"solid\"\ncolor = \"#FF8000\"",
            "effect = \"cycle\"\nperiod_ms = 5000\nphase_degrees = 180",
            "effect = \"pulse\"\ncolor = \"FF0000\"\nperiod_ms = 1000\nphase_degrees = 90",
            "effect = \"breathing\"\ncolor = \"00FF00\"\nperiod_ms = 2000\nphase_degrees = 270",
            "effect = \"triggered-fade\"\ncolor = \"0000FF\"\nfade_ms = 750",
            "effect = \"confetti\"\nstep_ms = 200\nseed = 42",
            "effect = \"sun\"\nperiod_ms = 5000\nphase_degrees = 45",
            "effect = \"twilight\"\nperiod_ms = 5000\nphase_degrees = 315",
        ];
        for effect in effects {
            let source =
                format!("duration_seconds = 5\nframe_interval_ms = 50\n[zones.wheel]\n{effect}\n");
            let program: SoftwareLightingProgram = toml::from_str(&source).unwrap();
            assert_eq!(program.validate(), Ok(()), "failed effect:\n{effect}");
        }

        let unknown_zone: SoftwareLightingProgram = toml::from_str(
            r#"
                duration_seconds = 5
                [zones.moon]
                effect = "solid"
                color = "FFFFFF"
            "#,
        )
        .unwrap();
        assert!(validate_pulsefire_raid_lighting_program(&unknown_zone).is_err());

        for source in [
            r#"
                duration_seconds = 5
                typo = true
                [zones.wheel]
                effect = "solid"
                color = "FFFFFF"
            "#,
            r#"
                duration_seconds = 5
                [zones.wheel]
                effect = "cycle"
                phase_degree = 180
            "#,
        ] {
            assert!(toml::from_str::<SoftwareLightingProgram>(source).is_err());
        }
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
    fn cli_exposes_only_explicit_ngenuity_legacy_profile_commands() {
        assert!(Cli::try_parse_from([
            "hyperx-cli",
            "profile",
            "inspect-ngenuity-legacy",
            "Base Settings.hxp",
        ])
        .is_ok());
        assert!(Cli::try_parse_from([
            "hyperx-cli",
            "profile",
            "diff-ngenuity-legacy",
            "before.hxp",
            "after.hxp",
        ])
        .is_ok());
        assert!(Cli::try_parse_from([
            "hyperx-cli",
            "profile",
            "diff-ngenuity-legacy",
            "before.hxp",
            "after.hxp",
            "--all-raw",
        ])
        .is_ok());
        assert!(Cli::try_parse_from([
            "hyperx-cli",
            "profile",
            "import-ngenuity-legacy",
            "Base Settings.hxp",
            "base-settings.toml",
        ])
        .is_ok());
        assert!(Cli::try_parse_from([
            "hyperx-cli",
            "profile",
            "import-ngenuity-legacy",
            "Base Settings.hxp",
        ])
        .is_err());
        assert!(Cli::try_parse_from([
            "hyperx-cli",
            "profile",
            "inspect-ngenuity",
            "Base Settings.hxp",
        ])
        .is_err());
    }

    #[test]
    fn ngenuity_legacy_diff_reports_semantics_but_ignores_source_ids() {
        use hyperx_protocol::ngenuity_legacy::NgenuityDpiStage;

        let before = NgenuityPreset {
            container: NgenuityContainer::Export,
            embedded_length: 100,
            version: 40,
            name: "Preset".to_owned(),
            dpi_stages: vec![NgenuityDpiStage {
                source_id: [1; 16],
                dpi: 800,
                alpha: 255,
                color: RgbColor::new(1, 2, 3),
            }],
            active_dpi_stage: 1,
            macros: Vec::new(),
            key_assignments: Vec::new(),
        };
        let mut identifiers_only = before.clone();
        identifiers_only.dpi_stages[0].source_id = [2; 16];
        assert!(semantic_ngenuity_legacy_changes(&before, &identifiers_only).is_empty());

        let mut changed = identifiers_only;
        changed.dpi_stages[0].dpi = 900;
        changed.active_dpi_stage = 2;
        assert_eq!(
            semantic_ngenuity_legacy_changes(&before, &changed),
            vec![
                "DPI stage 1: 800 DPI #010203/alpha 255 -> 900 DPI #010203/alpha 255",
                "stored active-stage value: 1 -> 2",
            ]
        );
    }

    #[test]
    fn raw_ngenuity_legacy_diff_tracks_offsets_and_length_changes() {
        assert_eq!(
            raw_byte_changes(&[0x10, 0x20, 0x30], &[0x10, 0x21, 0x30, 0x40]),
            vec![
                RawByteChange {
                    offset: 1,
                    before: Some(0x20),
                    after: Some(0x21),
                },
                RawByteChange {
                    offset: 3,
                    before: None,
                    after: Some(0x40),
                },
            ]
        );
    }

    #[test]
    fn cli_exposes_only_capture_backed_button_targets_and_assignments() {
        assert!(Cli::try_parse_from(["hyperx-cli", "buttons", "list"]).is_ok());
        for control in [
            "wheel-click",
            "button4",
            "button5",
            "button7",
            "button6",
            "button8",
            "dpi",
            "wheel-tilt-left",
            "wheel-tilt-right",
        ] {
            assert!(
                Cli::try_parse_from(["hyperx-cli", "buttons", "set", control, "disabled",]).is_ok()
            );
        }
        assert!(
            Cli::try_parse_from(["hyperx-cli", "buttons", "set", "left-click", "disabled"])
                .is_err()
        );
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
            "dpi",
            "mouse",
            "dpi-toggle",
        ])
        .is_ok());
        assert!(
            Cli::try_parse_from(["hyperx-cli", "buttons", "set", "dpi", "keyboard", "a",]).is_ok()
        );
        assert!(Cli::try_parse_from([
            "hyperx-cli",
            "buttons",
            "set",
            "dpi",
            "keyboard",
            "left-shift",
        ])
        .is_ok());
        assert!(Cli::try_parse_from([
            "hyperx-cli",
            "buttons",
            "set",
            "dpi",
            "keyboard",
            "definitely-not-a-key",
        ])
        .is_err());
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
            Cli::try_parse_from(["hyperx-cli", "buttons", "set", "button4", "disabled",]).is_ok()
        );
        assert!(
            Cli::try_parse_from(["hyperx-cli", "buttons", "set", "button4", "mouse", "back",])
                .is_ok()
        );
        assert!(Cli::try_parse_from([
            "hyperx-cli",
            "buttons",
            "set",
            "button5",
            "multimedia",
            "volume-down",
        ])
        .is_ok());

        assert!(ButtonAssignmentCommand::Mouse {
            function: ButtonMouseFunctionArg::DpiToggle,
        }
        .into_assignment(PulsefireRaidControl::Dpi)
        .is_ok());
        assert!(ButtonAssignmentCommand::Keyboard {
            key: KeyboardUsage(0x05),
        }
        .into_assignment(PulsefireRaidControl::Dpi)
        .is_ok());
        assert!(ButtonAssignmentCommand::Disabled
            .into_assignment(PulsefireRaidControl::Dpi)
            .is_ok());
        assert!(ButtonAssignmentCommand::Mouse {
            function: ButtonMouseFunctionArg::Forward,
        }
        .into_assignment(PulsefireRaidControl::Dpi)
        .is_err());
        assert!(ButtonAssignmentCommand::Disabled
            .into_assignment(PulsefireRaidControl::Button4)
            .is_ok());
        assert!(ButtonAssignmentCommand::Mouse {
            function: ButtonMouseFunctionArg::Back,
        }
        .into_assignment(PulsefireRaidControl::Button4)
        .is_ok());
        assert!(ButtonAssignmentCommand::Mouse {
            function: ButtonMouseFunctionArg::Forward,
        }
        .into_assignment(PulsefireRaidControl::Button4)
        .is_err());
        for function in [
            ButtonMultimediaFunctionArg::VolumeUp,
            ButtonMultimediaFunctionArg::VolumeDown,
        ] {
            assert!(ButtonAssignmentCommand::Multimedia { function }
                .into_assignment(PulsefireRaidControl::Button7)
                .is_ok());
        }
        assert!(ButtonAssignmentCommand::Multimedia {
            function: ButtonMultimediaFunctionArg::VolumeDown,
        }
        .into_assignment(PulsefireRaidControl::Button5)
        .is_ok());
        assert!(ButtonAssignmentCommand::Disabled
            .into_assignment(PulsefireRaidControl::Button7)
            .is_ok());
        assert!(ButtonAssignmentCommand::WindowsShortcut {
            shortcut: ButtonWindowsShortcutArg::Copy,
        }
        .into_assignment(PulsefireRaidControl::Button7)
        .is_err());
    }

    #[test]
    fn macro_toml_supports_chords_nonuniform_timings_and_mouse_clicks() {
        let example =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/macros/ab-20ms.toml");
        let example_assignment = load_button_macro(PulsefireRaidControl::Button5, &example)
            .unwrap()
            .0;
        let example_definition = example_assignment
            .macro_definition()
            .expect("the TOML example must produce a macro assignment");
        assert_eq!(example_definition.events.len(), 4);

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
        assert!(PulsefireRaidRuntimeAssignment::macro_timeline(
            PulsefireRaidControl::Button5,
            macro_ab
        )
        .is_ok());

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
        assert!(PulsefireRaidRuntimeAssignment::macro_timeline(
            PulsefireRaidControl::Button5,
            shift_a
        )
        .is_ok());

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
        assert!(PulsefireRaidRuntimeAssignment::macro_timeline(
            PulsefireRaidControl::Button5,
            nonuniform
        )
        .is_ok());

        let mouse_clicks: MacroDefinition = toml::from_str(
            r#"
                playback = "once"
                [[events]]
                type = "mouse-button-down"
                button = "left"
                delay_ms = 25
                [[events]]
                type = "mouse-button-up"
                button = "left"
                delay_ms = 40
            "#,
        )
        .unwrap();
        assert!(PulsefireRaidRuntimeAssignment::macro_timeline(
            PulsefireRaidControl::Button5,
            mouse_clicks
        )
        .is_ok());

        let invalid = MacroDefinition {
            playback: MacroPlayback::Once,
            events: vec![MacroEvent::KeyDown {
                key: "unknown-key".to_owned(),
                delay_ms: 20,
            }],
        };
        assert!(PulsefireRaidRuntimeAssignment::macro_timeline(
            PulsefireRaidControl::Button5,
            invalid
        )
        .is_err());
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
