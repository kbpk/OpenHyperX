# Macro format

Software macros are TOML files with a playback policy and an ordered event
timeline. The model is platform-independent; a device driver must separately
confirm that it can encode every requested event.

```toml
playback = "once"

[[events]]
type = "key-down"
key = "left-shift"
delay_ms = 0

[[events]]
type = "key-down"
key = "a"
delay_ms = 20

[[events]]
type = "key-up"
key = "a"
delay_ms = 0

[[events]]
type = "key-up"
key = "left-shift"
delay_ms = 20
```

`delay_ms` is the interval after that event and before the next one. Separate
down/up transitions keep keys held across later events; a zero delay between
two down events represents a chord such as `LeftShift+A`.

Recognized playback names are `once`, `toggle-repeat` and
`repeat-while-held`. Event types are:

- `key-down` and `key-up`, with `key` and `delay_ms`;
- `mouse-button-down` and `mouse-button-up`, with `button` and `delay_ms`.

Parsing and device support are deliberately separate. Pulsefire Raid currently
accepts only the three exact Play Once timelines established by local captures:

- A press/release with 20 ms on each event;
- A press/release with 300 ms on each event;
- A press/release followed by B press/release, all at 20 ms.

The working AB example is [examples/macros/ab-20ms.toml](../examples/macros/ab-20ms.toml).
Chord, mouse-button, nonuniform timing and other playback files parse into the
core model but are rejected before device discovery until isolated captures
confirm their Pulsefire Raid encoding.
