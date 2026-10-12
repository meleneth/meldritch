# Architecture

## Architectural direction and status (2026-10-11)

MELDRITCH's next architecture is an evolution of its authoritative Rust model,
not a parallel engine. [FEATURES.md](FEATURES.md) owns product requirements,
[INVARIANTS.md](INVARIANTS.md) owns enforceable constraints, and
[ROADMAP.md](ROADMAP.md) owns the proposed acceptance sequence. All behavior in
this section beyond the named existing boundaries is **unimplemented**.

The strongest foundation is examples 25–33: typed source edits, detached readonly
projections, compile-before-accept patch batches, controller pickup, background
rendering and base-bound deterministic Lua persistence. Example 29 is accepted
headlessly for its bounded composed workflow, not just its earlier parameter
foundation. It cannot open an empty directory or save a standalone palette.
The first example 34 checkpoint adds `load_project_directory`/`ProjectDirectory`
around the existing importer, `ProjectOverview` as a detached app projection,
and no-subcommand empty startup. It creates no seed, files or devices. Native `n`
creates an in-memory silent palette patch through the existing typed batch.
Legacy roots use the same song cockpit; native palette authoring/binding/playback
compose through the same source/compiler/host. Standalone persistence remains missing.
Examples 31–33 add sampler-slot authoring and standalone Plaits engines without
changing those ownership boundaries. Physical acceptance is still separate.

### Project boundary and compatibility

A project owns a palette of definitions, dependency resolution, compositions,
control declarations and captures. A composition references definitions and
instantiates them; runtime instances own transport, selected material, control
overlays and active graph revisions. A track is a routing/mixing role in a
composition, not an instrument or a pattern's permanent owner.

Introduce project-root loading around the existing semantic importer rather
than rename every `Song` symbol. `load_song_directory` and `tui-song` retain their
historical `main.mlperformance` behavior. A project opened without a selected
composition is valid and silent. With legacy `main.mlperformance`, adapt it to
one composition and its referenced palette; do not move files or change hashes.
Later multiple compositions require explicit selection when ambiguous, not
filesystem-order guessing. Captured session documents are never compositions.

