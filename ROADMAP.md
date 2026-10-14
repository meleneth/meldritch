# Roadmap

## Next architecture: planned vertical slices (2026-10-11)

This sequence supersedes the original phases below as the active direction.
Slice 1 now has **bounded headless acceptance**. Later slices remain
**proposed/unimplemented**. Empty/legacy opening, native patch/pattern authoring,
independent mono bindings, playback, instance runtime level, typed capture and
self-contained save/fresh reopen compose through existing Rust machinery.
Historical examples 25–33 remain
accepted within their scopes. Requirements live in [FEATURES.md](FEATURES.md),
the code/reuse audit in [ARCHITECTURE.md](ARCHITECTURE.md), and the active queue
in [TODO.md](TODO.md). Each slice starts with a readable scenario and invalid
neighbors, then binds acceptance to actual app/compiler/host boundaries.

Example 34 has opening/creation/binding requests, invalid neighbors and scoped tests.
Its composed construction/persistence contract is accepted headlessly. Example 35
also accepts the bounded native shared-palette/two-composition workflow headlessly.
Example 36 now has its first executable tremolo checkpoint. Names 37–41 below are
reserved planning names, not existing files, schema or capability claims.
No slice requires rewriting working examples or introducing another engine.

### 1. Empty project to a saved playable patch (headless accepted)

**Scenario/example:** [34-empty-project-patch](songs/examples/34-empty-project-patch/ACCEPTANCE.md).
In a new directory run `meldritch`, enter construction, create one mono
oscillator/ADSR/VCA/output patch (optional existing low-pass), create a short
note pattern with the native keyboard, explicitly connect pattern pitch to
patch pitch and pattern gate to patch gate, play, modify and save/reopen.

**Sound/control:** independent pitch/gate bindings and traces are visible;
disconnecting the composition gate binding supplies an explicitly documented
low gate and silences the finite render after release without changing pitch
events; reconnect restores notes. Internal patch cables remain explicit.
Construction parameter/note edits publish changed finite audio; runtime exposed
level changes do not overwrite defaults. Mode transitions preserve transport.
Keep one instance, one pattern and the already executable mono chain; broader
graphs are unnecessary for this first usable workflow.

**Negative tests:** duplicate IDs, missing/type-invalid endpoints, unsupported
topology, stale draft, invalid notes, lossy export, save failure, malformed/unknown
version, escaping paths and incompatible references. Failed commands leave
accepted source/history/publication intact; failed saves retain dirty work.

**Persistence:** additive self-contained readable authored format for this
subset, chosen in its example; no required pre-existing TOML base or controller
script. Save graph, pattern IDs/counter/ticks, bindings, defaults and exposed
controls. Do not serialize transport, pickup or runtime overrides. Opening an
empty root creates no files; save creates only necessary authored files.

The bounded native format/entry is now project-v2 Lua at `main.project.lua`;
Ctrl-S/F6 atomically saves authored state, and project opening restores it before
devices. Native/legacy musical entry ambiguity, external modifications, symlinks
and foreign temporaries reject. A composed native-key test covers worker/callback
changes, mode continuity, failures/retry and isolated fresh-process entry loading.
Native captures embed their opening source, surviving later saves and root removal.
Timed replay and multi-composition layout stay deferred.

**Reuse/compatibility:** move existing source ownership out of the synth-only
editor as needed; extend importer/builders, `PatchEdit`, `NoteEdit`, note
compilation, host/publication and Lua serializers. Native note editing dispatches
existing typed note operations. Legacy entrypoints and v1/v2 exports keep their
unchanged-base behavior and hashes. No general package manager/DAG/state migration.

**Complete when:** a fresh-process keyboard-only acceptance completes the whole
workflow and reproduces exact supported source/f64 audio without original seed,
MIDI or journals; historical compatibility and applicable full crate suites pass.
PCM revision publication is reported honestly; no continuous DSP-state claim.

### 2. One palette, two compositions, two independent pattern sources

**Scenario/example:** [35-project-palette-routing](songs/examples/35-project-palette-routing/ACCEPTANCE.md): reuse one bass
definition in two compositions; melody supplies pitch, rhythm supplies gate.
One pattern also feeds two separately identified instances.

