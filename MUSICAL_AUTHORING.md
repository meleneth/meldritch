# Create, save and return to a musical idea

> **Meldritch 1 archive:** This document records completed research and its
> historical plans. Meldritch 2 is the active project; this is not
> its current architecture, acceptance queue, or compatibility contract.

This is the implemented bounded workflow over an existing song base. The next
[project construction slice](ROADMAP.md) and
[example 34 contract](songs/examples/34-empty-project-patch/ACCEPTANCE.md) remove
that entry barrier. They are design only; current project v1 exports retain
their base-bound meaning and are not standalone directory projects.

Example 29 composes existing instrument, patch, piano-roll and assignment editors.
Automated acceptance uses actual keyboard mapping, MIDI decoding, Lua gestures,
typed commands, worker publication and surface projections. Physical gesture and
listening acceptance remain pending.

```sh
cargo run -- tui-song songs/examples/29-lua-authored-roundtrip --lua main.lua
```

Ctrl-E opens the selected synth; Up/Down chooses a parameter, Left/Right edits.
Ctrl-G opens typed patch commands (see `compose.patch`); commit publishes them.
Escape returns to parameters. Ctrl-N displays authoritative notes.

APC pad 116 creates `roll`; the grid has eight pitches, C3 upward, across eight
eighth-note steps. Empty pads insert. Hold a start and press a later pad on the
same row to include that step. Rust stores an exclusive end; adjacent notes can
meet without overlap. Buttons 106/107 page eight steps; held gestures can cross
pages. Start taps toggle off on release; body taps select. Buttons 104/105 move
eight semitones. Hold 112 to delete, 113/114 to lower/raise velocity. Disconnect
cancels gestures. These positions and modifiers are Lua-authored policy.

Ctrl-M opens assignments/mixer. Up/Down selects a fader, Left/Right chooses a
declared target, Enter binds, Delete removes. Bind faders 1/2/3 to cutoff, volume
and pan. Move through the current value to acquire pickup; observation, value
and pickup appear in the pane. Reassignment rearms pickup. Pan uses stereo
balance: center sends mono equally to both channels, endpoints silence the
opposite channel. Stereo input has its opposite side attenuated without crossfeed.
See [control assignments](CONTROL_ASSIGNMENTS.md) for exact semantics.

Ctrl-E/N/M switch directly between editors. Escape returns to performance;
Ctrl-Tab toggles dense inspection. Transport continues through edits/navigation.
Live output starts muted for four seconds. q exits and finishes a separate journal.

**F6 saves composed music** (Ctrl-Shift-S is also accepted where the terminal
can distinguish it) to new `authored/project-*.lua` and reports
its path. Lua uses `{name="save_project"}` (APC 115 here). Ctrl-S retains scoped
exports and rejects unrelated edits. Saving never rewrites TOML, scripts or journals.

Restart with the reported relative path:

```sh
cargo run -- tui-song songs/examples/29-lua-authored-roundtrip --lua main.lua --authored authored/project-TIMESTAMP.lua
```

The version-1 `kind="project"` envelope contains existing instrument v1/v2,
pattern v1 and controls v1 representations tied to an unchanged base fingerprint.
The base directory remains necessary. This bounded export supports one selected
existing synth graph, one selected pattern and declared mixer/control values and
bindings. It rejects omitted edits elsewhere. Stable IDs, cables, notes, durations
and values restore; transport, held pads, observations and pickup reset. Loading
reuses validation/compiler boundaries and requires no session journal.

Execute into a new output directory:

```sh
cargo run -p meldritch-control --example authoring_acceptance --offline -- --output /tmp/meldritch-authoring-acceptance
cargo run -q --offline -- validate-song songs/examples/29-lua-authored-roundtrip --authored edited-project.lua
cargo run -q --offline -- render-authored songs/examples/29-lua-authored-roundtrip --authored edited-project.lua --output /tmp/meldritch-authored.wav
```

The runner exits and launches a fresh child, comparing authoritative JSON and
raw f64 samples exactly, including split renders. Artifacts include Lua, JSON,
PCM, TUI snapshots and clean journals. Production rendering needs no controller
script/hardware. This fixture uses deterministic oscillators; random Plaits models
retain their authorized RNG exception.

See [acceptance](https://github.com/meleneth/meldritch/blob/main/songs/examples/29-lua-authored-roundtrip/ACCEPTANCE.md),
[results](https://github.com/meleneth/meldritch/blob/main/songs/examples/29-lua-authored-roundtrip/RESULTS.md) and the
[physical checklist](https://github.com/meleneth/meldritch/blob/main/songs/examples/29-lua-authored-roundtrip/PHYSICAL_ACCEPTANCE.md).
Next: shorten assignment selection and display active pitch/time page plus modifier
legend together. Patch commands remain textual; Stream Deck transport is simulated.
Multiple edited instruments/patterns and general routing remain future examples.
