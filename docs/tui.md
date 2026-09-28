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
& '.\target\x86_64-pc-windows-msvc\debug\hyperx-tui.exe' --demo --view macros
```

If launching directly from WSL would give the Windows executable redirected
stdin/stdout, use Windows Terminal instead. The TUI refuses noninteractive
streams rather than hanging. Linux/macOS can run their native executable.

`--demo` loads the repository example with a DEMO DATA badge. A file loads FILE
DRAFT settings, not live state. No arguments create an empty partial target
profile with no DPI/polling/binding/lighting defaults. Suggested size is 100x30;
small windows display a resize notice instead of panicking. `--view` selects
performance, buttons, macros, lighting or profiles on startup and in `--render`;
it cannot be combined with validation-only `--check`.

Profile views use readable action/macro names and explicit `<not present>`,
`<unknown>` or `<not saved>` labels instead of Rust `Some(...)`/enum debug text.
Imported source IDs and names are displayed as escaped single-line values, so
control characters cannot masquerade as another terminal row. Model limits are
declared capabilities, never live measurements.

`d` groups the semantic FILE diff by Performance, Buttons, Macros, Lighting
and metadata, with separate before/after values. An omitted value is shown as
`<not present>` and does not mean disabled or reset. `v` shows offline encoding
readiness and, when validation can identify an exact file field, its field path
and section. Press `g` in that report to open the section; it does not edit the
draft or access the mouse. Errors without a known file field are not assigned a
guessed location.

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

Supplied DPI and lighting colors have truecolor terminal swatches beside their
exact `#RRGGBB` text. Explicit black uses a neutral outlined swatch so it is
visible on dark backgrounds; missing colors have no swatch and remain marked
`<not present / not read>`. A terminal without truecolor still shows the hex
value and retains the same clickable color editor.

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

Buttons renders all model-declared physical controls in a clickable table.
Click a non-primary row, or select it with Up/Down and open it with Enter/F2.
The binding selector reads its choices from `hyperx-app`: Mouse, Multimedia,
Windows Shortcut, Keyboard, Disabled and existing library macros. Left/Right
or Tab/Shift+Tab changes category; the clickable `<` / `>` buttons do the same.
Type or paste to search the current category, including canonical key names
such as `left-shift`; Ctrl+A then typing replaces the search. Up/Down,
PageUp/PageDown, Home/End or a row click selects a **preview**, not a file edit.
Enter, Ctrl+S or Accept binding explicitly changes the draft; Esc discards it.
Changing category never selects its first item automatically.

Not specified omits one file assignment, preserving device state; it is not
Disabled, a factory reset or a request to delete unresolved provenance. Primary
rows explain that Standard/Swapped must be changed atomically in Performance.
Imported aliases and unsupported bindings are not silently normalized by
opening the selector. Macro IDs refer to existing library definitions, which
remain unchanged. Rejected target/macro combinations stay visible with their
validation reason and cannot be accepted. Runtime/onboard modes and event/delay
limits are app-declared encoding support, not proof that a connected mouse
supports or has received that assignment. No controls send HID reports.

The wheel away from a DPI bar scrolls the view. Text dialogs preselect the old
value, so typing replaces it; Ctrl+A selects all again. Invalid input leaves the
dialog open and preserves the document; cancel discards only that input. Changes
take effect in the draft immediately after a successful control edit; `s` or
Save NEW writes a new file. This is **not live configuration or Save to mouse**.

## Macro library and timeline

In Macros, click New or press Insert/`n` to create a local draft with a fresh
file-library ID. Existing definitions and unresolved references reserve IDs;
creating a macro never silently resolves or assigns one. Click a library row
or select it with Up/Down and Enter/F2 to edit. Delete asks for confirmation and
refuses a definition still referenced by a button or unresolved source.

The editor has its own draft, separate from the profile document. F2 edits the
name; F3 cycles Play Once, Toggle Repeat and Hold Repeat. Insert appends an
unconfigured Key down event. Select a row to set its type (F4), named key or
mouse input (F5/Enter), and delay after that event (F6). Input search requires
an explicit selection and acceptance; it never chooses a key merely because
the search matches. Delete removes a row; F7/F8 or Shift+Up/Down reorders it.
The corresponding buttons and input/row selections also support mouse clicks.
The wheel scrolls event selection, input choices or the validation report.

A Shift+A chord, for example, is Key down `left-shift`, Key down `a`, Key up
`a`, Key up `left-shift`. Zero delay after the first down keeps Shift held when
A goes down; every event can have a different delay. The editor shows file
delays, not measured playback. No key presses are recorded or simulated.

Enter accepts a name/input/delay field **into the local macro draft**. Ctrl+S
from the timeline accepts the entire macro into the profile document. Replacing
a changed, referenced definition requires confirmation listing affected
references; cancellation retains the local draft. Esc closes an unchanged
macro or asks before discarding a changed/new draft. Field cancellation does
not discard the surrounding timeline. Identity is immutable; stale or duplicate
source definitions cannot be silently replaced. The editor needs at least
45x18 cells; smaller windows keep the draft and show a resize notice.