Empty-directory state should exist in the same Rust definition model before
files exist. Do not generate a secret seed song or repurpose the unrelated
backing showcase pattern as project truth. `AppController.project_source` now
owns immutable `base` provenance and the accepted `source` revision;
`InstrumentEditor` holds selection, presentation and staged patch operations.
`enable_project_source` loads executable definitions without creating an
instrument panel. Pattern, parameter and patch commands use that shared owner.
The owner still contains a legacy `ValidatedSong`: empty definitions use empty
scheduling metadata (120 BPM/48 kHz), not a selected composition. `PatchCreation`
adds a mono palette definition through `preview_patch`/`replace_patch`; renderer
`validate_palette_synth` reuses voice lowering without a pattern/track. Native
`n` expands an inspectable oscillator/ADSR/VCA/output recipe. The empty cockpit
uses the existing app/controller with empty, sample-free scheduling scaffolding;
that legacy coordinator pattern is never project truth or persisted.
`SelectInstrument` attaches the accepted editors without replacing provenance;
new definitions have no imported defaults to canonicalize. `create_palette_pattern`
adds independent NotePatternDefinition material. `PatternMaterial` and the native
palette inspector carry no track field; legacy AuthoredPattern/export adapters
still require one track. NoteEdit and the existing frame/counter validation are
shared, while unconnected patterns reject sampler slots. Dirty track ranges
follow explicit current pattern references; unconnected patterns dirty no audio.
`song/voices.rs` adds bounded mono `PatternVoiceDefinition` instances to the same
accepted source, independently binding pitch/gate and declaring length/loop,
initial pitch and output level. Native v/p/g author and inspect them without
making patterns track-owned. `compile_pattern_voice` reuses the ordinary
`CompiledNotePatch` oscillator/envelope/filter loop and existing mixed renderer;
there is no new scheduler. Gate ends and loop gate reset precede held-pitch
updates, which precede gate rises. Pitch holds across ends and loop boundaries;
unbound pitch holds its explicit initial value, unbound gate is low. Velocity
does not implicitly travel through either binding. Simultaneous pitch starts,
overlapping gates, sample-event translation, clock mismatch and polyphonic
instances diagnose instead of gaining hidden combination semantics. Legacy
compilation and fingerprints are unchanged when no new instances exist.
CLI `project_playback.rs::NativeProjectAudio` adapts accepted root revisions to
the existing `SongAudioHost::replace_source`, rerender worker and generation-tagged
immutable publication. TUI `ProjectPlaybackHost` keeps devices outside presentation;
Space opens output only after an explicit instance and prepared current audio.
This initial native stream has a fixed stereo, one-bar PCM layout; other clocks
diagnose before submission. Mono renders promote through the existing host.
Mode changes reuse CockpitMode and preserve source/publication/transport;
performance blocks native construction gestures. Device setup failure retains
authored memory, but a failed stream setup consumes the current engine and cannot
retry in that cockpit. Physical listening/deadlines remain unverified. Runtime
output-level overlays are implemented. `SynthDefinition.output_level_control` is an
explicit patch-owned declaration; the effective control key is (instance ID, control
ID). `AppController.output_levels` owns runtime gains; typed SetOutputLevel validates
the declaration and finite 0..1 value before changing runtime/history. Source Arc,
fingerprint and defaults stay unchanged. `SongAudioHost::set_voice_levels` updates
the existing worker's live overrides; the mixed renderer replaces only that instance's
output gain, with independent identities from legacy lane gains. Construction patch
edits preserve the declaration/overlay. Loading resets overlays; mode changes do not.
This additive graph-snapshot field leaves historical .mlsynth files, hashes and empty
declarations unchanged. Native disk capture now reuses PerformanceSessionCapture
and its timestamp/reservation/TOML/session categories. App CommandRecord retains
accepted results, observed control frames and source fingerprints before/after.
`native_capture.rs` is a bounded serialization/observation adapter, not a second
source model or scheduler: decoded commands dispatch through AppController.
The first accepted command starts capture; native append checkpoints copy existing
events with fixed buffers, retain at most eight pending events and atomically rename
complete files. A failure preserves pending events/last good file; native TUI hooks
defer subsequent gestures/clean quit until checkpoint retry succeeds. Legacy writer
mode/encoding remain unchanged; native payload/version metadata is additive.
Headless reconstruction includes authored and runtime state after history eviction;
timed replay, u64/epoch sessions and rotation remain proposed.
`control::native_project` now adds bounded project-v2 Lua serialization over
`dsl::NativeProjectDefinition`. That DTO projects the existing accepted source;
restoration reuses graph import, stable note validation and voice relationships,
then control validates every palette patch and the complete executable composition.
Export checks full restored-source equality and Lua round-trip equality, rejecting
unsupported legacy state instead of dropping it. Tests prove fresh-process raw
oscillator PCM and authored identity equality without original-root dependencies;
runtime overlays are excluded. `NativeProjectStore` owns atomic disk replacement
at `main.project.lua`, bounded read-only entry discovery and expected-byte conflict
checks. CLI selects native versus legacy musical entries before devices; ambiguity
rejects. App native loading restores inspector selections without recording fake
gestures; the native host submits loaded voices to the existing worker immediately.
TUI Ctrl-S/F6 invokes the host save boundary in both modes and inspectors, while
saved/unsaved status compares accepted source to the last successful save.
Capture remains separate. Native session headers embed the opening project-v2/v3
source before filesystem creation; bounded `load_native_capture_baseline` restores
it through the same importer/compiler and checks its fingerprint. Later saves and
root removal cannot replace that baseline. A composed native-key/worker/callback
test and isolated fresh-process reload establish headless workflow acceptance.
Reconstruction uses existing typed dispatch; timed replay is unimplemented.
Example 35 adds app `SelectPalettePattern` and native PageUp/PageDown navigation
over independent palette material. It changes only inspector selection and is
captured with explicit identity; p/g still authors each binding separately.
Worker generation/publication and runtime gains stay unchanged. The shared-patch
fixture reuses the existing independent-signal compiler and per-voice overlays.
Named compositions now live in the same `ValidatedSong` as a map of
`CompositionDefinition { id, voices }` and an explicit authored selected ID.
`voices` references the one project-global instance registry; patch/pattern
definitions are never copied. `active_pattern_voices` projects membership and the
existing mixed compiler validates inactive instances before excluding them.
Project-v3 saves all definitions and membership/default selection; v2 retains its
implicit full list and rejects named fields. Canonical IDs/membership and selection
participate in semantic fingerprints.

App `CreateComposition` / `SelectComposition` / `SetCompositionMember` are construction actions: validate
and compile before acceptance, reject pending Play or unacknowledged Stop, and
retain instance runtime overlays. Native b/F7/F8 lowers to those commands. The
existing worker prepares finite PCM while stopped; NativeProjectAudio checks
source fingerprint and ready generation before restart. Publication stays on the
prior revision until preparation succeeds. Inactive runtime gains are retained in
the app and filtered from the current worker submission. Neither DSP-state
transfer nor seamless graph/interface activation is claimed. Native a/d edits
selected membership idempotently through the same stopped candidate compiler.
`SelectPatternVoice` owns app/editor selection by project-unique instance ID,
independent of patch and pattern inspectors. Creating an instance selects it;
membership/composition changes preserve that identity, including inactive selection.
Performance Up/Down explicitly chooses active exposed controls, and level gestures
require the exact selected instance to be active. No unrelated control is silently
selected. Selection captures through typed history without altering source/audio;
fresh opening selects the first active ID without recording synthetic gestures.
Native hardware pickup/mapping and general revisioned dynamic surfaces remain open.
Current SelectComposition explicitly changes the authored default. Future live
composition activation must use a separate runtime/scheduled action and cannot
silently reuse that persistence effect. This static membership checkpoint is not
the complete metapattern scheduling model.
It does not provide general dynamic surfaces or
stateful DSP migration.
Compilers may receive a selected composition projection, but that view
must not become another mutable palette.

