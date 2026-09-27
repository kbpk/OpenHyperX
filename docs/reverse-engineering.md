# Reverse-engineering workflow

The rule is one deliberate UI change per capture. Keep firmware-update prompts
closed and do not capture or replay firmware, bootloader or DFU sessions.

**Runtime-query safety incident (2026-09-27):** do not use the old `info`,
runtime getters, dry-run, ACK probes or save wrappers to test a cold device.
The selector `07 03 04 64` alone later reproduced physical loss of operation,
without request `81` or GET; USB reconnect recovered it. These CLI paths are blocked before
discovery; `info` reads only standard metadata/descriptor. Historical workflows
below remain as evidence, not current authorization to replay. Analyze existing
captures offline before designing another explicitly approved hardware test.

## Isolated GET_REPORT-only lab experiment

`lab raid-feature-get --unsafe` is a hidden Windows diagnostic, not an override
for suspended runtime commands. It opens only Raid `0951:16E4`, release `1124`,
interface 1 / usage `FF01:0001`, and requires the exact recorded 24-byte feature
descriptor. It issues one GET_REPORT for ID `07`, buffer length 264, **without
any preceding SET_REPORT**, selector, initializer, ACK probe or profile write.
There is no arbitrary report ID, payload, retry or automatic restoration.
A returned packet is opaque evidence, not a validated current profile; even
successful HID transport does not prove that physical input is unaffected.

Use only with explicit operator agreement, writers closed and physical USB
reconnect available. After building the native Windows CLI, the fixed wrapper
resolves the current address and records a five-second device-only capture:

```bash
powershell.exe -NoProfile -ExecutionPolicy Bypass \
  -File "$(wslpath -w "$PWD/scripts/capture-raid-lab-windows.ps1")" \
  -Interface '\\.\USBPcap1' \
  -OutputPath '%TEMP%\openhyperx-passive-feature-get.pcapng' \
  -ConfirmPassiveGet
```

The controller is an example, not a persistent address. The wrapper refuses
existing output/log files and competing writers, checks command availability
through help before recording, and bounds the child CLI to three seconds.
Review the actual capture for exactly one feature GET (`A1 01`, value `0307`,
index 1, length 264) and **zero feature SET requests**. Retain the raw response,
including an empty or unrecognized body. tshark's `usb.data_fragment` can omit
GET responses: use raw frame bytes and the USBPcap header length as needed.
Then ask the operator about cursor, primary clicks and lighting. On any failure,
stop; unplug/replug is an operator action, never an excuse to retry the query.
This experiment alone must not unblock the two-SET runtime query or setters.

## Isolated read-request / GET experiment (no selector)

`lab raid-read-request-get --unsafe` uses the same strict identity/descriptor
checks but sends exactly one captured `07 81` SET_REPORT, zero-filled to 264
bytes, waits 110 ms, then requests feature ID `07` once. The request is matched
byte-for-byte in two Legacy launch captures; its standalone effect without
the usual selector remains under investigation. There is no `07 03` selector,
startup, ACK handle, settings/image write or persistence. Unknown/empty RX is
retained without assuming it belongs to runtime or onboard. A TX error/short
write stops before waiting or GET; RX failure never triggers another send.

Obtain consent for this precise sequence separately from the passive GET.
The fixed `capture-raid-lab-windows.ps1` wrapper (formerly
`capture-passive-feature-windows.ps1`) requires exactly one experiment-specific
consent flag; GET-only consent never enables a SET or selector:

```bash
powershell.exe -NoProfile -ExecutionPolicy Bypass \
  -File "$(wslpath -w "$PWD/scripts/capture-raid-lab-windows.ps1")" \
  -Interface '\\.\USBPcap1' \
  -OutputPath '%TEMP%\openhyperx-read-request-get.pcapng' \
  -ConfirmReadRequestGet
```

Inspect all control requests and raw response before another operator action.
Require exactly one feature SET (value `0307`, index 1, length 264, full
`07 81` + zero-fill), followed by one GET with the same framing; no other
vendor/class request may be silently discarded. Verify target identity, order,
timing, USB status and physical cursor/click/lighting behavior. Retain failures
and capture/stdout/stderr logs outside Git. Stop on physical failure; a normal
transport result is not evidence of safe runtime selection or usable state.

## Isolated runtime selector (potentially disruptive; no GET)