Library editing deliberately allows empty, unbalanced or unsupported drafts,
subject to file size/name checks. File delays are unsigned integers 0–65535 ms;
they are not clamped to the target encoder's limits. F9 checks the local
timeline against each implemented macro target without committing it. Raid's
current encoding supports at most 14 events and 9999 ms per delay: runtime
Once/Toggle/Hold on Button 4, runtime Once on Button 5, and onboard Once on both.
These are capture-backed implementation limits, not proven hardware maxima.
Unknown imported key aliases and longer timelines are preserved. Acceptance
into the library does not imply device readiness; assigning a library macro
through Buttons separately checks its target. No macro action accesses USB.

After accepting a macro, `s` / Save NEW still writes the document to a new file.
Editing a referenced macro changes those FILE references' definition, not the
live mouse or its onboard memory.

## Profile files without TOML

In Profiles, F2 / Browse opens a file browser at the opened/saved file's parent
directory (or the process working directory for an unnamed/demo draft). F3 /
Name edits only the profile name; it never renames a filesystem entry, changes
device settings or clears imported provenance. Explicit names must be nonblank,
at most 128 UTF-8 bytes and contain no control characters. Even a profile for an
unknown model can be renamed and retained as an unsupported file draft.

F4 / Copy NEW chooses a directory and a new filename, initially
`<source-stem>-copy.toml` or `profile-copy.toml`. It copies the **current draft**,
including unsaved edits, macro definitions and unresolved/source fields. The
original file stays untouched. After a successful copy, the new file becomes
the document's path and clean file baseline; copying is not a device backup.

The browser lists directories first, then `.toml` files (case-insensitive
extension), without recursively scanning or parsing their contents. Click a row
or use arrows/PageUp/PageDown/Home/End to preview. Enter opens a directory or,
in Browse mode, reads the selected file through the bounded app parser. Right
enters directories; Left/Backspace or Up goes to the parent. F5 refreshes. `p`
optionally accepts an absolute directory or one relative to the displayed
directory. Symlink entries and special files are skipped. Directory scans stop
after 4096 entries and show a warning if incomplete; smaller directories can be
chosen. Paths are retained as OS paths, not reconstructed from display labels.

In Copy NEW mode, F2 / NEW filename (or `s`) opens the filename field. Enter
creates exclusively in the selected directory. Use one `.toml` filename, not
a path; directory separators, drive/stream syntax and control characters are
rejected. A destination that already exists, even if created after browsing,
is never overwritten. A failed save keeps the field, draft and old baseline.
Esc cancels a field first, then the browser. The browser needs at least 45x15
cells; shrinking keeps its state and blocks hidden mouse targets.

Opening a new file when the current document is dirty asks for consent only
after the selected file was successfully read and parsed. `n` / Esc keeps the
current document and returns to the browser; `y` adopts that parsed snapshot.
Bad TOML, unreadable, non-UTF-8 or over-1-MiB files never discard current edits.
The existing global `o` / `s` path prompts remain available as advanced shortcuts.
F5 / Unresolved opens the explicit source/target/library selector below. The
advanced `r` / `x` prompts remain available as alternatives.

## Unresolved source selector

In Profiles, F5 / Unresolved lists the exact unresolved entries from the FILE.
No entries means only that this file contains none; nothing was read from USB.
Click a row or use arrows to preview; typing/pasting filters the list without
selecting a result. Enter moves through Source → Target → Library macro →
Confirmation. Each step requires an explicit choice. There are no default
targets, auto-selected matching IDs or fabricated macro timelines.

Targets and rejection reasons come from `hyperx-app`. A normalized
`runtime:button5` diagnostic is locked to Button 5. Opaque Legacy source IDs
have no inferred physical target; `macro_source_id` is a provenance hint, not
an instruction to choose a same-named library definition. Targets with an
existing supplied binding cannot be overwritten during resolution. The library
list checks the real selected timeline and playback against the chosen target;
blocked choices remain visible. If no definition exists, close the selector
and create/import the real timeline in Macros before trying again. Unknown
device models do not gain invented targets. F1 shows the complete selected
reason in a scrollable report; it is informational, never a commit.

At the final summary, `y` / Ctrl+S confirms a FILE edit: one macro reference
is assigned to the chosen control and exactly one unresolved entry is removed.
The library, other settings and other source/provenance entries are retained.
Enter does not confirm. The source and chosen definition are checked again
before acceptance; changed or ambiguous IDs and newly supplied bindings are
rejected without losing the document. Passing this isolated target preflight
does not repair unrelated unsupported fields or prove hardware playback.

Delete / Omit from the source list opens a **different** confirmation. `y`
removes only that exact unresolved provenance entry: it does not disable/reset
a button, delete an existing supplied binding, remove a macro or change the
mouse. Duplicate source IDs cannot be resolved or omitted by selecting their
row number; inspect the advanced file editor instead. `n` / Esc returns from
a confirmation, and Esc steps back through the chooser until closing it. No
preview changes the file document. Use `s` / Copy NEW afterward to save a new
file. Minimum chooser size is 45x16; resize preserves selection and isolates
mouse targets. Arrows/Page keys and the wheel scroll reports and choices.

