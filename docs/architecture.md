# Architecture

## Goals

The core design separates device behavior from USB transport and presentation.
A TUI or Tauri UI must not know HID paths, report IDs or packet layouts. Adding a
mouse should normally mean adding one driver and registry entry, not changing
the CLI or GUI.

**2026-09-27 safety suspension, extended in 2026-10:** descriptor-only CLI
`info` no longer constructs the model driver or queries runtime settings. The
CLI's runtime and save/ACK openers reject access before discovery. The model
driver now also rejects every public runtime/profile read, setter, preview,
apply, session/ACK probe and onboard save before any HID report. Its retained
capture-backed sequence is available only through a private mock-transport
test constructor; normal clients cannot enable it. Direct RGB uses a distinct
opener and no runtime selector/request. Offline operations retain their codecs
and mocks. This is a safety block, not a protocol fix.
An explicitly consented isolated `07 03 04 64` SET later reproduced physical
loss of mouse operation without request `81` or GET; USB reconnect recovered it.
This selector must be treated as potentially disruptive profile activation,
not a passive addressing step. The empty-runtime activation mechanism and
safe preparation remain unproven; separate lab probes do not lift the gate.
An operator-confirmed cold Legacy launch then used the same two `07 07`
startup packets yet read an all-zero runtime body. Legacy subsequently uploaded
a macro definition and a populated runtime image, and the operator confirmed
normal input/lighting. The startup ACK is therefore not proof of a usable
runtime baseline. Historical successful warm-session reads below must not be
generalized to cold-device bootstrap.

```text
CLI device operations / offline TUI + Tauri via hyperx-app
        |
        v
public device API (hyperx-core)
        |
        v
model driver (hyperx-devices)
        |                 \
        v                  v
packet codecs          capability declaration
(hyperx-protocol)
        |
        v
HidTransport (hyperx-hid)
        |
        v
hidapi / Windows HID
```

The current implementation includes discovery, standard report-descriptor
reads, a capture-backed runtime-profile read, the capture-backed volatile RGB
operation with separate wheel/logo colors, a portable foreground software
effect renderer, runtime setters for active DPI and polling, and an offline
Pulsefire Raid profile patcher. Runtime DPI setters can edit values and colors,
select a stage, append up to five contiguous stages, and remove only the final
stage.
It also recognizes confirmed button records and exposes a target-aware runtime
assignment API. The nine general controls accept capture-backed Mouse,
Multimedia, Windows Shortcut, Disabled and named-keyboard families. The two
primary controls use a separate coupled standard/swapped API so no caller can
construct the intermediate state forbidden by NGENUITY Legacy. The
platform-independent macro model stores an ordered key/button down/up timeline
and a delay on every event. A capture-backed Raid encoder now supports Play
Once Button 5 keyboard chords, nonuniform timings and left/right/middle mouse
clicks, with a conservative 14-transition limit matching the largest local
capture. The same runtime framing is independently captured for Button 4,
including the exact Toggle Repeat and Hold Repeat mode pairs. Repeat modes
remain target-gated to runtime Button 4 and cannot be converted onboard. Other
targets, longer macros and unconfirmed mouse events
remain absent. A separate offline parser reads confirmed fields from
NGENUITY Legacy version-40 `.hxp` presets and converts them to a partial
software profile without opening a HID device. Current NGENUITY is a distinct,
unsupported source format and must not share the Legacy parser without
evidence.

