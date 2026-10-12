# Testing Strategy

The engine should be tested like a compiler and like an audio system.

## Next-architecture acceptance obligations

Follow [ROADMAP.md](ROADMAP.md) and example 34. Untested requirements remain
proposed. Assert Rust state, compiler behavior, worker publication and callback
consumption separately from presentation, then fresh-process persistence.

Native capture's current checkpoint proves typed-command reconstruction in a fresh
app, source/runtime separation, bounded pending storage, failure/retry without lost
or duplicated events, clean/unclean markers, history/source-continuity diagnostics
and path confinement. It does not prove timed replay, u64 epochs or authored reopen.

- Empty opening creates no musical files or Git repository; saved music reopens
  without a seed, hardware or replay.
- Construction/performance mutations have distinct owners; runtime gestures do
  not change authored fingerprints/defaults until explicit promotion.
- Pitch/gate streams, transforms, rate/unit adapters and normalization have
  inspectable traces; prove sound and reject ambiguous signal combination.
- Rejection preserves source/runtime/history/audio. Dynamic interface and graph
  revisions agree; identity/pickup/stale-input protection survive surface changes.
- Test across graph revisions, not only pre-roll within one graph: phase/envelope/
  filter history, tails/gate continuity, activation/missed deadlines, cancellation,
  supersession and checkpoint/chunk equivalence.
- Capture/composition tests cover u64 frames above u32, loop/rewind epochs,
  same-frame order, bounded conditions/expansion, crash/retry and harvesting.
- Dependency tests distinguish pin/copy/override/update, missing/incompatible
  content, offline cache and portable sample bundles.

Protect historical fixtures, entrypoint precedence, hashes and Lua versions.
Plaits native f32/RNG exceptions keep their scoped tests. Physical sound,
gestures and deadline evidence remain separate from headless acceptance.

## Test layers

### 1. Pure model tests

Test:
- IDs
- commands
- validation
- graph construction
- dependency edges
- dirty ranges
- node properties

These tests should not touch audio devices.

### 2. DSL fixture tests

Fixtures:
- minimal project
- missing sample
- invalid role
- invalid relation
- sidechain project
- event-aware send project
- multiband ducking project

Test:
- parse success/failure
- diagnostics
- typed model conversion
- stable fingerprints

### 3. Timeline tests

Test:
- BPM to frame conversion
- step to frame conversion
- gate length
- pattern loop boundaries
- probability seed determinism
- ratchets
- polymeter later

### 4. Graph invalidation tests

Must test both audio and control dependencies.

Examples:
- changing hat invalidates drum bus but not bass sidechain
- changing kick invalidates drum bus and bass sidechain target
- reverb tail expands dirty range
- lookahead expands dirty range backward
- nonlinear node creates cache boundary

### 5. Render plan tests

Test:
- source graph compiles to expected render ops
- render op fingerprints are stable
- equivalent linear mixes canonicalize where intended
- nonlinear barriers prevent invalid reuse

### 6. Offline audio tests

Test:
- render deterministic buffer
- render same artifact twice equals same fingerprint
- cache hit produces same output
- changing one event affects expected frame range
- final output contains finite samples only

Avoid brittle “exact floating point waveform” tests except for tiny deterministic kernels.
Prefer:
- fingerprints for known fixtures
- RMS/peak ranges
- no NaN/infinity
- expected silence/non-silence
- monotonic envelope behavior
- known impulse responses for simple filters

### 7. DSP kernel tests

Test:
- envelope follower attack/release behavior
- ducking gain never negative unless explicitly allowed
- param smoothing reaches target
- filters remain finite
- denormal prevention for long tails
- limiter catches overs

### 8. Realtime safety tests

Some realtime rules are hard to prove automatically, but enforce what we can.

Test/scan:
- no filesystem calls in callback modules
- no logging in hot callback
- no thread spawning in callback
- no unbounded allocation in callback path where practical
- callback consumes preallocated buffers

Use code review and module boundaries as part of the test.

### 9. Worker/cache tests

