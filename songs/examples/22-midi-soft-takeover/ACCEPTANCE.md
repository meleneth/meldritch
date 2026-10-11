# MIDI soft takeover

Status: `accept` for the headless contract below. Nothing is stable yet.

Two lanes share an APC fader on two authored pages. The initial lane levels
are unity. Lua LCD/APC widgets can set either level independently of the fader.
The example uses the existing validated TOML model and command path.

## Required headless behavior

- The first distant fader observation does not edit state, render or record.
  A subsequent crossing, or arrival within one normalized MIDI step, acquires
  control. Following movements keep control, including app value quantization.
- A page switch rearms pickup even when switching away and back without moving
  the fader. Another interface's accepted value edit also rearms pickup.
- Acquired movement produces the existing typed control command, shared audio
  publication and captured session row. Suppressed movements produce none.
- Normalized centered/overdrive mappings retain their existing semantics;
  step buttons, modifiers, note actions and unconfigured ports retain routing.
- Enabled MIDI routing uses one 64-message FIFO for note/CC/pitch observations;
  page actions preceding fader events take effect before mapping those faders.
  Overflow is counted and reported. Lua-owned pads cannot also apply TOML actions.
- Invalid targets, nonfinite values, malformed MIDI and registry capacity have
  explicit diagnostics and cannot mutate authoritative state.
- Historical projects remain unchanged. `tui-song --soft-takeover` opts TOML
  projects into pickup; `--lua` enables it automatically. The original TOML
  default input policy remains supported. No mapping or pickup enters audio.

## Run

```sh
cargo run -- tui-song songs/examples/22-midi-soft-takeover --lua main.lua --midi-debug
```

Track buttons 1/2 choose page A/B. Fader 1 controls the selected lane level.
The grid's first two pads set A to 0.5 or B to 0.75; simulated LCD keys use
the same widgets. Move fader 1 through the selected lane's value to acquire it.
On page B, hold Shift to control lane B's octave instead of level. Releasing
the layer rearms level pickup. Octave steps retain the existing modifier policy.
Physical interaction/listening acceptance and USB reconnection remain separate.

Verify with `cargo test -p meldritch-control --test live_performance` and
`cargo test -p meldritch-cli control_thread_mapper`. The negative observation
fixture exercises invalid values, targets and packets. Historical compatibility
includes this project in addition to all previously pinned fixtures.

Pickup tolerance is 1/127 of the normalized parameter range after the existing
authored curve. This protects known static control values; an unknown or
out-of-range initial value is diagnosed and blocked until another interface
establishes an authoritative value. Both the queue and physical registry have
64 slots. Overflow drops observations with a count, never accepted facts.
