# TUI backlog

The TUI remains a supported offline client alongside the Tauri GUI. Both clients
must reuse `hyperx-app`; completing GUI work must not remove terminal features.

## Presentation

- [ ] Consistent spacing, panel hierarchy, selected/focused states and short labels.
- [x] Replace debug `Some(...)`/enum output with readable values and explicit unknowns.
- [x] Color swatches/palette, with text values retained for accessibility.
- [x] Main views retain visible scroll position and mouse actions at the 45x12 minimum.
- [x] Extend compact layouts to editors/dialogs and verify keyboard/mouse reachability at 45x12.
- [ ] Review keyboard shortcuts and focus navigation alongside mouse controls.
- [ ] Native Windows Terminal operator smoke test for mouse, paste and restoration.

## Buttons

- [x] Clickable table of all model-declared physical controls.
- [x] Selectors for Mouse, Multimedia, Windows Shortcut, Keyboard, Disabled and library macros.
- [x] Searchable named-key picker without exposing vendor/HID records.
- [x] Target-aware legal choices and runtime/onboard macro limits from the app layer.
- [x] Preserve coupled primary-click layout and unresolved assignment provenance.

## Macros

- [x] Timeline table: ordered down/up events, per-event delays, add/remove/reorder.
- [x] Explicit chord construction and Once/Toggle/Hold playback selection.
- [x] Named-library management with confirmation before replacing referenced definitions.
- [x] Keep editing limits separate from claims about the hardware maximum.
- [ ] Investigate recording separately: terminal events do not reliably expose all key releases.

## Profiles

- [x] Bounded file browser, direct draft naming and safe NEW-file copying.
- [x] Explicit unresolved-entry selectors without TOML, with distinct resolve/omit confirmation.
- [x] Better diff presentation and a validation summary linked to the offending section/field path.
- [x] Deliberate FILE overwrite with explicit confirmation, stale-file refusal and a recovery copy; Save NEW stays the default.
- [x] Bounded in-session undo/redo of complete FILE drafts without stripping provenance.
- [x] Recover accepted FILE document edits after restart from an explicit private snapshot.
- [x] Add an interactive TUI panel to list, restore and discard FILE-draft snapshots.
- [x] Recover unaccepted text-editor fields and macro timelines separately from FILE drafts.
- [ ] Decide whether transient file-browser/binding/resolution search filters merit crash recovery.

## Connected mode — blocked on hardware evidence

- [ ] Reconnect and obtain a usable fresh baseline with the operator present.
- [ ] Diagnose the empty readback after composed `profile apply`; no blind retry/rollback.
- [ ] Add an explicit connected app-session API only after verified driver behavior.
- [ ] Separate Preview, Apply and confirmed Save to mouse, preserving every evidence gate.
- [ ] Never add firmware, bootloader, update or DFU paths.

Completed: [offline controls, file editing, validation and diff](tui.md).
GUI development continues separately in [gui.md](gui.md).
