---
name: add-hyperx-device
description: Add or extend support for a HyperX USB/HID device in this Rust workspace, from safe discovery through capture-backed protocol commands and tests. Use for new device models, capabilities, packet codecs, HID transport work, or device-specific CLI commands; do not use for firmware, bootloader, or DFU work.
---

# Add a HyperX Device

Work in small evidence-backed increments. Preserve the separation between
`hyperx-core`, `hyperx-hid`, `hyperx-protocol`, `hyperx-devices`, and clients.

Before changing a driver, read the relevant sections of:

- `docs/architecture.md` for crate and safety boundaries;
- `docs/research.md` for confirmed facts and current unknowns;
- `docs/adding-device.md` for repository conventions.

When implementing any vendor report or persistent operation, also read
[references/protocol-safety.md](references/protocol-safety.md) and follow its
evidence gate.

## Required approach

1. Inspect the current worktree and preserve unrelated changes.
2. Identify the exact VID/PID and HID collection tuple from hardware output or
   an attributable primary/public implementation. Never infer IDs or interfaces
   from a sibling HyperX model.
3. Add the model descriptor and hardware capabilities in `hyperx-devices`.
   Keep “hardware supports it” distinct from “the driver implements it.”
4. Enumerate first. Open only the verified vendor collection; leave standard
   mouse and keyboard collections alone.
5. Implement typed encoders/decoders and driver operations only when every
   meaningful byte is supported by evidence. Unknown data requires a targeted
   capture experiment and a precise TODO, not a guessed packet.
6. Add golden packet tests, invalid-input tests, capability tests, and an exact
   TX/RX test using `MockHidTransport` before testing on hardware.
7. Update `docs/research.md` with sources, local observations, confidence, and
   remaining unknowns. Attribute GPL or other prior art and independently write
   this project's implementation.
8. Run the Windows checks from WSL with
   `scripts/check-windows.ps1`. The Windows binary, not a Linux build, is the
   acceptance target.

Mutable hardware testing must state what will change and whether it is volatile
or persistent. Exercise the narrowest confirmed operation and verify normal
cursor/buttons still work afterward. Do not run NGENUITY and this project as
simultaneous writers.

Never implement or transmit firmware flashing, firmware update, bootloader, or
DFU commands. Do not replay an unknown report. A future raw-send facility must
be isolated, require explicit `--unsafe`, warn immediately, and block known
firmware paths; its existence is not required for adding a device.

Finish with a concise report of implemented capabilities, commands actually
tested on Windows hardware, checks run, and protocol areas still blocked on
captures.
