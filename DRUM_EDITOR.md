# APC drum editor

```sh
cargo run -- tui-song songs/examples/31-apc-drum-editor --lua main.lua
```

Playback starts with an empty pattern. Press NEW (note 116) to create `roll`, or
compose directly into the seed. Each column is a sixteenth note; 106/107 navigate
between two eight-step pages in the one-bar loop. Rows from bottom to top are
kick, snare, clap, closed hat, open hat, low tom, high tom and ride.

Press an empty pad to insert a hit at velocity 0.8; press it again to remove
that hit. Releases never change drum notes, and duplicate note-ons while held
are ignored. Different held rows create simultaneous hits. Hold 113/114 and
press a hit to lower/raise velocity by 0.1. Hold 112 for delete-only operation.
The drum policy has no pitch-page or held-duration mode.

Hits are green, selected hits yellow, and the playing column white/gray. The
TUI's Ctrl-N inspector reads the same pattern and includes slot names. 115 or
Ctrl-S saves a unique `authored/pattern-*.lua`. Ctrl-E opens the historical synth
reference required by TOML, which does not supply the sampler track's sound.
Saving preserves source definitions and performance journals.

The kit is **909-inspired**, generated from original inspectable oscillator/
noise formulas. There is no TR-909 engine in MELDRITCH. These are not Roland
recordings or a faithful circuit model. Formulas and provenance live under
`songs/examples/31-apc-drum-editor/samples/` in `generate.py` and `README.md`.
All rows use existing one-shot playback; hits layer and tails continue
independently of note duration. Hat choke groups and live drum synthesis
parameter editing remain unimplemented.

Lua owns the row-to-slot map and gestures. Rust's additive `insert_sample` note
command requires an existing explicit one-shot slot on a sampler track. It
validates existing time/pitch/velocity boundaries before compilation and source
acceptance. There is one note model and the existing audio renderer. The worker
publishes the latest whole-song snapshot while transport continues. Positive
exclusive note ends remain authoritative; one-shot tails are not truncated.

Version-1 Lua pattern definitions add an optional `slot` per note; ordinary synth
exports remain byte-compatible. Synth insertion rejects slots, sampler insertion
requires them, and unknown/loop slots or slice/root/transpose authoring reject.
Stable IDs/counters, velocities, musical ticks, slot references and assignment
survive reload. WAV files stay in the referenced bank, outside the Lua definition.
Replacing a WAV changes sound under the existing sampler behavior, even if the
bank definition is unchanged. Composed exports remain outside this slice.

```sh
cargo run -- validate-song songs/examples/31-apc-drum-editor --pattern edited-pattern.lua
cargo run -- render-pattern songs/examples/31-apc-drum-editor edited-pattern.lua --output /tmp/drum-editor.wav
cargo run -- tui-song songs/examples/31-apc-drum-editor --lua main.lua --pattern edited-pattern.lua
cargo test -p meldritch-control --test drum_roll
cargo run -p meldritch-control --example drum_roll
```

Substitute your saved relative path for `edited-pattern.lua`. The headless script
verifies creation, toggling, velocity, paging, publication and independent reload
through actual control paths. Tests prove distinct finite non-silent row audio,
exact layering, rejection atomicity and shared TUI/APC projections. Physical
orientation, controls, LEDs, gestures and listening remain untested.