**Current checkpoint:** PageUp/PageDown selects independent palette patterns
through typed app commands without rebinding or rerendering. Native keys can now
author melody and rhythm separately and bind their pitch/gate independently.
The executable v2 fixture/tests prove one shared patch, two independent runtime
gains, save/reopen and capture reconstruction. Named composition ownership/selection
and the composed empty-root workflow now have headless acceptance.
Project-v3 stores named membership/default selection; b snapshots all instances
and F7/F8 selects after stop acknowledgement. Worker preparation gates restart;
inactive definitions validate and runtime overlays apply only to active members.
Both selections and edited membership reopen in fresh processes. Native multiple
instances, identity-based selection/bindings/levels and stopped a/d membership now
have a checked checkpoint. The completed audit checks negative candidates/typed
requests, shared edits across compositions and both fresh-process selections.
See the acceptance file for the executable mapping and explicit deferred scope.

**Sound/control:** changing rhythm changes articulation without transposing;
changing melody changes pitch without moving gate boundaries. Composition
selection exposes palette/instance identities and retains the playing instance
until its replacement is prepared. Explicit stop/start transition policy is
acceptable here; preserving every DSP state is not claimed.

**Negative tests:** ambiguous selection, missing definitions, conflicting
instance IDs, multi-source input without combiner, unsupported chord-to-mono
binding, deletion of referenced palette material and gate-end collisions.

**Persistence/compatibility:** project-wide save includes all accepted definitions
and both composition bindings; no permanent `AuthoredPattern.track` owner in
new data. Adapt legacy track lists and base-bound exports. Keep existing duration,
voice and sampler behavior; reject unsupported translation rather than lose it.

**Complete when:** both compositions reopen independently, shared definitions
remain equal, edits/instance overrides have documented scope, routing traces and
full/split audio prove independence. Depends on slice 1.

### 3. A modulated bass through explicit graph controls

**Scenario/example:** [36-patch-lfo](songs/examples/36-patch-lfo/ACCEPTANCE.md): one beat-synchronized LFO feeds
filter cutoff and oscillator pitch through separate explicit unit adapters;
envelope controls LFO depth and pattern gate resets it. A free-running variant
demonstrates the different transport contract.

**First checkpoint implemented:** mono LFO → explicit bipolar/unit-interval
scale/clamp and envelope multiplier → VCA, inside the existing note compiler.
Static audio-rate/per-instance settings, Hertz/beats phase, cabled gate reset,
shared fan-out, typed graph drafts, capture and fresh-process persistence now have
headless tests. Worker/callback tests prove playing rate edits and invalid-unit
publication continuity. A second checkpoint now adds explicit PitchModulate
vibrato/depth, shared amplitude/pitch fan-out and typed draft/capture/save/reopen
tests with independent sample references. One oscillator adapter is supported;
Plaits and pitch chains reject. A third checkpoint adds a typed Hertz cutoff
adapter, authored clamping, cable/default precedence, independent filter-tail
references, fresh reopen and playing publication.
ControlAttenuate now also enables envelope-controlled shared depth with explicit
units, independent amplitude wiring, capture reconstruction and signal references.
An explicit Hertz rate input now integrates phase samplewise, with all-waveform,
reset, tempo, capture, isolated reopen and playing feedback-rejection checks.
Full example remains design: event/control cadence and broader scopes are required.

**Sound/control:** shared phase sweeps both destinations; depth/rate/reset are
inspectable. Demonstrate explicit sum/scale/clamp, event versus audio/control
cadence and per-instance scope. Reuse `Lfo` kernels and DSP functions while
extending the existing song compiler's supported topology; do not invoke the
legacy matrix as hidden routing.

**Negative tests:** unit/family/rate mismatch, multiple cables without combination,
zero/invalid period, undeclared scope, unbroken feedback, unbounded expansion.

**Persistence/compatibility:** store module identity, rate/scope/reset/seed,
adapters and combinations. Historical matrix summation/clamping and Plaits
exceptions stay compatible; no automatic conversion changes old music.

**Complete when:** shared fan-out, modulated-rate integration, reset/stop/rewind,
tempo and split-render tests pass, audible assertions show expected modulation,
and save/reopen preserves the graph. Depends on slice 1; slice 2 supplies expanded
instance-scope acceptance without blocking the first mono demonstration.

### User-prioritized keyboard work alongside slices 3–4

