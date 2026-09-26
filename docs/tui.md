# Offline terminal UI

`hyperx-tui` is the second application client. It depends on `hyperx-app` and
`hyperx-core`, not transport/protocol crates. The app layer adapts existing model
validators and metadata; transitive driver/HID build dependencies do not imply
device access. It has no discovery/open/send/apply/save-onboard operation.

## Run

Build the workspace normally; Rust 1.88+ is required. Run the Windows executable
from an interactive Windows Terminal PowerShell, with the repository as current
directory:

```powershell
& '.\target\x86_64-pc-windows-msvc\debug\hyperx-tui.exe' --demo
& '.\target\x86_64-pc-windows-msvc\debug\hyperx-tui.exe' '.\my-profile.toml'
```

If launching directly from WSL would give the Windows executable redirected
stdin/stdout, use Windows Terminal instead. The TUI refuses noninteractive
streams rather than hanging. Linux/macOS can run their native executable.

`--demo` loads the repository example with a DEMO DATA badge. A file loads FILE
DRAFT settings, not live state. No arguments create an empty partial target
profile with no DPI/polling/binding/lighting defaults. Suggested size is 100x30;
small windows display a resize notice instead of panicking.

## Mouse and direct controls

Mouse capture is enabled only while the interactive TUI runs, and is disabled
on exit. Click tabs, bottom action buttons and dialog Accept/Cancel buttons.
Open dialogs block underlying controls. Hit targets are rebuilt from the visible
layout after each frame; resizing removes stale targets. Click inside text fields
to position the cursor. Windows Terminal is the preferred native Windows host.

Performance renders one slider per supplied DPI stage. Click or drag its bar to
set linked X=Y values, snapped to the model's step and clamped to its range
(Raid: 200–16000, step 50). Terminal cells cannot represent every DPI step on a
short bar: use +/- buttons, the wheel over the bar, or click the numeric value
for exact input. Changing one stage never changes another stage or its color.
Independent imported X/Y values remain visible; editing a stage's DPI explicitly
sets both axes to the entered value.

Click a stage label to select it for keyboard edits, Activate to choose it as
the file's active stage, or its hex color to edit that color. Add level asks for
an explicit DPI value and states that the new color is white (editable afterward).
Adding does not invent an active index. The model stage limit is enforced.
Remove last cannot remove the only stage or the active last stage: choose a
different active stage first. Source provenance is not silently normalized.

Polling has model-supported value buttons (Raid: 125/250/500/1000 Hz). Standard
and Swapped change the coupled primary-button layout in the FILE. Lighting has
one explicit Solid color field per declared zone; `#000000` is off. If a zone
is missing it stays unknown until supplied, rather than defaulting to black.
Both Raid zone colors are required for encoding readiness, just as before.

The wheel away from a DPI bar scrolls the view. Text dialogs preselect the old
value, so typing replaces it; Ctrl+A selects all again. Invalid input leaves the
dialog open and preserves the document; cancel discards only that input. Changes
take effect in the draft immediately after a successful control edit; `s` or
Save NEW writes a new file. This is **not live configuration or Save to mouse**.

## Keys

| Key | Action |
| --- | --- |
| Tab / Shift+Tab, 1–5 | switch tab (left/right also do so outside Performance) |
| Left/Right or +/- in Performance | edit selected stage by one model DPI step; Shift+arrow uses ten steps |
| [ / ] in Performance | select previous/next stage |
| F2 / F3 in Performance | exact DPI / stage color input |
| Enter / Insert / Delete in Performance | activate selected stage / add stage / remove last stage |
| F4 in Performance | cycle supported polling values |
| F2 / F3 in Lighting | edit first/second declared zone color |
| Up/Down, PageUp/PageDown, Home | scroll view |
| e | edit this tab's typed TOML section |
| a | edit the full profile, including name/source/unresolved entries |
| Ctrl+S in editor | parse and accept draft; does not save to disk |
| Esc in editor | cancel, preserving current document |
| Ctrl+A in editor | select all text for replacement |
| o | open profile path; confirm discard if document has unsaved edits |
| s | save as a NEW path; never overwrite an existing file |
| v | offline device-encoding validation |
| d | semantic file diff against opened/last-saved baseline |
| m | import separate macro timeline under a fresh library ID |
| r | resolve one exact source ID into an explicitly chosen control/macro |
| x | deliberately omit one exact unresolved source ID |
| q / Esc | quit, with unsaved document confirmation |
| Ctrl+C | same cancel/quit behavior as Esc |

