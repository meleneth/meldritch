# Compose a typed patch

This is the accepted bounded construction editor. Project transactions, authored
normalization, patch-owned interfaces and quantized stateful activation extend
this boundary; see [ARCHITECTURE.md](ARCHITECTURE.md). Continued playback currently
uses PCM replacement, not DSP-state migration. Single-cable/fixed-topology rules
are current implementation limits, not permanent musical constraints.

Start `tui-song songs/examples/30-compose-typed-patch --no-midi-controls`.
Press Ctrl-E, then Ctrl-G. Type a command and Enter to stage it:

| Command | Behavior |
| --- | --- |
| `add ID TYPE` | Instantiate oscillator, plaits, adsr, vca, low_pass, audio_output, lfo, control_scale, control_multiply or pitch_modulate with inspectable defaults |
| `remove ID reject` | Remove only when no dependent cable remains |
| `remove ID disconnect` | Explicitly remove the module and all its dependent cables |
| `connect MODULE.PORT MODULE.PORT` | Connect one compatible output to an unoccupied input |
| `disconnect FROM TO` | Remove exactly that existing cable |
| `set MODULE PARAM VALUE` | Use the ordinary typed instrument parameter validator |
| `lfo ID SHAPE beats/hertz VALUE phase PHASE reset none/gate_rise` | Configure sine/triangle/saw/square, cycle beats or Hertz, phase and explicit reset; audio/per-instance scope |
| `scale ID GAIN OFFSET MIN MAX` | Configure explicit bipolar → unit-interval affine scaling and clamping |
| `pitch-depth ID SEMITONES` | Configure pitch_modulate depth in -24..24; output Pitch = input Pitch + bipolar Control × depth, audio/per-instance |
| `commit` | Validate complete graph and compile before replacing source/publishing |
| `clear` | Discard staged commands |
| `save` / Ctrl-S | Save the committed selected instrument, never an incomplete draft |

PgUp/PgDn scroll the inspector; the input prompt remains visible at 80×24.
Esc returns to parameter editing. Tab there selects an existing instrument and
clears its draft. Start transport before opening the command pane; playback
continues during staging and publication. Lua/controller edits while a draft is
open cause a stale-base rejection: clear and restage instead of committing
against a changed source.

[The keyboard script](https://github.com/meleneth/meldritch/blob/main/songs/examples/30-compose-typed-patch/keyboard-script.txt)
removes the old modules with explicit cable disconnection, then constructs a
source/ADSR/VCA/filter/output patch from defaults and typed commands. The existing
synth identity, external pitch/gate inputs, polyphony, output layout, track and
pattern reference are retained. This slice edits an existing synth graph; new
song/track identities, arbitrary graphs and DSP graph authoring are future work.
Pitch → oscillator/Plaits, gate → ADSR, source → VCA, ADSR → VCA level, VCA →
optional filter → output are the executable topology. Plaits can use audio or
aux. There are no implicit adapters, hidden routes or alternate graph compiler.
Example 36 extends the amplitude control path with a bounded control DAG:
LFO → scale and ADSR → multiplier → VCA, plus explicit PitchModulate vibrato.
Every control node must reach VCA level or the oscillator pitch adapter;
units/rate validate and multiple cables still require an explicit combiner.
Inspect settings and typed ports in the scrolling graph pane. A gate-rise-reset
LFO requires input.gate → ID.reset; reset-none rejects that cable. Unsupported
filter modulation, Plaits/chain pitch lowering, polyphonic control lowering, event cadence and globals
remain future work; the fixed audio chain remains the current executable subset.

The inspector lists module IDs/types, typed input/output endpoints, authored
values/ranges, explicit cables and draft validation/compilation diagnostics.
Rust owns `PatchEdit { synth, operations }`; Lua returns `name="edit_patch"`
with that typed batch. See the example's `main.lua` for the complete declaration.
`view.instrument.graph` and `ports` are recursively read-only projections of
the same Rust model. Accepted batches appear as typed journal facts and source
revisions; transient command drafts have no musical authority.

Validation rejects duplicate IDs, unknown ports, incompatible signals, multiple
cables to one input, dangling endpoints, cycles (including control edges),
unsupported configurations/topologies, invalid parameter values, automation
ownership violations and broken existing parameter/assignment references.
Supported synths have at most 32 modules/128 cables; batches have at most 128
operations. Rejection keeps source, accepted musical history and published audio.
The render worker invalidates the song horizon, including release/filter tails;
app history identifies affected tracks. Rapid accepted edits use latest-wins
publication. Staged commands never reach the audio callback.

A graph that parameter-only replay cannot reproduce exactly exports `kind="instrument", version=2` through the existing
Ctrl-S/`save_instrument` and `--instrument` load boundary. It contains the
immutable base fingerprint, selected instrument and the full existing typed
`SynthDefinition` as `graph`: inputs, module IDs/kinds/settings, typed cables,
polyphony and synth ID. It is a snapshot, not a second patch language.
Fields are deterministically sorted; module/cable order and all IDs remain
exact; numbers round-trip. Fresh load reconstructs the same graph, fingerprints
and samples through the ordinary semantic importer and compiler.
Standalone exports reject unrelated authored changes. [Example 29](MUSICAL_AUTHORING.md)
now combines the selected graph/pattern, declared mixer values and bindings
through F6; other edited instruments/patterns remain unsupported.

Exactly representable parameter-only instrument exports remain version 1. Save creates a unique
`authored/instrument-*.lua` atomically and does not overwrite TOML, scripts,
earlier exports or performance journals. A changed/missing base, unknown fields,
version, forged cable types or unsupported graph rejects reload.

Compatibility: historical TOML files and version-1 Lua files stay loadable with
unchanged source fingerprints. The note renderer now actually processes VCA
before the filter as its required cables declare; this corrects historical
filtered-note samples/release tails. The separate automated filter-before-VCA
compiler retains its declared order. Headless tests prove these boundaries;
physical controller interaction and listening remain pending.