Example 36's MIDI entry
checkpoint now connects the MicroLab mk3/generic keyboard to native construction
patterns through existing typed edits, capture, worker publication and save/reopen.
Unique port matching, bounded queues, stale epochs, mode gating and overlap
rejection are checked headlessly; host discovery/connection was observed.
The bounded keyboard objective now supports explicit live Pitch/Gate binding and
playing a named mono instance without changing its saved patterns. Accept note-on/off/zero-velocity,
retrigger and all-notes cleanup on overflow/disconnect/target changes, runtime/source
separation, bounded worker/callback scheduling and deterministic capture. Show
measured latency and listening evidence separately. Sustain/bend must have authored
routing; do not emulate live audition by rewriting patterns or looping a held-note
PCM buffer. Reuse the existing voice compiler and scheduler, not a second engine.
The renderer prerequisite now exists: LiveMonoRenderer shares MonoState with the
offline mono renderer and preserves DSP across bounded, prevalidated u64 blocks.
Varied partitions exactly match offline tremolo/reset/rate/filter/release and
FM/harmonic Plaits; independent pitch/gate and atomic rejection are checked.
App runtime lifecycle now adds explicit named-mono binding, bounded channel/key
priority, retrigger/fallback/release and cleanup epochs. Capture reconstructs
identical worker events/PCM without changing authored source. Native F9 now
connects stopped-transport mono audition, epoch-tagged ingress and loss cleanup
to the existing callback through a bounded timed PCM ring and shared-DSP worker.
Headless held-key PCM, source/runtime capture and callback allocation safety pass.
Explicit keyboard velocity now has a unit-interval Control port and existing
combiner wiring, with soft/hard/fallback, capture and persistence acceptance.
Explicit bipolar bend now reuses PitchModulate with authored depth, independent
phase/partition references, typed capture and real store reload. Production
worker/callback continuity and loss centering are checked headlessly.
The bounded explicit GateHold route now accepts key/pedal independence,
held/retrigger/release sound, loss cleanup, typed capture and authored reopen
headlessly. The native patch editor now also stages the three supported
expression-port declarations with their consumers in one transaction, then
captures/saves/reopens them. An independent `u` binding now routes a pattern's
note-start velocity to the same authored Control without changing pitch/gate;
headless loop, capture and persistence acceptance pass. Physical pedal interaction
remains a gap; broader external source types require separate examples.
Simultaneous arranged/live routing, MIDI/device timestamp mapping, timed replay
and graph-state preservation remain separate unfinished contracts.

### 4. Normalled patch and an atomic dynamic interface recipe

**Scenario/example:** proposed `37-normalled-patch-recipe`: a default envelope
controls amplitude; one transaction replaces that route with LFO control,
introduces a combiner/macro and exposes its rate/depth controls. Disable the
normal explicitly and inspect resolution.

**Sound/control:** default notes, replaced route and explicit blended control
produce distinguishable rendered behavior. Graph/normalization/interface share
one revision; removed controls show unavailable, new controls need pickup.
Reuse example 30 batches, typed assignments, readonly Lua widgets and pickup.

**Negative tests:** ambiguous normal priorities, invalid complete graph, stale
transaction/interface, duplicate semantic control IDs, stale fader events and
implicit mixing. Failure cannot partially accept a macro or route.

**Persistence/compatibility:** save authored policy, recipe expansion, stable
module/cable/control IDs and transforms; runtime resolved values are not defaults.
Existing explicit cable files retain exact meaning and v1/v2 reload behavior.

**Complete when:** route/interface changes commit atomically, old mappings cannot
retarget, pickup/reconnect pass and saved graphs reproduce all variants. Immediate
finite PCM publication is sufficient; performance-state migration remains slice 6.
Depends on slice 3's control ports and slice 1's transaction/save path.

### 5. Event transformations in the existing scheduler

**Scenario/example:** proposed `38-pattern-event-patch`: melody passes through
transpose/quantize and rhythm through seeded probability/ratchet/gate-length
processing; an LFO controls probability sampled at each original event boundary.
Keep an explicit fork of the original Pitch/Gate source: one branch feeds the
ordinary note/envelope path while another feeds mutation processing. Both
branches must be observable at the same time; mutation on one branch cannot
silently replace the direct branch or change its event identity/timing. Derived
mutation events retain explicit provenance. Typed adapters must be declared
where a processor consumes Event rather than Pitch or Gate.

**Sound/control:** changed subdivisions/pitches and deterministic committed
outcomes appear in event traces and rendered audio. Each transform declares
time, identity, retrigger and loop semantics; children retain provenance.

**Negative tests:** expansion beyond declared budget, invalid offsets/durations,
same-frame ambiguity, dropped gate ends, inappropriate continuous-rate evaluation,
feedback without a timing break, incompatible event adapters and two sources
driving one input without an explicit combiner or selector.

**Persistence/compatibility:** store transforms, seed/commit policy and occurrence
provenance; a fresh render/replay produces the same supported outcomes. Preserve
legacy step clipping and authored note durations through explicit adapters.