Git may be required installed. Opening local music must not need `.git`, a
commit, network access, initialization or a manifest. Authored metadata is
created when saving needs it. Git operations belong on the host/control plane;
MELDRITCH must not auto-commit, reset or rewrite a musician's repository.

### Construction, performance and promotion

Four owners are explicit: authored project definitions; active runtime instances;
captured accepted events; explicitly promoted authored changes. Existing
`CockpitMode::{Performance, AllParameters}` controls density, not this semantic
distinction. Preserve Ctrl-Tab behavior while adding deliberate construction/
performance interaction state. Navigation/mode changes do not rebuild audio or
reset transport, selections, queued launches, parameter state or pickup ownership
unless an actual binding context changes.

Existing `EditInstrument`, `EditNote`, `EditPatch` and assignment `EditParameter`
are construction edits. `SetCuratedControlNormalized`, lane selection/mute/solo and
`SongLiveOverrides` are runtime precedents. In performance mode a fader aimed
at an authored parameter must write a validated instance override, not call the
construction edit by accident. An explicit promote command validates an absolute
snapshot or selected captured trajectory against the authored base. Journals
remain immutable facts; neither save nor replay silently promotes them.

### Patterns and one scheduling path

Retain `.mlpattern`, `NoteEventDefinition`, stable authoring note IDs and the
existing event/frame compiler. Remove permanent track ownership through explicit
composition bindings in new examples, while the old track pattern list adapts
to the same bindings. A note source lowers to independent pitch, gate, velocity
and event outputs. A minimal monophonic contract holds pitch between starts,
uses end-exclusive gates, and declares initial pitch, disconnected gate behavior,
retrigger and same-frame ordering. Polyphonic outputs additionally require
occurrence/voice identity; do not flatten chords to one global pitch wire.

Extend `compile_note_track_for_pattern` to lower those bindings into the existing
compiled plan. Its current imperative renderer sets frequency and triggers an
envelope from the same event. Extract that scheduling into independent compiled
streams; do not add a widget-local sequencer or another event loop. Reuse the
core's `Pattern::events_between`/`Arrangement::events_between` timing contracts
where applicable, with explicit adapters between legacy steps and authored
duration notes. Their semantics differ: legacy step gates clip to the next step.

Transforms operate on identity-bearing occurrences (definition, instance,
event, loop epoch, transform path and generated ordinal). Transposition retains
time/identity; ratcheting derives child identities and bounded expansion;
probability commits a seed/outcome per occurrence. Offsets declare whether they
shift gate ends and how negative times/preroll are handled. Event parameters are
sampled at an authored evaluation boundary; continuous audio-rate modulation is
not silently the same contract. Stable ordering resolves same-frame gate ends,
starts, resets and parameter changes before consumers evaluate; exact ordering
must be specified by the first executable example, including adjacent retriggers.

### First-class modulation and normalization

Reuse `modulation::Lfo` waveform/seed kernels; its absolute-frame `value_at` is
not sufficient for modulatable rate, reset or per-instance phase. Variable rate
requires deterministic phase integration/checkpoints. Shared, per-pattern and
per-voice scopes allocate different instances of the same algorithm; fan-out
reads the same instance. Define free-running versus beat-synchronized phase,
start/stop/rewind/reset priority, reset edge latency and tempo-change integration.

Port descriptors must eventually carry family, units/domain, rate and scope.
Audio-rate continuous consumers, fixed control cadence and event-sampled
probability use explicit evaluation contracts, interpolation/latching and
smoothing. Add affine scaling/offset, clamping, pitch/unit adapters and declared
sum/product/select combiners as inspectable graph operations. Modulator rate and
depth become compatible inputs, without destination-specific code. Feedback
must pass through a specified sample/block/event delay; do not relax cycle checks
without executable delay semantics. Control dependencies, seeds, rate/scope,
reset history and combiner policies participate in fingerprints/invalidation.

