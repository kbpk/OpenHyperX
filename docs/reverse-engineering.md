# Reverse-engineering workflow

The rule is one deliberate UI change per capture. Keep firmware-update prompts
closed and do not capture or replay firmware, bootloader or DFU sessions.

## Capture procedure

1. Record mouse part number, firmware/release value, Windows version, exact
   NGENUITY product line (`current` or `legacy`), version and install source.
2. Close other software that may write RGB or profiles.
3. Start USBPcap on the controller containing `0951:16E4`, then open Wireshark.
4. Start the recorded NGENUITY variant and wait for background traffic to
   settle.
5. Change exactly one value once, wait several seconds, then stop the capture.
6. Save the original `.pcapng` outside Git if it contains unrelated USB data.
7. Export only relevant HID control/interrupt payloads as ordered hex lines.
8. Repeat the same transition at least twice and compare it with a no-op capture.

After identifying the USBPcap interface and device address with extcap, run a
bounded device-only capture from WSL:

```bash
powershell.exe -NoProfile -ExecutionPolicy Bypass \
  -File "$(wslpath -w "$PWD/scripts/capture-windows.ps1")" \
  -Interface '\\.\USBPcap1' \
  -VendorId 2385 \
  -ProductId 5860 \
  -DurationSeconds 5 \
  -OutputPath '%TEMP%\openhyperx-dpi-800-900.pcapng'
```

The controller is an example and can change after moving the device to another
port. Identity mode takes a temporary descriptor snapshot on that controller,
resolves `0951:16E4` to its current ephemeral address, deletes the snapshot,
then records only that address. This avoids reusing a stale address or probing
guesses. The script refuses to overwrite an existing file, stops after at most
300 seconds, expands Windows `%NAME%` environment variables in the output path,
and requests elevation only for the bounded capture process. Using `%TEMP%`
avoids hard-coding a Windows account name when invoking PowerShell from Bash. A
header-only pcapng is treated as a failed capture.

USBPcap may contain traffic from other devices on the same host controller.
Treat raw captures as potentially sensitive. The repository ignores `*.pcap`,
`*.pcapng`, `*.etl` and `captures/private/` by default.

For a matrix of related choices, use a reviewed JSON plan with the series
runner. It retains one UI transition per capture while using a single elevated
PowerShell session:

```bash
powershell.exe -NoProfile -ExecutionPolicy Bypass \
  -File "$(wslpath -w "$PWD/scripts/capture-series-windows.ps1")" \
  -Interface '\\.\USBPcap1' \
  -VendorId 2385 \
  -ProductId 5860 \
  -DurationSeconds 10 \
  -PlanPath "$(wslpath -w "$PWD/scripts/capture-plans/pulsefire-raid-button4-mouse-functions.json")"
```

The elevated window shows one instruction at a time. Press Enter, return to
NGENUITY Legacy and perform exactly the named change during that capture. The
runner resolves the current device address again for every file, rejects an
existing series prefix, checks every file through `capture-windows.ps1`, and
writes a prefixed manifest beside the raw captures directly under `%TEMP%`.
Keeping the files directly in the caller's existing Temp directory avoids the
different ACL inheritance of a directory created by an elevated process. Keep
the raw files outside Git.

## Non-persistent save-ACK diagnostic

Build the native Windows CLI first. With NGENUITY Legacy and other writers
closed, `profile check-save-ack` tests the existing runtime selector and its
interrupt-IN acknowledgement without selecting or writing onboard memory.
Use the fixed-purpose wrapper when a USB capture is needed:

```bash
powershell.exe -NoProfile -ExecutionPolicy Bypass \
  -File "$(wslpath -w "$PWD/scripts/capture-save-ack-windows.ps1")" \
  -Interface '\\.\USBPcap1' \
  -OutputPath '%TEMP%\openhyperx-save-ack-probe.pcapng'
```

