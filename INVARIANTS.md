# Project Invariants

These are non-negotiable. If an implementation choice violates one of these, the choice loses.

Authorized Plaits exception: the pinned upstream crate retains its native f32
DSP behind an explicit f64 module boundary. Plaits RNG is exempt from exact
sample determinism; authored source fingerprints and all other engines remain
deterministic. The first FM/harmonic subset uses no RNG. See [PLAITS.md](PLAITS.md).

## 1. The audio callback is sacred

The realtime audio callback must not:
- allocate on the heap
- block on locks
- wait on worker threads indefinitely
- parse DSL
- load files
- log in hot paths
- perform unbounded graph traversal
- perform UI work
- touch the filesystem
- depend on network state

Allowed:
- read precompiled state through lock-free or wait-free access
- drain bounded realtime-safe event queues
- render live overlays
- pull cached chunks
- mix buffers
- apply final safety limiting
- report minimal counters through atomics

## 2. Separate source truth from rendered artifacts

The system must distinguish:

```text
Source graph:
  patterns, samples, instruments, effects, roles, scenes, macros

Relation graph:
  audio edges, control edges, sidechains, modulation, mix rules

Artifact graph:
  rendered chunks, stems, previews, spectra, analysis, cache entries

Realtime graph:
  the tiny set of state required to emit audio now
```

Rendered artifacts are disposable. Source and relation graphs are truth.

## 3. All edits go through commands

UI code must not directly mutate audio engine state.

Every edit is a command:

```rust
Command::SetStep { pattern, track, step, event }
Command::SetParam { target, value, smoothing }
Command::ToggleMute { track }
Command::AssignRole { source, role }
Command::SetRelation { relation_id, relation_def }
```

Command processing:
1. validate
2. mutate the declared authored or runtime owner (never implicitly promote)
3. bump revisions/fingerprints
4. compute dirty ranges
5. enqueue render jobs
6. publish compiled state safely

## 4. Audio and control dependencies both matter

A dependency is not only “audio flows from A to B.”

Control edges also invalidate downstream artifacts.

Example:

```text
kick audio -> drum bus
kick envelope -> bass sidechain compressor
```

If the kick changes, both the drum bus and ducked bass artifacts are dirty.

## 5. Do not enumerate the combination explosion

The power set of sources is not a render plan.

Represent combinations symbolically.
Materialize only wanted futures.

Wanted futures include:
- audio needed near the playhead
- queued scenes
- selected pattern previews
- likely performance gestures
- visible meters and analysis
- manually requested bounces/freezes

## 6. Exploit linearity, declare barriers

Every node must declare properties:

```rust
NodeProperties {
    linearity,
    latency_frames,
    tail_frames,
    can_cache,
    needs_group_signal,
}
```

Linear nodes can be factored and reused.

Nonlinear/group-dependent/time-variant/feedback nodes create cache boundaries.

Examples of barriers:
- bus compressor
- saturation
- limiter
- sidechain compressor
- feedback delay
- reverb
- spectral processors
- envelope followers used as control dependencies

## 7. Dirty invalidation must be precise and tail-aware

Edits invalidate ranges, not entire universes.

If a node has a tail, the dirty range expands:

```text
dirty_end += node.tail_frames
```

If a node has lookahead, the dirty range expands backward:

```text
dirty_start -= node.lookahead_frames
```

## 8. Determinism is a feature

For offline/precomputed artifacts:

```text
same source graph
+ same relation graph
+ same frame range
+ same sample rate
+ same seed/probability commit
= same artifact fingerprint
```

Random/probabilistic behavior must be seedable and commit-able.

## 9. The machine consumes the whole CPU only when work exists

The desired behavior:

```text
dirty work exists -> saturate available worker cores
dirty work complete -> sleep
```

No fake busy work.
No constant CPU burn for ego.
All-core hunger must be tied to useful jobs.

## 10. Realtime playback must degrade gracefully

