# Compose a typed patch

Native mono patches may declare `{ id = "velocity", signal = "control" }`.
The inspector identifies `input.velocity` as unit interval; keyboard F9 audition
supplies strike velocity / 127 independently of pitch and gate. To make amplitude
respond, explicitly multiply envelope/control output by velocity before VCA
level. Example 36's `velocity.project.lua` and atomic `velocity-patch.txt` show
that wiring. Any compatible unit-interval destination can consume this input;
bipolar or Hertz destinations require their existing explicit adapters. No cable
means no velocity effect. Fresh/offline independent-pattern instances initialize
this input to unity when no velocity pattern is bound. An explicitly bound
velocity pattern supplies its note-start values at authored f64 precision,
independently of pitch and gate. Ctrl-G can
now add `input.velocity` using `input add velocity control` in the same batch as
its consumer cables. This bounded editor also accepts `bend control` and
`sustain gate`; broader external source kinds remain future work.

Example 36's `bend.project.lua` also declares `{ id = "bend", signal = "control" }`.
The inspector labels this source bipolar. `bend-patch.txt` explicitly routes
input.pitch through PitchModulate and input.bend to its control port; pitch-depth
sets ±2 semitones. MIDI 8192 maps exactly to zero, 0 to -1 and 16383 to +1; lower
values divide the centered offset by 8192, upper values by 8191. Fresh/offline
independent-pattern instances initialize bend to zero. Keyboard motion changes
only this Control, so an unwired port cannot affect frequency or articulation.
This reuses the same audio-rate/per-instance adapter as LFO vibrato. Adding LFO
and bend together still requires an explicit supported combiner; implicit sum and
multiple pitch adapters remain unsupported. Plaits pitch-adapter lowering is open.

`sustain.project.lua` declares `input.sustain` as a separate Gate port.
Apply `sustain-patch.txt` in Ctrl-G after the velocity/bend drafts: the atomic
batch inserts `gate_hold` between `input.gate` and the ADSR and connects the
pedal to `hold.hold`. Raw key Gate and pedal Gate retain independent identities.
Pedal down delays a raw Gate low; pedal up releases it unless a key remains
pressed. A new key high retriggers. Fresh/offline instances start pedal-up,
and saves retain wiring rather than the held pedal state. This bounded lowering
supports one mono oscillator GateHold feeding ADSR; Plaits and general Gate
fan-out reject at executable edit validation. The port declaration can be staged
with `input add sustain gate`; an unwired pedal cannot change sound.

`ports.project.lua` starts with only pitch/gate. `input-ports.txt` shows all
three expressive ports and their modules/cables accepted in one Ctrl-G commit.
`input remove ID reject|disconnect` either rejects a dependent cable or removes
it explicitly; the complete candidate must still compile. Base pitch/gate
inputs cannot be removed in this mono slice. The save contains only accepted
declarations and wiring, never an incomplete draft or runtime expression value.

This is the accepted bounded construction editor. Project transactions, authored
normalization, patch-owned interfaces and quantized stateful activation extend
this boundary; see [ARCHITECTURE.md](ARCHITECTURE.md). Continued playback currently
uses PCM replacement, not DSP-state migration. Single-cable/fixed-topology rules
are current implementation limits, not permanent musical constraints.

Start `tui-song songs/examples/30-compose-typed-patch --no-midi-controls`.
Press Ctrl-E, then Ctrl-G. Type a command and Enter to stage it:

| Command | Behavior |
| --- | --- |
| `add ID TYPE` | Instantiate oscillator, plaits, adsr, vca, low_pass, audio_output, lfo, control_scale, control_multiply, pitch_modulate, control_hertz, control_attenuate or gate_hold with inspectable defaults |
| `input add ID SIGNAL` | Declare `velocity control`, `bend control` or `sustain gate` on a native mono patch |
| `input remove ID reject` / `input remove ID disconnect` | Remove an expressive input, rejecting dependent cables or explicitly disconnecting them |
| `remove ID reject` | Remove only when no dependent cable remains |
| `remove ID disconnect` | Explicitly remove the module and all its dependent cables |
| `connect MODULE.PORT MODULE.PORT` | Connect one compatible output to an unoccupied input |
| `disconnect FROM TO` | Remove exactly that existing cable |
| `set MODULE PARAM VALUE` | Use the ordinary typed instrument parameter validator |
| `lfo ID SHAPE beats/hertz VALUE phase PHASE reset none/gate_rise` | Configure sine/triangle/saw/square, cycle beats or Hertz, phase and explicit reset; audio/per-instance scope |
| `scale ID GAIN OFFSET MIN MAX` | Configure explicit bipolar → unit-interval affine scaling and clamping |
| `pitch-depth ID SEMITONES` | Configure pitch_modulate depth in -24..24; output Pitch = input Pitch + bipolar Control × depth, audio/per-instance |
| `hertz ID GAIN OFFSET MIN MAX` | Configure explicit unit-interval → Hertz affine scaling and authored clamping, audio/per-instance |
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
Every control node must reach VCA level, oscillator pitch or filter cutoff,
including dependencies through an LFO rate input;
units/rate validate and multiple cables still require an explicit combiner.
Inspect settings and typed ports in the scrolling graph pane. A gate-rise-reset
LFO requires input.gate → ID.reset; reset-none rejects that cable. Unsupported
Plaits/chain pitch lowering, polyphonic control lowering, event cadence and globals
remain future work; the fixed audio chain remains the current executable subset.

ControlHertz adds one explicit unit-interval → Hertz adapter feeding low_pass.cutoff.
That cable replaces the configured cutoff default; the inspector labels the default
inactive, and cutoff overrides reject rather than bypassing the cable. Filter
resonance stays configured/overridable. The existing DSP's physical frequency bound
still applies after the authored adapter clamp. Configurable normalization is open.
ControlAttenuate takes bipolar Control at control and unit-interval Control at
amount, emitting their bipolar product. Ordinary add/connect drafts construct
envelope-controlled depth; no settings or hidden destination routing are required.
Lfo.rate is an optional Hertz input. Connect an explicit ControlHertz adapter with
declared clamps within 0.001..100 Hz; rate-patch.txt in example 36 demonstrates a
second LFO controlling the first one's speed. General Hertz clamps allow
0.001..20000 Hz, with unchanged 20..20000 defaults. The inspector labels cabled
rate as integrated phase and configured rate as inactive. Rate dependencies take
part in cycle validation; no implicit feedback delay or reset is introduced.

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
