# Reverse-engineering workflow

The rule is one deliberate UI change per capture. Keep firmware-update prompts
closed and do not capture or replay firmware, bootloader or DFU sessions.

## Capture procedure

1. Record mouse part number, firmware/release value, Windows version and
   NGENUITY version.
2. Close other software that may write RGB or profiles.
3. Start USBPcap on the controller containing `0951:16E4`, then open Wireshark.
4. Start NGENUITY and wait for background traffic to settle.
5. Change exactly one value once, wait several seconds, then stop the capture.
6. Save the original `.pcapng` outside Git if it contains unrelated USB data.
7. Export only relevant HID control/interrupt payloads as ordered hex lines.
8. Repeat the same transition at least twice and compare it with a no-op capture.

USBPcap may contain traffic from other devices on the same host controller.
Treat raw captures as potentially sensitive. The repository ignores `*.pcap`,
`*.pcapng`, `*.etl` and `captures/private/` by default.

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