Test:
- dirty jobs enqueue
- jobs complete into cache
- priority ordering
- hot jobs outrank cold jobs
- clean state has empty queue
- artifact cache evicts safely
- missed artifact fallback path is explicit

### 10. TUI state tests

Test:
- key command maps to typed command
- UI never mutates engine state directly
- inspector data can render active relations
- diagnostics can display dirty ranges and cache hits

## Golden fixtures

Keep small fixture projects under version control.

Suggested:
- `fixtures/minimal_kick.toml`
- `fixtures/basic_drums.toml`
- `fixtures/event_sends.toml`
- `fixtures/kick_ducks_bass.toml`
- `fixtures/reverb_tail_dirty_range.toml`
- `fixtures/nonlinear_barrier.toml`

## Property tests

Useful properties:
- dirty range always includes edited event frame
- tail expansion never shrinks dirty range
- command application either succeeds atomically or fails without mutation
- fingerprints change when semantically relevant inputs change
- fingerprints do not change when irrelevant display-only fields change
- all rendered samples are finite

## Performance tests

Example 34's self-contained serializer acceptance is
`cargo test -p meldritch-control --test native_project`. Its isolated child process
receives only authored Lua and expected raw PCM/fingerprint: the original root is
removed before launch. Check full/split oscillator rendering, deleted-note identity
counters, independent bindings, optional filter defaults, Plaits authored settings
(without exact RNG/PCM claims), empty/unbound palettes and runtime-overlay exclusion.
Negative cases cover bounded source/material, unknown fields/kinds/versions,
duplicate identities, dangling or incompatible wiring, malformed note/clock/value
data, unavailable Lua facilities and instruction budgets. Legacy source cannot
silently lose tracks/assets/DSP/controller state through v2 export. These tests
establish serialization/reconstruction. Additional store tests cover atomic native
save/reopen, failure retry without overwriting foreign temporaries, external edits,
symlink refusal and native/legacy ambiguity. CLI/native-host tests cover read-only
entry checks, inspector initialization without history/capture, initial worker and
production callback sound, saved revision status and runtime default restoration.
Real 80×24 terminal smoke covers native construction/binding/save/quit/fresh reopen.
`cargo test -p meldritch-cli native_keyboard_workflow` now composes actual key
lowering, typed commands, worker publication and the production callback without
opening a device. It checks mode continuity, gate disconnect/reconnect, published
duration/waveform changes, exact mono-to-stereo duplication, runtime mute, authored
save, failure/retry and an isolated fresh-process native entry reload. Full/split
oscillator PCM and source fingerprints must match. The native capture suite
reconstructs source/runtime/PCM from an embedded opening baseline after two saves
and removal of the original root; missing/forged/oversized baselines reject.
These establish bounded headless example-34 acceptance. They do not prove physical
listening, timed replay, Plaits RNG reproducibility or continuous DSP migration.

