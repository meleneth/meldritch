# Example 84: Composed ValueSequencer with APC Mini mk2 and TUI

Path: `crates/meldritch2/tests/fixtures/lua/engine/84-value-sequencer-apc.lua`

The numbered Lua graph composes an eight-step ValueSequencer from
Counter, Constant, and Switch components, with explicit note/128-to-Hz
adapters, a clock, oscillator, envelope and VCA.

- APC Mini mk2 faders 1–8 edit the eight normalized values, with soft pickup.
- APC grid columns 1–8 toggle each step's independent enable component.
  Green pads are on; dim pads are off. Every row addresses the same eight steps.
- Bespoke TUI shows eight numbered values and their enable states.
  Press 1–8 to toggle, left/right to select, up/down to adjust, Home/End for bounds.
  Length and loop are additionally exposed as ordinary controls.
- Both devices use the existing compiled ControlHandles, not shadow step state.
- The APC worker launches even without the legacy SequencerControl object,
  and uses the existing APC shutdown animation that clears all device LEDs.

The device lookup matches `APC mini` case-insensitively against the available
MIDI device names. Audio uses the host's default output.