To practice without a mouse, open the explicitly **synthetic** example (not an
actual capture or device backup):

```powershell
& '.\target\x86_64-pc-windows-msvc\debug\hyperx-tui.exe' '.\examples\profiles\pulsefire-raid-unresolved.toml' --view profiles
```

Choose F5, `runtime:button5`, Button 5 and the `ab` library entry; confirm only
after reviewing the summary. Its other opaque entry stays unresolved until
separately resolved or deliberately omitted. No DPI/polling/RGB defaults are
invented and no settings are sent to the mouse.

## Keys

| Key | Action |
| --- | --- |
| Tab / Shift+Tab, 1–5 | switch tab (left/right also do so outside Performance and binding dialogs) |
| Left/Right or +/- in Performance | edit selected stage by one model DPI step; Shift+arrow uses ten steps |
| [ / ] in Performance | select previous/next stage |
| F2 / F3 in Performance | exact DPI / stage color input |
| Enter / Insert / Delete in Performance | activate selected stage / add stage / remove last stage |
| F4 in Performance | cycle supported polling values |
| F2 / F3 in Lighting | edit first/second declared zone color |
| Up/Down, Home/End in Buttons | select model-declared physical control |
| Enter / F2 in Buttons | open selected control's binding selector |
| Left/Right or Tab/Shift+Tab in binding selector | switch category; no implicit assignment |
| Type/paste, Ctrl+A in binding selector | filter labels/canonical named keys; select search text |
| Up/Down, PageUp/PageDown, Home/End in binding selector | preview a choice |
| Enter / Ctrl+S in binding selector | explicitly accept selected semantic binding |
| Up/Down, Home/End in Macros | select library definition |
| Enter / F2, Insert / n, Delete in Macros | edit, create, remove library definition |
| F2 / F3 in macro timeline | edit name / cycle playback |
| Insert / Delete in macro timeline | append unconfigured event / remove selected event |
| F4 / F5 (Enter) / F6 in macro timeline | cycle event type / choose input / edit delay |
| F7 / F8 or Shift+Up/Down in macro timeline | move event up / down |
| F9 in macro timeline | inspect target encoding readiness of the local draft |
| Ctrl+S / Esc in macro timeline | accept entire macro / close or confirm discard |
| Enter / Esc in macro field | accept field into local macro draft / cancel field |
| F2 / F3 / F4 in Profiles | browse files / rename draft / copy draft to NEW file |
| F5 in Profiles | open unresolved source/target/library selector |
| Type/paste, arrows/click, Enter in source selector | filter, preview, advance explicit choice |
| Delete in source list | review provenance-only omission |
| F1 in source selector | complete scrollable reason; Esc returns |
| y / Ctrl+S in source confirmation | accept one file resolution/omission, not save/apply |
| n / Esc in source confirmation | return without modifying the document |
| Enter / Right in file browser | open selection / enter selected directory |
| Left / Backspace, F5, p in file browser | parent directory, refresh, optional directory prompt |
| F2 / s in Copy NEW browser | enter a new `.toml` filename in the chosen directory |
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
| g in validation report | open the section of a known offending file field |
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

All five views have direct controls; `e` and `a`
retain the advanced typed TOML editors. A global macro recorder is not provided.
Unknown fields and malformed TOML fail before modifying the
document; the editor retains text and the error for correction. A parseable
but unsupported value may remain in a draft with NOT READY status. Sections
missing from the replacement text are removed from the FILE, not disabled or
reset on hardware. Unrelated sections/provenance remain intact.

Macro events are separate down/up actions followed by individual delays. A
zero delay between key-down actions expresses a chord. The library preview shows
total file timing, not measured hardware playback. Limits come from target-specific
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
hyperx-tui --demo --render --view macros --width 120 --height 40
hyperx-tui --demo --render --view profiles --width 120 --height 40
hyperx-tui captured.toml --render
```

`--check` returns failure on unsupported/unresolved drafts, without terminal
initialization. `--render` uses Ratatui TestBackend and prints plain text even for
NOT READY drafts. It does not change files. Neither mode discovers/opens HID.
Tests cover rendering all tabs/modals at multiple sizes, Unicode/paste, Windows
key-release filtering, mouse click/drag/scroll and step/boundary behavior,
modal isolation, resized/scrolled hit targets, direct numeric/color input,
transactional edits, explicit resolution/omission,
semantic binding previews/acceptance, searchable keys, target-rejected macros,
Disabled versus omission, coupled primary protection and preserved aliases,
macro library identity/reference guards, chord construction, independent
delays, timeline reordering, field/draft cancellation and local target checks,
lazy/bounded directory browsing, parse-before-discard, Unicode/OS filenames,
metadata-only rename and race-safe new-copy behavior,
explicit source/target/library previews, confirmation and provenance-only
omission, ambiguous/stale source/definition guards and full rejection reports,
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