If a hot artifact misses its deadline:
- use a previous compatible chunk with fade
- render a simplified fallback
- silence the missing bus
- report the miss

Do not block the callback waiting for perfection.

## 11. Performance behavior must be inspectable

Relationship DSP must not feel haunted.

Every active rule should be explainable:
- why a source was ducked
- which role priority won
- which control edge fired
- which chunk was invalidated
- which cache artifact was used
- whether a fallback occurred

## 12. Source relationships are first-class

A source is not just audio. It can expose:
- audio stream
- event stream
- envelope stream
- transient stream
- spectral features
- role metadata
- density/intensity metrics
- tags from the sequencer

These may drive downstream DSP.

## 13. Use typed boundaries

Avoid “string soup” after parsing.

The DSL can contain strings.
The validated model should use typed IDs and typed enums.

```rust
TrackId
PatternId
NodeId
RelationId
ParamId
FrameRange
SampleRate
Fingerprint
```

## 14. No arbitrary user code in the audio callback

Scripting can exist later, but only outside the realtime path.

Scripts may generate source graph changes or command streams.
Scripts must not execute inside `process_block`.

## 15. Test graph behavior before chasing sound toys

The first correctness target is not a lush reverb.
It is:
- commands produce predictable graph changes
- dependencies are correct
- invalidation is correct
- fingerprints are stable
- render jobs are deterministic
- realtime callback stays bounded

## 16. Composition invariants and implementation obligations

