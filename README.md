# OpenHyperX

OpenHyperX is an experimental, open-source replacement for HyperX NGENUITY,
starting with the wired HyperX Pulsefire Raid on Windows 10/11 x64.

> **Device-safety suspension (2026-09-27):** the runtime selector
> `07 03 04 64` alone reproduced loss of mouse operation in the local session;
> USB unplug/replug restored operation. It is not a passive read-address selector.
> A cold Legacy launch then read an empty runtime image even after its two
> startup packets; Legacy supplied a populated image itself. Its image differs
> from the earlier onboard snapshot in 24 not-yet-explained body bytes, so
> copying the onboard profile is not an established recovery procedure.
> `info` now reads only identity and the standard HID descriptor, with no vendor
> reports. All CLI runtime DPI/polling/buttons access, composed apply (including
> dry-run), save-ACK probes and onboard save are blocked before HID discovery or
> opening. There is no unsafe override for these runtime paths. Offline tools,
> TUI and GUI remain available. Isolated lab probes require separate explicit
> consent and captures; they do not lift the runtime block (see
> [reverse-engineering workflow](docs/reverse-engineering.md)).
> The model driver independently blocks those runtime/profile methods before
> sending a report, so another client cannot bypass the CLI's early gate.
> Direct RGB uses a separate volatile write path and is not a recovery command.
> Firmware update, bootloader and DFU operations are deliberately out of scope.
> No current command writes firmware.

## Current status

- Windows-native HID enumeration and exact collection opening through `hidapi`
- macOS ARM64/x64 CI builds and CLI smoke tests (hardware support unverified)
- Pulsefire Raid recognition (`0951:16E4`)
- display of every HID collection, usage page, usage and device path
- optional `-v`, `-vv` and `--trace` diagnostics
- descriptor-only `info`, with no runtime-profile query or vendor reports
- protocol-independent `HidTransport` plus `MockHidTransport` for packet tests
- volatile RGB with independent wheel/logo colors and foreground Solid, Cycle,
  Pulse, Breathing, Triggered Fade, Confetti, Sun and Twilight renderers
- capture-backed runtime DPI and stage management with per-stage colors, a
  200–16000 range, 50-DPI step and up to five contiguous stages
- capture-backed runtime polling get/set for 125, 250, 500 and 1000 Hz
- runtime listing of all 11 button mappings
- capture-backed runtime assignments on all nine non-primary controls for all
  ten Mouse Functions, all seven Multimedia functions, all six Windows
  Shortcuts, Disabled and named keyboard keys
- capture-backed atomic standard/swapped layout for the two primary buttons,
  hardware-validated in both directions
- additional capture-backed Button 4 and Button 5 runtime TOML Play Once macros
  with keyboard chords, per-event timings and left/right/middle clicks;
  Button 4 `ab` and Button 5 examples were physically verified
- captured Toggle Repeat / Hold Repeat mode packets for runtime Button 4;
  automatic Windows packet/ACK/readback checks passed; physical repeat playback
  is not yet verified and onboard repeat is blocked
- raw hex capture parser/diff for protocol research
- offline NGENUITY Legacy version-40 `.hxp` inspection and partial import to a
  portable OpenHyperX TOML profile
- offline DPI-stage, polling and button-profile parser/patcher with golden tests
- software TOML profiles: offline validation, change-preview encoder and
  preservation-first runtime apply, with final full-image readback
  (whole-profile apply hardware validation is blocked; see profile docs)
- capture-backed, acknowledged onboard save for current DPI, polling, all 11
  button records, complete referenced Button 4/5 Play Once macros and independent
  wheel/logo Solid colors, hardware-verified across a physical power-cycle
- no non-Solid persistent firmware-lighting effects yet
- offline TUI and initial Tauri GUI sharing profile validation/editing operations

The implementation stops wherever protocol evidence stops. Known facts and
their confidence level are recorded in [docs/research.md](docs/research.md).
Runtime features above describe retained codecs, tests and historical hardware
evidence, not permission to use the currently suspended CLI paths. The query
itself sends `SET_REPORT`; absence of a profile-write packet does not establish
side-effect-free behavior on a cold device. Do not run an older executable's
`info` as a recovery diagnostic.

## Workspace

```text
crates/
  hyperx-app/       shared offline profile operations for all clients
  hyperx-core/      platform-independent models and capabilities
  hyperx-hid/       HID discovery and transport boundary
  hyperx-protocol/  packet formatting, decoding and wire tracing
  hyperx-devices/   per-model descriptors and future drivers
  hyperx-cli/       command-line application
  hyperx-tui/       offline terminal profile editor (Ratatui)
apps/
  hyperx-gui/       React/TypeScript frontend + optional native Tauri client
```

