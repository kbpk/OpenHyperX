# Tauri GUI — first offline iteration

The native GUI is an **offline profile editor**, not a live-device
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
  Editable coupled primary-button layout, Mouse/Multimedia/Windows Shortcut/
  Disabled assignments, searchable named keyboard keys and existing library
  macros, with per-target choices/validation from Rust.
- Macros: named-library creation/rename/deletion and editable ordered down/up
  timelines with individual delays, add/remove/reorder and Once/Toggle/Hold
  playback, without collapsing chords. Referenced replacements need confirmation.
- Lighting: independent wheel/logo Solid colors, palette and Off per zone.
- Profiles: naming, native TOML open/Save NEW dialogs, explicitly confirmed
  recoverable FILE overwrite, validation and baseline diff.
- Imported provenance: inspect unresolved source IDs, explicitly choose a legal
  physical target and existing library macro, or deliberately omit one source
  entry. A separate confirmation precedes either file edit; no source hint
  automatically selects a macro or button.

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

Save new file uses the app-layer `create_new` writer and never overwrites.
Profiles also offers a separate **Overwrite opened FILE** action only for a
dirty, previously opened/saved document. Its own review dialog names the
exact path; the native backend requires explicit confirmation and never accepts
an arbitrary frontend path. The shared app layer refuses symlinked, read-only,
non-regular or externally changed files, writes a complete replacement
snapshot, then moves the old bytes into a numbered sibling recovery directory
before installing the new file. The exact recovery path remains visible in
Profiles after success. A crash/I/O failure may leave the original path absent
or incomplete; inspect the recovery directory before retrying. This action is
FILE-only and never Save to mouse. Either save may preserve an invalid/partial
draft and does not grant hardware readiness.

Profiles also has Undo/Redo file edit controls backed by the shared bounded
32-snapshot document history. They restore complete typed drafts, including
partial/import provenance, and keep the opened/saved FILE baseline for dirty
and diff calculations. They are revision-checked, disabled while a local macro
timeline is pending and do not write disk or USB. A new edit after undo clears
the redo branch; history is in memory and is lost when the app exits.
Failed edits, loads and saves preserve the last successful document/baseline.
Replacing a dirty document and closing a dirty native window require confirmation.

The native GUI now snapshots each accepted unsaved FILE-document edit in its
private `%LOCALAPPDATA%\OpenHyperX\draft-recovery` directory on Windows (the
platform app-state directory elsewhere). A new complete snapshot is installed
before the previous one is retired. On restart, older snapshots appear in a
recovery notice with their profile names; **Restore offline draft** is available
only while the current document is clean and no macro timeline is pending.
Each listed snapshot also has a separate **Discard recovery snapshot** action
with a confirmation dialog; the backend refuses to delete the current
session's active snapshot or a path outside its private directory. Discard is
permanent and affects no ordinary profile or device. Restoration preserves the
original FILE baseline, so a later overwrite still
rejects files changed on disk. Save NEW or a confirmed FILE overwrite retires
the current snapshot. Corrupt snapshots remain visible and are not silently
deleted. This is not a device backup, an onboard save or a USB operation.
The separate local macro-timeline editor also stores a best-effort, size-bounded
snapshot in the desktop WebView's local storage after each change. After restart,
Macros offers **Restore macro timeline** or **Discard saved timeline** explicitly;
restoration requires the matching FILE path and unchanged original macro. Open
the original file (or restore its FILE draft) first if the editor says the
current document does not match. A recovered timeline is still not a FILE edit
until **Add macro to file** or **Update file macro** is pressed. Invalid local
snapshots are never silently overwritten. The desktop WebView also keeps
bounded, best-effort snapshots of unfinished profile-name, DPI-stage number,
stage-color and zone-color inputs. A restart requires an explicit per-field
**Restore** or **Discard**; restore is offered only for the same FILE path,
target model, profile name and original field value. A restored value is still
local input until confirmed in its editor. Save, replace, undo and native close
are guarded while these fields are unfinished. An unreadable snapshot is not
silently overwritten. This local storage is not a portable profile backup and
may be unavailable or cleared by the WebView. Transient selector searches,
unsubmitted binding choices and the Add stage dialog are not recoverable;
they are not FILE edits. The local-input snapshot is separate from accepted
FILE-document recovery.

## Editing bindings

Select a non-primary control on the graphic or table. Choose an assignment
category and an explicit action/key/macro, then press **Update file binding**.
Browsing categories and filtering keys are local UI state, not document edits.
The updated draft still needs Save new file for disk persistence; neither action
sends USB. Current aliases/imported unsupported assignments remain visible and
unchanged until explicitly replaced. Failed edits retain the draft and picker.

Not specified removes only that control from `[buttons]`: it means preserve an
omitted device setting on a future apply, not Disabled or a factory reset.
Disabled is a separate explicit binding. Left/right are edited only through the
coupled Standard/Swapped primary layout, never two independent assignments.

