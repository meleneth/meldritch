# Example 84: Composed ValueSequencer with APC Mini mk2 and TUI

Path: `crates/meldritch2/tests/fixtures/lua/engine/84-value-sequencer-apc.lua`

From the repository root, build and run the example with one command:

```bash
cargo run --release -p meldritch2 -- perform crates/meldritch2/tests/fixtures/lua/engine/84-value-sequencer-apc.lua
```

The numbered Lua graph composes an eight-step ValueSequencer from
Counter, Constant, and Switch components, with explicit root capture, scale-degree and note/128-to-Hz
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


## Harmony controls

The MicroLab mk3 on MIDI channel 1 supplies pitch and gate. Hold Stream Deck
XL key 1 (HOLD ROOT), play a new keyboard note, and release the button.
The captured absolute MIDI note persists until another armed note is played.
The keyboard does not directly sound the sequencer oscillator.

Keys 2–4 select I, IV and V (degree offsets 0, 3 and 4).
The default scale is major. Faders remain generic normalized controls;
the scale adapter divides their travel into seven equal regions and maps
each region to a scale degree. Diatonic shifts preserve the key and may
cross the octave boundary. The adapter also supports natural minor via
its minor parameter, but this example does not expose a mode selector.

The TUI lists the authored adapter chain and exposes the arm and shift
controls. The captured root is internal DSP state; live root/note-name
readout is not yet provided. The fader values shown are normalized values,
not degree names.
