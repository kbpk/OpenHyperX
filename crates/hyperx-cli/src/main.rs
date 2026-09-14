use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
    thread,
    time::{Duration, Instant},
};

use anyhow::{anyhow, Context, Result};
use clap::{ArgAction, Parser, Subcommand};
use hyperx_core::{
    ButtonBinding, DeviceDescriptor, HidInterfaceInfo, MouseFunction, MultimediaFunction, RgbColor,
    UsbId, WindowsShortcut,
};
use hyperx_devices::{find_supported_device, PulsefireRaid, PULSEFIRE_RAID};
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
        Command::DecodeCapture { before, after } => decode_capture(&before, &after),
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
        Ok(dpi_profile) => {
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
        Err(error) => println!("    <decode error: {error}>"),
    }

    println!("  Buttons:");
    for control in PulsefireRaidControl::ALL {
        if profile.has_confirmed_macro_reference(control) {
            println!(
                "    {:<17} Macro reference (definition is not present in the runtime profile)",
                format!("{}:", control.name())
            );
            continue;
        }
        match profile.button_binding(control) {
            Ok(binding) => println!(
                "    {:<17} {}",
                format!("{}:", control.name()),
                format_button_binding(&binding)
            ),
            Err(error) => println!(
                "    {:<17} <decode error: {error}>",
                format!("{}:", control.name())
            ),
        }
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

fn rgb(color: RgbColor, duration_seconds: u64) -> Result<()> {
    if duration_seconds > 3600 {
        return Err(anyhow!("--duration cannot exceed 3600 seconds"));
    }

    let discovery = HidApiDiscovery;
    let interfaces = discovery.enumerate()?;
    let configuration = select_configuration_interface(&interfaces)?;
    let transport = HidApiTransport::open(configuration).with_context(|| {
        format!(
            "failed to open the Pulsefire Raid configuration collection; close NGENUITY and retry: {}",
            configuration.path
        )
    })?;
    let mut device = PulsefireRaid::new(transport)?;
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