Choices come from shared `hyperx-app` metadata. Macro definitions are validated
against the selected target; missing/duplicate IDs and rejected timelines/modes
cannot be newly assigned. Rejected library entries stay visible with reasons.
Editing one binding preserves all other settings, macros, partial flags and
unresolved source provenance, even if those other fields still fail readiness.
Runtime encoding support does not imply onboard support or physical verification.

## Editing macros

Macros edits a local timeline draft until **Add macro to file** or **Update file
macro** is explicitly pressed. Choosing keys, changing playback/delays and
moving events do not submit IPC edits or send USB. Separate modifier/key down/up
rows express chords; delay is the interval *after* each row. Keyboard inputs and
left/right/middle mouse inputs come from Rust metadata, not HID/vendor records.
Imported aliases, unsupported inputs and long timelines remain visible without
automatic normalization or truncation.

An edited library entry keeps its opaque ID. Replacement requires explicit
confirmation if physical or unresolved source assignments reference it.
Referenced entries cannot be deleted; remove their assignments explicitly first.
New entries must have fresh IDs and never silently satisfy unresolved imports.
Library edits preserve other sections, partial flags and import provenance.

The file's event-delay range is 0–65535 ms; this is **not** a hardware limit.
The currently confirmed Raid encoder limit is 14 transitions and 0–9999 ms,
with target-specific runtime/onboard playback support shown separately.
Incomplete/unbalanced drafts can be retained in the file, but cannot be newly
assigned through the binding picker. Committing/saving a draft never grants
permission to apply it. No global recorder or keyboard hook is implemented.

Pending local timeline edits survive page navigation, prevent replacing/saving
the document until committed or discarded, and protect native window close.
The desktop GUI also offers explicit local-timeline recovery after a restart;
restoration never commits the macro automatically. Browser preview does not
persist these editor snapshots.
After updating the file draft, use Save new file for disk persistence.

## Resolving imported assignments

Profiles lists unresolved source entries from the file. Selecting a source
shows the retained macro-source hint, but never treats that hint as a library
ID or a physical control. Choose a model-declared target and an existing,
target-valid library macro explicitly. Rejected targets/definitions remain
inspectable with reasons from `hyperx-app`. Duplicate source IDs cannot be
resolved by picking a row number; the shared model rejects ambiguity.

**Review resolution** shows the exact source, control and macro before
**Confirm resolution** assigns the macro in the FILE and removes only that
unresolved entry. **Review omission** is separate: confirming it deletes only
one provenance entry, not a binding or macro, and never means Disabled/reset.
Both actions are revision-checked Rust edits and preserve unrelated settings,
partial flags and source metadata. A pending local macro timeline disables
resolution until committed or discarded. Save new file afterward; neither
action opens HID, applies settings or saves onboard memory.

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

## Build on Linux

