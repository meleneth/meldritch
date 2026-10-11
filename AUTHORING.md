# Interactive musical authoring investigation

## Current direction (2026-10-11)

The original investigation below is historical. Examples 25–33 now have headless
acceptance, including example 29's bounded composed persistence. Their guides
and contracts specify actual limits. Next work is project-root construction and
self-contained save/reopen in [ROADMAP.md](ROADMAP.md),
[ARCHITECTURE.md](ARCHITECTURE.md) and TODO section 21. Git is permitted as a
required installed dependency; a project need not be a Git repository.

Example 34's first checkpoint implements read-only empty-directory startup and
legacy entrypoint adaptation, with no created files/devices. Next: project-owned
editable source, patch/pattern creation, independent bindings and standalone saves.
The full example retains `design` status.

## Recommended immediate implementation prompt

Implement only [example 34](songs/examples/34-empty-project-patch/ACCEPTANCE.md):
empty directory → construct patch → create pattern → independent pitch/gate →
play → modify → save → fresh-process reopen, with native keyboard controls.
First add its smallest readable authored/invalid fixtures and failing acceptance
bindings. Choose additive save syntax there; invent no speculative schema.

Reuse the existing Rust definition/importer/fingerprint machinery, moving source
ownership out of the synth-only `InstrumentEditor` only enough to represent an
empty project and selected composition. Keep `load_song_directory` and historical
commands unchanged. Add no-subcommand project opening; do not copy a seed or
initialize Git. Compose the supported mono oscillator/ADSR/VCA/output chain
through `PatchEdit`, and route native note editing to existing `NoteEdit`.
Expose independent pattern pitch/gate and explicit composition bindings consumed
by existing note lowering, including the contract's disconnected-gate low policy.
Do not implement another event engine.

Distinguish construction edits from a single exposed runtime level control;
save authored defaults only. Reuse `SongAudioHost`, worker, generation telemetry,
immutable publication, pickup and host-side serialization. Extend serializers/
importer with a self-contained versioned subset, preserving base-bound project
v1 and instrument v1/v2. Reject lossy saves and unsupported graphs. Prove fresh
reopen without seed, hardware, controller script or journal. Do not claim DSP
continuity from PCM publication.

Reviewable checkpoints within the one accepted slice: empty/source ownership
and CLI; create patch/pattern/native typed inputs; bindings/play/runtime overlay;
save/fresh reload/compatibility. Partial checkpoints do not accept the whole
workflow. Run focused tests then affected full crate suites/docs checks. Update
statuses only to behavior actually proved; commit verified checkpoints only when
authorized. Defer general DAGs, LFOs, normalization, libraries, polyphonic signal
routing, quantized graph activation and full capture harvesting.

This comes first because example 29 proves editing/save over an existing seed,
while new-directory users cannot start that workflow. Each addition closes part
of the same usable loop; isolated infrastructure leaves the entry barrier intact.

## Historical authoring investigation (2026-10-10)

Implementation update: [typed patch composition](PATCH_EDITOR.md) and the
[pinned Plaits model catalog](PLAITS.md) now supersede the earlier topology/Plaits gaps.
Example 30 reuses the existing source graph/importer/compiler and exports changed
graphs as version-2 Lua instrument snapshots; example 32 verifies FM/harmonic
engines with authorized native f32/RNG exceptions and 48 kHz preference.
Example 33 extends this to all 24 bundled model choices and explicit source gate
connections, with headless keyboard/Lua/publication/persistence acceptance.

Implementation update: [example 28 / editable faders and mixer](CONTROL_ASSIGNMENTS.md)
now implements source-owned cutoff/resonance, single-track delay mix/feedback,
track volume/stereo balance, interactive Lua/TUI assignments, pickup and control
snapshot persistence. The historical inventory below records the earlier
investigation; its pan/mapping gaps are superseded for this supported subset.
General routing and catalog/device editing remain unfinished. Bounded
[composed example 29 persistence](MUSICAL_AUTHORING.md) now passes fresh-process
headless source/audio equivalence. Physical fader/listening acceptance remains pending.