**Complete when:** direct and chunked schedules/audio agree, loop/rewind and
gate cleanup pass, bounds are enforced and save/reopen is deterministic. Depends
on slices 2/3; extend existing event compilation instead of adding a sequencer.

### 6. One stateful graph revision at a musical boundary

**Scenario/example:** proposed `39-performing-graph-revision`: sustain a bass
while inserting a filter at the next bar, then remove a delay while draining
its tail. Start with one supported state compatibility contract, then expand.

**Sound/control:** unchanged oscillator/envelope state continues, inserted state
initializes by declared policy, removed gates release and old delay tail remains.
Show requested/ready/actual activation and consumed revision separately.

**Negative tests:** compilation misses boundary, stale completion, incompatible
state/version, superseded/cancelled revisions, resource limit and failed migration.
Never drop an accepted structural action through the knob worker's latest-wins slot.

**Persistence/compatibility:** record transaction/base IDs, state policy,
requested musical time, actual u64 epoch/frame and failure/reschedule facts.
Authored graph stays unchanged until promotion. Existing PCM publication and
callback startup gain remain compatible.

**Complete when:** cross-revision checkpoint/split tests preserve phase, envelope,
filter history and tails as declared; no stuck gates; bounded callback checks and
separate reproducible deadline measurements pass. Test a deterministic worker
checkpoint/segmented-render path first; PCM crossfade alone cannot pass continuity.
Depends on slice 4 and sufficient checkpoint/state work within this slice.

### 7. Capture, replay and harvest a metapattern

**Scenario/example:** proposed `40-performance-metapattern`: capture pattern
replacement, modulation macro and quantized recipe; harvest them into a reusable
composition metapattern and perform it with a live intervention.

**Sound/control:** replay applies the same supported actions at the same epochs/
frames; arranged and live precedence is inspectable. Loops/conditions have finite
budgets and stable same-frame ordering. Reuse `PerformanceLauncher`, phrases,
futures, typed requests/journal and source revisions; no callback Lua.

**Negative tests:** incompatible base/dependencies, >u32 frame truncation,
rewind/loop epochs, duplicate accepted facts, recursive/unbounded metapattern,
crash checkpoint/retry and stale activation.

**Persistence/compatibility:** versioned typed capture adapter preserves old
sessions and writes new u64 facts; harvest creates new authored files and leaves
capture/defaults intact. Declared RNG exceptions limit exact PCM guarantees.

**Complete when:** fresh-process timed replay proves supported state/audio,
harvested composition reproduces the selected material and crash/rejection tests
preserve facts. Depends on slices 2/5/6. Input epoch and capture fixes needed by
earlier slices must land earlier; this is not permission to postpone correctness.

### 8. Pin and update a reusable library patch with its sample assets

**Scenario/example:** proposed `41-pinned-library`: reference a pinned bass patch
and sample kit, instantiate local overrides, compare copying material, update
deliberately and bundle for another machine.

**Sound/control:** upstream edits leave existing music unchanged; adopted update
has an inspectable dependency/source change and audio result. Missing material
has actionable dependency status rather than partial playback.

**Negative tests:** wrong content digest, missing commit/asset, incompatible
version, path escape, changed cache, transitive mismatch and conflicting override.

**Persistence/compatibility:** lock identities, full revisions, content digests,
transitive definitions/assets and resolver version. Offline cache and portable
copy/bundle are explicit. Git is allowed/required installed, not auto-initialized;
local legacy references remain confined. No full package manager.

**Complete when:** fresh offline resolution reproduces pinned source/audio,
updates are atomic and portable bundle reopens. Depends on slice 2's palette;
may proceed independently of slices 3–7 once local ownership is accepted.

### Acceptance track alongside every slice

Physical APC/LaunchControl gesture, pickup, reconnect, LEDs and listening are
separate protocols. Preserve current headless `accept` scope; never infer physical
or deadline acceptance from finite renders. General bus/DSP DAGs, additional
signal families and polyphonic routing grow through further small examples, not
by shrinking the intended musical model to these first slices.

## Original phased roadmap (historical)

The original phases describe early engine layering and completed showcase work.
They are retained for rationale, not as current status or a waterfall prerequisite.

The current implementation queue for arrangement, automation, polyphony, chord
editing, and the long-form showcase lives in [`TODO.md`](TODO.md).

This roadmap is ordered to reduce risk. Do not build the shiny cockpit before the engine has bones.

## Phase 0: Repository skeleton

Deliverables:
- Rust workspace
- crates split by boundary
- basic CI
- formatting/linting
- no audio hardware requirement in tests
- sample project fixtures
- command-line offline test runner

