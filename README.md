# OpenHyperX

OpenHyperX is an experimental, open-source replacement for HyperX NGENUITY,
starting with the wired HyperX Pulsefire Raid on Windows 10/11 x64.

> **Experimental software:** discovery, report-descriptor reads and the
> capture-backed runtime-profile query are read-only; direct RGB is a confirmed
> but volatile hardware write. Runtime DPI and polling writes are
> capture-backed and do not save to onboard memory.
> Firmware update, bootloader and DFU operations are deliberately out of scope.
> No current command writes firmware or onboard profile data.

## Current status

- Windows-native HID enumeration and exact collection opening through `hidapi`
- macOS ARM64/x64 CI builds and CLI smoke tests (hardware support unverified)
- Pulsefire Raid recognition (`0951:16E4`)
- display of every HID collection, usage page, usage and device path
- optional `-v`, `-vv` and `--trace` diagnostics
- read-only `info` with raw HID descriptor, current polling rate, DPI stages
  and button bindings
- protocol-independent `HidTransport` plus `MockHidTransport` for packet tests
- volatile RGB with independent wheel/logo colors and foreground Solid, Cycle,
  Pulse, Breathing, Triggered Fade, Confetti, Sun and Twilight renderers
- capture-backed runtime DPI and stage management with per-stage colors, a
  200–16000 range, 50-DPI step and up to five contiguous stages
- capture-backed runtime polling get/set for 125, 250, 500 and 1000 Hz
- runtime listing of all 11 button mappings
- capture-backed runtime assignments on all nine non-primary controls for
  Disabled, Back, Volume Up/Down, DPI Toggle and named keyboard keys
- additional capture-backed Button 5 assignments for Forward, Copy and TOML
  Play Once macros with keyboard chords, per-event timings and
  left/right/middle mouse clicks
- raw hex capture parser/diff for protocol research
- offline NGENUITY version-40 `.hxp` inspection and partial import to a
  portable OpenHyperX TOML profile
- offline DPI-stage, polling and button-profile parser/patcher with golden tests
- no primary-click remapping or onboard hardware writes yet

The implementation stops wherever protocol evidence stops. Known facts and
their confidence level are recorded in [docs/research.md](docs/research.md).

## Workspace

```text
crates/
  hyperx-core/      platform-independent models and capabilities
  hyperx-hid/       HID discovery and transport boundary
  hyperx-protocol/  packet formatting, decoding and wire tracing
  hyperx-devices/   per-model descriptors and future drivers
  hyperx-cli/       command-line application
```

See [docs/architecture.md](docs/architecture.md) for dependency and safety
boundaries.

## Build and run on Windows from WSL

The commands below execute the Windows Rust toolchain and Windows binary; WSL
is only the shell and filesystem host.

```bash
powershell.exe -NoProfile -ExecutionPolicy Bypass \
  -File "$(wslpath -w "$PWD/scripts/check-windows.ps1")"

powershell.exe -NoProfile -ExecutionPolicy Bypass \
  -File "$(wslpath -w "$PWD/scripts/windows-cargo.ps1")" \
  run --target x86_64-pc-windows-msvc --bin hyperx-cli -- devices
```

Tracing can be enabled without changing normal output:

```bash
powershell.exe -NoProfile -ExecutionPolicy Bypass \
  -File "$(wslpath -w "$PWD/scripts/windows-cargo.ps1")" \
  run --target x86_64-pc-windows-msvc --bin hyperx-cli -- --trace devices
```

Read the standard HID report descriptor and the current runtime profile:

```bash
powershell.exe -NoProfile -ExecutionPolicy Bypass \
  -File "$(wslpath -w "$PWD/scripts/windows-cargo.ps1")" \
  run --target x86_64-pc-windows-msvc --bin hyperx-cli -- info --descriptor
```

`info` sends only the repeated, capture-backed runtime-profile read sequence;
it does not send the profile-write packet. Close NGENUITY first so two programs
do not access the configuration collection concurrently.

Read or change runtime DPI stages and polling rate:

```bash
powershell.exe -NoProfile -ExecutionPolicy Bypass \
  -File "$(wslpath -w "$PWD/scripts/windows-cargo.ps1")" \
  run --target x86_64-pc-windows-msvc --bin hyperx-cli -- dpi get

powershell.exe -NoProfile -ExecutionPolicy Bypass \
  -File "$(wslpath -w "$PWD/scripts/windows-cargo.ps1")" \
  run --target x86_64-pc-windows-msvc --bin hyperx-cli -- dpi set 800

powershell.exe -NoProfile -ExecutionPolicy Bypass \
  -File "$(wslpath -w "$PWD/scripts/windows-cargo.ps1")" \
  run --target x86_64-pc-windows-msvc --bin hyperx-cli -- \
  dpi stage set 2 --dpi 1600 --color CD00FF --active

powershell.exe -NoProfile -ExecutionPolicy Bypass \
  -File "$(wslpath -w "$PWD/scripts/windows-cargo.ps1")" \
  run --target x86_64-pc-windows-msvc --bin hyperx-cli -- \
  dpi stage add 16000 FFFFFF

powershell.exe -NoProfile -ExecutionPolicy Bypass \
  -File "$(wslpath -w "$PWD/scripts/windows-cargo.ps1")" \
  run --target x86_64-pc-windows-msvc --bin hyperx-cli -- \
  dpi stage remove-last

powershell.exe -NoProfile -ExecutionPolicy Bypass \
  -File "$(wslpath -w "$PWD/scripts/windows-cargo.ps1")" \
  run --target x86_64-pc-windows-msvc --bin hyperx-cli -- polling set 1000
```

These setters read the current runtime image, patch only confirmed fields and
write it back without invoking `Save to mouse`. All unrelated and unknown
profile bytes are preserved. Use a separate `get` to verify a changed value.

List all runtime button mappings or apply a typed, target-validated assignment:

```bash
powershell.exe -NoProfile -ExecutionPolicy Bypass \
  -File "$(wslpath -w "$PWD/scripts/windows-cargo.ps1")" \
  run --target x86_64-pc-windows-msvc --bin hyperx-cli -- buttons list

powershell.exe -NoProfile -ExecutionPolicy Bypass \
  -File "$(wslpath -w "$PWD/scripts/windows-cargo.ps1")" \
  run --target x86_64-pc-windows-msvc --bin hyperx-cli -- \
  buttons set button4 mouse back

powershell.exe -NoProfile -ExecutionPolicy Bypass \
  -File "$(wslpath -w "$PWD/scripts/windows-cargo.ps1")" \
  run --target x86_64-pc-windows-msvc --bin hyperx-cli -- \
  buttons set button5 mouse forward

powershell.exe -NoProfile -ExecutionPolicy Bypass \
  -File "$(wslpath -w "$PWD/scripts/windows-cargo.ps1")" \
  run --target x86_64-pc-windows-msvc --bin hyperx-cli -- \
  buttons set button7 multimedia volume-down

powershell.exe -NoProfile -ExecutionPolicy Bypass \
  -File "$(wslpath -w "$PWD/scripts/windows-cargo.ps1")" \
  run --target x86_64-pc-windows-msvc --bin hyperx-cli -- \
  buttons set dpi mouse dpi-toggle

powershell.exe -NoProfile -ExecutionPolicy Bypass \
  -File "$(wslpath -w "$PWD/scripts/windows-cargo.ps1")" \
  run --target x86_64-pc-windows-msvc --bin hyperx-cli -- \
  buttons set dpi keyboard b

powershell.exe -NoProfile -ExecutionPolicy Bypass \
  -File "$(wslpath -w "$PWD/scripts/windows-cargo.ps1")" \
  run --target x86_64-pc-windows-msvc --bin hyperx-cli -- \
  buttons set button5 macro examples/macros/ab-20ms.toml
```

The writable list is deliberately narrower than the mappings the decoder can
recognize. The nine non-primary controls accept Disabled, Mouse Back, DPI
Toggle, Volume Up/Down and named USB HID keyboard keys (`a-z`, digits,
navigation, F1-F24, keypad and modifiers). An isolated DPI-button A-to-B
capture proved that the key is the standard one-byte Keyboard/Keypad usage.
Button 5 additionally accepts its target-specific Forward, Copy and macro
captures. Left/right primary clicks remain read-only, and raw numeric usage
values are not accepted by the CLI. Macro files model playback plus an ordered
timeline of individual key/button down/up events and per-event delays,
including chords.
The current Raid encoder accepts Play Once macros of up to 14 balanced
transitions, common keyboard usages and the three captured primary mouse
buttons. Unsupported keys, playback modes and malformed timelines are rejected
before the mouse is opened. See [docs/macro-format.md](docs/macro-format.md).
These commands do not save onboard.

Inspect an exported NGENUITY preset or import every currently understood field
to a new OpenHyperX TOML profile:

```bash
cargo run --locked --bin hyperx-cli -- \
  profile inspect-ngenuity "/mnt/c/Users/you/Desktop/Base Settings.hxp"

cargo run --locked --bin hyperx-cli -- \
  profile import-ngenuity "/mnt/c/Users/you/Desktop/Base Settings.hxp" \
  base-settings.toml
```