Single-line path/source prompts accept Enter. Multiline TOML prompts accept
Ctrl+S. Windows release events are ignored to prevent duplicate actions;
bracketed paste, Unicode text, cursor movement and line joining are supported.
Ratatui's `run` manages raw/alternate-screen restoration, including returned
errors and panic; additional bracketed-paste and mouse modes have an independent guard.

## Editing semantics

Performance edits `[polling]`, `[dpi]` stages/colors/active index and the coupled
`primary_buttons` field. Buttons edits named `[buttons.CONTROL]` tables; primary
clicks stay one Standard/Swapped operation. Macros edits the `[[macros]]` library
and all ordered transitions/timings. Lighting edits `[lighting]` with explicit
wheel/logo Solid colors; the other runtime software effects are not silently
converted into profile Solid or firmware modes. Profiles edits the whole file.

Performance/Lighting have direct controls; `e` and `a` retain the advanced typed
TOML editors. Buttons and macro timelines still use those editors, not a macro
recorder or a binding dropdown. Unknown fields and malformed TOML fail before modifying the
document; the editor retains text and the error for correction. A parseable
but unsupported value may remain in a draft with NOT READY status. Sections
missing from the replacement text are removed from the FILE, not disabled or
reset on hardware. Unrelated sections/provenance remain intact.

Macro events are separate down/up actions followed by individual delays. A
zero delay between key-down actions expresses a chord. The view shows cumulative
file timings, not measured hardware playback. Limits come from target-specific
implemented encodings, not assumed hardware maxima.

`m` accepts this prompt, using the existing standalone macro TOML format:

```toml
path = "examples/macros/ab-20ms.toml"
id = "ab"
```

IDs must be nonempty and new; a library definition is not implicitly overwritten
or assigned. Validate after importing. To explicitly resolve a captured reference
after supplying its real macro definition:

```toml
source_id = "runtime:button5"
control = "button5"
macro_id = "ab"
```

Resolution validates the actual target/timeline, requires one exact source and
library ID, refuses to overwrite a supplied control binding and retains other
unknowns. A known capture target cannot be moved to a different control by
clearing its diagnostic label. Legacy opaque source targets are selected by the
operator, never inferred. `x` removes provenance only; any already explicit
binding remains. See [software profiles](profile-format.md).

## Saving and validation

`s` serializes before `create_new`; an existing destination is refused. A failed
save retains the document and file baseline. A successful save establishes a
new FILE baseline for dirty/diff state, not a device-state read. Comments and
original formatting are not retained; typed source/partial/unresolved metadata
is. An I/O failure may leave an incomplete new file and reports that possibility.
Partial/unsupported drafts can be saved for later work; passing offline
validation is not a permission to access hardware or evidence that composed
runtime apply was fixed. Save to mouse remains unavailable.

Input, section edits, macro imports and serialized profiles are bounded to
1 MiB. Input must be UTF-8 TOML (an optional UTF-8 BOM is accepted).

## Automated checks

```text
hyperx-tui --demo --check
hyperx-tui --demo --render --width 120 --height 40
hyperx-tui captured.toml --render
```

`--check` returns failure on unsupported/unresolved drafts, without terminal
initialization. `--render` uses Ratatui TestBackend and prints plain text even for
NOT READY drafts. It does not change files. Neither mode discovers/opens HID.
Tests cover rendering all tabs/modals at multiple sizes, Unicode/paste, Windows
key-release filtering, mouse click/drag/scroll and step/boundary behavior,
modal isolation, resized/scrolled hit targets, direct numeric/color input,
transactional edits, explicit resolution/omission,
dirty-state confirmation and save failures. Linux/Windows/macOS CI runs both
headless executable checks in addition to workspace tests.

## Dependencies

Use Ratatui **0.30.2**, the latest stable version verified for this work, and its
matching Crossterm **0.29.0** backend. Ratatui requires Rust 1.88; the workspace
MSRV was raised accordingly. Serde was updated to 1.0.228 because the new
dependency graph cannot resolve against the former exact 1.0.193 pin. No legacy
profile schema/version or protocol bytes were changed.

Sources: [Ratatui API](https://docs.rs/ratatui/latest/ratatui/),
[installation / MSRV](https://ratatui.rs/installation/),
[terminal lifetime wrapper](https://docs.rs/ratatui/latest/ratatui/fn.run.html).