Install Rust 1.90+, Node 24 LTS and the
[Linux desktop prerequisites](https://v2.tauri.app/start/prerequisites/#linux).
On Debian/Ubuntu, including WSL Ubuntu:

```bash
sudo apt-get update
sudo apt-get install --yes libudev-dev libwebkit2gtk-4.1-dev build-essential libxdo-dev libssl-dev libayatana-appindicator3-dev librsvg2-dev
cd apps/hyperx-gui
npm ci
npm run build
cd ../..
cargo test --workspace --all-targets --locked --features hyperx-gui/desktop,hyperx-gui/custom-protocol
cargo build --workspace --locked --features hyperx-gui/desktop,hyperx-gui/custom-protocol
./target/debug/hyperx-gui --smoke-test  # no display, window, dialogs or HID
./target/debug/hyperx-gui --demo       # requires a graphical desktop / WSLg
```

For development, run `npm run tauri -- dev -- --demo` in `apps/hyperx-gui`.
Linux CI compiles GTK/WebKit desktop code and smoke-tests the real executable
without starting a graphical session. This is an unbundled executable, not an
AppImage/deb release. Window/dialog behavior and packaging remain operator work;
the GUI does not open USB even if `/dev/hidraw` access is configured.

## Preview and tests

```bash
cd apps/hyperx-gui
npm run dev                    # read-only browser preview on localhost:1420
npx playwright install chromium
npm run test:e2e               # preview, navigation, modal and minimum-width checks
```

Rust facade tests run on Linux/macOS without GTK/WebKit packages: `desktop` is
opt-in. Native desktop CI builds/tests target Windows, Linux and macOS arm64/Intel.
Rendered-window/native-dialog operator validation is not yet verified.
`cargo build --workspace` alone does not enable the desktop GUI binary.

Frontend tests cover typed edits, slider release, validation, safe save intent,
dirty-document replacement, preview restrictions and serialized IPC revisions.
Macro tests cover separate chord transitions, mouse-button events, per-event
delays, reorder/removal, referenced replacement consent, stale-definition guards,
local-draft preservation and native close-guard notifications. A browser test
also exercises editable timeline layout using explicitly mocked native IPC;
this does not replace native-window operator verification.
The Chromium E2E suite also activates every section by keyboard and checks that
Tab/Shift+Tab wrap inside the Review dialog and Escape/Done return focus to its
opener. WebView2 and native Windows file dialogs still need the operator pass
below; passing browser E2E alone does not close that item.
Rust tests cover the real shared validation, preservation of unknown values,
failed actions, safe new-file writes and the demo JSON contract. Browser tests
do not replace an operator test of native Windows dialogs/WebView2.

### Native Windows operator pass (not yet verified)

Partial operator result (2026-10-03, Windows version not recorded): with the
newly built native GUI open, the operator confirmed that Tab/Shift+Tab stayed
inside Review and Escape returned focus to its Review opener. The native Open
dialog appeared on top; cancelling it preserved the same OFFLINE demo profile.
Opening a file, save/close flow and minimum-size pass below are still
unverified.

Run this on the rebuilt Windows EXE in a real desktop session. It is entirely
offline: do not use a device command, and do not treat the demo as live state.
Record Windows/WebView2 versions and any failed step. Use a unique filename in
`$env:TEMP` for Save NEW; keep the resulting file for inspection rather than
overwriting an existing profile.

1. Start `hyperx-gui.exe --demo`. Confirm the window shows **OFFLINE** and
   **DEMO PROFILE**, and that Apply/Save to mouse are disabled. Switch all six
   sections by mouse, then focus each sidebar button and activate it with Enter.
   At the minimum 900×640 window size, controls and status bar must remain
   reachable without horizontal clipping.
2. Focus **Review**, press Space, then cycle Tab and Shift+Tab several times.
   Focus must remain on the dialog's controls. Escape must close it and return
   focus to Review; reopen with Enter, choose Done and verify the same return.
3. Choose **Open profile** and cancel the Windows file dialog. The demo must
   remain loaded. Open `examples/profiles/pulsefire-raid.toml` explicitly;
   confirm the source becomes a local FILE and no device connection is claimed.
4. Change the profile name, choose **Save new file**, and select a unique TOML
   path under `$env:TEMP` in the native dialog. Verify the file exists and the
   GUI shows a clean FILE draft. Make another name edit, request window close,
   cancel the dirty-draft confirmation, then close again and confirm. No
   setting should be applied to the mouse in any of these steps.

If the native dialog opens behind the window, focus escapes, paste/typing is
lost, the window cannot be closed after cancel, or the file differs from the
shown draft, record the exact step and screenshot before changing anything else.

## Boundary

`apps/hyperx-gui/src-tauri` is maintained source, not generated scaffolding to
discard. Commit its Rust sources, `Cargo.toml`, `build.rs`, `tauri.conf.json`,
`capabilities` and application icon/source provenance. The workspace-level
`Cargo.lock` and frontend `package-lock.json` are committed too. Generated
`src-tauri/gen` schemas, autogenerated permissions, `target`, `dist` and
`node_modules` are ignored; they are recreated during install/build.

React → narrow local-window IPC commands → `Session` → `hyperx-app`.
UI requests use a typed edit plus the expected document revision. The backend
rechecks revisions after native dialogs; stale completion cannot overwrite a
newer document. The client serializes requests without automatic retry.

The local main-window capability grants only offline snapshot/edit/reset/open/
save/overwrite/undo/redo/recovery commands, the local timeline close-protection
notification, and Tauri core defaults. Native dialogs select profile paths in
Rust; recovery accepts only a listed basename resolved in the private recovery
directory. The frontend cannot supply arbitrary filesystem paths. No JS filesystem/dialog,
shell, updater or hardware permissions are granted. Production CSP allows no
external scripts, assets or connections.

Sources: [Rust IPC](https://v2.tauri.app/develop/calling-rust/),
[native dialogs](https://v2.tauri.app/plugin/dialog/),
[capabilities](https://v2.tauri.app/security/capabilities/).

## Next

- [x] General binding selectors, legal choices from shared model metadata.
- [x] Editable macro timeline with target-specific limits and library management.
- [x] Partial/unresolved import inspection and explicit resolution/omission controls.
- [ ] Keyboard navigation/operator tests of native dialogs and WebView2 (Chromium E2E done; native checklist above remains).
- [x] Deliberate recoverable overwrite of an opened FILE with a separate review.
- [x] Bounded in-session undo/redo for complete FILE drafts.
- [x] Recover accepted FILE-document edits after restart through an explicit offline choice.
- [x] Manage and explicitly discard old GUI recovery snapshots without granting arbitrary filesystem access.
- [x] Offer explicit best-effort recovery of uncommitted local macro timelines.
- [x] Recover unfinished profile name, DPI and color inputs with explicit restore/discard.
- [x] Keep transient binding/resolution selectors and the Add stage dialog out of crash recovery: choices are not FILE edits and must be explicitly submitted again; recover typed name/DPI/color fields instead.
- [ ] Connected app facade **only after** usable hardware baseline/readback is restored.
- [ ] Separate Preview, Apply and confirmed Save to mouse; no reports in React.
- [ ] Installer, signing and releases after native behavior is validated.

TUI remains supported: its separate backlog is in [tui-todo.md](tui-todo.md).
