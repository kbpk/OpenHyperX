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
reads, and the capture-backed volatile RGB operation. It does not expose
setters whose packets are still unknown.

## Crate responsibilities

### `hyperx-core`

Platform-independent value types: USB identity, HID collection metadata,
capabilities and, in later milestones, DPI, bindings, lighting, profiles and a
high-level device API. It must not depend on `hidapi` or a GUI toolkit.

Hardware capabilities and implemented protocol operations are separate facts.
For example, Pulsefire Raid advertises onboard memory, but no onboard write API
will exist until the command is verified.

### `hyperx-hid`

Owns enumeration and the `HidTransport` trait. The trait covers feature
reports, output reports and timed reads. `MockHidTransport` scripts expected TX
and queued RX reports for tests without hardware.

The real opened-device adapter, reconnect policy and Windows-specific errors
belong here. Device drivers select an exact HID collection and must not open
the standard mouse collection unless a documented operation requires it.

### `hyperx-protocol`

Owns protocol-neutral wire logging, HID descriptor/capture parsers, and typed
report encoders/decoders.
Encoders must validate ranges, use fixed packet sizes and have golden tests.
Raw TX/RX logging is emitted only at `trace` level.

### `hyperx-devices`

Owns model identities, capability declarations and per-model protocol drivers.
`pulsefire_raid` is the first module and currently exposes only volatile direct
RGB. A model driver may depend on the core,
protocol and transport traits, but never on CLI/Tauri types.

### `hyperx-cli`

Parses commands, selects devices and renders results. It contains no vendor
packet constants. `devices` is currently the only command.

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

No permanent service is planned. Operations that need short-lived activity can
run in the foreground. OpenRGB indicates Pulsefire Raid direct RGB needs a
roughly one-second keepalive; an RGB CLI command must therefore either expose
an explicit duration/foreground mode or use a separately confirmed persistent
profile command. It must not silently install a service.
