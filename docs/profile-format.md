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

Unresolved Legacy assignments block validation/application. Explicitly resolve
them into named controls or remove the entries to request only known fields;
opaque IDs are never interpreted as physical buttons. Partial profiles without
unresolved assignments can apply their explicitly supplied settings.

## Runtime safety and persistence

Every path requires a usable baseline layout, enabled DPI in range and confirmed
binding records; a valid header with an empty body blocks even RGB-only apply.
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