The driver also owns the acknowledged Pulsefire Raid onboard-save transaction.
It validates runtime layout, all enabled X/Y DPI values and binding records
before selecting onboard memory. It reads the existing onboard image and
patches only the confirmed DPI, polling and 11
button-record fields. Unknown onboard bytes are preserved. A referenced Button
4 or Button 5 macro requires its caller-supplied typed definition because the
event stream cannot be recovered from the profile image. Every save-stage interrupt
acknowledgement is checked and an unexpected response aborts without retry.
Numeric DPI bounds use the same validator as the offline setter. Invalid source
values abort after the runtime read but before onboard selection, auxiliary
lighting or any persistent write. The destination patch also validates before
mutation; a rejected source leaves all destination bytes unchanged. Inspection
still decodes unusual raw DPI values for research rather than clamping them.
The acknowledgement read is now armed before each feature report on Windows
to reduce a possible read-posting race; hardware probing showed that this
alone does not restore missing ACKs. A
separate non-persistent CLI probe exercises only the known runtime selector
and its acknowledgement path.
`PulsefireRaidOnboardMacros` validates timelines, targets and uniqueness without
HID I/O. Every referenced Button 4/5 slot must have a supplied definition;
ordinary bindings must not receive one. The repeated capture order is Button 5
then Button 4, regardless of caller order. Unknown onboard bytes remain intact.
Feature reports use the interface-1 configuration collection, while the
eight-byte acknowledgements are read through a separate interface-2 HID
handle. The complete path was hardware-validated across a physical power-cycle,
but a later session lacked ACKs and aborted before onboard selection.
Repeated Legacy launches and power-cycle probes now establish the fixed
two-report volatile vendor-session startup. It is exposed through an
explicit non-persistent diagnostic flag. Fresh-reconnect and already-active
hardware checks verified its ACKs and an unchanged complete runtime image;
the operator confirmed normal physical operation and unchanged lighting after
the first check. Saves now initialize once before the acknowledged runtime
read, never as an automatic retry after an ACK failure. Runtime validation
and macro-reference checks still precede any onboard selection. The old
single-Button-5 driver entry point delegates to the multi-slot save API.
No arbitrary phase/mode values
or unrelated Legacy startup profile writes are replayed.

## Software-profile application

Portable profiles in `hyperx-core` contain optional performance, named binding,
coupled primary-layout, macro-library and lighting sections. No field contains
USB reports. Omitted sections mean preserve, not reset. Source provenance and
unresolved Legacy/capture assignment IDs are never treated as configuration commands.

`PulsefireRaidSoftwareProfile::new` in the model driver validates all supplied
values/references offline. Its private fields preserve the evidence gate for
all clients. Preview reads a runtime snapshot and returns semantic changes and
warnings, without setting writes. Apply rebuilds its plan from a fresh snapshot
and validates every state-dependent patch before sending any mutation.
All paths require a usable baseline layout, valid enabled DPI and confirmed
binding records; a valid header with an empty body is not a usable snapshot.
Conservative one-second host pacing separates writes, but its hardware behavior
and the minimum necessary interval remain unverified after a failed native run.

The same baseline validator now gates every individual runtime-profile setter:
DPI, polling, ordinary bindings, the primary pair and macros. Validation happens
before patching the source image, not just before sending the result; otherwise
replacing an invalid field could hide a bad read. A macro-reference image is
also validated before uploading its definition. The shared full-image write
method validates again as defense in depth. Offline snapshot inspection remains
permissive so unusual captured images can still be investigated. Live CLI
inspection no longer runs the vendor query: baseline validation after a read is
too late to prevent its side effects.

The plan composes existing captured operations, one profile write per changed
setting family/assignment. Unknown bytes and omitted fields remain intact. A
macro definition precedes its reference, even if the reference matches already:
the timeline cannot be read from the image. Complete final image readback is
required before direct RGB. This is not an atomic transaction; an error stops
without retries or guessed rollback and earlier changes may remain. It never
selects onboard memory. The CLI handles optional bounded foreground keepalive;
RGB's stored effect may return afterward. Persistence remains a separate,
explicit, acknowledged save API.

Readback mismatches carry typed byte differences (offset including report ID,
expected value, actual value) and an explicit empty-body flag. The CLI error
renders all differences in hex, including opaque fields; this describes the
observed response and does not infer a cause or prove the device's actual state.

## Crate responsibilities

### `hyperx-app`

Shared offline application operations for CLI/TUI/GUI: bounded profile
and macro-file parsing, safe new-file serialization, section edits, file-session
baseline/diff state, model metadata and separate offline readiness. Explicit
macro resolution/omission never guesses Legacy physical targets or timeline
content. `macro_resolution_targets` shares target identity/overwrite/encoding
gates with actual offline resolution; target legality alone does not validate
playback or reconstruct a definition. Rejected model controls stay inspectable.
It delegates timeline gates to the existing model validator and refuses
implicit ID/control overwrites. Invalid/partial drafts remain inspectable.
Typed value edits validate the requested field against model metadata while
preserving other sections, unread values and unresolved provenance; UI controls
never need to rebuild a profile or guess missing defaults.
Named-library operations preserve macro identity and all unrelated fields.
Fresh IDs cannot collide with existing definitions or resolved/unresolved
references; replacing a referenced definition requires explicit confirmation,
and deletion is refused until references are explicitly removed. Empty or
unsupported timelines remain valid *file drafts*, within the file-size limit;
assignment preflight and hardware-readiness validation remain separate gates.
This is not yet a hardware session facade; existing CLI device handlers continue
to use the model driver. The app layer exposes no discovery/HID/send API.