The following are enforceable requirements for new slices. They are **not
claims that the current runtime already satisfies them**. Existing exceptions
and gaps are identified in [ARCHITECTURE.md](ARCHITECTURE.md#architectural-gap-analysis).
Historical formats retain explicit compatibility adapters.

| Boundary | Required invariant | Current evidence / outstanding work |
| --- | --- | --- |
| Project ownership | A root owns definitions, compositions, local overrides and dependency resolution. Empty roots are valid; no hidden musical database or Git repository is needed. Referenced definitions are immutable; local instances own overrides. | Empty/legacy roots, confinement, app-owned palette and mono instances, native v2/v3 save/reopen exist. V3 named compositions reference project-unique instance IDs in one registry; selection never copies palette definitions. All compositions/default selection survive fresh-process reload; inactive definitions validate. Native/legacy entry ambiguity rejects. Git may be required installed. |
| State ownership | Every accepted mutation identifies authored construction or active performance state. Journals describe facts; only an explicit promotion writes runtime material into authored definitions. Save never silently promotes. | Native output-level overlays preserve source Arc/fingerprint/defaults; loading resets overlays, modes retain them. Native accepted typed facts persist separately, including before/after source fingerprints. Project-v2 encoding accepts only authored source and excludes runtime/editor state. Capture is not authored save or timed activation. Legacy editable faders write source. |
| Signals | Pitch, Gate, Trigger, Audio, Control, Event, Feature and Metadata retain independent types. Conversion and rate/unit changes have explicit inspectable semantics. | Song ports implement only Audio/Pitch/Gate/Control. Example 34's mono bindings execute pitch and gate separately; legacy track notes retain coupled articulation. General event processors/rate adapters remain missing. |
| Graph validity | Only complete, type/topology-valid, executable candidates enter accepted musical state. Incomplete drafts have no audio authority. Every endpoint resolves; unsupported execution rejects explicitly. | Example 30 proves a bounded synth subset; validated graphs are not all executable. |
| Normalization | Authored policy, resolved routes and current values are distinct. Override/disable never implicitly adds signals. Multiple sources require a declared combiner/selector. | No authored normalization model; current single-input rule must not become a permanent restriction. |
| Modulation | Each connection declares family, units, rate/interpolation/latching, instance scope and combination. Fan-out shares a source instance unless authored otherwise. Reset/random state is fingerprinted; cycles need executable delay semantics. | Example 36 adds bounded mono audio-rate/per-instance LFO, explicit bipolar/unit-interval scale/clamp and multiplication, fan-out and cabled gate reset. PitchModulate explicitly adds bipolar Control × semitone depth to independent Pitch; one oscillator adapter is supported. ControlHertz explicitly converts unit-interval to clamped Hertz and owns the cabled filter cutoff, rejecting cutoff overrides. ControlAttenuate explicitly multiplies bipolar Control by unit-interval amount; envelope depth and VCA articulation remain separate cables. Units/rate metadata and settings participate in validation/fingerprints. Cabled LFO rate is Hertz within declared 0.001..100 Hz clamps: reset precedes current-phase evaluation, then this sample advances phase by Hertz/sample-rate. Static-rate timing remains unchanged. Broader cadence/scopes and delayed feedback remain missing. Legacy matrix sums/clamps its fixed catalog unchanged. |
| Transactions | Transaction ID, base revision and stable entity identities determine acceptance; complete graph/interface validation precedes commit. Rejection retains source, runtime, accepted journal and published audio. Activation and acceptance are distinct facts. | Clone/compile acceptance and keyboard stale-base checks exist; cable IDs, general base checks and scheduled graph activation are missing. |
| Dynamic surfaces | Semantic control identity is independent of position. Removed/changed targets invalidate mappings; no unrelated target inherits identity. Interface and graph share one revision, and absolute controllers rearm. | Static targets/pickup exist. Native instance selection is independent of patch/pattern selection; membership changes retain the ID, and inactive selection cannot address another instance's level. General revisioned dynamic interfaces and native hardware pickup remain missing. |
| Publication | Submitted, compiled, published, activated and callback-consumed revisions are distinguishable. Workers cannot activate stale work. Callback work/storage stays bounded. Stateful changes declare preserve/reset/drain/crossfade policies and gate continuity. | Generation-tagged finite PCM remains arrangement playback. Native stopped audition now streams the shared mono DSP through bounded timed epoch-tagged PCM; callback rejects stale/late data and reports gaps. Continuous fixed-graph state is proved headlessly, not graph migration/general activation. Consumption is not proof of DAC sound. |
| Time and ordering | Absolute positions are u64; loop/rewind use explicit epochs. Same-frame events have deterministic order and bounded expansion. End-exclusive notes/gates cannot be left stuck by disconnect, cancellation or graph replacement. | Core/typed Lua use u64; legacy host/session and finite PCM lengths use u32. Native audition clock/scheduled frames use u64 with ingress/PCM epochs and bounded ordered event expansion; disconnect/overflow reset Gate, source/transport release binding. General graph/event epochs, musical-time device replay and deadline acceptance remain open. |
| Reproducibility | Saves retain every accepted authored semantic input, stable identity, dependency pin, asset content fingerprint and supported random commitment, or reject loss. Missing/incompatible references diagnose before activation. Updates are deliberate. Capture baselines must remain independent of subsequent authored saves. | Base-bound Lua and bounded self-contained project-v2 save/reopen exist; v2 checks full restored source equality and fresh-process oscillator PCM. Native replacement rejects external edits/symlinks and preserves the previous good entry on failure. Native captures embed their opening source; bounded import checks its fingerprint and reconstructs after later saves/root removal. Older captures require their original source. Dependency locks and timed replay remain open. Plaits f32/RNG exceptions above remain authorized. |

Runtime keyboard ownership requires separate acceptance of production cleanup.
The app's current bounded mono keyboard lifecycle separates typed Pitch/Gate
outputs from authored patterns,
retains channel identity, tags commands with binding epoch/u64 frame and rejects
stale/budget/invalid commands atomically. Disconnect/overflow/panic reset commands
clear held keys and advance the epoch; source revisions invalidate the binding.
Adapters must discard old ingress and issue resets before fresh input after loss.
Native stopped-transport F9 audition now implements that loss handling with
ingress epochs, reserved release capacity and a fixed timed PCM ring. Worker
compilation/DSP/locks/allocations remain off the callback; it consumes only finite
atomic samples with bounded stale-slot examination. Device clock advances through
silence as u64, late PCM is discarded and missing data is counted. Source or
transport changes release audition; mode/selection preserve it. Tests prove
headless worker/callback PCM continuity and zero callback allocation/deallocation,
not physical deadlines, graph migration or simultaneous arranged/live routing.
Runtime keys/bindings are not persisted as authored defaults.

Live velocity is a separate unit-interval Control source, never implicit note
gain or articulation. Its declared `input.velocity` port and explicit cables
determine consumption; incompatible units and undeclared ports reject atomically.
Repeated presses replace held velocity; winner release restores prior held pitch
and velocity without Gate retrigger. Final release/reset holds the last value and
lowers Gate. Accepted values are MIDI-resolution 0..127; normalization occurs once
at the graph input. Fresh DSP initializes this Control to unity, independently of
legacy coupled-note velocity. An explicit independent velocity-pattern binding
now supplies authored f64 note-start values to that same Control; values hold
across note ends, and the velocity pattern's pitches/durations cannot articulate
Gate or change oscillator pitch. At one frame, gate end/reset precedes pitch and
velocity updates, which precede gate rise. Unbound pattern velocity stays unity.
Runtime keyboard values are captured, never saved as patch defaults; old captures
without velocity replay at unity. These are bounded mono guarantees, not general
event-transform, cadence or controller-smoothing requirements.

Live wheel motion changes only a declared bipolar Control input: no key selection,
Gate articulation, amplitude change or implicit pitch range. MIDI-resolution values
are 0..16383 with center 8192, normalized to exact -1/0/+1 endpoints. Typed events
retain channel/epoch/u64 frame, but the current mono binding owns one shared bend
value across key channels. Explicit adapter settings own musical units/depth.
Reset centers a noncentered wheel even with no active gate; cleanup reserves two
event slots before acceptance. Source/transport invalidation clears bend ownership;
fresh DSP starts centered. Key fallback and mode/selection cannot reset or retarget
the wheel. Captures preserve raw events; authored persistence excludes wheel state.
No smoothing, simultaneous LFO/bend summation or per-channel bend allocation is
implied by this bounded checkpoint.

Live sustain is a separate authored Gate input and an explicit GateHold processor,
not an implicit MIDI note-off rule. The bounded mono lowering requires exactly
one `input.gate → GateHold.gate`, `input.sustain → GateHold.hold` and
`GateHold.gate → ADSR.gate` route. A high key Gate retriggers; a low key Gate
releases only when hold is low. A high hold never resurrects a released gate.
Fresh DSP starts hold-low; reset/disconnect/overflow clears it even if no key is
down. Accepted typed pedal events retain epoch, channel and u64 frame, and capture
records them. Authored saves retain declarations/cables but exclude runtime pedal
state. The CLI CC mapping is opt-in; unrecognized CC cannot silently latch Gate.
Backpressure reserves three cleanup events before accepting more input. Current
support is one mono oscillator and one ADSR route; general Gate processors,
Plaits routing and physical behavior remain future acceptance obligations.

Native expression-input edits use the existing typed PatchEdit batch. A staged
declaration remains private until complete importer, topology and executable
validation succeed. A failed or stale commit retains accepted source, history and
published audio. The current editor supports only velocity/bend Control and
sustain Gate; base mono pitch/gate inputs cannot be removed. Input deletion must
declare whether dependent cables reject or disconnect; the final graph must still
compile. Authored saves contain only accepted inputs and cables, while runtime
expression values remain separate.

Acceptance must separately prove source correctness, published/consumed audio,
state-transition semantics and physical behavior. Conservative full-horizon
invalidation is the current authoring fallback; it must be reported honestly
until dependency/range precision is proved. No future continuity requirement
licenses allocating, compiling, running Lua/Git or doing filesystem/network work
in the audio callback.