Legacy `ModulationMatrix` explicitly sums/clamps a fixed catalog. Keep that
behavior for historical showcases and lower it to explicit combiners when
exposing it as graph data; do not transfer implicit summation into new cables.
Example 36 now loads Lfo/ControlScale/ControlMultiply in the ordinary synth graph.
`control_graph::control_order` validates a bounded control DAG, unit compatibility,
explicit reset and reachability to VCA level. `song_control::ControlPlan` lowers
indexed dependencies and reuses the static-rate Lfo kernel; a 33-value/32-origin
worker stack evaluates sources once per sample/instance. Gate rises set that
LFO's origin before evaluation when the authored reset cable/policy requires it.
End/reset, held pitch and gate rises retain the existing voice ordering. Graph
settings enter the semantic fingerprint; old absent settings add no hash input.
Typed configurations reuse PatchEdit, capture and whole-project save/reopen.
The vibrato checkpoint adds PitchModulate to the same dependency order: one
explicit input.pitch → adapter.pitch → oscillator.pitch path combines absolute
Pitch with bipolar Control times depth_semitones. Lowering evaluates the offset
once per sample and converts the combined pitch to oscillator frequency; gate
state remains independent. Fan-out shares the LFO with the amplitude path.
Plaits pitch lowering and adapter chains reject until supported.
ControlHertz converts unit-interval Control through explicit gain/offset/clamps
to Hertz Control; a single low_pass.cutoff cable owns the filter frequency.
ControlPlan lowers that adapter alongside amplitude/pitch dependencies and passes
its per-sample value to the existing stateful filter. The configured cutoff is an
inactive fallback while cabled; overrides reject. Uncabled defaults/overrides and
filter physical limits remain compatible. Authored normalization is still open.
The historical automated-filter renderer keeps its distinct fixed audio order;
it rejects graph controls/cutoff cables until those combinations have a supported
lowering. It must not discard connections or run a parallel graph implementation.
ControlAttenuate adds bipolar control × unit-interval amount → bipolar control.
It lowers to the same product kernel as ControlMultiply with different validated
units, without changing historical unit-interval multiplication. ADSR may shape
shared modulation depth while the separate VCA connection determines articulation;
neither the LFO nor attenuator knows a destination. Fan-out evaluates that source
once per sample/instance. No new event engine or implicit combination is involved.
Lfo.rate accepts Hertz through the same ControlHertz adapter, whose declared
clamps must fit 0.001..100 Hz. Control dependencies include rate cables and reject
cycles. A 32-phase worker stack integrates cabled rate: reset authored phase first,
evaluate current phase, then advance by this sample's Hertz/sample-rate. Static
rate retains the absolute-frame kernel. Multiple Hertz adapters may feed distinct
consumers; cutoff lowering resolves its actual cable rather than the first adapter.
`song_render::mono::MonoState` now holds the shared mono DSP execution kernel.
Offline rendering retains its pre-roll/scheduling behavior; LiveMonoRenderer
advances that same state through 1..4096-frame worker blocks with at most 128
prevalidated independent Pitch/Gate events. Absolute event frames/cursor are u64;
each fresh instance's modulation age begins at zero at its explicit origin.
State survives block boundaries, but this does not migrate state across graph
revisions. Invalid blocks cannot partially change oscillator/envelope/filter/control
history. Runtime input ownership, scheduling, streaming publication and device
audition remain open; the current native device host still loops finite PCM.
Control ModulePort metadata exposes bipolar/unit-interval units and audio cadence;
scope is explicitly per instance. This is a bounded amplitude/pitch-control extension,
not general audio DAG execution. Low-pass cutoff is now a typed signal input;
Plaits timbre remains an editable target. Event cadence, global/per-voice scopes
and seeded graph randomness remain open. Legacy matrix summation/clamping and
its seeded waveform behavior are unchanged.

Normalization is patch-authored policy resolved into active connections, with
current values projected separately. Static normal routes use explicit priority
and replacement/disable semantics. Dynamic selectors/switches are executable
components. Multi-input module ports may exist, but arbitrary cables to an
ordinary input never imply sum. Current one-cable-per-input rejection is a
supported subset restriction, not a permanent musical invariant.

### Transactions and transition truth

Extend example 30's `PatchEdit` candidate/compile boundary. Add typed transaction
identity, expected project/instance revision, stable cable identity, authored
normalization and control-interface changes only as examples need them. Validate
the whole candidate, including references, bindings, topology, rates, budgets and
surface identities. Intermediate drafts may be incomplete. Operation-local
checks currently require endpoints to exist and inputs to be freed before a
connect; a future unordered declarative candidate must not inherit accidental
operation ordering as musical semantics.

Acceptance, preparation, publication, activation and callback consumption are
different stages. Latest-wins replacement suits refreshable knob previews;
it must not silently erase already accepted scheduled structural gestures.
Quantized revisions need an ordered bounded activation queue, explicit cancel/
supersession facts and a missed-boundary policy. Recommended first policy:
keep the last valid graph, report the miss and record rescheduling to the next
boundary. Never claim a requested frame as the actual activation frame.

Current `SongRerenderWorker` renders finite PCM from zero; callback publication
preserves transport position but does not migrate a running oscillator phase,
ADSR, filter history, delay buffer or gate lifecycle. Pre-roll/split equivalence
proves reconstruction within one definition, not continuity across revisions.
Stateful activation needs compatible stable instance IDs plus module-version/
configuration compatibility. Define preserve, explicit reset, drain-tail or
crossfade per module; new graph states must be bounded and prepared off realtime.
Open gates need explicit continuation/release and voice-allocation rules.
Invalidation includes migration/reset inputs, lookahead and old/new tails.

Prefer worker-side state checkpoints and segmented artifact rendering first,
consistent with render-ahead architecture. A later bounded realtime DSP executor
is a separate decision requiring deadline evidence, not a prerequisite for empty
projects. Crossfading PCM can mitigate clicks but does not prove DSP continuity.
Activation tests must span revisions, callback block boundaries and partial
renders, with no old-tail loss or stale completion rollback.

### Patch interfaces and metapatterns

