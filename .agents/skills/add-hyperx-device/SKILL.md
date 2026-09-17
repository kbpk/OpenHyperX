---
name: add-hyperx-device
description: Implement or extend capture-backed HyperX protocol, driver, and CLI support after device identity and HID topology are known. Use for codecs, mutable commands, captures, and hardware verification; use discover-hyperx-device instead for read-only discovery. Never use for firmware, bootloader, or DFU work.
---

# Develop a HyperX Device

Assume discovery is already complete. Reuse the exact identity, collection
selector, and USBPcap controller recorded in `docs/research.md` or handed off
by `$discover-hyperx-device`. Never trust a cached numeric USBPcap device
address: it can change without an obvious reboot or reconnect. Do not repeat
VID/PID research, HID enumeration, process checks, or controller discovery
unless the relevant state could actually have changed. If identity or topology
is missing, stop this workflow and perform discovery once.

Preserve the separation between `hyperx-core`, `hyperx-hid`,
`hyperx-protocol`, `hyperx-devices`, and clients. Read only the task-relevant
sections of:

- `docs/architecture.md` for crate and safety boundaries;
- `docs/research.md` for confirmed facts and current unknowns;
- `docs/adding-device.md` for repository conventions.

For any capture, vendor report, mutable operation, or hardware test, read
[references/protocol-safety.md](references/protocol-safety.md). For a capture,
also read the relevant experiment section of `docs/reverse-engineering.md`.

## Workflow

1. Inspect the worktree and the narrow existing API involved in the request.
2. Separate confirmed bytes from hypotheses. An unknown requires one targeted
   capture experiment and a precise TODO, never an inferred send path.
3. Implement typed codecs and validate all input before discovery or HID I/O.
4. Add golden packet, invalid-input, capability, and exact `MockHidTransport`
   TX/RX tests before a hardware test.
5. Test the narrowest confirmed operation, then record the result and remaining
   unknowns in `docs/research.md`.
6. Run format and Clippy on Linux. Once code is stable, run one aggregated
   native Windows test/build/smoke invocation with `scripts/check-windows.ps1`.

Do offline codec and fixture work without touching Windows hardware. When a
Windows call is necessary, combine related read-only checks and the one planned
operation instead of issuing a sequence of exploratory commands. A capture
must use the identity mode of `scripts/capture-windows.ps1`; it resolves VID/PID
to the current address immediately before recording, under the same elevation.
Never probe guessed USB addresses one by one or ask the operator to repeat an
experiment before fully analyzing the capture already obtained.

For a manual capture:

1. State the single UI transition and have the operator ready before starting.
2. Run one bounded identity-resolving capture, for example with
   `-VendorId 2385 -ProductId 5860`; do not run the locator separately first.
3. After it returns, explicitly verify the file is larger than a pcapng header
   and contains traffic for the resolved device. Do not infer success from the
   parent PowerShell exit code.
4. Before requesting another operator action, extract and inspect all candidate
   feature reports and prove whether the expected transition is present.

Keep raw captures outside Git and treat them as potentially sensitive.

Mutable hardware testing must state what will change and whether it is volatile
or persistent. Check competing writers in the same aggregated invocation as
the test when practical. Do not run NGENUITY and this project as simultaneous
writers.

Never implement or transmit firmware flashing, firmware update, bootloader, or
DFU commands. Do not replay an unknown report. A future raw-send facility must
be isolated, require explicit `--unsafe`, warn immediately, and block known
firmware paths; its existence is not required for adding a device.

Finish with the implemented capability, commands actually tested on hardware,
checks run, and protocol areas still blocked on captures.