This inventory records the pre-implementation investigation. The subsequent
[instrument editor slice](INSTRUMENT_EDITOR.md) implements example 25 and the
instrument-only persistence foundation of example 29. Statements below about
missing parameter commands/inspection/export describe the original baseline;
the linked guide and capability matrix describe the current supported subset.

The subsequent [piano-roll slice](PIANO_ROLL.md) implements example 26/27 note
creation/assignment, duration/velocity/delete editing, Lua controller gestures
and independent pattern exports. Example 29 now composes a selected synth graph/pattern with declared mixer
values and bindings; multiple edited instruments/patterns remain unsupported.

The [drum editor](DRUM_EDITOR.md) adds explicit one-shot sample-slot authoring
through the same note source, commands and Lua pattern definition. Its generated
kit approximates a 909 sound palette; it is not a native 909 implementation.

This is a documentation/design slice, based on the repository inspected on
2026-10-10. Interactive authoring is the immediate priority. Section 20 of
[TODO.md](TODO.md) supersedes the older next-step recommendations without
closing their unfinished work. No new engine, Lua music API, editor framework,
or schema is implemented here.

## Implementation inventory

Evidence below names actual implementation symbols rather than completed
showcase milestones. Existing automated tests are evidence for their own
contracts, not for the new authoring contracts.

| Area | Exists | Limits for authoring |
| --- | --- | --- |
| Oscillation | `render/src/dsp.rs`: `Oscillator`, sine, triangle, polyBLEP saw/pulse, bounded `SyncFold` hard sync/phase modulation/wavefolding | `song_render::parse_waveform` accepts sine/triangle/saw/pulse/square, not SyncFold; no Lua waveform edit |
| Envelope and amplification | `AdsrEnvelope`, seconds-based attack/decay/release, sustain, note-on/off; `.mlsynth` ADSR and VCA | No loaded-song ADSR authoring command or Lua parameter inspector |
| Filters | Stateful `StateVariableFilter` with low/band/high-pass; `BassVoiceSettings` cutoff/resonance/filter-envelope depth | `.mlsynth` exposes low-pass only; live song controls override cutoff/resonance, not arbitrary voice fields |
| Voices | `BassVoice`: oscillator/sub, glide, envelopes, drive before/after filter, accent/duck/hat influences; fixed-capacity `PolyphonicSynth` with deterministic allocation/stealing and independent state | Rich showcase settings are not a modular `.mlsynth` definition. Song polyphony uses independent note voices rendered serially; do not equate its scheduling with the showcase bank |
| Modulation | `modulation.rs`: Hz/beat LFO, sine/triangle/saw/square/seeded sample-and-hold; routes to filter octaves, resonance, drive octaves, level; core stepped/linear automation | Rust/showcase facilities, not loadable LFO modules in `.mlsynth`; parameter patterns are parsed separately, not hidden cables |
| DSP | `effects.rs`: tag-based delay/reverb sends; `dynamics.rs`: role-aware envelope follower/band ducking; `stereo_fx.rs`, `mastering.rs`, `performance_fx.rs`: ping-pong delay, phaser, stereo/modulated reverb/freeze, compressor/clipper/limiter and macro rack; `transforms.rs`: reverse/reslice/freeze/smear | These implementations do not imply corresponding typed modules or Lua graph-edit commands. `.mldsp` executable examples use tempo delay |
| Samples | `.mlsamples` slots/paths/modes/level/pitch/slices; `.mlsampler` pitch tracking, level, polyphony and pitch envelope; mixed renderer one-shot WAV/slice playback | Loop mode and live sampler pitch/slice edits remain open; sampler is not a `ModuleKind` patch primitive |
| Source graph | `dsl/src/song.rs`: `SynthDefinition`, `DspDefinition`, `ModuleDefinition`, `CableDefinition`, stable definition IDs, semantic fingerprints and confined references | Definitions are loaded snapshots with private fields/getters, not an interactive mutable song document |
| Patch types | `ModuleKind`: Oscillator, Adsr, Vca, LowPass, TempoDelay, AudioOutput. `SignalType`: Audio, Pitch, Gate, Control | Broader Event/Feature/Trigger/rate-adapter vocabulary in EURORACK_PRINCIPLES is doctrine/planned support, not implemented song port types |
| Ports | `resolve_port`: oscillator pitch-in/audio-out; ADSR gate-in/control-out; VCA audio-in/level-in/audio-out; low-pass and delay audio-in/out; output audio-in; declared external `input.*` sources | Filter and delay parameter targets are separately validated; they are not arbitrary signal cable inputs. Cable endpoints and many IDs remain validated strings, not universally typed newtypes |
| Compilation | `song_render.rs`: drone, note, filtered note, delayed note, automated feedback, mixed synth/sample compilers; required cables/type compatibility; deterministic pre-roll/split-range tests | Pattern-specific lowering uses `exactly_one_module`, `optional_one_module`, required topology. Validation of a patch is not proof arbitrary graph execution exists. No general DAG lowering, automatic adapters or recipe compiler |
| Relations | `core::Command`: AddSource/AddNode/SetRelation/ClearRelation/MarkDirty; typed audio/control relations, node properties, downstream dirty propagation | These are dependency operations, not song module/cable authoring commands. Do not build an unrelated second patch model from them |

