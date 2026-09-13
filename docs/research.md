# Pulsefire Raid research

Last updated: 2026-09-13.

This document separates manufacturer facts, public implementation evidence,
local observations and hypotheses. Do not promote a hypothesis into a device
command without a capture or an independently reviewed implementation.

## Sources and licensing

- [HyperX product page](https://row.hyperx.com/pl/products/hyperx-pulsefire-raid-gaming-mouse)
  lists 11 programmable buttons, Pixart PMW3389, up to 16,000 DPI, three
  factory DPI presets, RGB, USB 2.0 and one onboard profile.
- [HyperX user guide](https://media.kingston.com/support/downloads/HyperX-Pulsefire-Raid-User-guide.pdf)
  identifies part number `HX-MC005B`.
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

## Known RGB report from OpenRGB

Status: confirmed in public OpenRGB code, descriptor-confirmed locally, and
successfully transmitted by OpenHyperX on 2026-09-07.

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
identity remained unchanged. Visual confirmation and a manual check of all
buttons still require the person at the machine.

The manufacturer page calls this one RGB lighting zone while OpenRGB exposes
two logical LEDs. Treat zone semantics as unresolved until checked in NGENUITY
and on hardware.

## Local NGENUITY captures

Captures were made with NGENUITY `5.38.0.0`, USBPcap and Wireshark on Windows
11. They were filtered to the Pulsefire Raid device address before capture;
raw files remain outside Git because they contain normal mouse input reports.

With only `NGenuity2Helper` running, NGENUITY sends the confirmed direct RGB
feature report about every 62 ms. The locally observed all-red transaction is:

- `bmRequestType = 0x21`, `SET_REPORT`, feature report ID `0x07`;
- `wIndex = 1`, `wLength = 264`;
- payload prefix `07 0A FF 00 00 FF 00 00 A0`, followed by zero fill.

Opening the NGENUITY GUI without changing a setting produced the same control
traffic. This locally confirms the OpenRGB report layout and shows that
NGENUITY's keepalive cadence is substantially faster than OpenRGB's timeout
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

NGENUITY displayed these same five values. The fifth level displayed 16000
DPI and white, independently confirming `0x0140 * 50 = 16000`. Separate X/Y
editing has not been tested, so the UI's axis-control behavior remains
unknown.

The active stage is a zero-based index at offset `0x31`: isolated `1 -> 2`
and `2 -> 3` UI transitions changed it from `00 -> 01 -> 02`. Stage enabled
flags are bytes `0x32..0x36`, one byte per stage, with `00` disabled and `01`
enabled. NGENUITY keeps enabled stages contiguous. Starting with three enabled
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
reverses. NGENUITY emitted an additional no-op read/write transaction before
the actual removal transaction; the no-op changed only the response/write
opcode.

Observed configuration sequence:

1. feature write with prefix `07 03 04 64`;
2. feature write with prefix `07 81`, otherwise zero-filled;
3. `GET_REPORT` for feature report `0x07`, length 264;
4. feature write containing the modified full profile image, prefix
   `07 01 04`.

The purpose and allowed values of the two prelude writes are unknown. No DPI
command may replay this sequence until they and the profile framing are
understood well enough to preserve every unrelated field.

### Polling-rate profile observations

The NGENUITY profile image changed only offset `0x18`. Repeated transitions
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
The surrounding transaction still needs to be implemented safely before
exposing a setter.

### Onboard save observations

Two captures of an explicit NGENUITY `Save to mouse` click were made on
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
(500 Hz) and first-stage DPI code `0x10` (800 DPI). NGENUITY wrote the current
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
NGENUITY preserved all other bytes in the no-op case.

NGENUITY and its helper were then closed, the mouse was physically unplugged
for about five seconds and reconnected, and all seven HID collections returned
with the same identity. A capture of the next NGENUITY startup read a profile
with prefix `07 81 04`, polling code `0x01` and DPI codes `0x14` at offsets
`0x1A` and `0x26`. NGENUITY's corresponding `07 01 04` write differed only in
the opcode. This confirms persistence of the saved performance values across
a power-cycle and shows that sections `0x01` and `0x04` share the confirmed
performance-field layout.

The person at the machine confirmed that cursor movement and the basic mouse
buttons still worked normally after the power-cycle. The lighting did not
remain at the previously visible static red: with NGENUITY and its helper
stopped, the mouse displayed a rainbow effect. This is consistent with the red
being supplied by NGENUITY's periodic volatile `07 0A` direct-RGB reports and
the mouse returning to a different stored lighting effect. It confirms
performance-field persistence only; persistent lighting storage is not yet
understood and must be tested separately.

The semantics and legal contents of the three `0x18` packets remain unknown.
They are part of the captured save transaction and block a safe replay until
isolated captures establish their variable fields. Raw `.pcapng` files remain
outside Git.

### Runtime lighting and persistence observations

On the tested NGENUITY profile, the Lighting UI showed `Solid`, red, target
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
identical to the earlier red save. After NGENUITY and its helper stopped, the
mouse briefly displayed static red and then returned to its standalone
rainbow effect. The green color therefore did not persist.

Changing the NGENUITY effect from `Solid` to its cycle/rainbow option produced
289 direct-RGB reports containing 131 distinct pairs of colors during one
20-second capture, and no non-direct feature report. On this unit and
NGENUITY version, both Solid and cycle lighting are software-rendered through
the volatile `0x0A` path. There is no local evidence that `Save to mouse`
persists a lighting effect, despite its `0x18` color payloads. Treat those
payloads as unknown until their purpose is isolated; do not use them as a
persistent-lighting encoder.

## Unknowns and required evidence

| Area | Current state | Required next experiment |
| --- | --- | --- |
| firmware/device info query | unknown | capture NGENUITY startup with no setting changes; identify repeated IN/feature queries |
| report descriptor for configuration collection | confirmed | optionally dump the two other vendor collections for research without sending reports |
| direct RGB transport | accepted locally; NGENUITY Solid and Cycle both use it, and lighting returns to rainbow without keepalive | isolate timeout/revert timing and confirm both physical LED zones |
| RGB off semantics | unknown | compare NGENUITY static black vs explicit lighting-off capture |
| persistent RGB/effects | Solid color and Cycle are software-rendered; red/green save captures did not persist lighting | determine whether Raid firmware exposes any hardware-lighting mode before claiming support; otherwise provide an explicit foreground engine |
| DPI and stages | five big-endian X/Y values, 50-DPI units, active index, enable flags and per-stage colors confirmed; parser remains offline/read-only | capture DPI edits on stages 2-5 and min/max boundaries; determine whether independent X/Y values are supported; understand the surrounding transaction before exposing writes |
| polling | all four interval codes captured; 1000 Hz persisted through save and power-cycle | implement only after the complete save transaction is understood; verify with an external rate tester |
| button bindings | unknown | isolated Back, Forward, Volume Up, Volume Down and Disabled captures |
| onboard save | repeated transaction captured; read-modify-write and performance persistence confirmed | identify the three `0x18` packets, acknowledgements, timing and failure behavior before replay |
| NGENUITY locking | unknown | run `devices`, then future read-only `info`, with NGENUITY open and closed; record open errors |
| admin requirement | configuration collection opens without elevation | retest on a second Windows machine/account |

Firmware update, bootloader and DFU traffic is excluded from captures intended
for replay. If such traffic appears, label it and quarantine it; never add it to
fixtures used by a send path.
