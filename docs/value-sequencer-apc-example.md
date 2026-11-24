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


## Example 85: Serum harmony and drums

Run the real-plugin performance from the engine root:

```sh
cargo run --release -p meldritch2 -- perform crates/meldritch2/examples/85-serum-harmony-drums.lua
```

Requires native Serum 2 and PULSE Drum Synth plus the captured Serum factory
catalog. The default lead and bass are Classic Techno Lead and Gnarly Reese.
Set MELDRITCH_SERUM_CATALOG to override the catalog directory,
MELDRITCH_SERUM_LEAD and MELDRITCH_SERUM_BASS to select captured names or IDs,
and PULSE_STATE to restore an optional trusted PULSE kit state. Without
PULSE_STATE the installed plugin's default kit is used.

The lead and bass share root capture and diatonic transposition but use
independent eight-slot Boards. Bass is two octaves below its resolved melody.
Four 16-step velocity Boards drive PULSE's GM notes 36, 38, 42, 46 on
channel 10. A common 8 Hz clock supplies sixteenth notes at 120 BPM.

Stream Deck row 1 keys 5–6 select the lead or bass fader bank.
Changing banks resets fader soft pickup. Track buttons enable steps in
the selected bank. APC grid rows 1–2 edit kick, 3–4 snare, 5–6 hat,
7–8 percussion, with each pair spanning steps 1–16. Pads toggle the
ordinary velocity control between zero and 0.85, with LED state derived
from the same control. Existing nonzero authored velocities are preserved
until the corresponding pad is edited.

Stream Deck rows 2 and 3 provide instrument and individual drum lane mutes.
Stereo levels and master headroom are exposed in the TUI. Serum selectors
request indices from the existing asynchronous preset catalog, not a new
processor per preset. The initial patch names are shown; active patch
names and pending state are not yet projected into this TUI.

Independent clock_divider contracts feed the lead and bass advance and
gate paths. Default divisors are 1 and 2. The TUI controls expose 1–16;
the processor supports rounded divisors 1–256. A rising reset clears
phase and suppresses the coincident pulse. Divide-by-one passes all
pulses. Passed pulses retain their input high duration.

Real-plugin startup, sound quality and realtime throughput require local
validation. Live root, resolved note names and automatic topology
inspection remain outstanding.
