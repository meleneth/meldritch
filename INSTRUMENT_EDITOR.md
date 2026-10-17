# Edit an existing instrument

> **Meldritch 1 archive:** This document records completed research and its
> historical plans. Meldritch 2 is the active project; this is not
> its current architecture, acceptance queue, or compatibility contract.

The first authoring slice edits existing oscillator, ADSR and low-pass parameters
in a validated `.mlsynth`. Rust owns the song source; keyboard and Lua commands
both change that source through the same typed validation and existing compiler.
TOML songs remain loadable. [The Lua piano roll](PIANO_ROLL.md) now edits the
initial note-pattern subset. [Fader/mixer assignments](CONTROL_ASSIGNMENTS.md)
and the [Plaits model catalog](PLAITS.md) are implemented.
[Typed patch composition](PATCH_EDITOR.md) now uses Ctrl-G in this editor.

Start the small supported bass patch without a physical controller:

```sh
cargo run -- tui-song songs/examples/25-edit-existing-synth --no-midi-controls
```

Press **Ctrl-E** to open the instrument editor. It shows the selected instrument,
module ID/type, parameter, current authored value, unit/range, imported default,
automation ownership and existing cables. Up/Down (or j/k) select parameters;
Tab/Shift-Tab select instruments. Left/Right (or -/+) edit values: 100 Hz for
cutoff, 0.01 for envelope/resonance values, and one waveform choice. Home resets
to the imported TOML default. Invalid steps report a rejection rather than clamp.
Press p to play/stop, r to rewind, Ctrl-S to save, Esc to close, q to quit.
Ctrl-Tab retains the existing cockpit mode command.

| Module | Editable parameter | Authored type and range |
| --- | --- | --- |
| plaits | model | 24 typed choices; see [Plaits](PLAITS.md), cycle with Left/Right |
| plaits | harmonics, timbre, morph | finite normalized values, 0 through 1 |
| oscillator | waveform | `sine`, `triangle`, `saw`, `pulse`, `square` |
| ADSR | attack, decay, release | finite seconds, >= 0 |
| ADSR | sustain | finite level, 0 through 1 |
| low-pass | cutoff_hz | finite Hz, > 0 |
| low-pass | resonance | finite level, 0 through 1 |

`square` preserves the historical pulse alias as a distinct authored value.
The existing DSP clamps effective cutoff to 0.49 times sample rate and resonance
to 0.99; the source retains the valid authored values. Polyphony, output channels,
frequency and output layout remain outside parameter editing. Ctrl-G exposes
the separate typed module/cable command draft; see the patch guide. A parameter targeted by a loaded automation pattern is inspected
with its owner and rejects a changed source value. A no-op remains a no-op.

Accepted edits compile before replacing the source. They invalidate the full
song render horizon and use the existing background worker and atomic audio
publication. Rapid edits supersede older jobs. The TUI reports pending versus
applied to published audio; callback consumption remains separate audio telemetry.
Applied does not prove physical sound. Existing live curated controls remain
runtime overrides; those temporary values are not exported as instrument defaults.
An authored edit clears the matching filter override and rearms fader pickup.

## Lua commands and authored definitions

The existing Lua control-plane v1 accepts this additional command from a widget:

```lua
return {
  name = "edit_instrument",
  edit = {
    target = { synth = "edited-bass", module = "env", parameter = "release" },
    value = 0.25,
  },
}
```

Declare the `instrument` dependency to inspect the read-only `view.instrument`
projection: `id`, source `fingerprint`, one-based `selected_parameter`, typed
`parameters` and imported `defaults`. This projection carries no independent
musical state. Example 25's `main.lua` sends the same release edit from APC pad
0, simulated deck button 0 and terminal button 0. Run with `--lua main.lua` to
load it; MIDI requires actual connected hardware.

Ctrl-S writes a new `authored/instrument-<unique-token>.lua` beneath the song root
and displays the relative path. Authored definitions are separate from
timestamped `performances/*.mlperformance` journals. Saving never overwrites a
prior export, TOML definition, composition script or journal. Failure retains
the edited source and reports that it remains unsaved.

The explicitly versioned instrument format is a Lua chunk returning
`kind = "instrument"`, `version = 1`, `base_fingerprint`, `instrument`, and
`parameters`. See [the complete example](https://github.com/meleneth/meldritch/blob/main/songs/examples/25-edit-existing-synth/edited-instrument.lua).
It stores absolute values for every supported parameter of the **selected
instrument**, including unchanged values, in stable module/parameter order.
Other instruments remain as authored in the base. Saving several edited
instruments as one combined definition is not supported in this slice.
Standalone exports reject unrelated source changes. Use F6 in the
[composed workflow](MUSICAL_AUTHORING.md) to save the selected graph together
with its selected pattern, declared mixer values and controller bindings.

The base is the same song root's unchanged `main.mlperformance` and referenced
TOML definitions. Its semantic fingerprint preserves graph IDs, cables, patterns
and references; the Lua file requires that base and is not a standalone patch
language. A missing or changed base, duplicate/unknown parameter, unsupported
type/range/version or extra field rejects reload. Changed topology uses the version-2 graph snapshot documented in
[PATCH_EDITOR.md](PATCH_EDITOR.md). No lossy topology translation or arbitrary
script rewriting occurs. Serialization uses round-trip `f64`
numbers, escaped strings and stable ordering; identical source exports identical
text, while filenames remain unique. Source size and sandbox budgets are bounded.

Reload the checked-in definition independently, without session replay:

```sh
cargo run -- validate-song songs/examples/25-edit-existing-synth --instrument edited-instrument.lua
cargo run -- render-instrument songs/examples/25-edit-existing-synth edited-instrument.lua --output /tmp/edited-bass.wav
cargo run -- tui-song songs/examples/25-edit-existing-synth --instrument edited-instrument.lua --no-midi-controls
```

Substitute the saved `authored/...lua` relative path for your own edit. Reload
passes through the ordinary song loader, typed edit validation and existing song
compiler. The original TOML files remain the default entry point.

## Verification and limits

`cargo test -p meldritch-control --test instrument_authoring` executes example
25's keyboard/Lua edits, every supported parameter, rejection cases, publication,
callback consumption, atomic saves and independent semantic/audio round trips.
The workspace compatibility suite also loads historical TOML and Lua examples.
Headless rendered output and callback consumption are verified; physical APC
interaction, DAC output and listening acceptance remain pending physical testing.
