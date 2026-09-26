# Software profile format

OpenHyperX software profiles are TOML files owned by `hyperx-core`. They are
application data, not HID reports and not onboard-memory images. Reading or
creating one never opens a device.

## NGENUITY Legacy import

The offline importer currently accepts exported `.hxp` files and NGENUITY
Legacy's internal preset representation when their embedded format version is
`40`. This is the format observed in Microsoft Store NGENUITY Legacy
`5.38.0.0` (`9P1TBXR6QDCX`), not the format of current NGENUITY:

```text
hyperx-cli profile inspect-ngenuity-legacy SOURCE.hxp
hyperx-cli profile diff-ngenuity-legacy BEFORE.hxp AFTER.hxp
hyperx-cli profile import-ngenuity-legacy SOURCE.hxp OUTPUT.toml
```

Current NGENUITY profiles have not been collected or identified yet. They
must get a separate parser and command namespace if their format is confirmed;
the Legacy parser must not be extended by guessing that the formats match.

`diff-ngenuity-legacy` reports normalized changes to decoded DPI and macro
fields, then lists exact byte offsets in the embedded presets. It compares
source macro references by their position rather than their regenerated
identifiers. The raw section deliberately remains available for finding
polling, lighting and assignment fields that are not decoded yet. By default
it prints at most 256 changed bytes; `--all-raw` removes that display limit. If
file lengths differ, offsets after the first insertion/removal may be shifted.

The output path must not already exist. The importer currently normalizes:

- preset name;
- DPI values and RGB colors;
- Play Once macros containing known USB HID keyboard transitions;
- left, right and middle mouse-button macro transitions;
- Standard Timing overrides and recorded per-event timing.

The import is deliberately marked `partial = true`. Version-40 files examined
so far do not provide a decoded physical-device identity, and OpenHyperX has
not yet mapped their polling, lighting or physical key-assignment fields.
Source assignment identifiers and macro references are retained under
`unresolved_button_assignments` so later decoders can resolve them without
silently inventing a target.

The observed source active-stage value is retained as
`dpi.source_active_stage`. `dpi.active_stage` remains absent until a controlled
Legacy export comparison establishes whether NGENUITY Legacy stores that field as a
zero-based index, a one-based index or another enum.

## Profile shape

A shortened imported profile looks like this:

```toml
name = "Base Settings"
device = "pulsefire-raid"
partial = true

[source]
format = "ngenuity-legacy-hxp"
format_version = 40

[dpi]
source_active_stage = 1

[[dpi.stages]]
x = 800
y = 800
color = "#2B00FF"

[[macros]]
source_id = "00112233445566778899aabbccddeeff"
name = "New Macro"
playback = "once"

[[macros.events]]
type = "key-down"
key = "a"
delay_ms = 300

[[macros.events]]
type = "key-up"
key = "a"
delay_ms = 300
```

`active_stage`, when known, is a zero-based OpenHyperX index. `source_id`
values are opaque provenance identifiers and must not be interpreted as
physical controls without evidence.

## Validate, preview and apply

```text
hyperx-cli profile validate examples/profiles/pulsefire-raid.toml
hyperx-cli profile apply examples/profiles/pulsefire-raid.toml --dry-run
hyperx-cli profile apply examples/profiles/pulsefire-raid.toml
hyperx-cli profile apply examples/profiles/pulsefire-raid.toml --lighting-duration 30
```

