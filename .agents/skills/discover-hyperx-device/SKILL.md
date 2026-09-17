---
name: discover-hyperx-device
description: Perform read-only discovery of a HyperX USB/HID device on Windows from WSL. Use to identify VID/PID, HID collections, descriptors, supported-device matching, or the USBPcap controller; do not use for protocol writes, behavioral captures, or feature implementation.
---

# Discover a HyperX Device

Produce a small handoff containing the exact device identity, configuration
collection, evidence source, and only when requested for an upcoming capture,
the USBPcap controller. Do not hand off a cached numeric device address.

Start with the relevant identity/topology section of `docs/research.md`. Stable
facts already confirmed for the exact model do not need to be rediscovered or
searched again. Run the native Windows `hyperx-cli devices` only when current
physical enumeration matters. Do not open standard mouse or keyboard
collections.

Keep discovery read-only and cheap:

- issue one aggregated Windows diagnostic invocation where possible;
- do not list processes, inspect USBPcap, or locate `tshark` unless the requested
  outcome needs those facts;
- resolve capture routing only immediately before a capture;
- treat the USBPcap device address as ephemeral even without an obvious reboot
  or reconnect;
- prefer `scripts/capture-windows.ps1 -VendorId ... -ProductId ...`, which
  resolves the address and starts the bounded capture under one elevation;
- when only the current address is needed, use
  `scripts/locate-usbpcap-device.ps1` once instead of testing candidate
  addresses;
- after reboot/reconnect, use one deterministic topology enumeration and verify
  `0951:16E4` (or the exact target identity) before handing routing off;
- never trial guessed USB addresses with repeated captures;
- if installed tooling cannot enumerate routing deterministically, stop and
  improve the discovery helper or ask for one exact UI/export result instead of
  probing.

The locator may take one short temporary descriptor snapshot, delete it, and
return only the matching address. This skill never records a behavioral
experiment, closes applications, sends feature/output reports, changes device
state, or performs implementation work. Hand protocol or driver work to
`$add-hyperx-device` after discovery is complete.
