# Pulsefire Raid research

Last updated: 2026-09-26.

This document separates manufacturer facts, public implementation evidence,
local observations and hypotheses. Do not promote a hypothesis into a device
command without a capture or an independently reviewed implementation.

## Sources and licensing

- [HyperX product page](https://row.hyperx.com/pl/products/hyperx-pulsefire-raid-gaming-mouse)
  lists 11 programmable buttons, Pixart PMW3389, up to 16,000 DPI, three
  factory DPI presets, RGB, USB 2.0 and one onboard profile.
- [HyperX user guide](https://media.kingston.com/support/downloads/HyperX-Pulsefire-Raid-User-guide.pdf)
  identifies part number `HX-MC005B`, documents the factory button layout and
  warns that the hardware factory reset clears onboard memory.
- [Official HyperX NGENUITY page](https://row.hyperx.com/pages/ngenuity)
  distinguishes current NGENUITY (2025) from NGENUITY Legacy (2020-2025).
  On 2026-09-18 its download buttons pointed to current NGENUITY `3.0.0` and
  NGENUITY Legacy `2.38.0.0`, and its Legacy compatibility list explicitly
  included Pulsefire Raid. The page confirms built-in dynamic RGB effects but
  does not document individual animation curves, palettes or timing semantics.
- [OpenRGB new-device issue #2097](https://gitlab.com/CalcProgrammer1/OpenRGB/-/issues/2097)
  reports `HID\\VID_0951&PID_16E4&REV_1124&MI_00`.
- [OpenRGB detector at the inspected revision](https://gitlab.com/CalcProgrammer1/OpenRGB/-/blob/1da6a652fd0ee484be5b5844f8673f8590257cbb/Controllers/HyperXMouseController/HyperXMouseControllerDetect.cpp)
  selects VID `0x0951`, PID `0x16E4`, interface `1`, usage page `0xFF01`, usage
  `0x0001`.
- [OpenRGB Pulsefire Raid transport code](https://gitlab.com/CalcProgrammer1/OpenRGB/-/blob/1da6a652fd0ee484be5b5844f8673f8590257cbb/Controllers/HyperXMouseController/HyperXPulsefireRaidController/HyperXPulsefireRaidController.cpp)
  and [header](https://gitlab.com/CalcProgrammer1/OpenRGB/-/blob/1da6a652fd0ee484be5b5844f8673f8590257cbb/Controllers/HyperXMouseController/HyperXPulsefireRaidController/HyperXPulsefireRaidController.h)
  document its direct RGB feature report.
- [OpenRGB RGB adapter](https://gitlab.com/CalcProgrammer1/OpenRGB/-/blob/1da6a652fd0ee484be5b5844f8673f8590257cbb/Controllers/HyperXMouseController/HyperXPulsefireRaidController/RGBController_HyperXPulsefireRaid.cpp)
  names wheel and logo LEDs and describes the keepalive behavior.
- [OpenRGB license](https://gitlab.com/CalcProgrammer1/OpenRGB/-/blob/1da6a652fd0ee484be5b5844f8673f8590257cbb/LICENSE)
  is GNU GPL v2; the relevant source files declare GPL-2.0-or-later.
- [USB HID reverse-engineering guide](https://santeri.pikarinen.com/pages/usb_hid_reverse_engineering/)
  gives a practical Windows/USBPcap/Wireshark workflow from an OpenRGB HyperX
  contributor.
- [USB-IF HID Usage Tables](https://www.usb.org/hid) define the standard
  Keyboard/Keypad and Consumer usage IDs used to interpret captured binding
  records. The USB-IF document is the source of truth for those usage values;
  it does not by itself prove the surrounding HyperX record format.

OpenHyperX does not copy OpenRGB code. Names, IDs, observed report layout and
USB behavior are recorded as attributed interoperability facts. Any later
encoder must be independently written and covered by local packet fixtures.

## Identity

| Field | Value | Confidence |
| --- | --- | --- |
| Model | HyperX Pulsefire Raid / HX-MC005B | confirmed by HyperX |
| VID | `0x0951` (Kingston Technology) | confirmed by OpenRGB and local Windows device |
| PID | `0x16E4` | confirmed by OpenRGB and local Windows device |
| observed release | `0x1124` | confirmed on one local unit and issue #2097 |
| observed manufacturer string | `Kingston` | confirmed on one local unit |
| observed product string | `HyperX Pulsefire Raid` | confirmed on one local unit |
| serial | empty/not exposed | confirmed only for one local unit |

No HP-rebranded VID/PID variant for Raid was present in the inspected OpenRGB
registry. Do not add one by analogy with other HyperX mice.

## HID topology observed on Windows 11

Read-only enumeration was run against the attached unit on 2026-09-07. hidapi
returned seven top-level collections over three USB interfaces:

| USB interface | Collection | Usage page | Usage | Windows role / interpretation |
| --- | --- | --- | --- | --- |
| `MI_00` / 0 | default | `0x0001` | `0x0002` | standard mouse; do not open for configuration |
| `MI_01` / 1 | `COL01` | `0x0001` | `0x0006` | keyboard |
| `MI_01` / 1 | `COL02` | `0x0001` | `0x0080` | system control |
| `MI_01` / 1 | `COL03` | `0x000C` | `0x0001` | consumer control |
| `MI_01` / 1 | `COL04` | `0xFF00` | `0x0001` | vendor-defined, purpose unknown |
| `MI_01` / 1 | `COL05` | `0xFF01` | `0x0001` | OpenRGB configuration/RGB collection |
| `MI_02` / 2 | default | `0xFF00` | `0xFF00` | vendor-defined, purpose unknown |

The matching `MI_01/COL05` path is the only one marked `[configuration]` by
`hyperx-cli devices`. Exact paths contain machine-specific instance IDs.

The `MI_01/COL05` report descriptor was read locally without a vendor command:

```text
06 01 FF 09 01 A1 01 85 07 09 20 15 00 26 FF 00
75 08 96 07 01 B1 02 C0
```

It is 24 bytes long and declares report ID `0x07`, 8-bit fields, count `0x0107`
(263), and a Feature main item. The resulting hidapi buffer is therefore 264
bytes including the report ID. This independently confirms the OpenRGB length.

USBPcap descriptor injection on the same unit confirmed these interrupt IN
endpoints:

| USB interface | Endpoint | Maximum packet size |
| --- | --- | --- |
| 0 | `0x81` | 8 bytes |
| 1 | `0x82` | 8 bytes |
| 2 | `0x83` | 8 bytes |

Feature reports for the configuration collection use class control transfers
on endpoint 0, with `wIndex = 1`. During the 2026-09-12 capture session the
device was address 7 on `\\.\USBPcap1`; both values are transient and must be
re-enumerated after reconnect or reboot.

After a later reboot, the 2026-09-13 button session used transient address 31
on the same capture interface.

After the next reboot, the 2026-09-14 button session used transient address 37
on `\\.\USBPcap1`. Querying each extcap interface separately was necessary to
attribute the device tree correctly.

## Known RGB report from OpenRGB

Status: confirmed in public OpenRGB code, descriptor-confirmed locally,
successfully transmitted by OpenHyperX on 2026-09-07, and independently
hardware-validated for both physical LEDs on 2026-09-17.

- transport: HID feature report via the `MI_01`, `FF01:0001` collection;
- total buffer length passed to hidapi: 264 bytes;
- report ID / byte 0: `0x07`;
- byte 1: `0x0A` (direct-mode start marker in OpenRGB naming);
- bytes 2..4: RGB for logical LED 0 (`Scroll Wheel` in OpenRGB);
- bytes 5..7: RGB for logical LED 1 (`Logo` in OpenRGB);
- byte 8: `0xA0` (direct-mode end marker in OpenRGB naming);
- remaining bytes: zero in OpenRGB;
- pacing: OpenRGB waits 10 ms after a write;
- lifecycle: OpenRGB resends after more than one second without an update,
  stating that otherwise the device returns to its default flashing effect;
- response: OpenRGB does not read or validate one for this operation;
- persistence: OpenRGB marks save support as absent for Raid direct mode.

A five-second local test sent the all-red report eight times at 750 ms
keepalive intervals. Windows/hidapi accepted every feature report and the HID
identity remained unchanged. Later independent-color tests visually confirmed
that the two triplets control separate physical LEDs as OpenRGB describes.

The manufacturer page calls this one RGB lighting zone while the direct
protocol exposes two independently controllable physical LEDs. OpenHyperX
uses the more useful physical `wheel` and `logo` zone model.

## NGENUITY generations and evidence scope

All NGENUITY captures, UI observations and `.hxp` files documented below were
made with Microsoft Store NGENUITY Legacy `5.38.0.0`. `winget search ngenuity`
identifies that Store product as `HyperX NGENUITY (Legacy)`, ID
`9P1TBXR6QDCX`. Current NGENUITY has not been installed, captured or used to
infer any packet or profile format in this repository.

The two product lines must remain separate in research notes, fixtures, parser
names and CLI commands. Similar UI labels do not prove matching USB protocols
or profile formats.

## Local NGENUITY Legacy captures

Captures were made with NGENUITY Legacy `5.38.0.0`, USBPcap and Wireshark on
Windows 11. They were filtered to the Pulsefire Raid device address before
capture; raw files remain outside Git because they contain normal mouse input
reports.

With only `NGenuity2Helper` running, NGENUITY Legacy sends the confirmed direct
RGB feature report about every 62 ms. The locally observed all-red transaction
is:

- `bmRequestType = 0x21`, `SET_REPORT`, feature report ID `0x07`;
- `wIndex = 1`, `wLength = 264`;
- payload prefix `07 0A FF 00 00 FF 00 00 A0`, followed by zero fill.

Opening the NGENUITY Legacy GUI without changing a setting produced the same control
traffic. This locally confirms the OpenRGB report layout and shows that
NGENUITY Legacy's keepalive cadence is substantially faster than OpenRGB's timeout
avoidance strategy.

### DPI profile observations

Changing the first DPI stage caused a read-modify-write sequence involving
264-byte feature reports on report ID `0x07`. The resulting full profile image
starts with `07 01 04`; offsets are relative to the complete report including
the report ID.

| First stage | X low byte `0x1A` | Y low byte `0x26` |
| --- | --- | --- |
| 800 DPI | `0x10` | `0x10` |
| 900 DPI | `0x12` | `0x12` |
| 1000 DPI | `0x14` | `0x14` |

The `900 -> 1000` and `1000 -> 900` transitions changed only these two bytes,
and a repeated `900 -> 1000` transition produced the same result. Later
stage-addition captures showed that each value is an unsigned 16-bit
big-endian integer in 50-DPI units. The zero high bytes for the initial values
had made the first captures look like single-byte fields.

Five DPI stages were mapped locally on 2026-09-13. Offsets point to the high
byte of each two-byte value:

| Stage | X offset | Y offset | Captured bytes | DPI |
| --- | --- | --- | --- | --- |
| 1 | `0x19` | `0x25` | `00 14` | 1000 |
| 2 | `0x1B` | `0x27` | `00 20` | 1600 |
| 3 | `0x1D` | `0x29` | `00 40` | 3200 |
| 4 | `0x1F` | `0x2B` | `00 80` | 6400 |
| 5 | `0x21` | `0x2D` | `01 40` | 16000 |

NGENUITY Legacy displayed these same five values. The fifth level displayed 16000
DPI and white, independently confirming `0x0140 * 50 = 16000`. Separate X/Y
editing has not been tested, so the UI's axis-control behavior remains
unknown.

Isolated edits on stages 2 through 4 confirmed the same encoding at each
mapped offset:

| Transition | X change | Y change | Active-index change |
| --- | --- | --- | --- |
| stage 2: `1600 -> 1700` | `0x1C: 20 -> 22` | `0x28: 20 -> 22` | `0x31: 00 -> 01` |
| stage 2: `1700 -> 1600` | `0x1C: 22 -> 20` | `0x28: 22 -> 20` | none |
| stage 3: `3200 -> 3300` | `0x1E: 40 -> 42` | `0x2A: 40 -> 42` | `0x31: 01 -> 02` |
| stage 3: `3300 -> 3200` | `0x1E: 42 -> 40` | `0x2A: 42 -> 40` | none |
| stage 4: `6400 -> 6500` | `0x20: 80 -> 82` | `0x2C: 80 -> 82` | `0x31: 02 -> 03` |
| stage 4: `6500 -> 6400` | `0x20: 82 -> 80` | `0x2C: 82 -> 80` | none |

The reverse captures affected only the two DPI bytes plus the normal opcode
change. Editing a different level also selected it, accounting for the active
index in each forward capture. These pairs, together with the repeated stage-1
captures and exact stage-5 add/remove pair, cover all five mapped DPI slots.

The active stage is a zero-based index at offset `0x31`: isolated `1 -> 2`
and `2 -> 3` UI transitions changed it from `00 -> 01 -> 02`. Stage enabled
flags are bytes `0x32..0x36`, one byte per stage, with `00` disabled and `01`
enabled. NGENUITY Legacy keeps enabled stages contiguous. Starting with three enabled
levels, `Add level` set `0x35` for stage 4 and then `0x36` for stage 5. The UI
no longer offered `Add level` after the fifth stage.

Each DPI level has a three-byte RGB color at these offsets:

| Stage | RGB offsets | Locally captured color |
| --- | --- | --- |
| 1 | `0x69..0x6B` | `#2B00FF` |
| 2 | `0x6C..0x6E` | `#CD00FF` |
| 3 | `0x6F..0x71` | `#32FF00` |
| 4 | `0x72..0x74` | `#FF0000` |
| 5 | `0x75..0x77` | `#FFFFFF` |

The first three exact colors reflect the deliberately selected test values,
not claims about factory defaults. Isolated color changes affected only the
corresponding RGB triplet plus the normal profile opcode change.

Adding stage 4 initialized it to 6400 DPI and red. Adding stage 5 initialized
it to 16000 DPI and white. Removing stage 5 cleared its X value at
`0x21..0x22`, Y value at `0x2D..0x2E`, enable flag at `0x36`, and color at
`0x75..0x77`. The captured `4 -> 5` and `5 -> 4` byte changes are exact
reverses. NGENUITY Legacy emitted an additional no-op read/write transaction before
the actual removal transaction; the no-op changed only the response/write
opcode.

OpenHyperX has a profile patcher for DPI values, active stage, stage count and
colors. NGENUITY Legacy locally exposed a minimum of 200 DPI; the manufacturer
documents a maximum of 16000 DPI, and captures confirm a 50-DPI step. The
patcher therefore accepts multiples of 50 in the inclusive `200..=16000` range
and preserves unrelated profile bytes.

On 2026-09-16, the resulting runtime API and CLI were validated against the
physical release-`1124` unit with NGENUITY Legacy and other writers stopped. Starting
from four stages with stage 1 active, OpenHyperX appended stage 5 at 16000 DPI
and `#FFFFFF`, independently read all five stages back, removed and cleared
stage 5, and independently read the original four-stage profile back. It then
changed stage 2 to 1700 DPI and `#010203`, made it active, and read those values
back. Finally it restored stage 2 to 1600 DPI and `#CD00FF`, selected stage 1,
and read the original four stages and active 800-DPI value back. No onboard
save transaction was sent. The CLI deliberately removes only the final stage,
matching the contiguous layout and captured add/remove operation.

Observed configuration sequence:

1. feature write with prefix `07 03 04 64`;
2. feature write with prefix `07 81`, otherwise zero-filled;
3. `GET_REPORT` for feature report `0x07`, length 264;
4. feature write containing the modified full profile image, prefix
   `07 01 04`.

On 2026-09-15, TShark timestamps were checked for both complete repetitions in
`openhyperx-macro-ab-play-once-timing-20-create-1.pcapng`. The first-to-second
write intervals were 63.696 ms and 63.682 ms; the second-write-to-GET intervals
were 107.264 ms and 108.546 ms. The read-only driver therefore waits a
conservative 65 ms and 110 ms, respectively, requests report ID `0x07` once,
and rejects anything other than an exact 264-byte `07 81 04` response. It does
not retry and does not transmit the subsequent `07 01 04` profile write.
The OpenHyperX read path was then validated on the Windows host against the
physical `0951:16E4`, release `1124` unit with NGENUITY Legacy and its helper closed.
The device returned an exact 264-byte `07 81 04` response. The CLI decoded
1000 Hz polling, four DPI stages (800 active, 1600, 3200 and 6400), their four
captured colors, ten ordinary button records, and the confirmed Button 5 macro
reference. The runtime profile cannot by itself reveal that macro's event
definition, so the CLI deliberately labels it as a reference rather than
guessing its keys or timing.

The independent `1600 -> 1700` DPI capture placed its `07 01 04` write 0.039 ms
after the GET response; the control-transfer response arrived 0.379 ms later.
An independent `500 -> 1000` polling capture reproduced the same framing. No
additional acknowledgement payload or post-write command was observed. The
driver therefore sends one full write immediately after patching the response,
does not retry, and preserves every byte except the response/write opcode and
the selected setting field.

On the physical release-`1124` unit, OpenHyperX successfully performed and
read back `800 -> 900 -> 800` on both axes of active DPI stage 1. It then
performed and read back `1000 -> 500 -> 1000 Hz`. The final runtime state was
the original 800 DPI and 1000 Hz, and no onboard-save sequence was sent.

The detailed semantics and allowed variants of the two fixed prelude writes
remain unknown. They are not generalized: the driver exposes only the exact
runtime read/modify/write sequence confirmed above.

### Polling-rate profile observations

The NGENUITY Legacy profile image changed only offset `0x18`. Repeated transitions
confirmed that the value is the USB polling interval in milliseconds:

| Polling rate | Offset `0x18` |
| --- | --- |
| 1000 Hz | `0x01` |
| 500 Hz | `0x02` |
| 250 Hz | `0x04` |
| 125 Hz | `0x08` |

The `125 -> 500` and `500 -> 1000` transitions reproduced the mapping and each
changed only this byte. The initial `0x02 -> 0x01` capture was therefore
`500 -> 1000`, not the initially assumed reverse direction. The software UI
confirmed that the setting remained selected. Persistence of the `0x01` value
through an explicit onboard save and power-cycle was subsequently confirmed.
The runtime setter now uses the complete confirmed read/modify/write
transaction and preserves all non-polling bytes. It does not invoke the
separate onboard-save transaction.

### Onboard save observations

Two captures of an explicit NGENUITY Legacy `Save to mouse` click were made on
2026-09-13. Both used feature report ID `0x07`, interface 1 and 264-byte
buffers. The repeated ordered sequence was:

1. `SET_REPORT` prefix `07 03 01 64`;
2. three `SET_REPORT` packets with prefixes `07 18 01 00`, carrying indexes
   `00`, `01` and `02` at offset `0x04`;
3. `SET_REPORT` prefix `07 81`, otherwise zero-filled;
4. `GET_REPORT` returning a full profile image with prefix `07 81 01`;
5. `SET_REPORT` containing a full profile image with prefix `07 01 01`;
6. after about one second, `SET_REPORT` prefix `07 03 04 64`.

The first save read an onboard profile containing polling code `0x02`
(500 Hz) and first-stage DPI code `0x10` (800 DPI). NGENUITY Legacy wrote the current
1000 Hz / 1000 DPI settings. Comparing the complete read response with the
write found only these differences:

| Offset | Read | Write | Interpretation |
| --- | --- | --- | --- |
| `0x01` | `0x81` | `0x01` | response opcode to write opcode |
| `0x18` | `0x02` | `0x01` | 500 Hz to 1000 Hz |
| `0x1A` | `0x10` | `0x14` | first DPI X, 800 to 1000 |
| `0x26` | `0x10` | `0x14` | first DPI Y, 800 to 1000 |

The second save read the newly stored 1000 Hz / 1000 DPI values. Its full
read/write comparison differed only at offset `0x01`, confirming that
NGENUITY Legacy preserved all other bytes in the no-op case.

NGENUITY Legacy and its helper were then closed, the mouse was physically unplugged
for about five seconds and reconnected, and all seven HID collections returned
with the same identity. A capture of the next NGENUITY Legacy startup read a profile
with prefix `07 81 04`, polling code `0x01` and DPI codes `0x14` at offsets
`0x1A` and `0x26`. NGENUITY Legacy's corresponding `07 01 04` write differed only in
the opcode. This confirms persistence of the saved performance values across
a power-cycle and shows that sections `0x01` and `0x04` share the confirmed
performance-field layout.

The person at the machine confirmed that cursor movement and the basic mouse
buttons still worked normally after the power-cycle. The lighting did not
remain at the previously visible static red: with NGENUITY Legacy and its helper
stopped, the mouse displayed a rainbow effect. This is consistent with the red
being supplied by NGENUITY Legacy's periodic volatile `07 0A` direct-RGB reports and
the mouse returning to a different stored lighting effect. It confirms
performance-field persistence only; persistent lighting storage is not yet
understood and must be tested separately.

The two no-op saves reproduced the same timing as well as the same payloads.
The first indexed `0x18` packet was sent about 64 ms after `07 03 01 64`; the
second followed about 35 ms later, while the third and `07 81` request followed
at roughly 2 ms intervals. NGENUITY Legacy waited about 113--115 ms between `07 81`
and the `GET_REPORT`, wrote the full profile immediately after the response,
then waited about one second before selecting runtime section `0x04` again.

The indexed packets are not an empty handshake. For the two red saves,
index `0x00` contained `FF 00 00` at both offsets `0x08..0x0A` and
`0x0B..0x0D`. In the isolated green save both triplets changed to
`32 FF 00`; all remaining bytes in that packet and every payload byte after
the index in packets `0x01` and `0x02` were zero. The complete `07 01 01`
profile writes from both red saves and the green save were byte-for-byte
identical, proving that this observed color data lives outside the main
profile image. The two triplets correlate with the direct-RGB wheel/logo
colors, but their persistent meaning is not established: the saved green did
not survive without NGENUITY Legacy's volatile lighting stream.

Two further eight-second captures on 2026-09-18 isolated the zones while
retaining Solid, maximum opacity and no other effect. USBPcap identity mode
resolved the unit on `\\.\USBPcap1` as address 61. Both files were 63,426
bytes and remain outside Git:

- `openhyperx-save-wheel-red-logo-off-20260918.pcapng` carried
  `FF 00 00 00 00 00` at offsets `0x08..0x0D`;
- `openhyperx-save-wheel-off-logo-blue-20260918.pcapng` carried
  `00 00 00 00 00 FF` at the same offsets.

Together with the red and green captures, this confirms wheel-first/logo-second
ordering, independent black/off, and all three RGB channels for the Solid save
snapshot. Indexed reports 1 and 2 remained zero-filled. This evidence supports
a typed arbitrary-RGB encoder for that exact three-report Solid layout; it does
not establish a persistent firmware effect or the layout for non-Solid modes.

Both new captures also contained the existing 14-event Button 5 Play Once
macro. Immediately after the onboard-profile read and before `07 01 01`,
NGENUITY Legacy sent its complete definition with prefix `07 05 01 04`. The
remaining 263 bytes matched the previously captured runtime report
`07 05 04 04` byte-for-byte. Offset `0x02` is therefore confirmed as the
runtime/onboard section selector for this macro variant.

Every report in all four examined saves produced an eight-byte interrupt-IN
acknowledgement. The repeated successful forms are:

| Sent opcode | Acknowledgement |
| --- | --- |
| `03` | `00 00 07 03 00 00 00 00` |
| `18` | `00 00 07 18 00 00 00 00` |
| `81` | `00 00 07 81 FF 00 00 00` |
| `05` | `00 00 07 05 00 00 00 00` |
| `01` | `00 00 07 01 00 00 00 00` |

USB captures place these acknowledgements on endpoint `0x83`, which belongs
to HID interface 2 (`MI_02`, usage `FF00:FF00`), rather than the configuration
collection on interface 1 (`MI_01`, usage `FF01:0001`). Windows therefore
requires a second HID handle for the interrupt-IN acknowledgements. Attempting
an eight-byte input read on the feature-only configuration collection failed
with `ERROR_INVALID_USER_BUFFER (1784)`. That first OpenHyperX hardware test
stopped after the non-persistent runtime-selection prelude and transmitted no
onboard write.

The new driver transaction validates all runtime source fields before selecting
onboard memory, preserves unknown onboard bytes, checks each acknowledgement,
does not retry an ambiguous failure, and copies only confirmed DPI, polling and
button records. A Button 5 macro reference requires a caller-supplied validated
definition. Golden protocol tests and an exact `MockHidTransport` script cover
the macro and no-macro variants.

On 2026-09-18, OpenHyperX completed the transaction on the physical
release-`1124` unit using separate configuration and acknowledgement handles.
It saved 1000 Hz, four DPI stages (`800`, `1600`, `3200`, `6400`) with their
colors, all 11 button records, the confirmed 14-event Button 5 macro, and the
wheel-off/logo-blue Solid snapshot. Every transmitted stage received the exact
acknowledgement above. An immediate independent runtime read returned the
saved performance values and mappings.

On 2026-09-20, the mouse was physically unplugged for about five seconds and
reconnected without starting NGENUITY Legacy. A native Windows `info` read
again returned 1000 Hz, all four DPI stages and colors, all 11 mappings, and
the Button 5 macro reference. The operator also observed that the saved
wheel-off/logo-blue lighting state survived the power-cycle. This confirms the
OpenHyperX persistence path for the performance profile, mappings, macro
reference and this two-zone Solid snapshot. The macro event stream itself is
write-only in the currently understood protocol, so it cannot be confirmed by
profile readback. The operator then pressed Button 5 after the power-cycle and
confirmed that the saved macro executed normally. This functionally verifies
that its complete 14-event timeline was also stored onboard.

### Runtime lighting and persistence observations

On the tested NGENUITY Legacy profile, the Lighting UI showed `Solid`, red, target
`All Lights`, maximum opacity and `Visible`. Opening that tab changed the
mouse from its standalone rainbow effect to static red. This coincided with
the periodic direct-RGB report and did not require a profile transaction.

Changing only the UI color from red to a visually green value produced no
non-direct feature reports. The repeated direct report changed from:

```text
07 0A FF 00 00 FF 00 00 A0
```

to:

```text
07 0A 32 FF 00 32 FF 00 A0
```

The latter value is `#32FF00` for both physical LEDs; it was not pure
`#00FF00`. Clicking `Save to mouse` while this green preview was active
changed only the two RGB triplets at offsets `0x08` and `0x0B` of the first
indexed `0x18` report. The complete onboard profile write was byte-for-byte
identical to the earlier red save. After NGENUITY Legacy and its helper stopped, the
mouse briefly displayed static red and then returned to its standalone
rainbow effect. The green color therefore did not persist.

Changing the NGENUITY Legacy effect from `Solid` to its cycle/rainbow option produced
289 direct-RGB reports containing 131 distinct pairs of colors during one
20-second capture, and no non-direct feature report. On this unit and
NGENUITY Legacy version, both Solid and cycle lighting are software-rendered through
the volatile `0x0A` path. There is no local evidence that `Save to mouse`
persists a lighting effect, despite its `0x18` color payloads. Treat those
payloads as unknown until their purpose is isolated; do not use them as a
persistent-lighting encoder.

On 2026-09-17, with NGENUITY Legacy, OpenRGB and other device writers stopped,
OpenHyperX exercised the two direct fields independently on the physical
release-`1124` unit. For five seconds, `wheel=#FF0000, logo=#000000` lit only
the scroll wheel red. The reversed test, `wheel=#000000, logo=#0000FF`, lit
only the HyperX logo blue. A third test used `wheel=#FF0000, logo=#0000FF`;
both LEDs simultaneously displayed their distinct requested colors even though
the tested NGENUITY Legacy Solid UI did not expose separate per-LED colors. After each
foreground keepalive ended, both LEDs returned to their pre-test red state.
This confirms field-to-LED ordering, simultaneous independent colors,
per-LED black/off behavior and volatile reversion. None of these tests sent an
onboard-profile write.

OpenHyperX's `rgb cycle` is deliberately described as a software spectrum,
not as a decoded firmware effect or a byte-for-byte clone of NGENUITY Legacy's
animation. It computes portable RGB frames in `hyperx-core`, sends them through
the same confirmed two-LED direct report at roughly 60-ms intervals including
transport pacing, and stops after an explicit foreground duration. The target
can be both LEDs, the wheel only or the logo only; untargeted LEDs receive
black. It never writes the runtime or onboard profile.

The first physical test ran `rgb cycle --target all --duration 5 --period 2`
on the release-`1124` unit with other writers stopped. Both the wheel and logo
visibly moved through the spectrum in sync for the requested five seconds.
This validates the foreground renderer and sustained direct-report pacing on
Windows hardware; it does not imply persistence or reproduce NGENUITY Legacy's exact
animation curve.

The generalized program path was then tested with Cycle on the wheel at a
four-second period and purple Breathing on the logo at a 2.5-second period and
180-degree phase. Both distinct effects ran simultaneously for the requested
eight seconds. This confirms independent per-zone effects, periods and phase
state through the TOML-to-renderer-to-device path.

The Windows `triggered-fade` adapter was also validated on the same unit. With
the standard mouse HID collection left untouched, system mouse-button down
edges restarted an orange one-second wheel fade and a blue 1.5-second logo
fade. Repeated clicks restarted both envelopes, and the mouse continued to
operate normally. This validates the foreground trigger adapter and independent
per-zone fade durations; it does not establish NGENUITY Legacy's exact fade curve.

Two further eight-second programs exercised the remaining renderer families.
The first displayed the warm OpenHyperX Sun palette on the wheel and the cool
purple/blue/magenta Twilight palette on the logo, with independent 180-degree
phase state. The second displayed a repeating red Pulse on the wheel while the
logo stepped through deterministic pseudo-random saturated Confetti colors.
Both pairs ran independently and reverted after the foreground program ended.
Together with the earlier tests, this hardware-validates the TOML execution
path for Solid, Cycle, Pulse, Breathing, Triggered Fade, Confetti, Sun and
Twilight, but not visual parity with NGENUITY Legacy's undocumented definitions.

The observed NGENUITY Legacy device view exposes Solid, Cycle, Pulse,
Breathing and a triggered Fade effect. Its Light Sync view exposes Solid,
Breathing, Cycle, Confetti, Sun and Twilight. HyperX's public material does not
define these animations. OpenHyperX therefore models the same names as
portable software effects with explicitly project-defined curves and palettes;
it does not claim exact visual parity until each NGENUITY Legacy output stream is
captured and compared. Programs can assign a different effect and phase to
`wheel` and `logo`. Triggered Fade consumes foreground Windows system
mouse-button down edges and never opens the standard mouse HID collection.

### Button-remapping UI observations

The Pulsefire Raid page in NGENUITY Legacy `5.38.0.0` exposed all 11 physical
controls: left and right click, five side buttons, the DPI button below the
wheel, wheel click, wheel tilt left and wheel tilt right. The left and right
buttons were restricted to swapping left/right click. The other nine controls
offered these binding categories:

- Keyboard Function: one searchable keyboard key;
- Mouse Function: Left Click, Right Click, Middle, Back, Forward, Tilt L,
  Tilt R, DPI Toggle, Scroll Up and Scroll Down;
- Multimedia: Play/Pause, Stop, Next, Previous, Mute Volume, Volume Up and
  Volume Down;
- Macro;
- Windows Shortcut: Cycle Apps, Switch Apps, Cut, Copy, Paste and Undo;
- Disabled.

The macro UI offered `Add Macro`, recording of keyboard and mouse-button
events, and playback policies Play Once, Toggle Repeat and Hold Repeat.
`Standard Timing` is a separate optional numeric setting rather than a
playback enum. Its field accepts at least four digits and appeared to permit
`0..9999`, but that range has not been boundary-tested. NGENUITY Legacy became
unresponsive while many inputs were clicked in the first macro-recorder
exploration, so no packet inference is made from that earlier session.

The official manual confirms that the apparent `Forward`, `Back`, `Mute`,
`Volume Up` and `Volume Down` side assignments are the factory layout, not
necessarily accidental user edits. It labels them Button 5, Button 4, Button
8, Button 7 and Button 6 respectively. It also documents the factory-reset
gesture as holding the DPI and wheel buttons for five seconds, explicitly
stating that this clears onboard memory. That gesture was not used during
research because the current profile was intentionally preserved.

On 2026-09-13, isolated runtime-profile captures changed physical Button 5
through `Forward -> Disabled -> Forward -> Back -> Volume Up -> Copy -> A ->
Forward`. Every UI edit performed one `07 81 04 ...` profile read followed by
one `07 01 04 ...` write. Apart from the expected opcode byte, only Button 5's
four-byte record at `0x8C..0x8F` changed:

| Binding | Four-byte record | Evidence |
| --- | --- | --- |
| Disabled | `00 00 00 00` | isolated forward/reverse capture |
| Mouse Forward | `02 F9 00 04` | isolated forward/reverse capture; an existing functional record used `02 F9 00 00` |
| Mouse Back | `02 F8 00 03` | isolated capture; an existing functional record used `02 F8 00 00` |
| Multimedia Volume Up | `04 00 00 E9` | isolated capture; `E9` is USB HID Consumer Volume Increment |
| Windows Shortcut Copy | `23 E0 06 00` | isolated capture; `E0`/`06` are Left Control and keyboard C usages |
| Keyboard key | `00 UU 00 00` | isolated A-to-B capture changed only `UU` from standard Keyboard/Keypad usage `04` to `05` |
| Mouse DPI Toggle | `71 F0 00 00` | isolated captures on the DPI control and Button 5 |

On 2026-09-14, a first isolated capture changed the DPI control from Keyboard
A to DPI Toggle. A second capture read DPI Toggle from Button 5 and then
changed that control back to Forward. Both slots contained the exact record
`71 F0 00 00`; the reverse Button 5 write restored `02 F9 00 04`. This also
left Button 5 on its factory Forward action after the experiment. The physical
DPI switch was reported as mechanically unreliable, which does not affect the
record evidence but makes a working side button a useful alternate DPI Toggle
target.

The read profile and the manual's named factory layout correlate the 11
physical controls with these record starts:

| Control | Offset | Confidence |
| --- | --- | --- |
| left click | `0x7C` | correlated profile record `02 F0 00 00` |
| right click | `0x80` | correlated profile record `02 F2 00 00` |
| middle click | `0x84` | correlated profile record `02 F1 00 00` |
| Button 4 / factory Back | `0x88` | manual plus functional captured profile |
| Button 5 / factory Forward | `0x8C` | isolated local remapping sequence |
| Button 7 / factory Volume Up | `0x90` | manual plus functional captured profile |
| Button 6 / factory Volume Down | `0x94` | manual plus functional captured profile |
| Button 8 / factory Mute | `0x98` | manual plus functional captured profile |
| DPI button | `0x9C` | position correlated; captured profile had been remapped to A |
| wheel tilt left | `0xA0` | position inferred from remaining records and `02 F5 00 00` |
| wheel tilt right | `0xA4` | position inferred from remaining records and `02 F6 00 00` |

The protocol crate decodes these known records and has a capture-backed runtime
patcher. Named keyboard keys, all NGENUITY Legacy Mouse and Multimedia
functions, and all six Windows Shortcuts are writable on the nine general
controls as described below. Left and right click are restricted to an atomic
swap of those two functions, matching the UI and the coupled capture series.
The capture-backed `DPI Toggle` record is also writable.
The ordinary binding API still rejects generic macro references; the one exact
minimal macro captured below is handled through a separate evidence-gated
type.

### Minimal macro observations

On 2026-09-14, a new macro named `openhyperx-a` was recorded with only one
press/release of keyboard `A`, Standard Timing set to 20 ms and Play Once.
Assigning it to Button 5 was captured twice. Both assignments produced the
same ordered non-RGB transaction:

1. `SET_REPORT` prefix `07 03 04 64`;
2. zero-filled `SET_REPORT` prefix `07 81`;
3. `GET_REPORT` returning the runtime profile with prefix `07 81 04`;
4. the exact 264-byte macro-definition `SET_REPORT` shown below;
5. the runtime profile write with prefix `07 01 04` and Button 5 record
   `53 00 00 04`.

The repeated macro-definition report was:

```text
07 05 04 04 00 00 00 00 00 01 80 14 04 00 14 04
```

All bytes after this 16-byte prefix were zero. The selected UI values and
functional test correlate `0x04` with keyboard usage `A`; pressing Button 5
emitted lowercase `a`, as expected for that usage without Shift. Byte
`0x09 = 01` correlates with Play Once, and byte `0x03 = 04` correlates with
Button 5 and the final `04` in its binding record. These interpretations remain
hypotheses until key, playback and target control are changed independently.

An isolated edit kept the same key, control and playback mode while changing
Standard Timing from 20 to 300 ms. The repeated event bytes changed as follows:

```text
press:   80 14 04 -> 81 2C 04
release: 00 14 04 -> 01 2C 04
```

`20 = 0x0014` and `300 = 0x012C`. This confirms that each event stores the
timing as seven high bits plus an eight-bit low byte, while bit 7 of the high
byte distinguishes press from release. The representation has 15 payload bits,
but neither the device's accepted range nor the apparent NGENUITY Legacy `0..9999`
range has been boundary-tested.

A separate macro recorded `A` followed by `B`, still using Play Once and
Standard Timing 20 ms. NGENUITY Legacy emitted the same complete transaction twice
during the single `Done` action. Both macro reports had this nonzero prefix:

```text
07 05 04 04 00 00 00 00 00 01
80 14 04 00 14 04
80 14 05 00 14 05
```

The first six event bytes are the previously captured A press/release pair;
the next six append B press/release with standard keyboard usage `0x05`.
Button 5 then typed lowercase `ab`, functionally confirming the ordering.
Offset `0x09` remained `01` when the macro grew from one key to two, proving
that it is not an event or key count and strengthening its correlation with
the unchanged Play Once mode.

NGENUITY Legacy can disable Standard Timing and retain different timing values between
events. The protocol model therefore stores timing on every input transition,
not as one global macro property.

A separate isolated reversal from this macro to Mouse Forward omitted the
`0x05` macro-definition report. Its read/write profile comparison changed only
the normal opcode plus `0x8C: 53 -> 02` and `0x8D: 00 -> F9`; offset `0x8F`
remained `04`. No `Save to mouse` action was used, so these observations cover
the runtime profile only.

The first protocol implementation contained an offline evidence gate for the
Button 5 macro reference and those three exact event lists. It rejected other
event lists until modifier, mouse and nonuniform-timing records were isolated.

On 2026-09-17, OpenHyperX's runtime setter was tested on the physical
release-`1124` unit with NGENUITY Legacy and other writers stopped. The initial read
showed the existing Button 5 macro reference. OpenHyperX changed only Button 5
to the captured Mouse Forward record, and an independent read decoded it as
Forward while the other ten mappings were unchanged. It then sent the exact
captured A-then-B/20-ms/Play-Once macro definition followed by the profile
reference; an independent read again found the macro reference and all other
mappings unchanged. Pressing Button 5 then emitted lowercase `ab`, functionally
confirming the OpenHyperX-generated macro definition and ordering. No
onboard-save transaction was sent.

Before the later full Mouse Function matrix, the driver's target-aware evidence
gate exposed the capture-derived ordinary
Button 5 record families (Disabled, Forward, Back, Volume Up, Copy, named
keyboard keys and DPI Toggle). It also exposes the portable records on the
other non-primary controls. A mock transport test verifies that the DPI target
changes only the runtime write opcode and its record at `0x9C..0x9F`. On the
physical release-`1124` unit,
OpenHyperX then changed the DPI control from keyboard A to DPI Toggle; an
independent runtime read returned DPI Toggle while the other ten records were
unchanged, and pressing the mechanically unreliable control at its working
contact point changed DPI stages normally. Other decoded ordinary bindings
remained read-only inferences at that point, even when their usage IDs came
from the USB HID standard. No onboard save was sent.

On 2026-09-18, an eight-second isolated DPI-control capture changed Keyboard A
to Keyboard B. USBPcap resolved the current unit dynamically as
`\\.\USBPcap1`, address `61`, VID/PID `0951:16E4`. The capture contained one
runtime-profile response and one profile write in the 61,610-byte capture
`openhyperx-dpi-keyboard-a-b-20260918.pcapng`, retained outside Git. Apart from
the normal opcode change `0x81 -> 0x01`, the only changed byte was
`0x09D: 04 -> 05`; the complete DPI record changed from `00 04 00 00` to
`00 05 00 00`. These are the standard USB HID Keyboard/Keypad usages for A and
B. Together with the previously captured macro letters and modifier usages,
this confirms the record shape `00 UU 00 00` for named keyboard assignments
rather than an A-specific enum. The CLI therefore accepts the finite
human-readable key set parsed by `KeyboardUsage` (letters, digits, punctuation,
navigation, F1-F24, keypad and modifiers), while rejecting raw numeric and
unknown usages. No onboard save was sent.

With NGENUITY Legacy and its helper stopped, OpenHyperX then read the captured
Keyboard B state, changed the DPI control to Keyboard A and independently read
back usage `0x0004`. It changed the same control to Keyboard B and independently
read back `0x0005`. The other ten mapping records were identical after both
writes. A guarded cleanup restored DPI Toggle, and a final independent read
returned `Mouse: DPI toggle` with the other records still unchanged. Every
operation used the volatile runtime-profile path; no onboard save was sent.

On 2026-09-17, an isolated `Back -> Disabled -> Back` capture for Button 4
contained three runtime profile writes. Disabling changed only the Button 4
record at `0x88..0x8B`, from `02 F8 00 03` to `00 00 00 00`; restoring Back
changed exactly those bytes back to `02 F8 00 03`. The driver and CLI therefore
initially exposed only Disabled and Mouse Back for Button 4. Other actions on
this target remained rejected before device discovery until the later matrix
below. No onboard save was sent.

With NGENUITY Legacy and its helper stopped, OpenHyperX then changed Button 4 from
Mouse Back to Disabled on the physical release-`1124` unit. An independent
runtime read returned Disabled while the other ten mappings were unchanged. A
guarded cleanup write restored Mouse Back, and a final independent read
confirmed the restoration. No onboard save was sent.

An isolated Button 7 `Volume Up -> Volume Down` capture on the same unit read
`04 00 00 E9` at `0x90..0x93`, then wrote `04 00 00 EA`. Apart from the runtime
write opcode (`0x81 -> 0x01`), only byte `0x93` changed (`E9 -> EA`). The
resulting Volume Down record exactly matched the pre-existing Button 6 record,
confirming this multimedia record across two physical controls. The driver and
CLI therefore expose Volume Up and Volume Down for Button 7; other actions on
that target remain blocked. No onboard save was sent.

After NGENUITY Legacy was stopped, OpenHyperX began from the captured Volume Down
state and exercised Button 7 through `Volume Down -> Volume Up -> Volume Down
-> Volume Up`. An independent runtime read confirmed every transition, the
other ten mappings remained unchanged, and the final state was Volume Up. No
onboard save was sent.

Before the later Mouse Function matrix, six ordinary record families repeated on at
least two physical controls: Disabled (Buttons 4/5), Mouse Back (Buttons 4/5),
Volume Up (Buttons 5/7), Volume Down (Buttons 6/7), keyboard usage (Button 5
and DPI, with A/B isolated on DPI) and DPI Toggle (Button 5 and DPI). NGENUITY Legacy
exposes the same assignment categories for the middle click, five numbered
side controls, DPI control and both wheel tilts. The runtime writer therefore
treats those six records as portable across these nine general controls while
keeping the primary left and right clicks unavailable. Forward, Copy and macro
references remained Button-5-specific until their portability was independently
established.

With NGENUITY Legacy stopped, OpenHyperX then applied the portable Mouse Back record
to Button 6, whose initial mapping was Volume Down. An independent runtime read
returned Mouse Back while the other ten mappings were unchanged. A guarded
cleanup restored Volume Down and a final read confirmed it. This validates the
portable-record path on a third target slot beyond the pairs used to establish
the record matrix. No onboard save was sent.

On 2026-09-20, one reviewed capture series exercised every Mouse Function on
Button 4 in NGENUITY Legacy `5.38.0.0`. The identity-resolving runner produced
13 separate 10-second files on `\\.\USBPcap1`, address 57, release `1124`;
their sizes ranged from 73,508 to 235,092 bytes and the raw files remain
outside Git. A no-op baseline contained no runtime-profile write. Each of the
12 named transitions contained exactly one `07 01 04` write for the resolved
`0951:16E4` device, and consecutive full-profile comparisons changed only the
Button 4 record at `0x88..0x8B`:

| NGENUITY Legacy Mouse Function | Button 4 record |
| --- | --- |
| Left Click | `02 F0 00 00` |
| Right Click | `02 F2 00 02` |
| Middle | `02 F1 00 01` |
| Forward | `02 F9 00 04` |
| Tilt L | `02 F5 00 00` |
| Tilt R | `02 F6 00 00` |
| DPI Toggle | `71 F0 00 00` |
| Scroll Up | `02 F4 00 00` |
| Scroll Down | `02 F3 00 00` |
| Back | `02 F8 00 03` |

The series repeated `Scroll Up -> Scroll Down -> Scroll Up -> Scroll Down` and
reproduced the same two records. This corrects the previous offline-only
interpretation, which had the `F3`/`F4` scroll directions reversed. The final
transition restored Button 4 to Back. Together with the same action families
already present on the factory middle/forward/tilt controls, this promotes all
ten Mouse Functions to the portable evidence gate for the nine general
controls. Primary left/right swaps remain separately capture-gated. No onboard
save was sent.

With NGENUITY Legacy closed, OpenHyperX then wrote the captured Scroll Up
record (`02 F4 00 00`) to Button 4. An independent runtime read returned
Mouse Scroll Up, and pressing the physical button scrolled a long page upward.
OpenHyperX restored Button 4 to Back afterward and a final read confirmed the
restoration without changing the other mappings. Both writes were volatile;
no onboard save was sent.

The next 2026-09-20 series exercised every Multimedia assignment on Button 4.
It produced 14 separate five-second captures on `\\.\USBPcap1`, address 57,
for release `1124`; sizes ranged from 37,188 to 87,868 bytes and the raw files
remain outside Git. The no-op baseline again contained no runtime-profile
write, and every named transition contained exactly one 264-byte `07 01 04`
write. Within the Multimedia sequence only byte `0x8B` changed. The final
transition changed the complete Button 4 record back to Mouse Back:

| NGENUITY Legacy Multimedia function | Button 4 record |
| --- | --- |
| Play/Pause | `04 00 00 CD` |
| Stop | `04 00 00 B7` |
| Next | `04 00 00 B5` |
| Previous | `04 00 00 B6` |
| Mute Volume | `04 00 00 E2` |
| Volume Up | `04 00 00 E9` |
| Volume Down | `04 00 00 EA` |

Play/Pause, Stop, Next, Previous and Mute Volume were each captured twice and
reproduced the same records. Volume Up/Down additionally match the earlier
isolated captures and factory records. The final write restored Button 4 to
`02 F8 00 03` (Back). This promotes all seven Multimedia functions to the
portable evidence gate for the nine general controls. No onboard save was
sent.

With NGENUITY Legacy closed, OpenHyperX then wrote the captured Play/Pause
record (`04 00 00 CD`) to Button 4. An independent runtime read returned
Multimedia Play/Pause, and pressing the physical button correctly toggled
media between playback and pause. OpenHyperX restored Button 4 to Back and a
final read confirmed that the other mappings were unchanged. Both writes were
volatile; no onboard save was sent.

A third 2026-09-20 series exercised all six Windows Shortcut assignments on
Button 4. It produced 14 separate five-second captures on
`\\.\USBPcap1`, address 57, for release `1124`; sizes ranged from 37,470
to 88,604 bytes and the raw files remain outside Git. The no-op baseline had no
runtime-profile write. Every named transition contained exactly one 264-byte
`07 01 04` write, and consecutive shortcut states changed only bytes inside
the Button 4 record at `0x88..0x8B`:

| NGENUITY Legacy Windows Shortcut | Button 4 record |
| --- | --- |
| Cycle Apps | `23 E3 2B 00` |
| Switch Apps | `23 E2 2B 00` |
| Cut | `23 E0 1B 00` |
| Copy | `23 E0 06 00` |
| Paste | `23 E0 19 00` |
| Undo | `23 E0 1D 00` |

Every shortcut was captured twice and reproduced the same record; Copy also
matches the earlier Button 5 capture. The standard HID usages identify Cycle
Apps as Left GUI+Tab and Switch Apps as Left Alt+Tab. This corrects the former
offline-only Cycle Apps hypothesis `23 E2 29 00`. The final write restored
Button 4 to `02 F8 00 03` (Back), promoting all six shortcuts to the portable
evidence gate for the nine general controls. No onboard save was sent.

With NGENUITY Legacy closed, OpenHyperX then wrote the corrected Cycle Apps
record (`23 E3 2B 00`) to Button 4. An independent runtime read returned
Windows Shortcut Cycle Apps, and pressing the physical button opened Windows
Task View exactly like Win+Tab. OpenHyperX restored Button 4 to Back and a
final read confirmed that the other mappings were unchanged. Both writes were
volatile; no onboard save was sent.

### Coupled primary-button layout

On 2026-09-20, the first attempted primary-button matrix exposed an important
UI constraint: NGENUITY Legacy does not permit two Left Click or two Right
Click assignments. Choosing the opposite action for either physical primary
button atomically swaps both records. That initial series was stopped instead
of inventing an impossible intermediate state, then replaced with the checked-in
five-step `pulsefire-raid-primary-click-pair-20260920` plan.

The corrected series produced five separate five-second captures on
`\\.\USBPcap1`, address 57, for release `1124`, with sizes 37,188, 39,286,
94,730, 41,056 and 39,662 bytes. The standard-layout no-op baseline contained
no runtime-profile write. Each of the four actual transitions contained one
264-byte `07 01 04` write. Both repeated states were byte-for-byte stable:

| Layout | physical left record at `0x7C` | physical right record at `0x80` |
| --- | --- | --- |
| Standard | `02 F0 00 00` | `02 F2 00 02` |
| Swapped | `02 F2 00 02` | `02 F0 00 00` |

Only offsets `0x7D`, `0x7F`, `0x81` and `0x83` changed between the two profile
images. The final transition restored Standard. No onboard save was sent. The
protocol and driver therefore expose this setting only as one coupled
`Standard | Swapped` value and reject individual primary-button patches.

On 2026-09-21, with NGENUITY Legacy closed, OpenHyperX changed the runtime
layout from Standard to Swapped. An independent read returned physical
left=`Right Click` and physical right=`Left Click`, and the operator confirmed
both behaviors by clicking. OpenHyperX then restored Standard; the final read
returned physical left=`Left Click` and physical right=`Right Click`. Both
writes were volatile and no onboard-save transaction was sent.

The public macro model is no longer shaped like those temporary fixtures. A
TOML macro contains a playback policy and an ordered timeline of keyboard or
mouse-button down/up events, each with its own delay. The checked-in AB/20-ms
example was applied through this TOML path on the Windows host; the device
accepted the same confirmed macro transaction and an independent profile read
returned the Button 5 macro reference.

### Chords, recorded timing and primary mouse clicks

On 2026-09-17, after a reboot changed the transient USB address, USBPcap extcap
topology plus an injected descriptor identified the unit as
`\\.\USBPcap1`, address `52`, VID/PID `0951:16E4`. The isolated capture
`openhyperx-macro-coverage-play-once-recorded-timing-20260917.pcapng` assigned
one Button 5 macro with Standard Timing disabled and Play Once. Its ordered
inputs were `LeftShift+A`, `LeftControl+B`, left click, right click and middle
click. NGENUITY Legacy's compact view grouped each chord, while Expanded View retained
the individual down/up transitions.

The complete nonzero macro report was:

```text
07 05 04 04 00 00 00 00 00 01
80 14 E1 83 B9 04 00 8D 04 02 FE E1
84 93 E0 84 56 05 01 58 05 01 C5 E0
87 62 B7 00 6E B7 84 B3 B8 00 5E B8
82 CE B9 00 AC B9
```

All remaining bytes through the 264-byte report were zero. Decoding every
three-byte record as `state|delay-high, delay-low, input-code` yields:

| Input transition | Delay (ms) | Code |
| --- | ---: | ---: |
| Left Shift down / A down / A up / Left Shift up | 20 / 953 / 141 / 766 | `E1 / 04 / 04 / E1` |
| Left Control down / B down / B up / Left Control up | 1171 / 1110 / 344 / 453 | `E0 / 05 / 05 / E0` |
| left mouse down / up | 1890 / 110 | `B7 / B7` |
| right mouse down / up | 1203 / 94 | `B8 / B8` |
| middle mouse down / up | 718 / 172 | `B9 / B9` |

This independently confirms that modifier chords remain separate transitions
on the wire, nonuniform timing uses the same 15-bit per-event field, keyboard
codes are USB HID Keyboard usages for letters and modifiers, and the three
primary mouse buttons use `B7`, `B8` and `B9`. The runtime profile write again
used Button 5 reference `53 00 00 04`; no onboard save occurred.

The encoder now accepts balanced Play Once timelines using those established
event families. It conservatively limits a macro to 14 transitions, the
largest locally captured timeline. Its `0..9999` ms validation mirrors the
observed NGENUITY Legacy numeric editor and fits the confirmed 15-bit field, but the
two boundary values have not yet been hardware-tested. Repeat modes, longer
timelines, other mouse inputs, other target controls and onboard macro storage
remain blocked.

Later on 2026-09-17, with NGENUITY Legacy and other device writers stopped,
OpenHyperX loaded `examples/macros/shift-a-20ms.toml` and assigned its four
20-ms transitions (`LeftShift` down, `A` down, `A` up, `LeftShift` up) to
Button 5 on the physical release-`1124` unit. An independent runtime-profile
read returned the macro reference and showed the other ten button records
unchanged. Pressing Button 5 emitted uppercase `A`, functionally confirming
modifier state, event ordering and the general TOML-to-report path. The mouse
continued to operate normally. No onboard-save transaction was sent.

### Play Once macro on Button 4

On 2026-09-25, NGENUITY Legacy assigned an existing `A`, `B`, Play Once macro
with 20 ms timing to Button 4 and restored Mouse Back twice. The checked-in
five-step capture plan used ten-second files on `\\.\USBPcap1`, dynamically
resolved to address 15 for the inspected target traffic, release `1124`.
The five `.pcapng` files remain outside Git and measured 73,054, 191,116,
101,910, 96,364 and 98,714 bytes. The no-op baseline contained no macro or
runtime-profile write. Every changed step contained exactly one 264-byte
`07 01 04` runtime-profile write.

Both assignments sent the same new 264-byte feature report on the known
`MI_01/COL05` configuration collection. Its nonzero prefix was:

```text
07 05 04 03 00 00 00 00 00 01
80 14 04 00 14 04 80 14 05 00 14 05
```

All remaining bytes were zero. The report is identical to the earlier Button 5
AB/20-ms report except for target byte `0x03: 04 -> 03`. NGENUITY also resent
the existing, separate 14-event Button 5 macro with target byte `04` before
each assignment, then wrote a full profile whose Button 4 record at
`0x88..0x8B` was `53 00 00 03`. Button 5 remained `53 00 00 04`. Each reverse
step resent only the existing Button 5 macro and restored Button 4 to
`02 F8 00 03`. Comparing consecutive profile writes changed only offsets
`0x88` and `0x89`; the other button records and all unrelated profile bytes
were stable. No `Save to mouse` action occurred.

This confirms a separate runtime macro slot for Button 4 and permits a typed
runtime write for that target. At this stage it did not establish the Button 4
onboard macro transaction, so the save path rejected that reference before
selecting onboard memory; the later persistent captures are documented below.
Only the AB/20-ms timeline was isolated and
functionally tested on Button 4; other supported event timelines reuse the
Button 5 event codec and still need Button 4-specific hardware checks. Other
button targets still need isolated captures.

With NGENUITY Legacy and its helper closed, OpenHyperX read Button 4 as Back,
sent only the captured Button 4 AB macro report followed by the runtime-profile
write, and independently read Button 4 as a macro reference while Button 5 and
all other mappings remained unchanged. Pressing physical Button 4 once typed
exactly lowercase `ab`, as confirmed by the operator. OpenHyperX then restored
Button 4 to Back; a final independent read showed the original mapping set.
No onboard save occurred. This also verifies that resending the existing
Button 5 macro definition, as NGENUITY does, is unnecessary for this volatile
Button 4 update.

### Button 4 Play Once macro in the onboard-save transaction

On 2026-09-25, a five-step NGENUITY Legacy capture series isolated a no-op
baseline, two consecutive `Save to mouse` clicks with the existing Button 4
AB/20-ms/Play-Once macro, a runtime return to Mouse Back, and a final
`Save to mouse` return to Back. The checked-in capture plan documents each UI
transition. USBPcap identity mode resolved `0951:16E4`, release `1124`, on
`\\.\USBPcap1` to temporary address 15 for this series. All five files were
nonempty (73,962, 105,214, 78,988, 94,954 and 77,876 bytes) and remain
outside Git. The no-op baseline contained no profile or macro write.

Both macro saves had the same ordered transaction already known for Button 5:
onboard selection `07 03 01 64`, three indexed `07 18 01 00` lighting reports,
`07 81` profile request/response, the existing Button 5 macro report
`07 05 01 04`, then a separate Button 4 macro report `07 05 01 03`, a full
`07 01 01` profile write, and runtime selection `07 03 04 64` about one second
later. Every stage received the previously documented eight-byte success ACK
on endpoint `0x83`. The Button 4 report was 264 bytes, with nonzero prefix:

```text
07 05 01 03 00 00 00 00 00 01
80 14 04 00 14 04 80 14 05 00 14 05
```

The rest was zero-filled. The two onboard Button 4 reports were byte-for-byte
identical. Compared with the independently captured runtime Button 4 AB report,
only offset `0x02` changed from section `04` to section `01`. The existing
Button 5 definition was also identical across the two macro saves and the
final Back save. The first onboard profile read still had Button 4 Back
`02 F8 00 03`; the first write changed it to `53 00 00 03`. The second save
read that reference and wrote the same full profile image, apart from the
read/write opcode byte. The final Back save read the macro reference and
changed only the Button 4 record to `02 F8 00 03`.

One unrelated byte changed on the *first* save: offset `0x83`, the last byte
of the right-click record, went from `00` to `02`. It remained `02` in the
second and final onboard reads/writes. This appears to be NGENUITY Legacy
normalizing an existing record, not part of the Button 4 macro transaction;
its precise semantics are not established and must not be used to invent a
new encoder. No other profile bytes changed. This capture proves the packet
ordering and repeated write/readback. The operator then repeated the known
NGENUITY Legacy save with Button 4's AB macro, closed the app and helper,
unplugged/replugged the mouse, and confirmed that physical Button 4 still
typed exactly `ab`. An independent OpenHyperX `info` read after reconnection
found both Button 4 and Button 5 macro references. This confirms persistence
of the Button 4 event stream, not merely its profile reference.

OpenHyperX subsequently changed Button 4 to Back in the runtime profile and
independently read it back. Its previously verified `profile save-to-mouse`
path was then used to restore Back onboard, but the first `0x03` prelude
received **zero** bytes on the acknowledgement handle within 500 ms. The
driver aborted before selecting onboard memory and did not retry. The operator
used NGENUITY Legacy to save Back again; after closing it and power-cycling,
physical Button 4 functioned as Back and the cursor and primary clicks worked
normally. The missed ACK is an ACK-path failure to diagnose separately,
not evidence for changing the captured Button 4 report format.

The pinned `hidapi` 2.6.1 Windows-native backend starts an overlapped
`ReadFile` only when `read_timeout` is first called. Our save path previously
called it *after* sending each feature report, leaving a possible race with
the fast interrupt-IN ACK. Pre-arming a zero-timeout read before TX is a
source-backed mitigation, but that causal explanation remains a hypothesis
until a narrow hardware ACK check or USBPcap trace confirms it. No automatic
retry of an ambiguous save is allowed.

### ACK diagnostic and NGENUITY Legacy startup

On 2026-09-26, pre-arming the read did **not** fix the missing ACK. The
non-persistent `profile check-save-ack` probe sent only the confirmed runtime
selector `07 03 04 64`; it did not select or write onboard memory. Its
automated eight-second USBPcap capture
`openhyperx-save-ack-probe-20260926.pcapng` was 6,616 bytes and contained 100
frames. Injected descriptors confirmed `0951:16E4`, release `1124`, at the
temporary address 19. Frame 99 contained the complete 264-byte feature report,
and frame 100 completed the control transfer successfully. There were no
endpoint-`0x83` transfers or ACK payloads in this capture. The CLI timed out
without retrying. This is evidence against a read-posting race being the sole
cause; it does not justify skipping ACK validation.

A separate 15-second capture of launching NGENUITY Legacy, without deliberate
setting changes or `Save to mouse`, produced
`openhyperx-ngenuity-legacy-startup-ack-20260926.pcapng` (119,386 bytes).
Descriptors again confirmed the same identity and release. The first two
264-byte interface-1 feature reports were:

| Frame | Time (s) | Prefix | Remaining bytes |
| --- | --- | --- | --- |
| 7 | 3.455358 | `07 07 00` | all zero |
| 9 | 3.563318 | `07 07 01` | all zero |

The first vendor ACK was frame 11 at 3.564226 s:
`00 00 07 07 00 00 00 00`, received on endpoint `0x83`. Subsequent known
runtime-selector, profile-request, Button 5 macro and runtime-profile reports
each had their expected ACK. NGENUITY Legacy automatically reapplied its
runtime profile and existing Button 5 macro; no onboard selector, indexed
onboard-lighting report or onboard-profile write was observed. The later
`02 03 ...` input packets are button events, not save acknowledgements.

**Hypothesis only:** opcode `0x07` and byte 2 may switch vendor input/ACK
delivery off/on. Startup ordering correlates with ACK availability, but a
single startup does not establish lifecycle, persistence or all side effects.
At this initial evidence stage neither report was encoded or transmitted by
OpenHyperX. Required evidence was an isolated close-from-tray capture, a
repeated launch/close lifecycle and a separate non-persistent known-selector
ACK probe with Legacy closed.
Do not infer firmware commands, bypass acknowledgements or retry a save from
this observation.

An isolated eight-second close-from-tray capture
`openhyperx-ngenuity-legacy-close-ack-20260926.pcapng` (69,674 bytes) contained
53 direct-RGB reports followed by one known onboard selector `07 03 01 64`
at 3.344453 s. Its expected `00 00 07 03 00 00 00 00` ACK arrived at
3.407799 s. There was no opcode-`0x07` report, profile write, macro write or
indexed lighting write. Selecting the existing onboard section is distinct
from persisting new settings.

With Legacy closed, the same fixed-purpose OpenHyperX probe then **succeeded**.
`openhyperx-save-ack-after-legacy-startup-20260926.pcapng` (9,154 bytes)
confirmed the same identity and contained exactly one feature report: runtime
selector at frame 151, 2.473356 s, followed by its expected ACK at frame 153,
2.536729 s (63.373 ms later). The wrapper exited successfully. No profile or
onboard-memory write occurred. This establishes that the ACK path works after
Legacy startup and remains available after its captured tray closure. It
does not yet isolate the side effects of opcode `0x07`; a USB power-cycle and
repeated startup are needed before introducing a typed initialization codec.

The operator then unplugged the mouse for five seconds, reconnected it with
Legacy still closed, and confirmed normal cursor movement and primary clicks.
The one-shot probe again timed out without retrying. Its eight-second capture
`openhyperx-save-ack-after-power-cycle-20260926.pcapng` (6,616 bytes)
confirmed `0951:16E4`, release `1124`, now at temporary address 20. Frame 99
at 2.457478 s contained the same complete runtime selector; there were no
endpoint-`0x83` responses. Thus ACK availability after Legacy startup survives
app closure but does **not** survive this physical power-cycle. The startup
reports still required a repeated isolated capture before implementation.

### Repeated volatile session startup

The reviewed `pulsefire-raid-legacy-ack-lifecycle-repeat-20260926` capture plan
then recorded three separate eight-second files: closed no-op (414 bytes),
launch (181,578 bytes), and close-from-tray (90,968 bytes). The completed
manifest and all descriptors confirmed the target `0951:16E4`, release
`1124`, at temporary address 20. The no-op contained only injected descriptors,
with no vendor reports.

Launch frames 3083 and 3089, at 4.354297 s and 4.460965 s, contained exactly
the same complete 264-byte `07 07 00` and `07 07 01` reports as the first
startup. Every byte after offset 2 was zero. The phase interval was
106.668 ms, compared with 107.960 ms in the first capture. Again there was
no ACK for phase 0; phase 1 received `00 00 07 07 00 00 00 00` at frame
3091, 4.461965 s. The next runtime selector was at 4.773274 s, 311.309 ms
after that ACK (311.253 ms in the first startup). All subsequent known
runtime-profile and Button 5 macro reports were acknowledged. No onboard
write was present. Closing again sent only the known onboard selector at
frame 703, 2.769138 s, with ACK at frame 705, 2.833380 s; there was no
opcode-`0x07` report or persistent write.

These independent launches establish a fixed normal **volatile vendor-session
startup** sequence, after which interrupt acknowledgements become available.
The individual phase bytes are not exposed as arbitrary enable/disable modes;
their internal semantics and other possible values remain unknown. OpenHyperX
now encodes only these two complete constant reports. An explicit diagnostic
`profile check-save-ack --initialize-session` sends phase 0, waits 110 ms,
sends phase 1 and checks its exact ACK, waits 315 ms, then probes the known
runtime selector. Any queued input, missing ACK or wrong ACK aborts without
retry. It sends no profile image, macro or lighting snapshot and does not
select/write onboard memory. At that diagnostic stage, saves did not yet
initialize automatically; the later integration is documented below.
Golden and mock tests cover the reports, ordering, waits and failure gates;
hardware validation of the OpenHyperX initializer after a fresh USB
power-cycle was the next step.

### OpenHyperX session initializer hardware check

With Legacy closed, the operator unplugged/reconnected the mouse again and
confirmed normal cursor and primary-button operation. Native Windows
`--trace info` read the runtime image before and after
`profile check-save-ack --initialize-session`; the complete two 264-byte
read responses were byte-for-byte identical. They retained 1000 Hz, the
four DPI stages/colors (800 `#2B00FF`, 1600 `#CD00FF`, 3200 `#32FF00`,
6400 `#FF0000`), active stage 1, all eleven mappings, Button 4 Back and
the existing Button 5 macro reference. This comparison does not read the
macro event stream or prove every physical button's behavior.

The eight-second `openhyperx-session-init-probe-20260926.pcapng` was
7,524 bytes; descriptors confirmed `0951:16E4`, release `1124`, at
temporary address 21. Its complete feature-report sequence was:

| Frame | Time (s) | Interface-1 feature prefix | Interface-2 ACK |
| --- | --- | --- | --- |
| 99 | 1.891228 | `07 07 00` | none, matching both Legacy captures |
| 101 | 2.002314 | `07 07 01` | frame 103 at 2.003891: `00 00 07 07 00 00 00 00` |
| 105 | 2.320162 | `07 03 04 64` | frame 107 at 2.383892: `00 00 07 03 00 00 00 00` |

All three reports were exactly 264 bytes with the captured constant bytes
and zero-fill. There were no profile, macro, indexed-lighting or onboard
writes. The diagnostic and subsequent independent `info` exited successfully.
This independently verifies the fixed startup's ACK effect without replaying
Legacy's automatic runtime configuration. Linux format, Clippy and 118 unit
tests passed; native Windows passed the same 118 tests, build and discovery
smoke. The operator subsequently confirmed normal cursor movement, primary
clicks, physical Button 4 Back, and unchanged lighting.

With no USB reconnect and Legacy still closed, an explicitly approved second
OpenHyperX initialization tested an already-active session. The capture
`openhyperx-session-init-active-repeat-20260926.pcapng` was again 7,524 bytes,
with the same identity/release at address 21. Frames 99 and 101 sent the same
complete startup reports at 2.454422 s and 2.567027 s. Again phase 0 had no
ACK and phase 1 received the expected opcode-`0x07` ACK at frame 103,
2.567788 s. The runtime-selector probe at frame 105, 2.885823 s, received
its expected ACK at frame 107, 2.949750 s. No other feature report or
persistent write was present. The diagnostic and independent native `info`
both exited successfully; its full runtime image and all decoded fields
remained unchanged. This verifies repeatable startup on this already-active
release-`1124` unit, not arbitrary phase values or other firmware revisions.
The following implementation integrates that verified startup before the
existing save sequence, retaining fail-closed ACK checks.

### Multi-slot save implementation

On 2026-09-26, the driver was extended to initialize the confirmed vendor
session once at the start of each explicitly requested onboard save. This is
normal transaction setup, not a retry after a missing acknowledgement. All
following stages retain exact ACK checks and fail closed.

Both Button 4 and Button 5 Play Once macro reports can now be converted to the
captured onboard section. The typed `PulsefireRaidOnboardMacros` validates
targets, uniqueness and complete timelines without HID I/O. Every runtime
macro reference must have its definition, and a supplied definition without
the corresponding reference is rejected before onboard selection. Reports
are ordered Button 5 then Button 4 exactly as in the repeated two-slot captures.
The existing single-Button-5 entry point remains a compatibility wrapper.

CLI `--macro-definition` accepts repeatable `button4=FILE` / `button5=FILE`;
a bare path retains the original Button 5 meaning. Files and encoders are
validated before discovery. The original macro timelines cannot be recovered
from profile references, so supplying the correct event streams remains the
caller's responsibility. The command does not silently replace runtime macros.
Golden/mock tests cover Button 4's full 264-byte onboard report, two-slot order,
unknown-byte preservation, missing/mismatched/duplicate definitions, startup
failure, and abort before profile commit on an unexpected Button 4 macro ACK.
### OpenHyperX two-slot persistent save capture

The explicitly approved `openhyperx-button4-save-ab-20260926.pcapng` capture
was 23,848 bytes. Injected descriptors identified `0951:16E4`, release `1124`,
at temporary address 21. NGENUITY Legacy and other competing writers were
closed. The fixed-purpose `scripts/verify-button4-save-windows.ps1` first read
the baseline, assigned AB/20-ms/Play-Once to Button 4 in runtime, and performed
one acknowledged onboard save without retrying. Button 5's existing
`coverage-recorded-timing.toml` timeline and wheel-off/logo-blue snapshot were
explicitly supplied, not inferred from profile references.

Frames 247/249 contained the two fixed startup reports, separated by 111.350 ms;
frame 251 contained the exact `00 00 07 07 00 00 00 00` ACK. The save selected
onboard at frame 263, sent the three indexed lighting reports at 267/271/275,
read onboard at 283/284, wrote Button 5's macro at 285, Button 4's macro at 289,
committed the profile at 293, and restored runtime selection at 297, 1.001 s
after the commit ACK. All acknowledged stages returned the complete expected
eight-byte ACK on endpoint `0x83`; phase 0 intentionally had no ACK.

The full 264-byte macro reports at 285/289 were byte-identical to the repeated
Legacy persistent captures. Comparing the baseline runtime response (frame 76)
with the pre-save runtime response (262) changed only offsets `0x88: 02 -> 53`
and `0x89: F8 -> 00`. Comparing the onboard response (284) with the commit (293)
changed those same two mapping bytes and only the read/write opcode at `0x01`.
Every other byte, including DPI stages and colors, polling, all other button
records and unknown onboard fields, was preserved. A separate native `info`
read after the save exited successfully; it finished outside the eight-second
capture window. Physical USB power-cycle validation follows separately.

With Legacy still closed, the operator physically disconnected USB for five
seconds, reconnected, and confirmed that pressing Button 4 once still typed
exactly lowercase `ab`. The cursor and primary clicks worked normally, wheel
remained off, and logo remained blue. This establishes OpenHyperX persistence
of the Button 4 event stream alongside the existing Button 5 macro, rather
than only a successful profile-reference write.

The separate restoration capture
`openhyperx-button4-save-back-20260926.pcapng` was 27,548 bytes and resolved
the same identity/release to address 22 after reconnection. Its first full
runtime read (frame 72) was byte-identical to the macro runtime image from the
previous save (frame 262), independently confirming all decoded settings and
opaque runtime bytes across USB power loss. Before startup, ordinary runtime
reads/writes produced no interrupt ACKs; after the two fixed startup reports
(233/235), the initializer and every save stage returned the expected ACK.

Restoration changed only `0x88: 53 -> 02` and `0x89: 00 -> F8`. The onboard
commit at frame 275 differed from its source response (270) only in those
mapping bytes and the opcode at `0x01`. The single Button 5 onboard report at
271 remained byte-identical to Legacy's captured timeline; there was no Button
4 macro report during Back restoration. After runtime reselection (279), a
separately opened CLI read (352) confirmed the same two-byte restoration and
no unrelated runtime changes. The lab script now captures for twelve seconds
to include this independent after-read, with no extra manual UI transitions.

The operator then disconnected/reconnected USB again with Legacy closed and
confirmed physical Button 4 Back, normal cursor and primary clicks, wheel off
and blue logo. Both the macro save and the explicit Back restoration therefore
passed independent physical power-cycle checks.
A final independent native `--trace info` read after this second reconnection
returned all seven expected HID collections and a complete 264-byte runtime
profile byte-identical to the original pre-test Back baseline. Polling remained
1000 Hz and active DPI remained stage 1 at 800; the four stage values/colors,
Button 5 macro reference and all other bindings were unchanged.

## NGENUITY Legacy `.hxp` preset format

On 2026-09-18, an exported `Base Settings.hxp` from NGENUITY Legacy `5.38.0.0`
was examined offline. The file was not copied into the repository because it
contains local identifiers and recorded macro content. It is an uncompressed,
versioned binary serialization rather than a HID capture.

The 5,972-byte export begins with little-endian `HEADER_ALL = 0x7D6583CA` and
an embedded length of 5,904 bytes. The embedded preset begins with
`HEADER = 0x4D2C83CE`, format version `40`, a .NET-style seven-bit-length UTF-8
name, and serialized model data. The 60-byte export footer uses the observed
icon/game-link markers `0x12345670`, `0x12345672`, `0x12345673`, `0x12345671`
and `0x23456780`. The embedded payload differed from NGENUITY Legacy's
contemporaneous local `Master.hxp` only in two 16-byte identifiers.

The mouse object uses header `0x1A4A0099`. Its observed DPI list contains
64-byte records with a 16-byte source identifier, a direct little-endian DPI
integer and an ARGB color. The examined file decoded to 800 `#2B00FF`, 1600
`#CD00FF`, 3200 `#32FF00` and 6400 `#FF0000`. A following value was `1` and is
probably the selected stage, but its indexing semantics are not confirmed;
the importer therefore preserves it only as `source_active_stage`.

Macro objects use header `0x2545C654`; their fixed 38-byte event objects use
header `0x1B35A31E`. The object fields expose the macro name, Standard Timing
flag/value, raw playback mode, play count and event count. Observed event type
`1` contains USB HID keyboard usages with separate down/up actions. Type `2`
contains left actions `1/2`, right `4/5` and middle `7/8`. The 14-event recorded
macro reproduced every keyboard, modifier, mouse transition and timing from
the earlier USB capture. When Standard Timing is enabled, the global value is
the effective wire timing even though the preset retains recorded item times.
Raw playback mode `1` corresponds to the independently captured Play Once
configuration; other numeric modes remain unconverted.

Key-assignment objects use header `0x2CD854CB` and were 93 bytes each in this
preset. One assignment contained the exact 16-byte source identifier of the
14-event macro. The mapping from assignment identifiers to physical controls
has not been established, so imports preserve these references without
creating button bindings.

`hyperx-protocol::ngenuity_legacy` now parses both the version-40 export
wrapper and the internal preset form with bounds checks and synthetic fixtures.
The CLI can inspect the decoded data, compare two presets semantically and byte
by byte, or import confirmed DPI/macro fields to a partial OpenHyperX TOML
profile. These operations are offline and have no HID or onboard write path.
Polling, lighting, physical assignment targets and other Legacy preset
versions require isolated export comparisons before being decoded. Current
NGENUITY requires separate discovery and must not be passed to this parser by
assumption.

## Unknowns and required evidence

| Area | Current state | Required next experiment |
| --- | --- | --- |
| firmware/device info query | unknown | capture NGENUITY Legacy startup with no setting changes; identify repeated IN/feature queries |
| report descriptor for configuration collection | confirmed | optionally dump the two other vendor collections for research without sending reports |
| direct RGB transport | hardware-validated for independent wheel/logo colors and black/off; NGENUITY Legacy Solid and Cycle both use it, and the previous lighting state returns without keepalive | optionally measure the exact timeout/revert interval |
| RGB off semantics | direct black independently extinguishes both physical LEDs | compare NGENUITY Legacy's explicit lighting-off UI, if present, only to determine whether it differs from black |
| persistent RGB/effects | the two-zone Solid snapshot (including independent off/blue) persists through the OpenHyperX onboard-save path; foreground effects and the standalone firmware rainbow are separate, and other hardware effect selectors remain unknown | identify confirmed hardware-mode selectors and save semantics for rainbow and other non-Solid effects |
| DPI and stages | runtime read/set, per-stage value/color edits, active-stage selection and final-stage add/remove are implemented and hardware-validated; five big-endian X/Y slots, 200-16000 range and 50-DPI units are confirmed; OpenHyperX onboard save was verified across a power-cycle | determine whether independent X/Y values are supported |
| polling | all four interval codes captured; runtime 1000→500→1000 set/readback validated; 1000 Hz persisted through a separate NGENUITY Legacy save and power-cycle | verify effective USB report rate with an external rate tester |
| NGENUITY Legacy `.hxp` | version-40 container, DPI records, Play Once keyboard/primary-click macros and macro references are parsed offline; imports are explicitly partial | compare Legacy exports differing only in active stage, polling, lighting, one physical assignment and each repeat mode |
| button bindings | all 11 mappings are readable; all ten Mouse Functions, all seven Multimedia functions, all six Windows Shortcuts, Disabled and named keyboard usages are writable on the nine general controls; the physical primary pair has a repeated capture-backed Standard/Swapped encoding and OpenHyperX's atomic swap/restore was read back and functionally verified; Button 4 and Button 5 have separate captured Play Once runtime macro slots, with OpenHyperX's Button 4 AB assignment independently read back and functionally verified; the full Button 4 Mouse, Multimedia and Shortcut matrices are capture-backed, and OpenHyperX's Scroll Up, Play/Pause and Cycle Apps writes were read back and functionally verified on Button 4; portable writes are also hardware-tested on Button 6 and DPI; bounded Play Once macros support chords, nonuniform timing and primary clicks on confirmed targets | capture additional macro targets, repeat modes, longer timelines and remaining macro mouse events |
| onboard save | preservation-first driver/CLI transaction is covered by golden/mock tests and hardware-validated across a power-cycle for DPI, polling, all ordinary mappings, complete Button 4/5 Play Once macros and independent wheel/logo Solid colors; saves now initialize the verified volatile session once and check every ACK without retrying; the two-slot OpenHyperX save matched repeated Legacy macro packets exactly and preserved every unrelated onboard byte; Button 4 AB survived physical USB reconnection with Legacy closed | capture other macro targets and repeat modes; investigate non-Solid persistent lighting separately |
| NGENUITY Legacy locking | unknown | run `devices`, then future read-only `info`, with NGENUITY Legacy open and closed; record open errors |
| admin requirement | configuration collection opens without elevation | retest on a second Windows machine/account |

Firmware update, bootloader and DFU traffic is excluded from captures intended
for replay. If such traffic appears, label it and quarantine it; never add it to
fixtures used by a send path.