See [docs/architecture.md](docs/architecture.md) for dependency and safety
boundaries.

## Offline TUI

Rust 1.88+ is required. `hyperx-tui` uses Ratatui 0.30.2 with Crossterm 0.29.
It currently edits **files only**, not a live mouse. Run it in an interactive
terminal (Windows Terminal / PowerShell on Windows):

```powershell
& '.\target\x86_64-pc-windows-msvc\debug\hyperx-tui.exe' --demo
& '.\target\x86_64-pc-windows-msvc\debug\hyperx-tui.exe' '.\my-profile.toml'
```

For native Windows Cargo builds without an explicit target, binaries are under
`target\debug` instead. On Linux/macOS use `cargo run --bin hyperx-tui -- --demo`.
Demo settings are visibly labeled, never presented as detected device state.
No input file starts an empty, partial Raid-targeted draft, not default mouse
settings. The views are Performance, Buttons, Macros, Lighting and Profiles.

Click tabs and action buttons, drag DPI sliders or click a DPI number for exact
input. Performance provides stage add/remove, active selection, stage colors,
polling choices and the primary-button layout. Lighting has independent Solid
color fields for wheel/logo. These controls edit the offline draft without TOML.
Buttons has a clickable physical-control table and a target-aware binding picker,
including searchable keys and existing library macros; Enter/Accept commits the
selection to the file draft. Primary clicks remain a coupled layout.
Macros has a named library and interactive timeline editor: separate key/mouse
down/up events, individual delays, reorder controls and Once/Toggle/Hold selection.
Enter accepts one field; Ctrl+S accepts the local macro into the file draft,
with confirmation before replacing a referenced definition. F9 checks target
encoding readiness without accepting or sending the draft. No recorder is used.
Profiles has a file browser (F2), name field (F3) and Copy NEW workflow (F4).
Browse by mouse/keyboard, then choose a new `.toml` filename; existing files
are never overwritten. Copying retains the whole draft and leaves its source
file untouched. Bad files are rejected before asking to discard current edits.
F5 opens unresolved-source selectors: explicit source, legal target and real
library macro, then confirmation. Delete offers provenance-only omission,
never Disabled/reset; F1 displays complete rejection reasons. Try the synthetic
`examples/profiles/pulsefire-raid-unresolved.toml` without a connected mouse.
The wheel over a DPI bar adjusts one step; elsewhere it scrolls the view.
Use `Tab` or `1`–`5` to switch views, `e` to edit the current section's advanced TOML,
`a` for the complete document, `v` for offline validation and `d` for the file
diff. Inside the editor, `Ctrl+S` accepts the draft and `Esc` cancels. Outside
it, `s` saves to a **new** path, `o` opens a file, `m` imports a macro timeline,
`r` resolves an explicit unknown assignment and `x` deliberately omits one.
`q` exits, with confirmation for unsaved document changes. Unknown values stay
unknown; invalid-but-parseable drafts remain editable and are clearly NOT READY.
Save to mouse is unavailable; neither demo nor normal TUI opens/enumerates HID.

Headless executable checks require neither a terminal nor a mouse:

```text
hyperx-tui --demo --check
hyperx-tui --demo --render
hyperx-tui --demo --render --view macros
hyperx-tui --demo --render --view profiles
hyperx-tui my-profile.toml --render --width 120 --height 40
```

See [TUI controls and limits](docs/tui.md) and the [TUI backlog](docs/tui-todo.md).
Use `--view macros` in interactive mode to open the library directly.
Real-time device control remains blocked until hardware communication is stable.

## Offline GUI

The first Tauri GUI uses React/TypeScript and the same `hyperx-app` operations.
It has Device, Performance, Buttons, Macros, Lighting and Profiles views. DPI
stages/sliders/colors, polling, primary layout and independent Solid zone colors
edit files directly. Buttons also edits model-approved mouse/media/shortcut/key/
Disabled bindings and existing library macro references. Macros supports a named
library and editable down/up timelines with individual delays, reordering and
Once/Toggle/Hold playback; replacing referenced definitions requires confirmation.
In Buttons, click any of the 11 controls on the Raid's top/left-side render to
inspect its file binding. Selection is synchronized with the table and supports
Tab + Enter/Space; it neither edits the profile nor communicates with hardware.
Native file dialogs open TOML or save to a **new** file, with validation and diff.
No USB is opened. Apply and Save to mouse are unavailable, not merely hidden.

After the [GUI build steps](docs/gui.md), launch on Windows:

```powershell
& '.\target\x86_64-pc-windows-msvc\debug\hyperx-gui.exe' --demo
```

Normal launch creates an empty partial draft, not assumed factory defaults.
GUI builds target Windows, Linux and macOS (Apple Silicon/Intel), using the same
`src-tauri` backend. See the [macOS](docs/gui.md#build-on-macos) and
[Linux build steps](docs/gui.md#build-on-linux).
GUI requires Rust 1.90+ and Node 24 LTS for building; Windows uses WebView2.
A built executable embeds assets and needs no Node/background server.
`npm run dev` in `apps/hyperx-gui` provides a clearly labeled read-only preview.

The GUI includes the NGENUITY Legacy Pulsefire Raid product render, with top/side
views and file-color markers. This manufacturer-owned PNG is **not MIT**; see
[third-party asset provenance and rights status](THIRD_PARTY_ASSETS.md).
Original code and SVG artwork remain MIT. No installed NGENUITY is needed.

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

Read identity and the standard HID report descriptor, without vendor reports:

```bash
powershell.exe -NoProfile -ExecutionPolicy Bypass \
  -File "$(wslpath -w "$PWD/scripts/windows-cargo.ps1")" \
  run --target x86_64-pc-windows-msvc --bin hyperx-cli -- info --descriptor
```

`info` no longer sends the captured runtime query, and does not report current
polling, DPI or button settings. Close every NGENUITY variant first so two
programs do not access the configuration collection concurrently.

Retained runtime DPI/polling command syntax (currently blocked before HID I/O):

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
  buttons primary-layout swapped

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
  buttons set button4 multimedia play-pause

powershell.exe -NoProfile -ExecutionPolicy Bypass \
  -File "$(wslpath -w "$PWD/scripts/windows-cargo.ps1")" \
  run --target x86_64-pc-windows-msvc --bin hyperx-cli -- \
  buttons set button4 windows-shortcut cycle-apps

powershell.exe -NoProfile -ExecutionPolicy Bypass \
  -File "$(wslpath -w "$PWD/scripts/windows-cargo.ps1")" \
  run --target x86_64-pc-windows-msvc --bin hyperx-cli -- \
  buttons set dpi mouse dpi-toggle

powershell.exe -NoProfile -ExecutionPolicy Bypass \
  -File "$(wslpath -w "$PWD/scripts/windows-cargo.ps1")" \
  run --target x86_64-pc-windows-msvc --bin hyperx-cli -- \
  buttons set button4 mouse scroll-up

powershell.exe -NoProfile -ExecutionPolicy Bypass \
  -File "$(wslpath -w "$PWD/scripts/windows-cargo.ps1")" \
  run --target x86_64-pc-windows-msvc --bin hyperx-cli -- \
  buttons set dpi keyboard b

powershell.exe -NoProfile -ExecutionPolicy Bypass \
  -File "$(wslpath -w "$PWD/scripts/windows-cargo.ps1")" \
  run --target x86_64-pc-windows-msvc --bin hyperx-cli -- \
  buttons set button4 macro examples/macros/ab-20ms.toml

powershell.exe -NoProfile -ExecutionPolicy Bypass \
  -File "$(wslpath -w "$PWD/scripts/windows-cargo.ps1")" \
  run --target x86_64-pc-windows-msvc --bin hyperx-cli -- \
  buttons set button5 macro examples/macros/ab-20ms.toml
```

The writable list is deliberately narrower than the mappings the decoder can
recognize. The nine non-primary controls accept Disabled; Left Click, Right
Click, Middle, Back, Forward, Tilt Left/Right, DPI Toggle and Scroll Up/Down;
Play/Pause, Stop, Next, Previous, Mute Volume and Volume Up/Down; and named USB
HID keyboard keys (`a-z`, digits, navigation, F1-F24, keypad and modifiers);
and Cycle Apps, Switch Apps, Cut, Copy, Paste and Undo. An isolated DPI-button
A-to-B capture proved that the key is the standard one-byte Keyboard/Keypad
usage. Button 4 and Button 5 additionally accept capture-backed runtime macro
timelines.
The physical primary controls are changed only as the captured atomic pair:
`buttons primary-layout standard` or `buttons primary-layout swapped`.
Independent primary writes and raw numeric usage values are not accepted by
the CLI. Macro files model playback plus an ordered timeline of individual
key/button down/up events and per-event delays, including chords.
The current Raid encoder accepts Play Once macros on Button 4/5, and captured
Toggle/Hold Repeat modes only on runtime Button 4, with up to 14 balanced
transitions, common keyboard usages and the three captured primary mouse
buttons. Unsupported targets/modes, keys and malformed timelines are rejected
before the mouse is opened. See [docs/macro-format.md](docs/macro-format.md).
These commands do not save onboard.

Inspect implemented macro support or validate a macro file entirely offline:

```text
hyperx-cli buttons capabilities
hyperx-cli buttons validate-macro button4 examples/macros/ab-toggle-20ms.toml
hyperx-cli buttons validate-macro button5 examples/macros/coverage-recorded-timing.toml --onboard
```

These checks do not open HID or change the mouse. `--onboard` checks encoding
support only; it does not perform a save or read the currently assigned macro.

Validate and preview an OpenHyperX software profile before applying it.
Compare two OpenHyperX TOML files entirely offline:

```text
hyperx-cli profile diff BEFORE.toml AFTER.toml
hyperx-cli profile inspect PROFILE.toml
hyperx-cli profile inspect-capture captures/runtime.log
hyperx-cli profile inspect-capture captures/reports.hex --raw --all-raw
hyperx-cli profile export-capture captures/runtime.log captured.toml --report 3
```

Diff separates settings from metadata, includes macro playback, ordered events
and every delay, and treats omitted fields as absent rather than device resets.
It is a file comparison, not an apply plan or device-support validation.
Capture inspection accepts UTF-8 hex reports or OpenHyperX `--trace` text logs,
decodes known Raid packet families, flags empty snapshots and reports raw image
changes. Unknown packets remain uninterpreted. It never opens HID, replays
packets, changes files or parses binary `.pcapng`. See
[offline tooling](docs/profile-format.md#offline-comparison-and-capture-inspection).

`inspect` summarizes supplied settings, omissions, unresolved assignments and
macro timelines, with a separate offline device-readiness result. It can inspect
files that cannot be applied; use `validate` for an exit-status readiness gate.
`export-capture` creates a new, explicitly partial TOML from the selected
one-based report number shown by `inspect-capture`, never a guessed latest image.
Only a complete, usable runtime RX image is accepted; host writes, onboard
images, unknown directions and empty/malformed snapshots are refused. DPI,
polling and confirmed mappings are exported; lighting and macro timelines are
not inferred. Macro references become unresolved entries that block apply.
Existing destination files are never overwritten. Export never applies settings
or saves onboard, and a historical snapshot is not the mouse's current state.

Whole-profile apply is not yet hardware-validated: its first polling-only test
stopped on an empty runtime readback, and subsequent reads remain unusable.
After reconnect, another query returned an empty image and was followed by
physical loss of cursor, clicks and lighting. Even the query is now suspended;
do not retry it to obtain a baseline. The older individual-operation evidence
does not establish harmless access to a cold device. See docs/research.md.

```text
hyperx-cli profile validate examples/profiles/pulsefire-raid.toml
hyperx-cli profile apply examples/profiles/pulsefire-raid.toml --dry-run
hyperx-cli profile apply examples/profiles/pulsefire-raid.toml --lighting-duration 30
```

The example changes DPI, bindings and lighting: edit it to your preferences
first. Close NGENUITY/OpenRGB before accessing the mouse. Validation is offline.
Dry-run and apply currently stop before opening HID. The retained apply uses
confirmed runtime operations only, preserving omissions and unknown bytes. It stops on
error without retries/rollback; earlier changes may remain. Solid RGB needs
foreground keepalive and reverts afterward. **Apply never saves onboard.** See
[docs/profile-format.md](docs/profile-format.md) for fields, macro references,
partial-import restrictions and an opt-in Windows polling-only hardware test.

Check the acknowledgement path before saving to the mouse:

```bash
powershell.exe -NoProfile -ExecutionPolicy Bypass \
  -File "$(wslpath -w "$PWD/scripts/windows-cargo.ps1")" \
  run --target x86_64-pc-windows-msvc --bin hyperx-cli -- \
  profile check-save-ack
```

`check-save-ack` sends only the confirmed, non-persistent runtime-section
selector and verifies its interrupt-IN acknowledgement. It does not select or
write onboard memory. Close NGENUITY and other device writers before running
it; if it fails, do not blindly retry a save.

The optional `profile check-save-ack --initialize-session` explicitly tests the
fixed, capture-backed volatile Legacy startup sequence before that probe. It
does not replay Legacy's automatic profile writes or save onboard settings.
Hardware checks after a fresh USB reconnect and in an already-active session
passed and preserved the complete runtime profile. Close Legacy and other
writers first. `save-to-mouse` now performs this verified fixed startup once
before reading/validating runtime settings or selecting onboard memory. It
never retries a failed ACK.

To perform the persistent save after the acknowledgement path works:

```bash
powershell.exe -NoProfile -ExecutionPolicy Bypass \
  -File "$(wslpath -w "$PWD/scripts/windows-cargo.ps1")" \
  run --target x86_64-pc-windows-msvc --bin hyperx-cli -- \
  profile save-to-mouse \
  --wheel off --logo 0000FF \
  --macro-definition examples/macros/coverage-recorded-timing.toml \
  --confirm
```

When both Button 4 and Button 5 reference macros, supply both complete timelines:

```text
hyperx-cli profile save-to-mouse --wheel off --logo 0000FF \
  --macro-definition button4=examples/macros/ab-20ms.toml \
  --macro-definition button5=examples/macros/coverage-recorded-timing.toml \
  --confirm
```

`--macro-definition` is repeatable and accepts only captured Button 4/5 targets.
A bare file path remains a Button 5 alias for backwards compatibility. Every
referenced slot requires its exact definition; duplicate targets, invalid
timelines, missing definitions and definitions for ordinary runtime bindings
are rejected. Supply the macro currently assigned to that button, not an
unrelated file: the mouse profile contains only references, so the CLI cannot
verify its event stream against the device. The save command does not assign
new runtime macros by itself.

Close every NGENUITY variant and other device writers first. The command reads
and validates the runtime profile, reads the existing onboard image, copies
only confirmed DPI, polling and button fields, checks every eight-byte device
acknowledgement through the mouse's separate acknowledgement collection, and
restores the runtime section after the commit delay. Every referenced Button
4/5 macro requires its complete definition as described above. OpenHyperX's
two-slot save was captured, independently read back and physically power-cycle
tested with Legacy closed; Button 4 AB and its restoration to Back persisted.

Both `--wheel` and `--logo` are mandatory (`off` means black). They describe
the two-zone static snapshot carried by NGENUITY Legacy's save transaction.
Independent wheel/logo Solid colors, including `off`, were verified to persist
across a physical power-cycle. This does not establish persistence for any
other lighting effect. The `--confirm` flag is mandatory because this operation
writes onboard memory.

Inspect an exported NGENUITY Legacy preset or import every currently understood
field to a new OpenHyperX TOML profile:

```bash
cargo run --locked --bin hyperx-cli -- \
  profile inspect-ngenuity-legacy "/mnt/c/Users/you/Desktop/Base Settings.hxp"

cargo run --locked --bin hyperx-cli -- \
  profile diff-ngenuity-legacy before.hxp after.hxp

cargo run --locked --bin hyperx-cli -- \
  profile import-ngenuity-legacy "/mnt/c/Users/you/Desktop/Base Settings.hxp" \
  base-settings.toml
```

These are platform-independent offline operations: they do not enumerate or
open HID devices. The importer currently supports only the NGENUITY Legacy
preset format version 40 observed in Microsoft Store version `5.38.0.0`; it
does not claim compatibility with current NGENUITY. It creates, rather than
overwrites, its output. DPI stages and confirmed macro event types are
converted. Unknown physical button targets, polling, lighting and the
unconfirmed active-stage indexing are retained or reported as partial instead
of guessed. The resulting profile is not automatically applied to the mouse
and does not automatically invoke `Save to mouse`. See
[docs/profile-format.md](docs/profile-format.md).

`diff-ngenuity-legacy` compares decoded settings first and then prints exact
offsets for changes in the embedded binary preset. Export-wrapper/footer
differences are excluded. Raw output is limited to 256 changed bytes unless
`--all-raw` is used, and may include regenerated source identifiers.

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
Direct RGB does not write onboard memory. Close every NGENUITY variant and
other device/RGB writers before using it. `rgb cycle` is rendered only while
the CLI remains in the foreground; it does not install a service. Once
keepalive ends, the prior lighting state may return. Advanced programs assign
a different effect and phase to each LED; see [docs/lighting.md](docs/lighting.md).

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
   all Mouse/Multimedia functions and Windows Shortcuts on the nine general
   controls, bounded Play Once Button 4/5 runtime macros and the confirmed onboard-save
   path plus atomic primary-click swaps are implemented; repeat modes, longer
   macros and non-Solid firmware lighting remain capture-gated.
5. **Software profiles (first iteration implemented):** offline validation,
   current-state preview and experimental runtime apply (currently blocked on
   an empty native readback); onboard persistence stays
   separate and explicit.
6. **TUI (offline implemented):** shared application-layer profile editing,
   validation, diff and explicit unresolved-assignment handling. Hardware controls
   remain unavailable until confirmed separately.
7. **GUI:** offline Tauri client, binding selectors and macro timeline/library
   editing exist; native operator checks and later evidence-gated connected mode remain.

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