`lab raid-runtime-select-only --unsafe` sends one fixed `07 03 04 64`
SET_REPORT, zero-filled to 264 bytes, and closes. Exact identity/release and
descriptor guards still apply. It never sends `81`, GET, startup, a full
settings image, RGB, firmware or an onboard save. **This can disable cursor,
clicks and lighting** if selecting runtime activates an unusable image. It is
not a read-only command or a normal-user runtime override.

Require fresh explicit agreement to this packet and the risk, writers closed,
normal mouse behavior beforehand, and physical USB reconnect available. Use
the bounded fixed wrapper with only `-ConfirmRuntimeSelector`:

```bash
powershell.exe -NoProfile -ExecutionPolicy Bypass \
  -File "$(wslpath -w "$PWD/scripts/capture-raid-lab-windows.ps1")" \
  -Interface '\\.\USBPcap1' \
  -OutputPath '%TEMP%\openhyperx-runtime-selector-only.pcapng' \
  -ConfirmRuntimeSelector
```

Inspect the whole capture: exactly one class SET (`21 09`, value `0307`, index
1, length 264), correct constant bytes and zero-fill, **zero GET_REPORT** and
no other vendor/class operation. The CLI does not open the ACK collection;
inspect any captured interrupt responses without issuing a probe. Successful
USB completion does not establish physical behavior, valid state or persistence.
Ask the operator immediately after review; if input/light fails, stop all
hardware work and have the operator unplug for five seconds and reconnect.
Do not query the failed device, initialize, restore by software or repeat this
selector to investigate it. Any later test needs a new plan and consent.

## Cold-device NGENUITY Legacy startup capture

After the selector-only failure, capture **one launch**, not another OpenHyperX
query. Obtain explicit consent: the mouse must work after an operator USB
reconnect, Legacy/OpenRGB must have stayed closed, and Legacy can automatically
apply software state on launch. Do not change settings, accept update prompts,
or click Save to mouse. The ordinary runtime safety gate remains in force.

`scripts/capture-legacy-startup-windows.ps1` activates only the locally observed
Microsoft Store Legacy package `33C30B79.HyperXNGenuity`, version `5.38.0.0`,
AUMID `33C30B79.HyperXNGenuity_0a78dr3hq0pvt!App`. It must run **unelevated**:
only the identity-resolving capture helper requests UAC. The helper signals
recording through a caller-created, empty `.ready.json` sidecar in `%TEMP%`;
the wrapper refuses stale readiness and existing artifacts. It captures for
12 seconds, requests one activation, and never invokes our CLI or closes
Legacy automatically (tray closure can itself select a profile).

```bash
powershell.exe -NoProfile -ExecutionPolicy Bypass \
  -File "$(wslpath -w "$PWD/scripts/capture-legacy-startup-windows.ps1")" \
  -Interface '\\.\USBPcap1' \
  -OutputPath '%TEMP%\openhyperx-legacy-cold-startup.pcapng' \
  -ConfirmLegacyLaunch
```

Keep the capture, readiness and `.launch.json` manifest outside Git. Inspect
all target class/vendor requests, responses and interrupt/output reports,
including every operation preceding the first runtime selector; compare full
bytes and ordering with retained startup captures. A successful activation or
nonempty capture alone is not proof of safe mouse behavior. Ask the operator
about cursor/clicks/lighting before further work. On failure stop; do not issue
a recovery command. Unknown startup packets are evidence, not replay recipes.
The two selected profile-image reports from the completed experiment are
available as `crates/hyperx-protocol/tests/fixtures/cold-legacy-startup-images.hex`;
the macro event stream is deliberately omitted. Inspect this fixture with
`hyperx-cli profile inspect-capture` offline, and compare it with the earlier
`read-request-get-onboard.hex` and `warm-legacy-startup-images.hex`. See
`docs/research.md` for the full-byte diff and 32-file opaque-signature scan
and its evidence limits. The files are **not** a bootstrap or replay plan.

For future evidence, `hyperx-cli profile diff-capture-images BEFORE AFTER
--before-report N --after-report M` compares only the explicitly chosen complete
profile images, including envelope and opaque body bytes. It accepts the same
bounded UTF-8 hex/trace input as `inspect-capture`, never PCAPNG directly, and
does not infer that two reports are one transaction. For example, compare
report 2 of `read-request-get-onboard.hex` against report 2 of
`cold-legacy-startup-images.hex`; the 24 body-byte differences are evidence,
not a source for a cold-runtime encoder. Keep real macro-containing captures
outside Git.

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

