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

There is currently no whole-profile apply command. Applying a profile and
persisting it onboard are separate operations: each device driver must validate
capabilities and every mutable field, and onboard persistence remains blocked
until its transaction and failure behavior are understood.
