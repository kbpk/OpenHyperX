# Software lighting

Pulsefire Raid direct RGB accepts one RGB color for the scroll wheel and one
for the HyperX logo. OpenHyperX can render a separate foreground software
effect for each physical LED without sending an onboard-profile command.

Run a program with:

```bash
hyperx-cli rgb play examples/lighting/independent-cycle-breathing.toml
```

The portable TOML format is:

```toml
duration_seconds = 30
frame_interval_ms = 50

[zones.wheel]
effect = "cycle"
period_ms = 4000
phase_degrees = 0

[zones.logo]
effect = "breathing"
color = "#8000FF"
period_ms = 2500
phase_degrees = 180
```

`wheel` and `logo` are independent. A missing zone is black. Phase is measured
in degrees from `0` through `359`, so `180` starts halfway through a periodic
effect. Program duration is `1..=3600` seconds and frame interval is
`20..=1000` ms. Validation happens before the device is opened.

## Effects

The effect names match choices visible in NGENUITY Legacy `5.38.0.0`, but the
official HyperX documentation does not define their curves or palettes. Until
capture analysis establishes those details, the renderer uses documented
OpenHyperX semantics:

| Effect | Fields | OpenHyperX behavior |
| --- | --- | --- |
| `off` | none | black |
| `solid` | `color` | constant color |
| `cycle` | `period_ms`, `phase_degrees` | saturated RGB spectrum |
| `pulse` | `color`, `period_ms`, `phase_degrees` | instant full brightness followed by a linear fade |
| `breathing` | `color`, `period_ms`, `phase_degrees` | smooth dark-to-bright-to-dark curve |
| `triggered-fade` | `color`, `fade_ms` | a mouse-button down edge restarts a linear fade |
| `confetti` | `step_ms`, `seed` | deterministic pseudo-random saturated colors |
| `sun` | `period_ms`, `phase_degrees` | animated warm red/orange/gold palette |
| `twilight` | `period_ms`, `phase_degrees` | animated dark purple/blue/magenta palette |

Periodic effects accept `period_ms = 100..=60000`. `confetti` accepts
`step_ms = 50..=5000`, and `triggered-fade` accepts
`fade_ms = 50..=10000`. Colors are six hexadecimal RGB digits with an optional
leading `#`.

`triggered-fade` currently uses foreground Windows system mouse-button state.
It does not open or seize the standard HID mouse collection. On other operating
systems, programs containing this effect are rejected before device discovery;
the portable renderer is ready for a future platform input adapter.

All effects stop with the foreground CLI process. They do not install a
service, write the runtime profile or write onboard memory. The mouse may
return to its earlier lighting state when the program exits.
