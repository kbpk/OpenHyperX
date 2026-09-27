# Tauri GUI — first offline iteration

The native Windows GUI is an **offline profile editor**, not a live-device
controller. It never discovers HID devices, opens a mouse, sends reports or runs
RGB keepalive. NGENUITY does not need to be closed for file editing. No background
service, firmware updater, shell plugin or global keyboard hook is included.

## Available

- Device: model capabilities, explicitly not connected-device telemetry.
- Performance: DPI sliders/exact numeric input, stage colors, add/remove-last,
  explicit active selection and polling choices. Editing DPI links X/Y;
  asymmetric imported values remain visible until explicitly edited.
- Buttons: clickable top/left-side maps for all 11 Raid controls, synchronized
  with the assignment table and selected-control details. Tab + Enter/Space
  selects controls too; selection/hover never changes the document or sends IPC.
  Editable coupled primary-button layout; general binding selectors are not yet
  implemented.
- Macros: complete ordered event timelines, playback and individual delays;
  read-only in this iteration, without collapsing chords.
- Lighting: independent wheel/logo Solid colors, palette and Off per zone.
- Profiles: naming, native TOML open/save dialogs, validation and baseline diff.

The Raid product render uses an unchanged three-view NGENUITY Legacy atlas;
Device selects Top/Left/Right locally. Buttons overlays separately maintained
presentation geometry, filtered against the model's declared controls. Positions
and side-button identities follow the manufacturer's
[user guide, English Overview](https://media.kingston.com/support/downloads/HyperX-Pulsefire-Raid-User-guide.pdf)
(PDF page 4), not assumed current factory bindings. Wheel arrows select tilt
motions; the wheel center selects its press. Unknown models/missing assets remain
selectable in the table without invented geometry. Binding details come from the
file, never from the manufacturer image or a live device read.
Small zone markers show only colors supplied by the file, not simulated LED
pixels or live device state. Unknown models or missing assets show a text
placeholder, not a substitute mouse drawing. The manufacturer PNG is excluded from the project's MIT
license; [provenance/rights status](../THIRD_PARTY_ASSETS.md) is retained separately.
No NGENUITY installation or process is needed to display the bundled asset.

Missing values remain unknown, not factory defaults. The normal start creates
an empty partial draft; `--demo` loads the visibly labeled bundled example.
The browser preview is **read-only**, using the same bundled Rust snapshot.
It has no substitute JavaScript profile encoder or device implementation.

Save creates a **new** file using the existing app-layer `create_new` writer;
it never overwrites existing files, even after a native overwrite confirmation.
Save may preserve an invalid/partial draft and does not grant hardware readiness.
Failed edits, loads and saves preserve the last successful document/baseline.
Replacing a dirty document and closing a dirty native window require confirmation.

## Build on Windows from WSL

Requirements: Rust **1.90+** for `hyperx-gui`, Node **24 LTS** for the frontend,
the Windows MSVC C++ toolchain, and WebView2 to run the window. Other workspace
crates retain Rust 1.88. See [Tauri prerequisites](https://v2.tauri.app/start/prerequisites/).

Format/lint and frontend build in WSL, with Node LTS selected (e.g. `nvm use 24`):

```bash
cd apps/hyperx-gui
npm ci
npm run format:check
npm run typecheck
npm test
npm run build
cd ../..
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --all-targets --locked
```

Then native Windows tests and build, **without discovery or hardware operations**:

```bash
powershell.exe -NoProfile -ExecutionPolicy Bypass \
  -File "$(wslpath -w "$PWD/scripts/check-windows.ps1")" \
  -SkipDeviceDiscovery -Gui
```

Launch in PowerShell from the repository directory:

```powershell
& '.\target\x86_64-pc-windows-msvc\debug\hyperx-gui.exe' --demo
```

Omit `--demo` for an empty file draft. Use the Open profile button for TOML.
`--smoke-test` exercises the real executable without a window, dialogs or USB.
`custom-protocol` embeds `dist` into the EXE, so no Node/Vite process is needed
to run it. This iteration has no MSI installer or signing/release workflow.

For development on a Windows checkout with Windows Node/Rust available:

```powershell
cd apps/hyperx-gui
npm ci
npm run tauri -- dev -- --demo
# production executable, no installer:
npm run tauri -- build --no-bundle
```

## Build on macOS

The same frontend and `src-tauri` backend build natively on Apple Silicon and
Intel. Install Rust 1.90+, Node 24 LTS and the
[Apple desktop prerequisites](https://v2.tauri.app/start/prerequisites/#macos)
(Xcode or Xcode Command Line Tools). Run on a Mac, from the repository root:

```bash
cd apps/hyperx-gui
npm ci
npm run build
cd ../..
cargo test --workspace --all-targets --locked --features hyperx-gui/desktop,hyperx-gui/custom-protocol
cargo build --workspace --locked --features hyperx-gui/desktop,hyperx-gui/custom-protocol
./target/debug/hyperx-gui --smoke-test  # no window, dialogs or HID
./target/debug/hyperx-gui --demo       # native offline window
```

For development, run `npm run tauri -- dev -- --demo` in `apps/hyperx-gui`.
CI uses native `macos-latest` (arm64) and `macos-15-intel` runners, builds the
embedded frontend and checks the real GUI executable headlessly. It does not
exercise native dialogs or the rendered WebKit window. App icons include PNG,
ICO and ICNS derived from the original OpenHyperX mark, not a HyperX asset.

This is an unbundled native executable, not a signed/notarized `.app` or DMG.
Packaging/signing and operator window tests remain separate work. macOS builds
do not establish hardware-protocol verification; the GUI remains offline.

## Preview and tests

```bash
cd apps/hyperx-gui
npm run dev                    # read-only browser preview on localhost:1420
npx playwright install chromium
npm run test:e2e               # preview, navigation, modal and minimum-width checks
```

Rust facade tests run on Linux/macOS without GTK/WebKit packages: `desktop` is
opt-in. Native desktop CI builds/tests target Windows and macOS arm64/Intel.
Linux desktop builds and operator GUI validation are not yet verified.
`cargo build --workspace` alone does not enable the desktop GUI binary.

Frontend tests cover typed edits, slider release, validation, safe save intent,
dirty-document replacement, preview restrictions and serialized IPC revisions.
Rust tests cover the real shared validation, preservation of unknown values,
failed actions, safe new-file writes and the demo JSON contract. Browser tests
do not replace an operator test of native Windows dialogs/WebView2.

## Boundary

`apps/hyperx-gui/src-tauri` is maintained source, not generated scaffolding to
discard. Commit its Rust sources, `Cargo.toml`, `build.rs`, `tauri.conf.json`,
`capabilities` and application icon/source provenance. The workspace-level
`Cargo.lock` and frontend `package-lock.json` are committed too. Generated
`src-tauri/gen` schemas, autogenerated permissions, `target`, `dist` and
`node_modules` are ignored; they are recreated during install/build.

React → five narrow local-window IPC commands → `Session` → `hyperx-app`.
UI requests use a typed edit plus the expected document revision. The backend
rechecks revisions after native dialogs; stale completion cannot overwrite a
newer document. The client serializes requests without automatic retry.

The local main-window capability grants only snapshot/edit/reset/open/save
commands and Tauri core defaults. Native dialogs select paths in Rust; the
frontend cannot supply arbitrary filesystem paths. No JS filesystem/dialog,
shell, updater or hardware permissions are granted. Production CSP allows no
external scripts, assets or connections.

Sources: [Rust IPC](https://v2.tauri.app/develop/calling-rust/),
[native dialogs](https://v2.tauri.app/plugin/dialog/),
[capabilities](https://v2.tauri.app/security/capabilities/).

## Next

- [ ] General binding selectors, legal choices from shared model metadata.
- [ ] Editable macro timeline with target-specific limits and library management.
- [ ] Partial/unresolved import inspection and explicit resolution/omission controls.
- [ ] Keyboard navigation/operator tests of native dialogs and WebView2.
- [ ] Undo/recovery and a deliberate recoverable overwrite workflow.
- [ ] Connected app facade **only after** usable hardware baseline/readback is restored.
- [ ] Separate Preview, Apply and confirmed Save to mouse; no reports in React.
- [ ] Installer, signing and releases after native behavior is validated.

TUI remains supported: its separate backlog is in [tui-todo.md](tui-todo.md).
