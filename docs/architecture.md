# Architecture

## Goals

The core design separates device behavior from USB transport and presentation.
A future Tauri UI must not know paths, report IDs or packet layouts. Adding a
mouse should normally mean adding one driver and registry entry, not changing
the CLI or GUI.

```text
CLI now / Tauri later
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
capture. The same runtime framing is independently captured for Button 4;
other targets, playback modes, longer macros and unconfirmed mouse events
remain absent. A separate offline parser reads confirmed fields from
NGENUITY Legacy version-40 `.hxp` presets and converts them to a partial
software profile without opening a HID device. Current NGENUITY is a distinct,
unsupported source format and must not share the Legacy parser without
evidence.

The driver also owns the acknowledged Pulsefire Raid onboard-save transaction.
It validates the complete runtime source before selecting onboard memory, reads
the existing onboard image, and patches only the confirmed DPI, polling and 11
button-record fields. Unknown onboard bytes are preserved. A referenced Button
5 macro requires its caller-supplied typed definition because the event stream
cannot be recovered from the profile image. Every save-stage interrupt
acknowledgement is checked and an unexpected response aborts without retry.
The acknowledgement read is now armed before each feature report on Windows
to reduce a possible read-posting race; hardware probing showed that this
alone does not restore missing ACKs. A
separate non-persistent CLI probe exercises only the known runtime selector
and its acknowledgement path.
The save path rejects a runtime Button 4 macro before selecting onboard memory;
the onboard format has now been captured, but is not enabled in OpenHyperX
until the acknowledgement path is revalidated.
Feature reports use the interface-1 configuration collection, while the
eight-byte acknowledgements are read through a separate interface-2 HID
handle. The complete path was hardware-validated across a physical power-cycle,
but a later session lacked ACKs and aborted before onboard selection. Unknown
Legacy startup reports remain research-only pending lifecycle evidence.

## Crate responsibilities

### `hyperx-core`

Platform-independent value types: USB identity, HID collection metadata,
capabilities, DPI, bindings, lighting, portable software profiles and, in later
milestones, a high-level device API. It must not depend on `hidapi` or a GUI
toolkit. Imported profiles explicitly distinguish confirmed normalized fields
from unresolved source values.

Hardware capabilities and implemented protocol operations are separate facts.
For example, Pulsefire Raid advertises onboard memory, while its write API is
limited to the fields and transaction variants established by captures.

### `hyperx-hid`

Owns enumeration and the `HidTransport` trait. The trait covers feature
reports, output reports and timed reads. `MockHidTransport` scripts expected TX
and queued RX reports for tests without hardware.

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

### `hyperx-devices`

Owns model identities, capability declarations and per-model protocol drivers.
`pulsefire_raid` is the first module and exposes the confirmed runtime-profile
read, DPI-stage/polling/button runtime setters, volatile direct RGB and an
explicit onboard save. A model driver may depend on the core, protocol and
transport traits, but never on CLI/Tauri types. Its stage API uses zero-based
indexes internally; user-facing clients translate those to one-based numbers.
Device-specific, target-aware assignment evidence gates and persistent-write
validation live in the driver so a future GUI cannot bypass the CLI's
restricted writable set.

### `hyperx-cli`

Parses commands, selects devices and renders results. It contains no vendor
packet constants.

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