Paths in the table are under `crates/meldritch-*`. Relevant test boundaries are
DSL `tests/ml_examples.rs`, render `tests/song_examples.rs` and `tests/song_host.rs`,
app/TUI unit tests, and control `tests/acceptance.rs`, `tests/compatibility.rs`,
`tests/live_performance.rs`.

### Patterns, commands and input

`core::Step` stores a MIDI-number `u8` note, `f64` velocity/gate, probability
and event tags. `Pattern` arranges steps in track/note lanes; chord tones are
independent lanes. `events_between` rounds gate × step length to frames and
clips it at the next step boundary. Increasing this gate does not create a
multi-step sustained note.

`LiveEditCommand` has SetStep/ClearStep/ToggleStep. `LivePatternEditor::apply`
clones the backing pattern, computes old/new dirty ranges and updates the
coordinator. `AppCommand::Edit`, selection, selected note/velocity/gate/probability
adjustments, chord transposition/inversion and `SetBassVoice` serve the existing
showcase TUI. They are useful bounded command precedents, not a loaded-song editor.

Authored `.mlpattern` notes instead use `NoteEventDefinition`: start/duration
`u64` frames, MIDI-number note, `f64` velocity and optional sample slot/slice/root/
pitch offset. Loader input uses note names, bar:beat:tick positions at 960 PPQ,
fractional note duration, pattern length and looping. It requires positive
duration, sorted starts and end within the pattern. Equal starts support chords.
Compilation gates the synth from start to start + duration; ADSR release may
continue after gate-off. Sample one-shot playback has separate envelope/playback
semantics; it is not the first duration-editing acceptance target.

The critical disconnect: `SongAudioHost::accepted` reacts to lane variation,
bank, mute/solo, octave and supported curated controls. It does not react to
`AppCommandResult::Edit` by updating its `ValidatedSong`. Editing the backing
TUI pattern therefore does not establish editing or persistence of loaded song
notes. `tui-song` audio uses a dedicated song publication rather than that
backing coordinator's audio.

MIDI uses `midir`, script device/channel/note/CC bindings and typed app inputs.
`devices::decode_midi` preserves note velocity/releases (including note-on-zero),
CC and pitch bend; APC Session codecs cover grid notes 0..63, buttons and faders
CC 48..56. The production 64-observation FIFO maps on the control thread with
pickup, page/layer routing and reconnect leases. Overflow is reported. Lua-owned
grid cells are exclusive: the CLI consumes releases but invokes
`LuaPerformance::press` only on press. `Input::Button` has no held-note lifecycle.
Held-pad editing must extend this path, clear holds on disconnect/overflow/page
changes and prevent duplicate TOML dispatch. MicroLab decoding is not live voice
allocation. Existing APC physical smoke evidence in example 19 does not cover
new note authoring or held gestures.

### Lua and publication