The example changes DPI, bindings and lighting: review or copy/edit it before
applying. `validate` is entirely offline. `apply --dry-run` opens only the
configuration collection and uses the confirmed runtime read sequence. Its
selector/request reports do not modify settings, send RGB, initialize a session
or select onboard memory. Close NGENUITY (also Legacy's tray process), OpenRGB
and other writers before preview/apply; do not change settings concurrently.

`name` and `device` are required; `partial` defaults to false. There is no
OpenHyperX `format_version`. Omitted settings remain untouched, regardless of
`partial`; this flag never implies resetting missing values. Unknown fields
and variants are rejected rather than silently ignored.

A minimal polling-only profile:

```toml
name = "Polling only"
device = "pulsefire-raid"

[polling]
hz = 500
```

The Raid driver supports these optional sections:

- `[polling] hz = 125 | 250 | 500 | 1000`.
- `[dpi]` with 1–5 `[[dpi.stages]]` containing `x`, `y`, `color`. Axes must
  be equal: independent X/Y writes are not hardware-confirmed. Values must be
  200–16000 in steps of 50. Optional `active_stage` is zero-based; omitting it
  preserves the current index. Removing that index without supplying a new one
  is rejected before any setting writes. `source_active_stage` is provenance
  only and is never used to select a stage.
- `primary_buttons = "standard" | "swapped"`, one coupled operation. Individual
  primary-button assignments are rejected.
- `[buttons.CONTROL]`, where controls are `wheel-click`, `button4`, `button5`,
  `button6`, `button7`, `button8`, `dpi`, `wheel-tilt-left`, `wheel-tilt-right`.
  Example bindings: `{ type = "mouse", action = "back" }`,
  `{ type = "keyboard", key = "left-shift" }`,
  `{ type = "multimedia", action = "volume-up" }`,
  `{ type = "windows-shortcut", action = "copy" }`, `{ type = "disabled" }`,
  `{ type = "macro", id = "ab" }`. Existing target-specific gates still apply.
- `[[macros]]` with `id`, `name`, `playback` and `[[macros.events]]` using
  [the macro timeline model](macro-format.md). Legacy's `source_id` is accepted
  instead of `id` and retained when serialized. IDs must be nonempty and unique;
  references resolve within this file. All definitions are validated, including
  unassigned macros (warned about, not uploaded). Current evidence limits remain:
  Once on Button 4/5, Toggle/Hold only on runtime Button 4, 14 balanced events
  maximum, and 9999 ms maximum per delay.
- `[lighting] mode = "solid"` with `[lighting.zones] wheel = "#RRGGBB"` and
  `logo = "#RRGGBB"`. Both zones are required: current direct colors cannot be
  read, so an omitted zone cannot safely be preserved. Black means off. Other
  effects and partial one-zone lighting are rejected in this profile path.

Unresolved Legacy/capture assignments block validation/application. Explicitly resolve
them into named controls or remove the entries to request only known fields;
opaque IDs are never interpreted as physical buttons. Partial profiles without
unresolved assignments can apply their explicitly supplied settings.

## Offline comparison and capture inspection

```text
hyperx-cli profile diff BEFORE.toml AFTER.toml
hyperx-cli profile inspect PROFILE.toml
hyperx-cli profile inspect-capture reports.hex
hyperx-cli profile inspect-capture runtime.log --raw --all-raw
hyperx-cli profile export-capture runtime.log captured.toml --report 3
```

`diff` compares OpenHyperX TOML files, distinct from `diff-ngenuity-legacy` for
`.hxp`. Settings and metadata are reported separately. DPI values/colors,
explicit active selection, polling, primary layout, bindings and lighting
zones are compared. Macro IDs are stable comparison keys: library/table order
is ignored, event order, event type/input, playback and every `delay_ms` are not.
Known keyboard aliases compare by HID usage; unknown names stay distinct.
Duplicate or empty macro IDs are rejected instead of dropping a definition.

`<not present>` means absent in that file, not disabled, black, a default or a
device reset. Omitted setting sections/buttons preserve current state on apply;
removing an event changes the macro timeline. Names, source metadata, partial
flags, unresolved assignment IDs and unconfirmed source-active values are
provenance, not hardware instructions. Partial/unresolved profiles can be
compared even when they cannot be applied. Comparison is not capability
validation, a device-state read, an apply plan or a guarantee that either file
can be applied. Files retain the same 1 MiB bounded UTF-8/TOML reader.

`inspect-capture` accepts a 16 MiB maximum UTF-8 text file containing either
one complete hex report per line (optional TX/RX, comments) or an OpenHyperX
`--trace` log. UTF-8 BOM, CRLF and ANSI console colors are supported. Trace
mode preserves source line/order/direction/interface, ignores unrelated console
lines and rejects malformed raw-report lines or mismatched declared report IDs.
Hex exports have unknown interface/direction where not supplied; neither is
invented. Timestamps and USB transfer descriptors are not inferred from text.
Binary `.pcapng`, UTF-16 PowerShell exports and arbitrary Wireshark columns are
not supported; export normalized UTF-8 reports first.

Known Raid packet families are recognized only using complete confirmed codec
layouts: runtime/onboard profile images, selectors/read requests, the fixed
volatile session phases, direct two-zone RGB, indexed Solid save snapshots,
runtime macros and exact observed acknowledgement patterns. Macro inspection
shows individual transitions and delays. Profile references alone never reveal
macro events/modes. Unknown, unsupported or malformed variants remain diagnostic
and never enter a send path. Current standalone rainbow and onboard macro
definition decoding are not added by this tool.

Profile settings are decoded permissively for investigation, but invalid runtime
baselines and empty bodies are marked clearly. Byte diffs compare successive
images within the same section and include envelope/opcode and opaque bytes;
this is NOT automatic write/readback correlation. By default the first 32
changed offsets are printed; `--all-raw` shows all, and `--raw` prints full
packets. Raw bytes/collection indexes do not establish physical-device identity;
the caller must supply isolated Raid reports. TX trace lines show attempted
sends, not transport success. ACK shapes prove neither transaction success nor
persistence. A detected empty snapshot does not diagnose the hardware failure.
Inspection/compare never discover/open HID, replay reports or change files.
Capture logs can contain macro keystrokes; keep real logs outside Git.

### Inspect a TOML profile

`inspect` uses the bounded TOML reader without requiring apply readiness. It
prints polling, DPI axes/colors and zero-based stage selection, primary layout,
each supplied binding, lighting zones, omissions and unresolved source entries.
Macros are shown in their original event order with down/up transitions,
individual delays and a cumulative timeline; zero-delay events allow chords.
`delay_ms` is after the event, including after the final event. Cumulative times
describe the file, not measured physical playback.

The final device-validation result is separate from successful inspection.
Parseable unsupported devices/settings, empty profiles, duplicate macro IDs
and unresolved imports can be displayed with `NOT READY`; inspection returns
success because the file was inspected, not because it can be applied. Malformed
TOML, unknown schema fields and files exceeding 1 MiB fail. Use `profile validate`
for a nonzero status on a profile that is not ready. Neither command checks
hardware state or guarantees the experimental composed-apply flow works.

### Export one runtime RX snapshot

Run `inspect-capture` first and explicitly choose `--report NUMBER`, a positive
one-based report index (not a source line or Wireshark frame). Export never
chooses a latest/best image automatically or falls back if the chosen image is
invalid. It requires explicit RX metadata, a complete runtime device-read
response and the existing full-baseline validator. Host-write/onboard images,
other collections, empty bodies and unknown/invalid settings are refused before
the destination is created. Isolated normalized `RX` lines may omit interface
metadata; they remain an explicit caller assertion that these are Raid reports,
not proof of model identity. Directionless hex cannot be exported.

The new TOML has `partial = true` and includes confirmed DPI stages/colors,
active selection, polling, the atomic primary pair and ordinary bindings.
Independent X/Y values are retained faithfully for inspection, but current
device validation still rejects those writes. Lighting remains absent, not
black/off: this profile image cannot recover current wheel/logo colors/effects.
Opaque vendor bytes are omitted; the file is not an exact image backup.

Macro references cannot reveal their definitions. Export omits the executable
binding and retains an `unresolved_button_assignments` diagnostic entry:

```toml
[[unresolved_button_assignments]]
source_id = "runtime:button5"
```

This exporter-generated label identifies the omitted public control, not a
vendor slot or an invented executable macro ID. Apply/validate remain blocked
until you provide a real macro definition and binding and remove its resolved
diagnostic entry, or explicitly remove the entry to preserve the omitted control.
Nearby macro uploads, ACKs or RGB packets are not used to guess missing state.

Export serializes and validates its selected image before using `create_new`
for the output; it never overwrites an existing file, including the input itself.
An I/O failure during writing may leave an incomplete new file and reports that
fact. Report index, source line and available interface are retained in comments
along with warnings, without private capture paths. There is no invented source
format version or root `format_version`. Comments are not executable settings
and may be lost on reserialization; partial/unresolved markers are TOML fields.
Nothing is applied or saved onboard, and historical captured values must not be
presented as a fresh read or as device recovery. Keep exports outside Git too.

## Runtime safety and persistence

Every path requires a usable baseline layout, enabled DPI in range and confirmed
binding records; a valid header with an empty body blocks even RGB-only apply.
Individual DPI, polling and button setters use the same full-baseline gate,
including before macro uploads. They will not "repair" an invalid snapshot by
overwriting the requested field. Read-only `info` remains available for inspection.
The driver plans and validates **all** steps against a fresh snapshot before
the first mutation. Existing capture-backed operations run in deterministic
order: polling, DPI, primary layout, individual assignments, then direct RGB.
Unknown bytes and omitted fields survive every intermediate profile write.
Matching ordinary settings are skipped, preserving raw binding aliases.
Steps have a conservative one-second host wait. This is not a decoded firmware
requirement; the minimum safe interval is unknown.

An unchanged macro reference does not prove unchanged events/playback: every
requested macro uploads its definition immediately before its profile reference.
After profile writes, a complete runtime-image readback must match the planned
image before RGB is sent. This verifies profile bytes, **not** unreadable macro
timelines, physical playback, LED output or power-cycle persistence.
On mismatch, the error lists every changed report offset with expected/actual
hex values, including unknown fields, and identifies an all-zero body explicitly.
For example: `0x0018 expected=0x01 actual=0x00`. These are observations, not a
diagnosis or a claim that the profile was restored.

Multi-setting apply is not atomic. It stops on the first error; earlier changes
may remain. There is no automatic retry or blind rollback. Inspect trace/state
before deciding how to recover.

Without `--lighting-duration`, direct Solid RGB is sent once and may revert
after about one second. The option keeps it alive in the foreground for 0–86400
seconds, never a service. It requires lighting and conflicts with `--dry-run`.

No apply variant saves onboard or writes firmware. Persist supported settings
separately using `profile save-to-mouse --confirm`, explicit wheel/logo colors,
and definitions for all referenced supported macros. Runtime repeat support
does not grant onboard repeat support.

Opt-in native Windows polling-only verification, after closing other writers:

```powershell
.\scripts\check-windows.ps1 -VerifySoftwareProfilePolling
```

It validates/previews, changes only polling, verifies the full image, restores
the original rate and checks a no-op. Logs and test profiles remain in a unique
Windows temporary directory. No DPI, binding, macro, RGB or onboard command is
sent. An ambiguous failure stops rather than restoring blindly. Normal build
and CI checks do not enable hardware writes.

**Hardware verification currently blocked:** the first native polling-only
apply test sent the expected image but its immediate readback had a valid
`07 81 04` header and an empty body. An independent read and the already
confirmed non-persistent session initialization did not restore usable reads.
No further setting writes or onboard save were attempted, and restoration
could not be verified. Physical USB reconnect and a valid baseline read are
required before continuing mutable tests. The new one-second pacing is an
untested precaution, not a demonstrated fix. Do not treat whole-profile apply
as hardware-validated yet.
