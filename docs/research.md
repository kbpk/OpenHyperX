# Pulsefire Raid research

Last updated: 2026-09-12.

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

| First stage | Offset `0x1A` | Offset `0x26` |
| --- | --- | --- |
| 800 DPI | `0x10` | `0x10` |
| 900 DPI | `0x12` | `0x12` |
| 1000 DPI | `0x14` | `0x14` |

The `900 -> 1000` and `1000 -> 900` transitions changed only these two bytes,
and a repeated `900 -> 1000` transition produced the same result. This
confirms a 50-DPI unit for the first stage and strongly indicates paired X/Y
values. Separate X/Y editing has not been tested, so the axis interpretation
remains a hypothesis.

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

The NGENUITY profile image changed only offset `0x18` in captures made from a
displayed 1000 Hz state:

| UI transition | Before | After | Confidence |
| --- | --- | --- | --- |
| 1000 -> 500 Hz | `0x02` | `0x01` | one isolated capture |
| 1000 -> 250 Hz | `0x02` | `0x04` | one isolated capture |

The UI appeared to reload 1000 Hz between the two experiments, so these are
not a continuous `1000 -> 500 -> 250` series. Repeat each transition, capture
125 Hz, and determine when NGENUITY commits or reloads the selected profile
before exposing a polling setter.

## Unknowns and required evidence

| Area | Current state | Required next experiment |
| --- | --- | --- |
| firmware/device info query | unknown | capture NGENUITY startup with no setting changes; identify repeated IN/feature queries |
| report descriptor for configuration collection | confirmed | optionally dump the two other vendor collections for research without sending reports |
| direct RGB transport | accepted locally and captured from NGENUITY | visually confirm red wheel/logo, cursor/buttons, and timeout/revert |
| RGB off semantics | unknown | compare NGENUITY static black vs explicit lighting-off capture |
| DPI and stages | first-stage 800/900/1000 values confirmed in profile image | map other stage offsets, stage count/active index, and the surrounding transaction |
| polling | 1000/500/250 profile enum candidates captured | repeat transitions, capture 125 Hz, establish commit/reload behavior, and verify with an external rate tester |
| button bindings | unknown | isolated Back, Forward, Volume Up, Volume Down and Disabled captures |
| onboard save | hardware advertised, command unknown | compare volatile edits with explicit “save to mouse” action and power-cycle; review every changed report before replay |
| NGENUITY locking | unknown | run `devices`, then future read-only `info`, with NGENUITY open and closed; record open errors |
| admin requirement | configuration collection opens without elevation | retest on a second Windows machine/account |

Firmware update, bootloader and DFU traffic is excluded from captures intended
for replay. If such traffic appears, label it and quarantine it; never add it to
fixtures used by a send path.