Separate typed signal ports, construction operations and patch-declared
performance controls. A control's semantic ID is namespaced by definition/
instance and stable across layout changes. Macros expose their transforms and
targets; recipes expose module/cable/control expansion in the same transaction.
Graph and surface publish one coherent revision. Removed controls become
unavailable; stale actions reject; controllers rearm when target meaning/range
changes. A Plaits model may change descriptions without reusing another
parameter's identity. Lua widgets present readonly projections, not musical truth.

Composition metapatterns compile to bounded scheduled typed actions using the
existing launcher/futures/phrase facilities. They can launch patterns, alter
bindings/modulation/mixing and request graph transactions. Loops/conditions
have declared expansion limits and evaluation boundaries; no unrestricted Lua
executes in the callback. Specify stable same-frame order, nested metapattern
instance IDs, cancellation, readiness and actual activation facts. Capture must
include base/dependencies, command IDs, instance/epoch, requested musical time,
accepted sequence and actual activation. Harvest selected events into a new
composition through typed validation; never mutate the original capture.

### Libraries and reproducible resolution

Keep local confinement; external dependencies resolve through an explicit host
resolver into pinned immutable roots, not by allowing arbitrary path escapes.
Distinguish copy/import, external reference, local instance override and update.
Use definition identity plus content fingerprint and pinned full Git commit;
pin transitive dependencies and asset contents too. A lock records resolver/
format versions, source locator, commit, content digest and dependency closure.
Existing semantic FNV fingerprints are useful cache keys, not secure dependency
integrity hashes; choose a content digest in the library slice.

Git checkout/cache work occurs off realtime. A local cached pin works offline;
missing/incompatible/dirty pinned contents diagnose before compilation. Never
follow a moving branch during ordinary load. Update computes a new lock and
validates affected project candidates before explicit adoption. Copied assets
are portable; externally referenced samples require pinned content/location and
an explicit bundle/copy workflow. Submodules can supply a pinned root but are
not mandatory. Lockfiles are unnecessary for an empty dependency-free directory.

### Architectural gap analysis

Paths below are under `crates/`. Symbols name current code; reuse does not imply
the missing behavior is implemented. Required tests are acceptance obligations.