## Macro playback experiment

The corrected plan
`scripts/capture-plans/pulsefire-raid-button4-macro-playback-assigned-20260926.json`
isolates the existing Button 4 AB macro's playback mode. Before the series,
assign AB with Play Once and unchanged 20 ms timing in NGENUITY Legacy; do
not record a new timeline and do not save onboard. Confirm whether changing
playback needs Done before starting. **The initial macro must already be
assigned to the device, not merely selected in the macro library.** Existing
captures verify that editing an assigned macro and clicking Done can update
its definition directly, without reassignment. Each mutable step changes only
the mode and clicks Done. Button 5 must keep its separate existing
macro; stop if the UI couples the edit to another assigned macro or control.

Use the series runner with `-DurationSeconds 5`. Prepare the editor before
pressing Enter at each prompt and perform the named change only after
`Capturing` starts. The five files isolate a no-op baseline, explicit
Once -> Toggle -> Hold -> Once edits, and a final Button 4 Back restoration.
Never press physical Button 4 during recording: this series identifies the
configuration packets, not macro execution. No Save to mouse action is allowed.

The earlier five-second editor-only plan remains as a record of the first
experiment: its first cycle emitted no macro write, while its second cycle
emitted real Button 4 reports with its macro already assigned before each edit.
The corrected plan makes that initial assignment explicit. See research notes
for the evidence: the operator later confirmed that the first three edits
completed before capture started. The corrected plan is retained for optional
future verification, not a prerequisite for the requested first runtime implementation.

## Automated runtime playback verification

After the native Windows build, `scripts/verify-button4-playback-windows.ps1`
can check Toggle -> Hold -> Once -> Back without manual settings changes.
Close NGENUITY/OpenRGB, start with Button 4 Back, and pass `-Interface` plus
an unused `-CapturePrefix '%TEMP%\openhyperx-button4-playback'`. It elevates once,
uses the confirmed volatile session initializer, captures each transaction
separately in identity mode, and verifies complete feature writes, exact ACKs
and a separate runtime readback before advancing. Only Button 4 changes.
On failure it stops without retry or automatic restoration; inspect the files
and result JSON before any next write. No onboard save or physical macro
execution occurs. Captures/logs/results remain outside Git. This verifies
configuration communication, not actual repeated output or button-release behavior.

Inspect every macro definition and full profile write, not just a suspected
mode byte. Compare both copies of each transition, prove unchanged event
timelines and Button 5 reports, and determine whether mode is carried in the
macro report, binding record, or both. Capture evidence for runtime modes
does not by itself authorize persistent conversion or define `.hxp` enum values.

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

Inspect one normalized capture or an existing OpenHyperX trace log offline:

```text
hyperx-cli profile inspect-capture captures/reports.hex
hyperx-cli profile inspect-capture captures/runtime.log --raw --all-raw
```

This recognizes only existing complete Raid codecs, displays macro transitions
and individual delays, decodes profile settings and flags empty bodies. Source
line/order and available trace interface/direction are retained. Successive
same-section images are compared byte by byte, including envelope and opaque
fields; a file's TX/RX ordering alone is not an acknowledged transaction or
evidence of persistence. Unknown reports remain uninterpreted. No reports are
replayed and no HID device is opened. Inputs must be UTF-8 text (up to 16 MiB),
not binary `.pcapng`; timestamps/USB transfer types are not reconstructed.
See [profile tooling](profile-format.md#offline-comparison-and-capture-inspection)
for limitations and privacy precautions.

To retain confirmed settings from one captured runtime read:

```text
hyperx-cli profile export-capture captures/runtime.log captured.toml --report 3
hyperx-cli profile inspect captured.toml
```

Select the positive report number displayed by `inspect-capture`, not a source
line/frame number. Only a complete usable runtime RX image is accepted; no
latest-image guessing, empty-image fallback, host-write or onboard export.
The destination must not exist. This offline partial export retains performance
and ordinary mappings, but never invents lighting or macro timelines from
references or adjacent uploads. Unreadable macros remain unresolved and block
apply. Exports are potentially sensitive historical data, not a live backup;
keep them outside Git. No reports are replayed or settings applied/saved.

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