`control::CommandRequest` exposes exactly set_mode, play, stop, rewind,
select_page, select_lane_variation, toggle_lane_mute, toggle_lane_solo,
set_control (normalized 0..1). It does not expose SetStep, SetBassVoice, pattern
bank selection, octave/modifier commands, synth/module/cable creation or mapping
edits. Its readonly projection includes mode/frame/page/playing, curated control
values, lane mute/solo/audible/variation and audio-generation telemetry; not note
events, synth topology, envelopes or all parameters.

`LuaProject::save/restore` stores exact Lua source/version/project identity and
song/control fingerprints in a TOML envelope, recompiling a fresh VM. It saves
definitions, not current musical edits or HSM state. Journals record typed
accepted requests with `u64` frames; replay requires equivalent initial app
state. Production capture still includes legacy debug-string commands/`u32`
frame limitations. Neither is a human-readable Lua music exporter.

`render::song_host::SongRerenderWorker` has one pending request and one completion
slot, latest-request replacement, full-history rerender and no in-flight cancel.
`SongAudioHost` compiles from a loaded song plus app lane state/derived overrides.
`AudioSnapshotPublisher` atomically swaps immutable `PublishedAudio`; generations
travel with samples. Callback acknowledgement and native/Lua audio projections
distinguish requested/published/consumed. Reuse this boundary; publication does
not prove human listening, DAC timing or quantized launching.

### Mixer

`lane:<id>/level` is a validated curated target with live `f64` level overrides
for synth/sample lanes. Fader curves provide unity/overdrive; mute/solo affect
inclusion. The mixed renderer serially sums tracks with `1/sqrt(included track
count)` gain, so changing inclusion changes normalization as well as content.
Instrument/sampler level and showcase master drive are distinct from lane gain.

There is no per-track pan command, Lua pan request, `.mlperformance` pan target
or generic editable mixer/bus/send topology. Showcase per-voice stereo spread
(`render_polyphonic_pattern_with_stereo_spread`) is not track pan. Existing
EffectBus Delay/Reverb/tag sends, sidechain relation edges and master processors
are real Rust facilities. Song track DSP assignments and explicit DSP cables
exist, but mixed playback is not a general editable bus router. Initial mixer
authoring accepts lane level; pan/bus authoring needs its own later example.

## Plaits dependency investigation

This section records the earlier inventory. Current integration supersedes the
old defer decision below: [PLAITS.md](PLAITS.md) documents the pinned crate,
verified 24-model catalog and authorized f32/RNG exceptions. Custom resources,
Voice/LPG emulation, CPU benchmarks and physical listening remain unimplemented
or unverified.

All repository files/manifests, Cargo.lock and `cargo tree --locked --offline`
were inspected: no Plaits dependency, vendored implementation or Rust clone is
present. SyncFold and the polyphonic voice bank are MELDRITCH implementations,
not Plaits.

