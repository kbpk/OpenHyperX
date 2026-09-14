# OpenHyperX

OpenHyperX is an experimental, open-source replacement for HyperX NGENUITY,
starting with the wired HyperX Pulsefire Raid on Windows 10/11 x64.

> **Experimental software:** discovery, report-descriptor reads and the
> capture-backed runtime-profile query are read-only; direct RGB is a confirmed
> but volatile hardware write.
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
- volatile static/off RGB for wheel and logo with optional foreground keepalive
- raw hex capture parser/diff for protocol research
- offline DPI-stage, polling and button-profile parser/patcher with golden tests
- no DPI/polling/button/onboard hardware writes yet

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

Apply volatile RGB once, or keep it active in the foreground for 30 seconds:

```bash
powershell.exe -NoProfile -ExecutionPolicy Bypass \
  -File "$(wslpath -w "$PWD/scripts/windows-cargo.ps1")" \
  run --target x86_64-pc-windows-msvc --bin hyperx-cli -- rgb static FF8000

powershell.exe -NoProfile -ExecutionPolicy Bypass \
  -File "$(wslpath -w "$PWD/scripts/windows-cargo.ps1")" \
  run --target x86_64-pc-windows-msvc --bin hyperx-cli -- rgb off --duration 30
```

Direct RGB does not write onboard memory. Close NGENUITY and other device/RGB
writers before using it. Once keepalive ends, the stored effect may return.

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
4. **Protocol exploration:** DPI, polling rate, bindings and onboard profiles,
   one capture-backed operation at a time.
5. **GUI:** Tauri client using only the public core API.

See [docs/reverse-engineering.md](docs/reverse-engineering.md) for the capture
workflow and [docs/adding-device.md](docs/adding-device.md) for registry rules.
The repository-local `$add-hyperx-device` skill in
`.agents/skills/add-hyperx-device` enforces this workflow for future devices.

## License and prior art

OpenHyperX is MIT-licensed. OpenRGB is GPL-2.0-or-later; this repository does
not copy its source. Public OpenRGB code is used as attributed protocol
research. See [docs/research.md](docs/research.md) for exact revisions and
links.