These are platform-independent offline operations: they do not enumerate or
open HID devices. The importer currently supports NGENUITY preset format
version 40 and creates, rather than overwrites, its output. DPI stages and
confirmed macro event types are converted. Unknown physical button targets,
polling, lighting and the unconfirmed active-stage indexing are retained or
reported as partial instead of guessed. The resulting profile is not
automatically applied to the mouse and cannot invoke `Save to mouse`. See
[docs/profile-format.md](docs/profile-format.md).

Apply one volatile color to both zones, set wheel and logo independently, or
keep the direct colors active in the foreground for 30 seconds:

```bash
powershell.exe -NoProfile -ExecutionPolicy Bypass \
  -File "$(wslpath -w "$PWD/scripts/windows-cargo.ps1")" \
  run --target x86_64-pc-windows-msvc --bin hyperx-cli -- rgb static FF8000

powershell.exe -NoProfile -ExecutionPolicy Bypass \
  -File "$(wslpath -w "$PWD/scripts/windows-cargo.ps1")" \
  run --target x86_64-pc-windows-msvc --bin hyperx-cli -- \
  rgb static --wheel FF0000 --logo 0000FF

powershell.exe -NoProfile -ExecutionPolicy Bypass \
  -File "$(wslpath -w "$PWD/scripts/windows-cargo.ps1")" \
  run --target x86_64-pc-windows-msvc --bin hyperx-cli -- \
  rgb static --wheel off --logo 00FF00

powershell.exe -NoProfile -ExecutionPolicy Bypass \
  -File "$(wslpath -w "$PWD/scripts/windows-cargo.ps1")" \
  run --target x86_64-pc-windows-msvc --bin hyperx-cli -- rgb off --duration 30

powershell.exe -NoProfile -ExecutionPolicy Bypass \
  -File "$(wslpath -w "$PWD/scripts/windows-cargo.ps1")" \
  run --target x86_64-pc-windows-msvc --bin hyperx-cli -- \
  rgb cycle --target all --duration 30 --period 5 --logo-phase 180

powershell.exe -NoProfile -ExecutionPolicy Bypass \
  -File "$(wslpath -w "$PWD/scripts/windows-cargo.ps1")" \
  run --target x86_64-pc-windows-msvc --bin hyperx-cli -- \
  rgb play examples/lighting/independent-cycle-breathing.toml
```

When the positional color is omitted, an unspecified zone is black. Zone
options also accept `off`; when a positional color is present they override it.
Direct RGB does not write onboard memory. Close NGENUITY and other device/RGB
writers before using it. `rgb cycle` is rendered only while the CLI remains in
the foreground; it does not install a service. Once keepalive ends, the prior
lighting state may return. Advanced programs assign a different effect and
phase to each LED; see [docs/lighting.md](docs/lighting.md).

## Build on macOS

Install Rust and the Xcode Command Line Tools, then build and exercise the
platform-neutral CLI normally:

```bash
cargo test --workspace --all-targets --locked
cargo build --workspace --locked
./target/debug/hyperx-cli --version
./target/debug/hyperx-cli devices
```

The macOS backend opens HID devices with shared access. CI covers native ARM64
and Intel builds, but discovery and writes against a physical Pulsefire Raid
have not yet been validated on macOS.

`devices --all` also prints unsupported HID collections and can be very noisy.
Paths may contain machine-specific identifiers, so inspect trace logs before
publishing them.

## Roadmap

1. **Discovery (implemented):** enumerate and recognize Pulsefire Raid without
   opening its standard mouse collection.
2. **Safe device info (implemented for known fields):** standard descriptor and
   capture-backed runtime-profile reads expose polling, DPI stages/colors and
   button bindings.
3. **RGB proof of concept (implemented):** typed static/off direct reports and
   explicit foreground keepalive.
4. **Protocol exploration:** DPI stages/colors, polling runtime control,
   capture-backed Button 5 and DPI-button bindings plus bounded Play Once
   Button 5 macros are implemented; other controls, repeat modes, longer macros
   and onboard profiles remain capture-gated.
5. **GUI:** Tauri client using only the public core API.

See [docs/reverse-engineering.md](docs/reverse-engineering.md) for the capture
workflow and [docs/adding-device.md](docs/adding-device.md) for registry rules.
Repository-local skills split read-only `$discover-hyperx-device` discovery
from capture-backed `$add-hyperx-device` protocol and driver development. They
live in `.agents/skills/` and deliberately avoid repeating discovery during an
ongoing development session.

## License and prior art

OpenHyperX is MIT-licensed. OpenRGB is GPL-2.0-or-later; this repository does
not copy its source. Public OpenRGB code is used as attributed protocol
research. See [docs/research.md](docs/research.md) for exact revisions and
links.
