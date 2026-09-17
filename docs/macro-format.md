# Macro format

Software macros are TOML files with a playback policy and an ordered event
timeline. The model is platform-independent; a device driver must separately
confirm that it can encode every requested event.

```toml
playback = "once"

[[events]]
type = "key-down"
key = "left-shift"
delay_ms = 20

[[events]]
type = "key-down"
key = "a"
delay_ms = 20

[[events]]
type = "key-up"
key = "a"
delay_ms = 20

[[events]]
type = "key-up"
key = "left-shift"
delay_ms = 20
```

`delay_ms` is the interval after that event and before the next one. Separate
down/up transitions keep keys held across later events, so the example emits
`LeftShift+A` even though its transitions have small nonzero delays.

Recognized playback names are `once`, `toggle-repeat` and
`repeat-while-held`. Event types are:

- `key-down` and `key-up`, with `key` and `delay_ms`;
- `mouse-button-down` and `mouse-button-up`, with `button` and `delay_ms`.

Parsing and device support are deliberately separate. The Pulsefire Raid
runtime encoder currently supports:

- `once` playback on Button 5;
- between 1 and 14 transitions, matching the largest isolated capture;
- an individual `delay_ms` from 0 through 9999 on every transition;
- balanced keyboard down/up transitions for letters, digits, F1–F24, common
  navigation/editing keys, keypad keys and left/right modifiers;
- balanced `left`, `right` and `middle` mouse-button down/up transitions.

Key names are case-insensitive, and underscores may replace hyphens. Useful
names include `a`–`z`, `0`–`9`, `f1`–`f24`, `enter`, `escape`, `tab`, `space`,
`backspace`, `insert`, `delete`, `home`, `end`, `page-up`, `page-down`, the
four `arrow-*` names, `keypad-0`–`keypad-9`, and modifiers such as
`left-control`, `left-shift`, `left-alt` and `left-windows` (plus their
`right-*` variants).

Every pressed input must be released, and a held input cannot be pressed a
second time. Invalid files are rejected before device discovery. Playback
`toggle-repeat` and `repeat-while-held`, more than 14 transitions, other mouse
buttons and onboard persistence remain blocked until their packet fields are
captured independently.

Examples:

- [AB with 20 ms timing](../examples/macros/ab-20ms.toml)
- [LeftShift+A chord](../examples/macros/shift-a-20ms.toml)