| Desired capability | Existing implementation / reuse | Missing boundary and risk | Required acceptance |
| --- | --- | --- | --- |
| MIDI keyboard input | control `devices::{decode_keyboard, MidiInputQueue, keyboard_step_command}`; CLI `keyboard::NativeKeyboard`, `midi-keyboard-check`, native host poll | Explicit unique input matching, bounded queue/reconnect epochs and construction palette entry reuse typed note commands, capture and publication. MicroLab host discovery/connection observed. Runtime live sources, voice lifecycle, sustain/bend routing and timed recording are missing. | Headless velocity/channel, mode/editor gating, clipping, mono overlap rejection, invalid messages, stale epochs/overflow, publication and save/reopen/capture reconstruction pass. Physical gestures, latency/listening and live note/gate cleanup still require their own acceptance. |
| Empty project / palette / compositions | DSL project/palette/voice/CompositionDefinition and NativeProjectDefinition; app `new_empty_project`, native source loading, composition commands/`SelectPatternVoice`; TUI authoring/binding/save and `ProjectPlaybackHost`; CLI `open_project`, `NativeProjectAudio`; control `SongAudioHost::replace_source`, `NativeProjectStore`, `load_native_capture_baseline` | Self-contained v2/v3 saves, named membership/default selection and native ID-based multi-instance authoring reuse the existing source/host. Avoid hidden templates and wholesale renaming. | Examples 34/35 are headless accepted, including empty-root independent-source/shared-patch/two-composition authoring, shared edits, playing callback publication, negative contracts and fresh-process reopen. Interactive palette deletion, physical listening, timed replay and seamless activation remain open. |
| Authoritative state and modes | `meldritch-app/src/project.rs`: `ProjectSource.base/source`, `enable_project_source`; `lib.rs`: `AppController::dispatch`, `CockpitMode`; render host `SongLiveOverrides` | Accepted source is app-owned independently of an instrument panel; native editors mutate the same source and preserve import provenance. Mapping faders still write authored values. Separate interaction mode and instance overlays without duplicate state. | Mode switch keeps audio/transport; performance moves leave authored fingerprint/export unchanged; explicit promotion reconstructs edits; failed promotion is atomic. |
| Patch scope / independent signals | DSL `SignalType`, `ModuleKind`, `resolve_port`, `CableDefinition`, `PatternVoiceDefinition`; render `compile_pattern_voice`, `CompiledNotePatch::render_with_filter_overrides` | Mono composition bindings execute independent pitch/gate; legacy track notes keep coupled articulation. Only four signal families and fixed voice topology execute; general graph execution remains missing. | Held pitch under an independent gate and split rendering are proved; non-amplitude envelope, broader graph execution and other families remain required. Unsupported execution must reject, never silently ignore cables. |
| Patchable patterns / transforms | DSL `NotePatternDefinition`, `NoteEventDefinition`; `song/notes.rs`: `PatternMaterial`, compatibility `AuthoredPattern.track`; `song/voices.rs`: separate bindings; core `Pattern::events_between`, `EventOccurrence` | Native palette patterns have no track owner and independently bind mono inputs. No event-transform graph, rate adapters or polyphonic source contract. Legacy step/duration semantics differ; occurrence identity must survive transformations. | Independent gate source, initial/held pitch, adjacent retriggers, multi-composition reuse/fresh reload and clock/chord/overlap rejection are checked. Transformed identities and probability/ratchet bounds remain open. |
| LFOs / rates / combination | Existing `modulation::Lfo`/`LfoRate`; DSL `control_graph`, ModuleKind Lfo/ControlScale/ControlMultiply; render `song_control::ControlPlan`; typed patch configurations and persistence | Mono VCA control DAG now executes explicit units/audio cadence/per-instance scope, static rate, fan-out and cabled reset. Explicit oscillator PitchModulate now combines bipolar Control with independent Pitch. Explicit Hertz cutoff adapter now executes in the same plan/filter worker. Cabled Hertz rate now integrates phase in the same bounded plan, with reset-before-evaluation ordering. Plaits parameter ports, event/control cadence and broader scopes remain missing. Legacy matrix stays separate/compatible. | Example 36 checks oscillator/phase/gate references, tempo/Hertz/split rendering, fan-out and separate reset origins, invalid graphs/units, typed edits/capture, worker publication and fresh reopen. All four rate-modulated waveforms, reset, capture and playing feedback rejection are checked. Broader scope/cadence/delayed feedback and physical tests remain open. |
| Normalization | Explicit synth inputs/cables and single occupied-input rejection in `song/patch.rs` | No authored policy/resolution. Blending defaults would hide a second graph. | Default route, override, disable, save/reload; combiner required for mixing; dynamic selector's decisions visible/fingerprinted. |
| Atomic graph revisions | DSL `PatchEdit`, `preview_patch`, `replace_patch`, `edit_patch`; app `edit_patch`, `patch_submit`; tests `patch_authoring.rs` | Bounded clone/compile transaction exists (32 modules/128 cables/128 operations), keyboard stale-base protection; no general transaction/cable IDs, normalization or interface revision. | Complete candidate succeeds despite incomplete draft; failures/stale bases keep source/history/audio; concurrent interface and graph update; preserved IDs and deterministic replay. |
| Stateful activation | render `SongRerenderWorker`; audio `AudioSnapshotPublisher::publish`, `PublishedAudio`; control `SongAudioHost` generations | Full finite rerender, fixed channels/frames/chunk layout, u32 PCM horizon, no live DSP migration/crossfade. Latest-wins cannot be a structural action log. | Quantized readiness/miss/cancel; preserve oscillator/envelope/filter checkpoints, drain delay tails, release removed gates; stale workers cannot roll back; allocation/deadline checks separately. |
| Patch-owned dynamic controls | DSL `CuratedControlDefinition`; app `AssignmentDefinition`, `EditableTarget`; control `ControlPlane`, `MidiPickup`, readonly projections | Static composition catalogs/device declarations; numeric assignments; no typed dynamic patch interface/macros/recipes. Stale mappings could retarget incorrectly. | Stable IDs after layout change; removed target unavailable; graph/interface same revision; pickup rearms, old input leases reject; multi-target action commits atomically. |
| Metapattern composition | core `Arrangement`, `ArrangementSection`; render `phrases::{PhraseBank,PhraseFutureLibrary}`, `futures::{PerformanceLauncher,LaunchQuantization}` | Showcase arrangements/quantization exist but loaded-song lane launches rerender immediately. No unified higher-order typed action timeline. | Bounded nesting/conditions, deterministic same-frame order; scheduled binding/graph/control activation; late work policy; composition selection shares palette without unintended mutation. |
| Capture / harvest / persistence | control `project::{encode_project,load_project,save_project}`, instrument/pattern/assignment envelopes; `session::PerformanceSessionCapture`; typed Lua `Journal` and replay | Project v1 is base-bound selected synth/pattern + controls, not a directory project. Legacy capture uses debug strings/u32 and retains history. Lua replay proves state, not timed PCM. | Complete independent saves, unchanged-source text; rejected/omitted edits cannot vanish; typed u64/epochs >u32, crash/retry; exact supported replay and non-destructive harvest; RNG exceptions declared. |
| Libraries / assets | Song-root confinement, IDs, semantic fingerprinting, pinned Plaits Cargo dependency | No project dependency resolver/lock or external definition instances; moving upstream assets can break reproducibility. | Copy versus pinned reference; offline cached resolution; upstream change does not change music; missing/wrong digest/version and override conflict reject; portable asset bundle. |

Unresolved decisions to settle with examples: multi-composition directory layout
and selection beyond the bounded native entry; polyphonic signal identity;
modulation cadence and smoothing; normalization priority vocabulary; state
compatibility descriptors and resource budgets; simultaneous metapattern/live
action precedence; library digest/override merge rules. The requirements above
fix semantics and ownership without inventing unsupported schema fields.

## Existing implementation and historical design sketches

The sections below combine implemented host descriptions with original design
sketches. Traits and pseudocode are not claims of a generic executable DSP DAG;
the audit above and accepted examples determine support. Fixed topology is an
initial implementation strategy, not the future expressive limit.