Example 35's scoped pattern checkpoint uses `cargo test -p meldritch-control
--test palette_routing`: native two-pattern key routing, cursor/note selection
semantics, failed selection/clock diagnostics, shared-patch signal/gain independence,
lossless native save/reopen and typed capture reconstruction. `cargo test -p
meldritch-cli native_palette_selection` proves actual host generation/publication,
runtime gain and advancing callback continuity. `cargo test -p meldritch-tui
native_pattern_selection` checks visible selection/hints in an 80×24 backend.
These pattern checks alone do not accept the whole example.

`cargo test -p meldritch-control --test compositions` checks project-v3 membership,
v2 compatibility, explicit selection/bounds/negative references, inactive binding
validation, typed composition capture reconstruction and two isolated fresh-process
reloads with full/split exact oscillator PCM. A direct v2 member-only compilation
must match each named mix. `cargo test -p meldritch-cli native_composition_transition`
checks pending Play, acknowledged Stop, prior-publication preservation, prepared
restart, inactive gain exclusion/retention and callback audio without a device.
`cargo test -p meldritch-tui native_named_compositions` checks visible composition
IDs and guarded native keys. `instance-keys.txt` adds native creation/selection of
two instances sharing a patch, exact-ID bindings/gains, idempotent membership,
unknown-reference/pending-transport rejection and capture reconstruction. Edited
membership also reopens in the isolated children; worker tests compare its exact
PCM while retaining inactive gains. TUI checks a single selected control and an
explicit inactive selection. The completed example-35 audit executes every
`invalid/audit.toml` case, checks shared patch edits across both compositions and
runs `keyboard-workflow.toml` from an empty root through playing shared edits,
worker/callback publication, save and isolated fresh-process reopen for each
selection. Its ACCEPTANCE.md maps every requirement to executable evidence.
Continuous DSP migration and physical listening are not proved.

Example 36's tremolo checkpoint uses `cargo test -p meldritch-control --test
graph_lfo` for independent per-frame oscillator/LFO/gate references, 60/120 BPM,
Hertz/fractional-beat timing, adjacent cabled reset, no implicit loop reset,
shared fan-out, per-instance reset origins, full/split equality, invalid graph/
settings/scope/cadence/bounds, typed native drafts/capture and an isolated persisted
child. `cargo test -p meldritch-cli native_graph_lfo` verifies worker/production
callback rate changes while playing, exact stereo PCM, invalid-unit rejection
without publication/history changes and authored saving. Callback code is unchanged;
the control plan allocates during worker compilation and evaluates bounded stack
arrays during worker rendering. Neither these checks nor finite PCM looping prove
continuous modulation state across publications or physical deadline/listening
acceptance. Vibrato tests also execute pitch-depth drafts, shared amplitude/pitch
fan-out, positive/zero/negative semitone references, capture command roundtrips,
isolated full/split reopen, and every invalid/pitch.toml case with source/history
continuity. The worker test adds the pitch adapter while playing and checks PCM.
Full example-36 status stays design until the remaining contract passes.

The filter checkpoint tests explicit Hertz scaling/clamps at positive/zero/negative
gain against independent coefficient/integrator recurrence, filter tails, typed
draft/settings/capture roundtrips, unit metadata and isolated full/split reopen.
Every invalid/filter.toml case preserves source/history. Cabled cutoff overrides
reject. The worker test combines tremolo/vibrato/filter while playing and verifies
exact stereo publication and invalid-input source/history/publication continuity.
The legacy automated-filter regression test rejects graph controls on its distinct
historical lowering, while the unchanged example-02 automation render still passes.
Depth tests execute the vibrato/depth keyboard drafts, attack/decay/release and
retrigger references using existing oscillator/ADSR kernels plus independent
LFO/composition arithmetic, isolated full/split reopen and full typed capture
reconstruction. A disconnected envelope/VCA variant remains audible after depth
is zero. All invalid/depth.toml cases preserve source/history. Playing worker tests
cover combined depth/pitch/filter publication and wrong-amount-unit continuity.
Rate tests independently integrate phase for sine/triangle/saw/square at each
sample, with tempo/Hertz source clocks and explicit gate reset before evaluation.
Typed keyboard drafts, full/split isolated reopen and capture reconstruction must
produce identical source and PCM. Every invalid/rate.toml case preserves source
and history, including out-of-range declared clamps and typed rate feedback.
The playing host combines two Hertz adapters for rate and cutoff, plus depth and
pitch fan-out, and checks that feedback rejection preserves published PCM.
`cargo test -p meldritch-cli keyboard` also exercises native MIDI step entry:
velocity/all channels, repeat selection, hidden/performance gating, end clipping,
mono overlap rejection, invalid message fixtures, stale epochs, worker/callback
publication, native capture reconstruction and save/reopen. Pure transport tests
check unique matching and bounded queue overflow. CLI parsing rejects keyboard
input with --check and invalid CC ranges. Physical MicroLab discovery/connection
was observed separately; these tests do not accept key-held audition or gestures.

Add later:
- offline render throughput
- worker saturation
- cache hit rate
- memory bandwidth hotspots
- callback time budget
- missed artifact count

## Philosophy

This project is graph/compiler-heavy. Test graph correctness early or every later feature becomes fog.
