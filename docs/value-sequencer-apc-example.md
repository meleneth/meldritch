# Example 84: Composed ValueSequencer with APC Mini mk2 and TUI

Path: `crates/meldritch2/tests/fixtures/lua/engine/84-value-sequencer-apc.lua`

From the repository root, build and run the example with one command:

```bash
cargo run --release -p meldritch2 -- perform crates/meldritch2/tests/fixtures/lua/engine/84-value-sequencer-apc.lua
```

The numbered Lua graph composes an eight-step ValueSequencer from
Counter, Constant, and Switch components, with explicit note/128-to-Hz
adapters, a clock, oscillator, envelope and VCA.

- APC Mini mk2 faders 1–8 edit the eight normalized values, with soft pickup.
- APC track buttons 1–8 (Volume, Pan, Send, Device, Up, Down, Left, Right)
  toggle each step's independent enable component. Lit buttons are enabled;
  unlit buttons are muted. The RGB grid and scene buttons are unassigned.
- Bespoke TUI shows eight numbered values and their enable states.
  Press 1–8 to toggle, left/right to select, up/down to adjust, Home/End
  for bounds, [ / ] to change active length, and l to toggle looping.
- Both devices use the existing compiled ControlHandles, not shadow step state.
- The APC worker launches even without the legacy SequencerControl object,
  and uses the existing APC shutdown animation that clears all device LEDs.

The device lookup matches `APC mini mk2 Control` case-insensitively against the available
MIDI device names. The separate Notes port must not be selected. Audio uses the host's default output.
Connection and LED errors are reported on stderr; the controller retries automatically.