The pinned [Plaits module](PLAITS.md) uses all 24 standalone upstream model choices
on the existing render worker, explicit external envelopes/cables and the normal
atomic publication path. Native f32 and Plaits RNG are authorized exceptions;
all surrounding source/DSP/mix state remains f64. Preferred/default omitted song
and project sample rate is 48 kHz.

Interactive fader/mixer authoring uses DSL `EditableTarget` and `ParameterEdit`
against the existing validated song source. App assignment state owns mappings;
Lua supplies declarations/policy and reads projections. The existing song
renderer applies post-DSP track volume/stereo balance and the existing host
publishes rerenders. Pickup observations and publication telemetry are transient,
separate from musical source and `kind="controls"` authoring snapshots. See
[CONTROL_ASSIGNMENTS.md](CONTROL_ASSIGNMENTS.md).

The immediate authoring roadmap is in [AUTHORING.md](AUTHORING.md). Its target
is a single editable Rust source/relation model, shared typed commands and
readonly projections for TUI/Lua/hardware, with host-owned Lua source export.
The first instrument subset is implemented; see [INSTRUMENT_EDITOR.md](INSTRUMENT_EDITOR.md).
`AppController` owns the editable `Arc<ValidatedSong>` and immutable import
provenance. DSL parameter descriptors/edits preserve source identity and enforce
types, ranges and automation ownership. App commands compile validated edits
before committing a revision. The song host consumes immutable derived source
revisions and invalidates the full render horizon through its existing worker;
Lua and the TUI read projections and dispatch the same commands. The backing
legacy pattern editor does not own these instrument defaults. Broader loaded-song
graph authoring now reuses that same SynthDefinition/importer/compiler through
bounded PatchEdit batches and version-2 Lua graph snapshots; see
[PATCH_EDITOR.md](PATCH_EDITOR.md). Mapping authoring is also implemented. The initial
[piano-roll subset](PIANO_ROLL.md) now shares the same editable song source with
instrument commands. Pattern selection stores UI identity only. Rust validates
typed notes, stable note identities/counters and musical ticks, while Lua owns
transient pad holds, modifiers, grid orientation and pages. Immutable app
projections drive the APC and TUI. Derived render/source snapshots never gain
independent mutation authority. Authored Lua pattern exports validate over the
immutable TOML base and remain separate from performance journals.

The current Lua/control-plane reuse boundaries, API and implementation limits
are documented in [CONTROL_PLANE.md](CONTROL_PLANE.md). Lua composes the existing
validated song model and dispatches existing app commands on a control thread;
audio publication and realtime ownership remain unchanged.

The TOML TUI and optional Lua APC host share `meldritch-control::song_host`
for accepted app results and `meldritch-control::session` for capture. Render
patches, live overrides and the bounded rerender worker live in
`meldritch-render::song_host`. These are control-thread adapters to existing
commands and immutable audio publication, with no second musical state model.

Song snapshot generations now travel with immutable samples. The callback emits
only atomic consumption telemetry; the song host maps retained revision metadata
to `AppViewModel::song_audio` outside realtime. Native TUI and Lua presentation
share that projection. Publication alone never establishes callback playback.

## Workspace shape

Initial Rust workspace:

```text
crates/
  gremlin-core/
    IDs, commands, source graph, relation graph, timeline types

  gremlin-dsl/
    TOML/RON parsing, validation, typed model construction

  gremlin-audio/
    audio buffers, nodes, effects, instruments, realtime engine

  gremlin-render/
    render jobs, artifact cache, invalidation, worker pool

  gremlin-tui/
    terminal UI, command cockpit, inspectors

  gremlin-cli/
    command-line entrypoint, offline render commands, diagnostics
```

Project names can change. Boundaries should not.



## Eurorack-style patch graph

The graph should prefer patchable modules with typed ports.

Convenience features should expand into patch recipes rather than hidden integrated behavior.

```text
recipe: kick_ducks_bass
  -> kick audio to envelope follower
  -> envelope follower to bass duck control
  -> bass audio to dynamic gain/EQ cell
  -> dynamic gain/EQ to bass bus
```

The expansion must remain visible to graph inspection and invalidation.

The compiler validates:
- port compatibility
- signal rate compatibility
- needed adapters
- audio dependencies
- control dependencies
- cache boundaries

## Data flow

```text
DSL / TUI commands
        |
        v
Command processor
        |
        v
Source graph + relation graph
        |
        v
Compiler passes
        |
        v
Render plans + realtime state
        |
        +------------------+
        |                  |
        v                  v
Worker render farm     Realtime callback
        |                  |
        v                  v
Artifact cache       Audio device output
```

## Graph layers

### Source graph

Truth about musical material.

Contains:
- samples
- instruments
- patterns
- tracks
- events
- scenes
- macros
- roles
- buses
- effects

### Relation graph

Truth about dependencies and interactions.

Contains:
- audio edges
- control edges
- sidechains
- modulation rules
- role-priority rules
- routing
- nonlinear/group barriers