Offline software-profile comparison belongs to `hyperx-core`: it produces
separate setting/provenance changes and matches macro IDs while preserving event
order/timings. It accepts partial files for inspection without granting a write
capability. CLI file parsing uses the bounded profile reader but does not require
the device driver's apply validator for a diff.

The protocol crate's text-capture reader preserves line/order/interface/direction
from hex/trace input and its Raid inspector recognizes existing complete codecs.
No transport handle is involved. CLI rendering labels unknown variants and
empty bodies, and compares successive same-section images without claiming
transaction correlation, successful TX, ACK completion or power-cycle persistence.
Neither tool contains a replay path; real captures stay outside Git.

The model driver owns the offline conversion of a single selected runtime RX
image to a portable partial profile. It reuses complete baseline validation and
stable public control IDs; the CLI only selects a report, renders warnings and
creates a new TOML file. It does not correlate nearby macro/RGB packets or infer
unreadable state. Macro references become non-executable unresolved assignments
that use the existing apply gate. TOML inspection is parse-first, with a separate
driver-readiness result, so unsupported/unresolved files remain inspectable
without opening transport or granting permission to apply them.

### `hyperx-core`

Platform-independent value types: USB identity, HID collection metadata,
capabilities, DPI, bindings, lighting, portable software profiles and, in later
milestones, a high-level device API. It must not depend on `hidapi` or a GUI
toolkit. Imported profiles explicitly distinguish confirmed normalized fields
from unresolved source values.

Hardware capabilities and implemented protocol operations are separate facts.
For example, Pulsefire Raid advertises onboard memory, while its write API is
limited to the fields and transaction variants established by captures.
`MacroCapabilities` describes implemented support for one physical control:
runtime and onboard modes are independent, with event and timing limits.
It is deliberately separate from hardware-level `CapabilitySet`.

### `hyperx-hid`

Owns enumeration and the `HidTransport` trait. The trait covers feature
reports, output reports and timed reads. `MockHidTransport` scripts expected TX
and queued RX reports for tests without hardware. It can inject short feature
writes and read/write errors, and records every feature TX attempt, including
unexpected/failed ones. Tests must inspect that history when asserting that a
failure stopped further I/O; merely consuming the queued script is insufficient.

The real opened-device adapter, reconnect policy and Windows-specific errors
belong here. Device drivers select an exact HID collection and must not open
the standard mouse collection unless a documented operation requires it.
The macOS build uses hidapi's shared-device mode so enumeration or future
configuration does not request exclusive ownership. CI verifies compilation
and no-device CLI behavior on ARM64 and Intel; physical-device behavior remains
unverified there.

### `hyperx-protocol`

Owns protocol-neutral wire logging, HID descriptor/capture parsers, offline
vendor-format parsers such as NGENUITY Legacy `.hxp`, and typed report
encoders/decoders.
Encoders must validate ranges, use fixed packet sizes and have golden tests.
Raw TX/RX logging is emitted only at `trace` level.
The Raid codec has one target-encoding table containing each confirmed macro
target's wire code and `MacroCapabilities`. Encoding, parsing and onboard
conversion all use this table; target gates are not duplicated in the driver.
`PulsefireRaidProfileObservation` is a read-only projection of one complete
device read-response image. It exposes the response section and validated
polling, DPI, primary-button layout and all 11 binding records, with macro
references kept distinct from definitions. It retains no raw report and has no
write conversion. The hidden lab diagnostic may display this projection after
its existing request/GET sequence; normal live device access remains gated.

### `hyperx-devices`

Owns model identities, capability declarations and per-model protocol drivers.
`pulsefire_raid` is the first module and exposes the confirmed runtime-profile
read, DPI-stage/polling/button runtime setters, volatile direct RGB and an
explicit onboard save. A model driver may depend on the core, protocol and
transport traits, but never on CLI/Tauri types. Its stage API uses zero-based
indexes internally; user-facing clients translate those to one-based numbers.
The runtime/profile entrypoints are presently suspended at the driver boundary
even if a client bypasses the CLI. Only mock-backed unit tests can run the
retained packet sequences; an integration test verifies the production API
does not attempt a feature TX/GET and direct RGB remains independent.
Device-specific, target-aware assignment evidence gates and persistent-write
validation live in the driver so a future GUI cannot bypass the CLI's
restricted writable set.
For macros, the driver exposes the codec's support through
`PulsefireRaidRuntimeAssignment::macro_capabilities(control)` and delegates
encoding validation to the codec. Adding a confirmed target must not require
another conditional in the CLI or driver.

