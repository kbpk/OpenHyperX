# TUI backlog

The TUI remains a supported offline client alongside the Tauri GUI. Both clients
must reuse `hyperx-app`; completing GUI work must not remove terminal features.

## Presentation

- [ ] Consistent spacing, panel hierarchy, selected/focused states and short labels.
- [ ] Replace debug `Some(...)`/enum output with readable values and explicit unknowns.
- [ ] Color swatches/palette, with text values retained for accessibility.
- [ ] Responsive layouts, visible scroll position and all actions reachable in small windows.
- [ ] Review keyboard shortcuts and focus navigation alongside mouse controls.
- [ ] Native Windows Terminal operator smoke test for mouse, paste and restoration.

## Buttons

- [x] Clickable table of all model-declared physical controls.
- [x] Selectors for Mouse, Multimedia, Windows Shortcut, Keyboard, Disabled and library macros.
- [x] Searchable named-key picker without exposing vendor/HID records.
- [x] Target-aware legal choices and runtime/onboard macro limits from the app layer.
- [x] Preserve coupled primary-click layout and unresolved assignment provenance.

## Macros

- [ ] Timeline table: ordered down/up events, per-event delays, add/remove/reorder.
- [ ] Explicit chord construction and Once/Toggle/Hold playback selection.
- [ ] Named-library management with confirmation before replacing referenced definitions.
- [ ] Keep editing limits separate from claims about the hardware maximum.
- [ ] Investigate recording separately: terminal events do not reliably expose all key releases.

## Profiles

- [ ] File picker/list, naming, duplicate and explicit unresolved-entry controls without TOML.
- [ ] Better diff presentation and a validation summary linked to the offending field.
- [ ] Deliberate safe overwrite workflow with recovery; current Save NEW stays the default.
- [ ] Session recovery/undo without guessing missing device settings or stripping provenance.

## Connected mode — blocked on hardware evidence

- [ ] Reconnect and obtain a usable fresh baseline with the operator present.
- [ ] Diagnose the empty readback after composed `profile apply`; no blind retry/rollback.
- [ ] Add an explicit connected app-session API only after verified driver behavior.
- [ ] Separate Preview, Apply and confirmed Save to mouse, preserving every evidence gate.
- [ ] Never add firmware, bootloader, update or DFU paths.

Completed: [offline controls, file editing, validation and diff](tui.md).
GUI development continues separately in [gui.md](gui.md).