### Artifact graph

Derived render products.

Contains:
- rendered audio chunks
- stems
- bus chunks
- previews
- waveform peaks
- spectra
- compiled event tables
- frozen probability outcomes

Artifacts are cacheable and disposable.

### Realtime graph

Small bounded state needed to emit audio now.

Contains:
- output device callback state
- live overlay voices
- current playhead
- current published compiled state
- realtime event queue consumer
- hot cache reader
- fallback renderer

## Commands

All mutations enter as typed commands.

```rust
pub enum Command {
    SetStep(SetStep),
    ClearStep(ClearStep),
    SetTrackRole(SetTrackRole),
    SetRelation(SetRelation),
    SetParam(SetParam),
    ToggleMute(ToggleMute),
    SelectScene(SelectScene),
    CommitProbability(CommitProbability),
}
```

Command result:

```rust
pub struct CommandResult {
    pub changed_entities: Vec<EntityId>,
    pub dirty_ranges: Vec<DirtyRange>,
    pub diagnostics: Vec<Diagnostic>,
}
```

## Node properties

Each graph node declares its behavior.

```rust
pub enum Linearity {
    Linear,
    Nonlinear,
    TimeVariant,
    Feedback,
}

pub struct NodeProperties {
    pub linearity: Linearity,
    pub latency_frames: u32,
    pub lookahead_frames: u32,
    pub tail_frames: u32,
    pub can_cache: bool,
    pub needs_group_signal: bool,
}
```

These properties guide:
- graph optimization
- cache boundaries
- invalidation range expansion
- render scheduling

## Edge types

```rust
pub enum EdgeKind {
    Audio,
    Control,
}
```

Audio edge:
- signal flows into another node

Control edge:
- signal/analysis/event metadata influences another node

Both affect invalidation.

## Audio buffer policy

Use block processing.

```rust
pub struct AudioBlock {
    pub channels: u16,
    pub frames: u32,
    pub samples: Vec<Sample>,
}
```

Planar buffers may be preferable for DSP:

```rust
pub struct StereoBlock {
    pub left: Vec<Sample>,
    pub right: Vec<Sample>,
}
```

Do not dynamic-dispatch per sample.
Dynamic dispatch per node per block is acceptable.

## Effects

```rust
pub trait Effect: Send {
    fn properties(&self) -> NodeProperties;
    fn process_block(&mut self, ctx: &ProcessContext, block: &mut AudioBlock);
}
```

## Instruments

```rust
pub trait Instrument: Send {
    fn note_on(&mut self, event: NoteOn);
    fn note_off(&mut self, event: NoteOff);
    fn process_block(&mut self, ctx: &ProcessContext, out: &mut AudioBlock);
}
```

## Event sources

Patterns compile to event sources.

```rust
pub trait EventSource {
    fn events_between(
        &self,
        start_frame: u64,
        end_frame: u64,
        out: &mut Vec<Event>,
    );
}
```

The realtime path should use preallocated event buffers.

## Render plans

User-facing graph composition compiles into flat render operations.

```rust
pub enum RenderOp {
    Clear { buffer: BufferId },
    RenderSource { source: SourceId, out: BufferId },
    ApplyEffect { effect: EffectId, buffer: BufferId },
    Mix { src: BufferId, dst: BufferId, gain: f64 },
    Analyze { src: BufferId, feature: FeatureId, out: ControlId },
}
```

Benefits:
- easier testing
- easier fingerprinting
- easier scheduling
- less runtime graph wandering
- cleaner cache keys

## Artifact keys

```rust
pub struct ArtifactKey {
    pub node: NodeId,
    pub range: FrameRange,
    pub sample_rate: u32,
    pub fingerprint: Fingerprint,
}
```

Fingerprint includes:
- source revision
- input artifact fingerprints
- effect parameters
- automation slice
- routing/rule state
- random/probability seed
- render mode

## Scheduler

The scheduler operates over wanted artifacts.

```text
wanted artifact
  -> discover dependencies
  -> check cache
  -> enqueue missing/dirty jobs
  -> execute in priority order
  -> publish artifacts
```

Do not generate the entire combination space.

## Worker pool

Use long-lived worker threads.

Workers are for:
- rendering future chunks
- resampling
- analysis
- waveform/spectrum previews
- frozen stems
- likely scene variants

The audio callback must not spawn workers.

## Realtime callback

Expected shape:

```rust
fn audio_callback(output: &mut [f32]) {
    // 1. Determine frame range.
    // 2. Pull hot cached artifact if available.
    // 3. Apply live overlays.
    // 4. Apply final safety limiter.
    // 5. Write output.
    // 6. Increment diagnostic counters.
}
```

No graph compilation. No parsing. No file IO.

## Publishing state

Use immutable compiled state with atomic/lock-free swap.

Possible crates:
- `arc-swap`
- `triple_buffer`
- SPSC ring buffers for realtime events

## First fixed topology

Do not start with fully arbitrary graphs.

Start with:

```text
sources/tracks -> buses -> master
```

Then add explicit relation/control edges.

Only generalize once the fixed topology proves too tight.