### `hyperx-cli`

Parses commands, selects devices and renders results. It contains no vendor
packet constants.

### `hyperx-tui`

Ratatui/Crossterm presentation and terminal events only. It depends on the
shared app/core, not protocol or transport. Performance/Lighting have direct
file controls, including model-bounded DPI sliders and exact inputs, while
advanced typed TOML editors remain available in all five views. Mouse targets
come from the visible frame and are isolated by modals; mouse/terminal modes
are restored on exit. The interactive macro editor keeps a separate local
timeline draft; explicit field acceptance is distinct from committing the
definition through the shared library API. Referenced replacements need consent,
referenced deletion is blocked, and target preflight never commits or executes
events. The profile browser performs bounded, nonrecursive filesystem listing,
never parses files during navigation, and parses an explicit file selection
before asking to discard unsaved changes. Direct rename uses the shared app
metadata edit; new-file copying uses `ProfileDocument::save_as` with exclusive
creation and updates the file baseline only after a successful write.
Macro events retain chords and every timing, including unsupported
file drafts. Unknown device settings
are not represented as defaults. New-file save, resolve, omission and validation
all use the shared app API. Diff/dirty state is against a FILE baseline, not a
live mouse read; Save to mouse is unavailable. TestBackend plus real executable
headless modes exercise the UI without a terminal or hardware. Future connected
control must preserve driver evidence gates and add explicit write intent.

### `hyperx-gui`

React/TypeScript presentation in `apps/hyperx-gui`, with an optional Tauri desktop
feature in its Rust facade. The default facade is testable on Linux/macOS without
GTK/WebKit; native desktop CI targets Windows, Linux and macOS (arm64/Intel). GUI edit IPC delegates
to `hyperx-app`, not protocol codecs. Immutable snapshots expose file readiness,
diff, model controls and capabilities, never transport handles or HID paths.
The narrow, revision-checked command surface grants no runtime apply, onboard
save, raw-report or arbitrary frontend-supplied filesystem path. Native dialogs
run outside the webview thread and revision checks run again on completion.
Dirty-window close checks cannot discard a newer revision than the one confirmed.
Uncommitted frontend timeline edits also protect native close via a narrow local
draft notification. Its generation is independent of document revision/diff,
and close-dialog completion checks both tokens before destroying the window.
The browser fixture is contract-tested against Rust and stays read-only.
Binding pickers use shared `hyperx-app` candidates and isolated assignment
preflight, not JavaScript device rules. The snapshot deduplicates choice records;
control indices reference that presentation catalog, never HID/vendor slots.
Typed binding IPC preserves unrelated invalid drafts/provenance and resolves
library IDs unambiguously before editing. Null omits one file assignment and is
distinct from Disabled. Physical primary clicks remain one coupled layout.
See [GUI controls, build and backlog](gui.md).

## Device API direction

Once read/write behavior is confirmed, `hyperx-core` will expose typed traits
for capabilities rather than one giant interface. Consumers will first inspect
capabilities, then request supported facets such as performance, buttons or
lighting. Unsupported operations return structured errors.

Mutable commands will be classified:

- `ReadOnly`: descriptor or confirmed query, no persistent change;
- `Volatile`: normal runtime configuration such as direct RGB;
- `Persistent`: confirmed onboard-profile writes with explicit user intent;
- `Forbidden`: firmware, bootloader and DFU operations, with no implementation.

This classification must be enforced below the CLI so a future GUI cannot
bypass it.

## Device grouping

hidapi enumerates top-level HID collections, not physical USB devices. At
milestone 1, known collections are grouped by VID/PID. This correctly presents
one attached Pulsefire Raid but can merge two serial-less identical mice.
Before multi-device control, the Windows backend should use SetupAPI/ConfigMgr
parent device information to derive a stable physical-device key.

## Background work

No permanent service is planned. Operations that need short-lived activity run
in the foreground. OpenRGB indicates Pulsefire Raid direct RGB needs a roughly
one-second keepalive. Static direct lighting therefore exposes an explicit
duration, while software effect programs run only for their requested
foreground duration. Zone effects and phase offsets are rendered in
`hyperx-core`; the CLI supplies time and platform trigger events, and the
device driver receives only a pair of RGB colors. No effect silently installs
a service or writes a profile.