Candidate inspected: sourcebox `mi-plaits-dsp-rs`, upstream master resolved to
[`effe9864bb67d23aaf5724722ac1ce165d4e7c9f`](https://github.com/sourcebox/mi-plaits-dsp-rs/commit/effe9864bb67d23aaf5724722ac1ce165d4e7c9f).
This is a source audit, not a compiled/benchmarked or production acceptance.

- The [README](https://github.com/sourcebox/mi-plaits-dsp-rs/blob/effe9864bb67d23aaf5724722ac1ce165d4e7c9f/README.md)
  describes a firmware-1.2 port, independent building blocks, `no_std`, MIT
  licensing and WAV-generating tests. It warns of audible differences away
  from the original 48 kHz rate. These are upstream statements, not MELDRITCH
  verification.
- [Cargo.toml](https://github.com/sourcebox/mi-plaits-dsp-rs/blob/effe9864bb67d23aaf5724722ac1ce165d4e7c9f/Cargo.toml)
  identifies package `mi-plaits-dsp` 0.1.0/edition 2024, with dyn-clone,
  num-traits/libm and spin dependencies. `lib.rs` uses `alloc`; `no_std` is not
  evidence of allocation-free processing.
- [voice.rs](https://github.com/sourcebox/mi-plaits-dsp-rs/blob/effe9864bb67d23aaf5724722ac1ce165d4e7c9f/src/voice.rs)
  provides a stateful 24-engine Voice, Patch/Modulations, resources, constructor
  block size/sample rate, initialization and `f32` out/aux block rendering.
  The wrapper combines engine dispatch, trigger conditioning, envelopes,
  low-pass gate and postprocessing; it is not a drop-in modular `f64` oscillator.

Precise future boundary: a worker-side DSP adapter in `meldritch-render`, reached
through the existing song compiler after DSL module/port/target validation.
MELDRITCH must own IDs, note/voice scheduling, explicit cables, invalidation,
fingerprints and publication. Lua/TUI/hardware only request typed commands.
Prefer an individually exposed upstream engine with explicit external envelope/
VCA routing. The combined Voice cannot silently replace a graph or hide its LPG/
modulation; any recipe must expose expansion. There is no existing generic
Instrument trait implementation slot in the song compiler to plug it into.

Before adopting: pin/audit the chosen engine and transitive licenses/resources;
resolve the `f32` internal arithmetic conflict with the project's `f64` default
(conversion alone does not provide `f64` internals); audit allocation/lazy init,
spin/global random state, block constraints and engine switching; test finite
extreme inputs, deterministic seeded reset and independent voices, trigger/gate
behavior, contiguous/split-range equivalence, sample-rate policy, memory/CPU and
offline throughput separately from callback deadlines. Upstream WAV tests alone
cannot establish these. Decision: defer integration until existing-module
authoring works; do not add the dependency in this slice.

## Lua gaps by authored concept

Lua v1 can reference an entire validated TOML song. That represents the song
indirectly; it cannot declare or export these concepts as Lua source truth.

| TOML concept | Lua inspect/edit gap |
| --- | --- |
| Synth identity/polyphony/modules/defaults, patch inputs/cables, output channels | No topology/voice inspection or source edit/create/delete |
| DSP definitions and track DSP assignment | No graph inspection/routing edit; only curated supported delay values |
| Notes, length/resolution/loop, pitch, velocity, duration; sample slot/slice/root | No event inspection, insertion, duration edit, pattern creation/assignment |
| Parameter patterns, target IDs, points/interpolation | No automation inspection/edit/export; set_control is a live override |
| Sample bank/assets/slices/modes and sampler/pitch envelope | No bank/voice authoring or inspector |
| Track/lane roles, patterns, banks, launch quantization and defaults | Limited live lane projection/variation/mute/solo; declarations/banks uneditable |
| Curated targets/ranges/steps/curves, physical bindings/devices/pages/modifiers | Lua declares presentation bindings; TOML musical mappings remain preauthored and uneditable |
| UI section order/key hints, tempo/sample rate, entrypoint/references | Lua presentation pages exist; underlying song declarations are not edited |
| Sessions/provenance | Journals and source envelope exist; no Lua authored-state snapshot or complete migration/replay |

Do not promise lossless Lua export of all TOML constructs in the first slice.
Import only the example-required subset with explicit diagnostics for unsupported
semantic content, or retain a confined immutable base reference and export every
accepted edit needed to reconstruct it. Reject lossy export. The first persistence
example chooses the latter bounded route, with fresh-state deterministic edits;
it is not a replay of toggle commands or raw input history. A later self-contained
Lua document can replace that base only with an example proving equivalence.

## Ownership and dependency order

The target is one Rust-owned editable source/relation model at the app command
boundary, initialized by either TOML import or Lua authoring. Extend the current
song model and validation rather than duplicating its semantics in Lua tables or
the showcase pattern. `SongAudioHost` should consume immutable revisions of that
model and keep derived compiled/override caches only. Runtime transport, queued
launches and playback telemetry stay distinct from authored defaults.

Lua declares policy and translates requests; readonly projections drive TUI and
hardware feedback. Hardware owns observations/pickup/connectivity; held-pad
selection is transient control interaction state, never a second pattern store.
Rust validates each edit atomically, records acceptance, revises/fingerprints,
computes audio/control dirty ranges with tails, schedules workers and publishes.
Save is a host filesystem operation outside the VM/callback. Musical-time source
values must survive tempo changes; compiled `u64` frames are derived. Persist
stable IDs and ordered definitions/edits, not derived samples or volatile holds.

Implement these narrow slices in dependency order:

1. Example 29 foundation: fresh import + deterministic human-readable Lua edit
   export/reload for one existing low-pass parameter. Versioned explicit opt-in
   loader, confined base identity/fingerprint, stable IDs, validated absolute
   edits, atomic save and unsupported-content diagnostics. Keep historical TOML
   entrypoint precedence. Define only the minimum syntax in that implementation
   slice; this investigation invents no API fields.
2. Example 25: source-owned low-pass/ADSR edits through TUI/typed commands;
   loaded song rerenders and shared inspections; extend Lua export to those
   parameters. Prove source and published state separately.
3. Example 26: create a one-bar note pattern, pitch/step grid and assignment;
   APC press/release adapter and TUI equivalent; export note musical positions.
4. Example 27: held-pad endpoint editing on that event model. Explicit duration
   rather than legacy gate clipping; lost-release cleanup and musical/control
   clock separation, including rewind/loop tests required by the interaction.
5. Example 28: edit physical mappings for existing cutoff/lane level, shared
   inspector and pickup rearming; extend export to bindings and authored level.
6. Example 30: create the already executable note/ADSR/VCA/low-pass graph using
   existing types; transactional topology validation, visible cables and export.
   Reject unsupported executable topologies rather than presenting silent success.
7. Compose example 29 round-trip across all six capabilities, then perform and
   record physical APC pad/hold/fader/listening acceptance. Only then expand
   engine variety, polyphonic editing, automation, pan or bus routing with examples.

Each slice starts with failing acceptance bindings, then implements only its
required commands, projections and persistence, runs affected crate/compatibility
tests and records a focused verified commit. Clock/capture fixes required by an
authoring test belong in that slice; full performance replay, HID, generalized
scheduling and the remaining ensemble backlog are deferred, not erased.

## Acceptance execution contract

Examples 25–30 contain Gherkin `.feature` specifications with concrete inputs,
observable assertions and invalid neighbors. At the original investigation
checkpoint they had no passing bindings. Subsequently Rust integration suites
accepted their headless contracts; there is no separate Gherkin runner.
Baselines refer to immutable
existing fixtures; feature tables are interaction/expected-source examples, not
proposed schema fields. This distinction must remain visible in test reports.

Bind Given to the real loader/controller/host in a temporary song root, When to
actual TUI inputs or decoded MIDI packets and typed dispatch, and Then to source
inspection, diagnostics, journals, completed worker publication and deterministic
render comparisons. Use the production callback renderer for consumed-generation
assertions. Never simulate success by changing a widget-local table. Missing step
bindings must fail/report undefined, never pass or be counted as accepted.

All cases inherit: rejected edits leave source fingerprint, accepted journal and
publication unchanged; accepted changed edits record exactly once; save/reload
into a fresh process reconstructs source semantics/IDs without hardware or session
replay. Unchanged source exports deterministically. Save uses temp-write/sync/
atomic replace, preserves previous good save on failure, and leaves failed edits
dirty for retry. Scripts cannot perform filesystem IO. Changed bases are diagnosed
before applying edits. UI/transport/holds are not serialized as authored notes.

Verification available now (existing behavior only):

```sh
cargo test -p meldritch-dsl --test ml_examples --offline
cargo test -p meldritch-render --test song_examples --offline
cargo test -p meldritch-control --test compatibility --offline
```

For each new feature implement named tests corresponding to every Scenario and
Scenario Outline row at the smallest affected boundary, then the relevant full
crate suites in AGENTS.md. The `.feature` files deliberately have no fictional
CLI invocation for unimplemented authoring. Physical scenarios are a separate
manual protocol and must retain pending status until tested on a real unit.

Documentation-slice verification: the existing DSL example suite (11 tests),
song render suite (22 tests) and historical control compatibility suite (2 tests)
passed offline. The VitePress build and `git diff --check` passed. These 35 tests
validate retained baselines only; none accepts examples 25–30 or new hardware
interaction. No upstream Plaits build, benchmark or hardware test was performed.
