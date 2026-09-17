# Pulsefire Raid research

Last updated: 2026-09-17.

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
- [HyperX NGENUITY page](https://hyperx.com/pages/ngenuity) confirms built-in
  dynamic RGB effects but does not document individual animation curves,
  palettes or timing semantics.
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

OpenHyperX has a profile patcher for DPI values, active stage, stage count and
colors. NGENUITY locally exposed a minimum of 200 DPI; the manufacturer
documents a maximum of 16000 DPI, and captures confirm a 50-DPI step. The
patcher therefore accepts multiples of 50 in the inclusive `200..=16000` range
and preserves unrelated profile bytes.

On 2026-09-16, the resulting runtime API and CLI were validated against the
physical release-`1124` unit with NGENUITY and other writers stopped. Starting
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
physical `0951:16E4`, release `1124` unit with NGENUITY and its helper closed.
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
The runtime setter now uses the complete confirmed read/modify/write
transaction and preserves all non-polling bytes. It does not invoke the
separate onboard-save transaction.

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

The two no-op saves reproduced the same timing as well as the same payloads.
The first indexed `0x18` packet was sent about 64 ms after `07 03 01 64`; the
second followed about 35 ms later, while the third and `07 81` request followed
at roughly 2 ms intervals. NGENUITY waited about 113--115 ms between `07 81`
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
not survive without NGENUITY's volatile lighting stream.

The legal contents and full semantics of the three `0x18` packets therefore
remain unknown. They are part of the captured save transaction and block a
general safe replay until captures isolate zones/effects and a save containing
a macro establishes how auxiliary macro definitions participate. Raw
`.pcapng` files remain outside Git.

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

On 2026-09-17, with NGENUITY, OpenRGB and other device writers stopped,
OpenHyperX exercised the two direct fields independently on the physical
release-`1124` unit. For five seconds, `wheel=#FF0000, logo=#000000` lit only
the scroll wheel red. The reversed test, `wheel=#000000, logo=#0000FF`, lit
only the HyperX logo blue. A third test used `wheel=#FF0000, logo=#0000FF`;
both LEDs simultaneously displayed their distinct requested colors even though
the tested NGENUITY Solid UI did not expose separate per-LED colors. After each
foreground keepalive ended, both LEDs returned to their pre-test red state.
This confirms field-to-LED ordering, simultaneous independent colors,
per-LED black/off behavior and volatile reversion. None of these tests sent an
onboard-profile write.

OpenHyperX's `rgb cycle` is deliberately described as a software spectrum,
not as a decoded firmware effect or a byte-for-byte clone of NGENUITY's
animation. It computes portable RGB frames in `hyperx-core`, sends them through
the same confirmed two-LED direct report at roughly 60-ms intervals including
transport pacing, and stops after an explicit foreground duration. The target
can be both LEDs, the wheel only or the logo only; untargeted LEDs receive
black. It never writes the runtime or onboard profile.

The first physical test ran `rgb cycle --target all --duration 5 --period 2`
on the release-`1124` unit with other writers stopped. Both the wheel and logo
visibly moved through the spectrum in sync for the requested five seconds.
This validates the foreground renderer and sustained direct-report pacing on
Windows hardware; it does not imply persistence or reproduce NGENUITY's exact
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
per-zone fade durations; it does not establish NGENUITY's exact fade curve.

Two further eight-second programs exercised the remaining renderer families.
The first displayed the warm OpenHyperX Sun palette on the wheel and the cool
purple/blue/magenta Twilight palette on the logo, with independent 180-degree
phase state. The second displayed a repeating red Pulse on the wheel while the
logo stepped through deterministic pseudo-random saturated Confetti colors.
Both pairs ran independently and reverted after the foreground program ended.
Together with the earlier tests, this hardware-validates the TOML execution
path for Solid, Cycle, Pulse, Breathing, Triggered Fade, Confetti, Sun and
Twilight, but not visual parity with NGENUITY's undocumented definitions.

The observed legacy NGENUITY device view exposes Solid, Cycle, Pulse,
Breathing and a triggered Fade effect. Its Light Sync view exposes Solid,
Breathing, Cycle, Confetti, Sun and Twilight. HyperX's public material does not
define these animations. OpenHyperX therefore models the same names as
portable software effects with explicitly project-defined curves and palettes;
it does not claim exact visual parity until each NGENUITY output stream is
captured and compared. Programs can assign a different effect and phase to
`wheel` and `logo`. Triggered Fade consumes foreground Windows system
mouse-button down edges and never opens the standard mouse HID collection.

### Button-remapping UI observations

The Pulsefire Raid page in NGENUITY `5.38.0.0` exposed all 11 physical
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
`0..9999`, but that range has not been boundary-tested. NGENUITY became
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
| Keyboard A | `00 04 00 00` | isolated capture; `04` is the keyboard A usage |
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

The protocol crate now decodes these known records and has an offline patcher.
The other keyboard, consumer-control and Windows shortcut values are derived
from standard USB HID usage IDs after their record families were established;
they remain offline inference and are not sent by the device driver. Left and
right click are restricted to swapping those two functions, matching the UI.
The capture-backed `DPI Toggle` record is also decoded and patched offline.
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
but neither the device's accepted range nor the apparent NGENUITY `0..9999`
range has been boundary-tested.

A separate macro recorded `A` followed by `B`, still using Play Once and
Standard Timing 20 ms. NGENUITY emitted the same complete transaction twice
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

NGENUITY can disable Standard Timing and retain different timing values between
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
release-`1124` unit with NGENUITY and other writers stopped. The initial read
showed the existing Button 5 macro reference. OpenHyperX changed only Button 5
to the captured Mouse Forward record, and an independent read decoded it as
Forward while the other ten mappings were unchanged. It then sent the exact
captured A-then-B/20-ms/Play-Once macro definition followed by the profile
reference; an independent read again found the macro reference and all other
mappings unchanged. Pressing Button 5 then emitted lowercase `ab`, functionally
confirming the OpenHyperX-generated macro definition and ordering. No
onboard-save transaction was sent.

The driver's target-aware evidence gate exposes the seven ordinary Button 5
assignments seen in local captures (Disabled, Forward, Back, Volume Up, Copy,
keyboard A and DPI Toggle). It also exposes keyboard A and DPI Toggle for the
DPI control, matching the isolated forward transition described above. A mock
transport test verifies that this second target changes only the runtime write
opcode and its record at `0x9C..0x9F`. On the physical release-`1124` unit,
OpenHyperX then changed the DPI control from keyboard A to DPI Toggle; an
independent runtime read returned DPI Toggle while the other ten records were
unchanged, and pressing the mechanically unreliable control at its working
contact point changed DPI stages normally. Other decoded ordinary bindings
remain read-only inferences even when their usage IDs come from the USB HID
standard. No onboard save was sent.

On 2026-09-17, an isolated `Back -> Disabled -> Back` capture for Button 4
contained three runtime profile writes. Disabling changed only the Button 4
record at `0x88..0x8B`, from `02 F8 00 03` to `00 00 00 00`; restoring Back
changed exactly those bytes back to `02 F8 00 03`. The driver and CLI therefore
expose only Disabled and Mouse Back for Button 4. Other actions on this target
remain rejected before device discovery. No onboard save was sent.

With NGENUITY and its helper stopped, OpenHyperX then changed Button 4 from
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

After NGENUITY was stopped, OpenHyperX began from the captured Volume Down
state and exercised Button 7 through `Volume Down -> Volume Up -> Volume Down
-> Volume Up`. An independent runtime read confirmed every transition, the
other ten mappings remained unchanged, and the final state was Volume Up. No
onboard save was sent.

Together with earlier captures, six ordinary records now repeat byte-for-byte
on at least two physical controls: Disabled (Buttons 4/5), Mouse Back (Buttons
4/5), Volume Up (Buttons 5/7), Volume Down (Buttons 6/7), keyboard A (Button 5
and DPI) and DPI Toggle (Button 5 and DPI). NGENUITY exposes the same assignment
categories for the middle click, five numbered side controls, DPI control and
both wheel tilts. The runtime writer therefore treats those six records as
portable across these nine general controls while keeping the primary left and
right clicks unavailable. Forward, Copy and macro references remain
Button-5-specific until their portability is independently established.

With NGENUITY stopped, OpenHyperX then applied the portable Mouse Back record
to Button 6, whose initial mapping was Volume Down. An independent runtime read
returned Mouse Back while the other ten mappings were unchanged. A guarded
cleanup restored Volume Down and a final read confirmed it. This validates the
portable-record path on a third target slot beyond the pairs used to establish
the record matrix. No onboard save was sent.

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
click. NGENUITY's compact view grouped each chord, while Expanded View retained
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
observed NGENUITY numeric editor and fits the confirmed 15-bit field, but the
two boundary values have not yet been hardware-tested. Repeat modes, longer
timelines, other mouse inputs, other target controls and onboard macro storage
remain blocked.

Later on 2026-09-17, with NGENUITY and other device writers stopped,
OpenHyperX loaded `examples/macros/shift-a-20ms.toml` and assigned its four
20-ms transitions (`LeftShift` down, `A` down, `A` up, `LeftShift` up) to
Button 5 on the physical release-`1124` unit. An independent runtime-profile
read returned the macro reference and showed the other ten button records
unchanged. Pressing Button 5 emitted uppercase `A`, functionally confirming
modifier state, event ordering and the general TOML-to-report path. The mouse
continued to operate normally. No onboard-save transaction was sent.

## Unknowns and required evidence

| Area | Current state | Required next experiment |
| --- | --- | --- |
| firmware/device info query | unknown | capture NGENUITY startup with no setting changes; identify repeated IN/feature queries |
| report descriptor for configuration collection | confirmed | optionally dump the two other vendor collections for research without sending reports |
| direct RGB transport | hardware-validated for independent wheel/logo colors and black/off; NGENUITY Solid and Cycle both use it, and the previous lighting state returns without keepalive | optionally measure the exact timeout/revert interval |
| RGB off semantics | direct black independently extinguishes both physical LEDs | compare NGENUITY's explicit lighting-off UI, if present, only to determine whether it differs from black |
| persistent RGB/effects | OpenHyperX effects are foreground-rendered; a standalone firmware rainbow was observed after NGENUITY stopped, but NGENUITY Solid/Cycle and red/green save captures did not select or persist it | identify a confirmed hardware-mode selector and its save semantics before exposing hardware rainbow or other persistent effects |
| DPI and stages | runtime read/set, per-stage value/color edits, active-stage selection and final-stage add/remove are implemented and hardware-validated; five big-endian X/Y slots, 200-16000 range and 50-DPI units are confirmed | determine whether independent X/Y values are supported; onboard persistence remains part of the separate save blocker |
| polling | all four interval codes captured; runtime 1000→500→1000 set/readback validated; 1000 Hz persisted through a separate NGENUITY save and power-cycle | verify effective USB report rate with an external rate tester |
| button bindings | all 11 mappings are readable; six records repeated across physical slots are writable on the nine general controls, while Forward, Copy and macros remain Button-5-specific; the portable path is hardware-tested on Button 6, and target-specific Button 4, Button 5, Button 7 and DPI writes are also hardware-tested; bounded Button 5 Play Once macros support keyboard chords, nonuniform timing and left/right/middle clicks | capture primary-click swaps and the remaining ordinary records; isolate macro portability, repeat modes, longer timelines, remaining mouse events and timing boundaries |
| onboard save | repeated transaction, timing and read-modify-write captured; performance persistence confirmed; `0x18[0]` carries two correlated RGB triplets while `0x18[1..2]` were zero for Solid/All Lights | capture isolated wheel/logo and non-Solid saves, then save a profile containing a macro; determine acknowledgements and failure behavior before replay |
| NGENUITY locking | unknown | run `devices`, then future read-only `info`, with NGENUITY open and closed; record open errors |
| admin requirement | configuration collection opens without elevation | retest on a second Windows machine/account |

Firmware update, bootloader and DFU traffic is excluded from captures intended
for replay. If such traffic appears, label it and quarantine it; never add it to
fixtures used by a send path.
