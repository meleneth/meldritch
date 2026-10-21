# Meldritch

Meldritch is a programmable environment for creating musical machines. It is
not a fixed synthesizer with a scripting layer. The instrument itself—its
behaviors, signals, modules, relationships, controls, and performance
surface—is the authored work.

Eurorack is an inspiration because it makes constructing an instrument part of
making music. It is not the ceiling. Software can create, clone, replace, and
destroy modules; change complete networks coherently; capture topology as
state; automate structure; and project the same instrument onto very different
interfaces. Meldritch exists to make those acts composable and performable.

The question guiding the project is:

> What becomes musically possible when constructing the synthesizer is itself
> a programmable act of composition?

## The division of responsibility

- **Lua is the composition and authoring language.** Authored instruments
  describe modules, connections, normalization, reusable compositions,
  exposed controls, performance surfaces, and eventually operations over
  structure.
- **Rust is the execution substrate.** It supplies DSP primitives, validation,
  lowering, scheduling, bounded real-time execution, state ownership, and
  device interoperability.
- **Interfaces are projections of instrument intent.** A TUI or hardware
  controller presents capabilities exposed by an instrument. It does not
  define what the instrument is or limit which instruments can exist.

This boundary is essential. Lua must become more than configuration for a
fixed Rust synthesizer, Rust must remain a coherent substrate rather than a
bag of unrelated plugins, and no particular UI or controller may become the
implicit instrument model.

## What works today

Meldritch 2 is the active implementation. It currently supports Lua-authored
graphs built from contract-defined Rust modules; named ports and parameters;
explicit connections and authored fallback normalizations; reusable nested
subpatches with exported ports and parameters; validation and compilation to a
bounded mono sample executor; stateful processing; control publication at
buffer boundaries; prepared graph replacement; instrument-declared controls
and terminal screens; and MIDI mappings, timed notes, and bounded polyphonic
voice assignment.

Those capabilities are foundations, not the product definition. Current
graphs are compiled as directed acyclic graphs, signals use explicit mono
ports (including paired stereo modules), normalizations are selected during
compilation, and structural performance operations remain intentionally
narrow. Feedback,
general signal domains, coherent structural automation, topology capture, and
adaptive controller projection remain intended capabilities rather than
implemented claims.

The executable Lua corpus lives in `crates/meldritch2/examples/`. Its focused
acceptance guide is in [examples/ACCEPTANCE.md](examples/ACCEPTANCE.md).

## Try the current system

```sh
cargo run -p meldritch2 -- crates/meldritch2/examples/01-basic.lua
cargo run -p meldritch2 -- perform crates/meldritch2/examples/66-screens.lua
cargo test -p meldritch2
```

The complete authored instruments can be performed directly:

```sh
cargo run -p meldritch2 -- perform examples/instruments/subtractive.lua
cargo run -p meldritch2 -- perform examples/instruments/drone.lua
cargo run -p meldritch2 -- perform examples/instruments/tr909.lua
```

The subtractive instrument uses MIDI channel 1 when a MIDI input is selected;
without one, the existing TUI note keys play it. The drone begins sounding as
soon as its audio stream starts and needs no MIDI input.

The TR-909 starts its authored pattern immediately. On first launch it fetches
only the eight required WAV files and stores pinned, hash-checked entries under
`$XDG_CACHE_HOME/meldritch/samples` (normally
`~/.cache/meldritch/samples`). Later launches are offline. Audio follows the
current system default; the instrument selects the connected APC mini mk2 by
name.

The APC mini mk2 Session-mode mapping is generic to the `step_grid` layout:

- pad rows are tracks and columns are the visible eight steps;
- Track Button 1 (note 100) and Track Button 2 (101) select Page A (steps 1–8)
  and Page B (9–16);
- the APC's Up/Down arrow buttons (Track Buttons 5–6, notes 104–105) select a
  track, while Left/Right (Track Buttons 7–8, notes 106–107) move the cursor
  across all 16 steps and switch pages at the boundary;
- Scene Launch 1 (note 112) toggles Play/Stop and Scene Launch 2 (113)
  restarts at step 1;
- Scene Launch 3 (note 114) clears the last-touched track only after a deliberate double
  press within 1.5 seconds.

Those identifiers and RGB messages follow the Akai APC mini mk2 communication
protocol. The TUI edits the same state with `1`–`8`, arrow keys, Tab, Space,
`+`/`-`, `r`, and Shift-C. Controller disconnects do not affect playback; the adapter
reconnects and restores the complete LED view.

The built-in `strudel` adapter uses the Strudel-compatible
`tidal-drum-machines.json` manifest pinned to dough-samples commit
`9eacfc86ec4393e68a463ff52b01c19cfaa77f38`, and assets pinned to
tidal-drum-machines commit `6577395a4d05031728ced2ec3e5637fa89d8be48`.
The eight keys are `RolandTR909_bd`, `sd`, `cp`, `hh`, `oh`, `lt`, `ht`, and
`rd`; no aliasing is necessary. The upstream sample repository does not
publish a license, so Meldritch does not redistribute or check in its audio;
users fetch it directly from the upstream repository at runtime.

Expected hardware can be selected by name in the Lua instrument, so normal
launches do not depend on device-list indices. Only declare hardware the
instrument actually expects; omitted roles keep their normal system default:

```lua
synth:devices {
  midi_input = "MicroLab mk3:MicroLab mk3",
  -- Extra inputs dispatch controls and Lua button hooks without owning the
  -- instrument's sample-timed note allocation.
  midi_controls = { "MPK mini" },
}
```

Names are case-insensitive unique substrings of the names printed by
`meldritch2 devices` and `meldritch2 midi-ports`. The `--device=INDEX` and
`--midi=INDEX` options remain available as one-run overrides. With no
`audio_output` declaration, playback follows the current system default.

MIDI buttons are connected to authored behavior in Lua, not assigned a fixed
meaning by the Rust device layer. `on_midi_button` accepts Note, CC, and Program
Change buttons and can distinguish press from release:

```lua
instrument:on_midi_button({
  name = "Stop", message = "note", channel = 10, number = 36,
}, function()
  sequence:stop()
end)

instrument:on_midi_button({
  name = "Start", message = "cc", channel = 1, number = 64,
  edge = "release",
}, function()
  sequence:start()
end)
```

See `crates/meldritch2/examples/68-midi-buttons.lua` for all three supported
message forms. A matching Note binding is consumed before note allocation, so
a pad used as an action does not also play a voice. This covers every control
on a MicroLab or MPK mini that actually emits one of those MIDI messages,
including MPK pads in Note, CC, and Program Change modes. Hardware-local mode
buttons are different: for example, octave and shift controls may change the
controller's own state without emitting an addressable MIDI event. No software
hook can observe such a press; the resulting notes and other emitted messages
remain available normally.

Audio and MIDI device commands depend on available host hardware. The active
repository check is:

```sh
bash tools/check.sh
```

Read [PHILOSOPHY.md](PHILOSOPHY.md) for the design constitution and
[ARCHITECTURE.md](ARCHITECTURE.md) for the boundary between verified reality,
current limitations, and intended architecture.