The wrapper refuses competing writers, resolves the current device identity,
starts an eight-second capture, then invokes exactly one known-selector probe.
It does not expose arbitrary command execution or retry a failed probe. Inspect
the captured transfer and endpoint `0x83` responses before another experiment.
A timeout must never be treated as permission to continue an onboard save.

For ACK lifecycle research, isolate app launch and close-from-tray in separate
captures without settings changes. Legacy may automatically reapply its runtime
profile when opened; inspect all feature reports, not just the new opcode.
Unknown startup/closure reports remain non-replayable until repeated captures
establish their fields and effects.

The fixed Raid session startup has since been captured twice; see
`docs/research.md` for complete bytes, ACKs, pacing and power-cycle observations.
To validate this narrow initializer independently, close all writers, unplug
and reconnect the mouse, then use the same wrapper with `-InitializeSession`
and a new output path. It sends only the two constant startup reports and the
known runtime-selector probe. Do not use it as an automatic retry after an
ambiguous save. No profile, macro, lighting snapshot or onboard data is written.

## Fixed-purpose Button 4 onboard verification

`scripts/verify-button4-save-windows.ps1` is a persistent lab test, separate
from the non-persistent ACK probe. Use it only with operator agreement and
the recorded local state: Button 5 has the exact `coverage-recorded-timing`
timeline, wheel is off, and logo is `#0000FF`. It requires `-ConfirmSave`.
`-Action save-ab` assigns the captured AB/20-ms/Play-Once runtime macro to
Button 4, supplies both definitions, and performs one acknowledged save.
`-Action restore-back` restores Button 4 Back and saves only the existing
Button 5 macro definition. Both actions capture all traffic through identity
mode, perform independent before/after reads, and refuse competing writers.
On any failure, stop and review the entire capture; there is no retry or
automatic restoration. After a successful capture review, physically reconnect
USB with Legacy closed and test Button 4 in Notepad (AB) or navigation (Back).

## Minimal text fixture format

Until a capture parser is justified, normalize reports into a small reviewable
text file:

```text
# experiment: rgb static FF0000
# device: 0951:16E4 release 1124
# interface: MI_01 usage FF01:0001
TX 07 0A FF 00 00 FF 00 00 A0 00 00 00
```

Store the complete report, not only differing bytes. Add timestamp/order only
when it matters. Never place firmware traffic in a replayable fixture.

Compare two normalized files with:

```bash
powershell.exe -NoProfile -ExecutionPolicy Bypass \
  -File "$(wslpath -w "$PWD/scripts/windows-cargo.ps1")" \
  run --target x86_64-pc-windows-msvc --bin hyperx-cli -- \
  decode-capture captures/baseline.hex captures/changed.hex
```

The command reports changed report numbers, lengths, directions and byte
offsets. It intentionally does not parse `.pcapng`; export only the relevant
payloads first.

## Experiment matrix

- DPI: `800→900`, `900→1000`, `1000→1600`
- polling: `125→250`, `250→500`, `500→1000`
- RGB: `000000`, `FF0000`, `00FF00`, `0000FF`, `FFFFFF`
- one button: Back, Forward, Volume Up, Volume Down, Disabled

For each series, compare byte positions and test likely encodings only after a
consistent pattern appears. Endianness, units, checksums, profile indexes and
apply/save transactions must be established separately.

## Promotion checklist for a new command

A mutable command can enter a device driver only when:

- its interface and report type are known;
- all constant and variable bytes have an evidence note;
- bounds and legal values are explicit;
- a golden encoder test exists;
- a `MockHidTransport` test checks exact TX and any expected RX;
- volatile versus persistent behavior is known;
- the report is not related to firmware, bootloader or DFU;
- failure and disconnect behavior has been tested.

Raw send is intentionally absent. If introduced for lab work, it must be in a
separate unsafe command path, require `--unsafe`, print a prominent warning and
reject known firmware/bootloader interfaces and opcodes. “Unknown” is not the
same as “safe.”