Done when:
- `cargo test` runs
- crates compile
- fixtures can be loaded as raw text

## Phase 1: Core timeline and event model

Deliverables:
- typed IDs
- `FrameRange`
- tempo/BPM conversion
- pattern length
- steps
- events
- tags
- probability seed model
- event scheduling into frame ranges

Done when:
- tests prove step-to-frame conversion
- events can be queried for a frame range
- probability can be deterministic when seeded

## Phase 2: DSL parse and validation

Deliverables:
- project format, likely TOML or RON
- instruments
- samples
- tracks
- patterns
- roles
- buses
- simple relations
- validation diagnostics

Done when:
- fixture project parses into typed model
- missing references produce useful diagnostics
- parsed strings are replaced by typed IDs in validated model

## Phase 3: Headless single-thread offline render

Deliverables:
- `AudioBlock`
- sample loader stub or WAV loader
- sample playback
- gain/pan
- basic mixer
- offline render command
- WAV output or raw buffer test output

Done when:
- a pattern renders to deterministic audio
- golden/fingerprint tests pass
- no TUI/audio device required

## Phase 4: Realtime audio output

Deliverables:
- device output backend
- realtime callback skeleton
- preallocated buffers
- event queue
- basic playback transport
- underrun/miss counters

Done when:
- audio plays from a simple pattern
- callback avoids forbidden operations
- tests still run headless

## Phase 5: TUI command cockpit

Deliverables:
- transport panel
- pattern grid
- selected step inspector
- command dispatch
- logs/diagnostics panel
- basic edit commands

Done when:
- user can start/stop playback
- toggle steps
- edit velocity/gate
- see diagnostics
- TUI edits go through command model only

## Phase 6: Source and relation graph

Deliverables:
- source graph
- relation graph
- audio/control edges
- node properties
- dirty range calculation
- graph inspector data

Done when:
- tests prove invalidation through audio edges
- tests prove invalidation through control edges
- nonlinear barriers are represented

## Phase 7: Render cache and artifact keys

Deliverables:
- `ArtifactKey`
- fingerprinting
- artifact cache
- chunk ranges
- cache lookup
- dirty invalidation
- render plan fingerprints

Done when:
- repeated renders hit cache
- source edits invalidate only affected artifacts
- tail/lookahead expansion is tested

## Phase 8: All-core worker pool

Deliverables:
- long-lived worker threads
- priority queue
- render horizon
- wanted artifact discovery
- hot/warm/cold priorities
- worker diagnostics

Done when:
- dirty future work saturates worker cores
- clean state sleeps
- realtime callback does not spawn workers
- missed hot artifacts degrade gracefully

## Phase 9: Event-aware effects

Deliverables:
- event tags
- tag predicates
- effect send rules
- delay/reverb placeholder bus
- explanation data for active sends

Done when:
- accents can feed delay while normal notes do not
- ghost notes can feed reverb while main hits stay dry
- TUI can show why a send happened

## Phase 10: Role-aware sidechain and dynamic ducking

Deliverables:
- source roles
- role priority table
- envelope follower
- sidechain relation
- simple ducking
- multiband ducking prototype
- explanation model

Done when:
- kick can duck bass
- ducking can be limited to selected bands
- changing kick invalidates bass ducked artifacts through control edge
- TUI can show active relation and attenuation amount

## Phase 11: Chunk transforms

Deliverables:
- chunk transform source type
- reverse
- reslice
- freeze/smear prototype
- transformed chunks become artifacts/sources
- TUI command to create transform

Done when:
- user can transform a bar into a new playable source
- transformed artifact participates in cache/fingerprint system

## Phase 12: Speculative performance futures

Deliverables:
- likely gesture model
- queued scene pre-rendering
- mute/fill variants
- cache prioritization by recency/selection
- visible status panel

Done when:
- current/queued patterns render ahead
- likely mutes/fills are precomputed
- worker pool sleeps when all desired futures are clean

## Phase 13: Polish and hardening

Deliverables:
- better diagnostics
- graph visualization/export
- audio safety limiter
- CPU/memory profiling
- artifact cache bounds
- project save/load stability
- crash-resistant fixtures
- docs

Done when:
- demo can run repeatably
- missed artifacts are visible
- glitches are diagnosable
- project can be understood from TUI panels

## Explicitly later

Not in the first major milestone:
- arbitrary graph UI
- plugin hosting
- full synth workstation
- DAW arrangement editor
- cloud/library features
- arbitrary scripting in realtime
- maximal spectral processing
