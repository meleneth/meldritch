# Active TODO

Immediate priority: graph LFOs and explicit modulation semantics in section 21 and
[ROADMAP.md](ROADMAP.md). The bounded empty-project and shared-palette workflows
are accepted headlessly.
Sections 20/19 retain accepted work, unfinished work and historical
verification; their older next-step recommendations are superseded.
The completed showcase milestones remain completion history.

## 21. Coherence under composition: project architecture

Requirements: [FEATURES.md](FEATURES.md). Code audit, conflicts and reuse:
[ARCHITECTURE.md](ARCHITECTURE.md). Future/current invariants:
[INVARIANTS.md](INVARIANTS.md). Detailed ordered slice contracts and dependencies:
[ROADMAP.md](ROADMAP.md). Git is permitted as a required installed dependency;
an empty project requires no Git repository or initialization.

- [x] Audit actual source ownership, accepted examples 25–33, persistence,
  pattern/event execution, modulation, typed batches, pickup and PCM publication
- [x] Reconcile project requirements/invariants and preserve historical contracts
- [x] Specify [example 34](songs/examples/34-empty-project-patch/ACCEPTANCE.md)
  as design only; select the immediate implementation prompt in AUTHORING.md
- [x] Slice 1 (headless): empty-directory project, create supported patch/pattern, independent
  pitch/gate bindings, keyboard authoring, play/edit/self-contained save/reopen
- [x] Slice 1 opening checkpoint: root adapter, no-subcommand empty overview,
  explicit headless `open-project --check`, unchanged legacy loader/cockpit,
  no initialization/files/devices on empty open and entry diagnostics
- [x] Slice 1 ownership checkpoint: app-owned provenance/accepted source,
  source-only loading, typed pattern/parameter/patch commands independent of an
  instrument panel; existing detached views and save/reload formats preserved
- [x] Slice 1 palette checkpoint: file-free empty source, typed bounded mono
  patch creation reusing existing batch/importer/voice lowering, native `n`,
  visible independent inputs/modules/cables, silence without tracks/patterns
- [x] Slice 1 native authoring checkpoint: source-preserving instrument selection,
  Ctrl-E/G parameter/atomic patch editing over newly created definitions;
  independent palette patterns and native cursor/insert/select/duration/velocity/
  delete via existing NoteEdit, with unchanged legacy exports/projections
- [x] Slice 1 binding checkpoint: independent mono pattern pitch/gate relationships,
  native v/p/g inspection, shared voice rendering, worker/publication/callback
  headless checks; explicit held pitch and end-exclusive gate fallbacks
- [x] Slice 1 native playback checkpoint: lazy device opening via existing
  SongAudioHost, worker and immutable publication; typed transport and mode
  transitions with native construction gestures blocked in performance
- [x] Slice 1 runtime level checkpoint: explicit patch-owned output-level declaration,
  typed instance overlay, native x/arrows/Home/End; source/defaults unchanged,
  existing worker/callback mute/restore and in-memory accepted facts checked
- [x] Slice 1 capture checkpoint: lazy typed accepted sessions over shared writer,
  per-command source/control observations, bounded pending buffers, atomic
  checkpoints/failure retry, clean quit and fresh-app source/runtime/PCM reconstruction
- [x] Slice 1 serializer checkpoint: self-contained project-v2 Lua over existing
  graph/note/voice validators, whole-source loss detection, stable identities,
  authored-only controls and fresh-process deterministic oscillator PCM equality
- [x] Slice 1 disk checkpoint: atomic main.project.lua save, deterministic native/
  legacy entry diagnostics, authored dirty status, fresh native inspectors and
  initial worker publication without capture/devices; runtime defaults restored
- [x] Slice 1 composed acceptance: actual native keys through worker/callback,
  save/failure/retry and isolated fresh-process reopen; embedded capture baselines
  survive later saves and root removal. No timed replay or physical listening claim
- [x] Slice 2: shared palette, two compositions and independent pattern sources
- [x] Slice 2 pattern-selection checkpoint: typed SelectPalettePattern, native
  PageUp/PageDown without rebinding, visible selection, separate melody/rhythm
  source key workflow, shared patch/two-instance render and independent gains;
  source/runtime/publication continuity, persistence and typed capture checks
- [x] Slice 2 named composition checkpoint: project-v3 explicit membership over
  project-unique instances, native b/F7/F8, stopped selection and prepared PCM
  restart; inactive definition/gain scope, capture and fresh-process save/reopen
- [x] Slice 2 instance checkpoint: native stable-ID multi-instance selection,
  independent bindings/levels and stopped composition membership authoring
- [x] Slice 2 composed acceptance and negative-contract compatibility audit
- [ ] Slice 3: graph LFO, explicit unit/rate/combination and reset/scope contracts
- [x] Slice 3 tremolo checkpoint: explicit LFO/scale/clamp/multiply control DAG,
  audio/per-instance rate/phase/cabled reset, fan-out, typed native drafts/capture,
  worker publication, independent sample/split tests and fresh-process save/reopen
- [x] Slice 3 vibrato checkpoint: explicit PitchModulate combination/depth,
  shared LFO amplitude/pitch fan-out, typed drafts/capture, rejection and reopen
- [x] Slice 3 filter checkpoint: typed Hertz adapter/cutoff port, explicit clamp
  and cable/default precedence, sample/tail/split references and playing publication
- [x] Slice 3 depth checkpoint: bipolar ControlAttenuate × unit-interval amount,
  envelope-shaped fan-out, independent amplitude wiring, capture and fresh reopen
- [x] Slice 3 rate checkpoint: explicit Hertz input, deterministic phase integration,
  reset ordering, independent waveform references, capture and playing publication
- [ ] Slice 3 next: explicit control/event cadence and broader instantiation scopes
- [x] User-prioritized MIDI keyboard checkpoint: MicroLab/generic input selection,
  diagnostic, bounded reconnect queue, construction pattern entry with velocity,
  typed capture/save/reopen and playing publication; host MicroLab connection observed
- [x] Keyboard audition: native F9 explicit stopped-transport mono binding,
  bounded shared-DSP worker streaming, ingress epochs/loss cleanup, typed capture
  and callback PCM/allocation acceptance headlessly
- [x] Bounded keyboard adapters: explicit velocity, bend and GateHold pedal routes,
  headless capture/save/reopen and worker/callback evidence
- [ ] Next keyboard slices: physical interaction/listening/deadlines and broader
  Gate routing; simultaneous arranged/live routing and timed
  device capture/replay need their own activation/clock contracts
- [x] Keyboard renderer foundation: one shared mono kernel for offline/incremental
  worker execution, independent bounded u64 Pitch/Gate blocks, preserved DSP state,
  exact partition/offline references and atomic invalid-block recovery
- [x] Keyboard runtime foundation: typed explicit mono-instance binding, bounded
  channel/key priority, retrigger/fallback/release, reset epochs, capture and exact
  worker replay; source revisions invalidate binding, fresh saves restore defaults
- [ ] Slice 4: authored normalization, atomic recipe and dynamic control identity
- [ ] Slice 5: bounded identity-bearing event transforms in existing scheduling
- [ ] Slice 6: quantized graph activation and proved compatible DSP-state continuity
- [ ] Slice 7: typed u64/epoch capture, timed replay and metapattern harvesting
- [ ] Slice 8: pinned library definitions/assets, local overrides and deliberate updates
- [ ] Record physical interaction/listening and independent deadline evidence

No implementation status for examples 00–33 changes in this planning slice.
Example 29 is currently headless `accept`; older verification paragraphs below
record prior checkpoints where its composed contract was still unfinished.

Architecture/planning verification (2026-10-11): documentation/runtime status
only; no Rust behavior or historical musical fixtures changed. `cargo fmt --all
-- --check`, `git diff --check`, `npm run docs:build` and local Markdown target/
include checks pass. Missing published routes found by the first docs build were
fixed with include-only pages for requirements and example 34; dead-link checking
remains enabled. Offline tests passed: DSL `ml_examples` 11, render `song_examples`
23, control `compatibility` 2, `patch_authoring` 11 and `project_authoring` 6
(53 top-level tests; its ignored child entry executes under the fresh-process
parent). Historical example 09 validates with fingerprint `41472b38b65b60ed`.
These checks protect existing support; they do not implement or accept example
34, physical interaction, DSP-state migration or realtime deadlines.

Opening checkpoint verification: `cargo test --workspace --offline` passes,
including four new DSL root tests, two CLI checks (both binary targets) and two
native TUI tests. A real no-subcommand 80×24 PTY run displays the empty root,
quits via q, restores the alternate screen and leaves the directory untouched;
the explicit headless check also creates nothing. Legacy example 29 opens through
the same validator with fingerprint `54745c74a42665cf`; example 09 retains
`41472b38b65b60ed`. Formatting/diff checks and docs build pass. Changed-crate
all-targets Clippy passes with the existing workspace allowances (collapsible_if,
if_same_then_else, manual_checked_ops, too_many_arguments,
chunks_exact_to_as_chunks). No callback/DSP/MIDI code or historical musical
fixtures changed. Empty opening is read-only; editable project ownership,
construction/bindings and standalone persistence remain the next checkpoints.

Ownership checkpoint verification: the workspace suite, then the final app and
control suites pass, including source-only typed pattern creation/note editing,
patch and instrument edits, rejection without source/history changes, shared
view revisions and immutable provenance. Historical Lua/project persistence,
patch drafts, Plaits and pickup acceptance remain green. Formatting/diff checks,
app all-targets Clippy with the existing allowances and docs build pass. This
does not yet provide empty editable definitions or construction in the empty
cockpit; those and explicit bindings/standalone saves remain open. No formats,
historical examples, rendering algorithms or callback code changed.

Palette checkpoint verification: workspace tests pass, including the new DSL
request/negative fixtures, app atomic rejection/silence/32-definition bound and
typed in-memory history replay, and native TUI gesture/complete 80×24 graph
inspection tests. A real no-subcommand PTY opens a temporary empty directory,
accepts two `n` presses, displays voice-2 and all typed cables/unsaved notice,
quits and restores the alternate screen without creating files. Formatting,
diff checks, affected-crate all-targets Clippy (existing allowances) and docs
build pass. Legacy 09/29 fingerprints remain `41472b38b65b60ed` and
`54745c74a42665cf`. This is silent in-memory palette creation only, with no
physical device/listening, disk replay, saved project or performance claim.

Native authoring checkpoint: the readable keyboard-patch/pattern specifications
bind to headless native gesture tests. New material creates no track/instrument
assignment. Imported provenance remains immutable; parameter edits preserve new
definitions absent from that provenance, and incomplete/stale patch drafts
retain the accepted revision. Unsupported new-project saving reports retained
in-memory edits. Full workflow status remains design pending bindings, playback,
runtime control and self-contained save/reopen. Remote pushes are batched at
larger milestones; verified intermediate commits remain local.

Independent binding checkpoint verification: the workspace suite passes; final
app/TUI and focused DSL/render suites check source-owned separate bindings,
reject ambiguous pitch/gates and clock mismatch, preserve rejected source/history,
and replay accepted typed commands in memory. Raw f64 full/split renders agree;
held pitch, explicit initial pitch, adjacent retriggers and release tails follow
the declared order. The existing worker publishes audible/disconnected/reconnected
PCM consumed by the production callback without resetting transport. Native
v/p/g inspect both bindings and fallbacks; device playback and capture/save remain
unfinished. No historical example/format or callback implementation is changed.

Native playback checkpoint verification: workspace tests pass for both CLI binary
targets; the adapter is tested against the existing worker, generation-tagged
publisher and production callback without opening a device. Mode switches retain
source/publication/playhead, independent gate disconnect updates consumed PCM,
and unsupported native clocks retain host generation/publication. Native 80×24
PTY opening, empty play diagnostic, patch/pattern/binding gestures, every cable,
mode switching, blocked performance edits and clean quit/restoration pass with
zero files and no audio device. Formatting/diff checks, affected-crate Clippy
(existing allowances) and docs build pass. Physical listening/deadlines remain
unverified. That checkpoint preceded runtime controls; see the checkpoint below.

Runtime level checkpoint: `keyboard-level.txt` specifies explicit construction
exposure and performance gain gestures. App tests reject unknown/unexposed identities,
invalid/renamed declarations and nonfinite/out-of-range values without source/history
changes, isolate two instances, preserve overlays across modes and patch edits, and
reset them on load. Native host/callback tests mute and restore consumed PCM while
playhead advances; identical settings retain generation/publication. TUI checks
native keys and stable instance/control identities at 80×24. Disk capture and
self-contained save/reopen remain unfinished; no physical listening claim.
Workspace tests, focused final DSL/render/both-CLI checks, formatting/diff checks,
affected-crate Clippy (existing allowances) and the docs/reference build pass.
Linear half-gain full/split raw f64 renders agree, and the legacy example 09
fingerprint remains `41472b38b65b60ed`. This checkpoint is committed locally.

Native capture checkpoint: `capture.txt` plus valid/invalid typed payload fixtures
specify the additive native command version in the existing session envelope.
Commands/results/source fingerprints/control observations are captured from typed
app history; navigation/no-ops/drafts are retained and rejections omitted. Capture
starts lazily and never becomes an entry point. Native checkpoints retain at most
eight pending events, copy durable history through 64 KiB buffers, and bound a file
at 64 MiB. Write failures preserve prior TOML/pending events; further gestures and
clean quit wait for successful retry. Tests prove reconstruction beyond history
eviction, no duplicate/lost events on retry, source-continuity/history-gap diagnostics,
unknown payload/version rejection and confined directory/temp-symlink handling.
The native callback test also checks capture observations and clean termination.
Workspace tests, final native capture/CLI checks, affected-crate Clippy (existing
allowances), formatting/diff checks and docs build pass. Real 80×24 PTY checks
confirm file-free empty open/quit, twelve typed construction/runtime events and
clean final gain, plus capture-path failure blocking another gesture/quit and
recovering without duplication. Sparse file-limit and finished-session tests pass.
No audio device is opened by these native capture smoke tests. Commit stays local.
Standalone saves, CLI capture reconstruction, timed replay/epochs, capture rotation,
performance harvesting and physical audio/hardware acceptance remain deferred.

Self-contained serializer checkpoint: `authored-project.lua`, its invalid neighbor
and `persistence.txt` define additive project-v2 Lua. The encoder accepts the
authoritative source rather than editor/runtime projections; import reuses mono
graph validation, shared stable-note restoration, independent voice bindings and
the existing compiler. Full restored-source equality refuses unsupported or lossy
state. Five parent tests plus an isolated fresh-process child prove authored-only
round trips, deleted-note counters, empty/unbound palettes, edited filter defaults,
independent bindings, Plaits authored settings, bounded/invalid input diagnostics
and byte-identical full/split oscillator PCM after deleting the original root.
`cargo test --workspace --offline`, DSL/control all-targets Clippy with existing
allowances, formatting/diff checks and docs build pass. Examples 09 and 29 retain
fingerprints `41472b38b65b60ed` and `54745c74a42665cf`. No native entry discovery,
disk save or TUI fresh reopen is claimed; those remain the next checkpoint.
Legacy project-v1/instrument-v1/v2 exports and examples 25–33 are unchanged.

Native disk checkpoint: `keyboard-save.txt` specifies Ctrl-S/F6 in either mode
and inspectors. Typed SaveProject accepts the bounded source independently of
legacy editor panels; NativeProjectStore owns expected-byte checks, bounded entry
reads, synced new temporary files and atomic rename to main.project.lua. Foreign
temporaries/symlinks/external edits/ambiguous legacy entries reject without loss.
Opening restores native inspectors without synthetic accepted history and submits
loaded voices to the existing worker immediately, with stopped transport and
authored runtime gains. Tests prove saved versus authored-dirty status, untouched
source/runtime/publication/generation on failed save, retry and fresh defaults.
Workspace tests pass; final focused store checks (seven parents plus isolated child),
CLI tests (37 per binary), affected-crate Clippy with existing allowances,
formatting/diff checks and docs build pass. Real 80×24 PTY construct/bind/save,
runtime mute/save, quit/reopen/default reset and saved read-only open/quit pass;
fresh-process CLI --check validates. No device is opened by these checks.
At this checkpoint full composed acceptance and durable capture baselines still
needed audit; the completion checkpoint below supersedes that status.
Timed replay/epochs, physical listening, hardware and deadlines
remain deferred. Commit remains local; historical files/exports stay unchanged.

Slice 1 completion: `keyboard-workflow.txt` composes real key lowering, authoritative
commands, worker publication and production callback output with a fresh-process
native reopen. Duration/waveform edits change published PCM; fixed stereo publication
duplicates authored mono exactly. Gate disconnect/reconnect, mode continuity,
runtime mute without authored promotion, rejected edits and failed save/retry are
checked. `capture-baseline.txt` embeds the opening project-v2 source in native
session headers, bounds parsing and checks its fingerprint. Reconstruction survives
two subsequent saves and deletion of the original root. Existing sessions without
embedded source require their original baseline; no automatic replay is added.
Example 34 is accepted headlessly. Next is Slice 2's example-first shared palette
and independent pattern-source routing, with explicit composition selection.
Validation: workspace offline tests, all eight native capture tests, affected
root/CLI/control all-targets Clippy with existing allowances, formatting/diff and
VitePress reference/build checks pass. Examples 09/29 retain fingerprints
`41472b38b65b60ed` / `54745c74a42665cf`. Physical listening, device deadlines,
sample-timed replay/epochs and harvesting remain deferred. This is a local checkpoint;
no push is required.

Slice 2 pattern checkpoint: [example 35](songs/examples/35-project-palette-routing/ACCEPTANCE.md)
defines the full contract as design, with an executable project-v2 routing fixture
and negative selection/clock requests. Native PgUp/PgDn now selects palette
patterns at the overview or pattern editor in stable ID order. Bindings remain
explicit p/g actions; selection resets cursor/note only when its identity changes.
The app command validates before changing selection and does not replace source;
typed native capture records it. UI navigation excludes track-owned/parameter
patterns, draft panels, performance mode and repeats/releases. Control tests prove
native two-pattern routing, scoped shared-patch gains, raw oscillator sound,
save/reopen and capture reconstruction. The CLI host test proves selection retains
runtime gains and active source/publication/generation while callback transport
advances. At this checkpoint named compositions and native multi-instance selection were missing;
example 35 is not accepted in full.
Validation: workspace offline tests, four palette-routing integration tests,
native selection screen/host checks, affected-crate all-targets Clippy with existing
allowances, formatting/diff and VitePress reference/build checks pass. Read-only
CLI `open-project --check` validates the new fixture (`ea41a978f40edec0`);
examples 09/29 retain their historical fingerprints. No devices or physical
listening evidence are claimed. Keep this checkpoint local rather than pushing.

Slice 2 named composition checkpoint: `compositions.txt` and the additive
`compositions.project.lua` variant retain the original v2 example unchanged.
CompositionDefinition owns canonical membership IDs over the one global instance
registry; source fingerprints include every composition plus authored default
selection. Native v3 requires explicit selected membership and preserves unused
palette/instances; v2 rejects named fields and retains its implicit full mix.
Typed b creates an all-instance snapshot; F7/F8 selects only at the construction
overview after transport stop. App guards pending Play and callback-observed Stop,
compiles before source acceptance and captures exact typed facts. The host retains
prior PCM until prepared, gates restart by source/generation and applies only active
instance overlays; inactive values survive returning to a composition. Tests check
both selections through isolated fresh-process store loading/full-split oscillator
PCM, inactive-invalid binding rejection, v2 compatibility, bounds/references,
typed capture and worker/callback transition without a physical device. Native
arbitrary membership editing and multi-instance selection now have a checked
checkpoint. Example 35 stays design pending its full negative-contract audit.
Timed/metapattern actions and continuous DSP migration
remain deferred.

Named checkpoint validation: workspace offline tests, affected-crate all-targets
Clippy with existing allowances, formatting/diff and VitePress reference/build
checks pass. Read-only CLI opening validates the v3 variant (`4b1628b55a48a382`)
before devices. Examples 09/29 retain `41472b38b65b60ed` / `54745c74a42665cf`.
Composition tests include both fresh-process children; native key/UI and real
worker/production-callback checks stay headless. Commit locally without a push.

Instance checkpoint: native v creates/selects another instance of the same palette
patch, retaining the first PATCH-instance ID and assigning later numeric suffixes.
Up/Down selects stable instance IDs independently of patch/pattern inspectors;
p/g and exposed performance levels address exactly that identity. Membership a/d
uses SetCompositionMember through the stopped candidate validation/compiler,
preserving palette, other compositions and inactive selections/gains. Selecting an
inactive instance cannot silently control another. `instance-keys.txt` specifies
these keys; native UI/negative/capture tests and worker/callback tests prove
selection leaves publication unchanged and membership waits for prepared PCM.
Edited membership reopens in isolated processes with exact full/split oscillator
audio. Workspace offline tests, affected all-targets Clippy with existing allowances,
formatting/diff and VitePress build pass; historical 09/29 fingerprints remain
unchanged. Full example 35 remains design until its composed negative-contract
audit is complete. No physical listening, timed replay or DSP migration claim.

Slice 2 completion supersedes the earlier design-status checkpoints above.
Example 35 now accepts its bounded headless contract. `invalid/audit.toml` executes
duplicate instances, referenced patch/pattern removal (including inactive material),
chords/overlapping gates, multiple selection/source lists and length/loop mismatches.
Typed duplicate/chord requests preserve accepted source/runtime/selection/history;
worker tests additionally retain playing publication/generation on failed bindings
and chords. A shared oscillator edit reaches every referencing composition without
copying definitions or changing runtime gains. `keyboard-workflow.toml` authors
one patch, three patterns, two instances and two distinct compositions from an
empty root. Production-callback tests play, edit the shared patch during playback,
save and reopen both selections in isolated processes with exact full/split f64
PCM and authored defaults. The acceptance file maps every contract to its test.
Referenced-material deletion is an external candidate validation boundary; an
interactive palette-delete command remains future work. No historical examples
were rewritten and no runtime/schema changes were needed for this audit.
Workspace offline tests and affected all-targets Clippy with existing allowances
pass. Formatting/diff, VitePress and unchanged 09/29 fingerprint checks complete
the checkpoint. Next: Slice 3, graph LFOs with explicit unit/rate/combination and
phase/reset semantics; write example 36 and inspect the existing modulation first.

Slice 3 tremolo checkpoint: [example 36](songs/examples/36-patch-lfo/ACCEPTANCE.md)
extends the existing typed synth graph with Lfo, ControlScale and ControlMultiply.
Complete candidate validation checks unit/rate/scope/reset and all bounded control
dependencies; compilation lowers dependency indices, and the ordinary mono voice
worker evaluates one source per sample/instance in bounded stack arrays. The Lfo
kernel remains destination-independent; waveform/rate/phase and cabled gate-reset
settings fingerprint alongside affine/clamp policy and explicit multiplication.
Legacy matrix behavior and absent-settings fingerprints are unchanged. Native
Ctrl-G drafts configure settings and show ports/units/cadence; EditPatch capture
reconstructs the same source. Independent sample references, 60/120 BPM, Hertz and
fractional periods, adjacent reset, fan-out, separate instance reset origins,
negative settings/graphs and full/split renders pass. Isolated fresh-process saves
retain graph/PCM without replay. Worker/production callback tests prove rate edits
while playing and failed-unit publication/history continuity. Full example status
stays design: pitch/filter adapters, variable-rate integration, event cadence and
broader scopes remain open. No callback code or historical musical fixtures changed.

Verification: offline workspace tests, the graph-LFO suite (six parent tests plus
its isolated reopen child), and affected all-targets Clippy with existing allowances
pass. Formatting/diff checks and the VitePress build pass. Read-only project checking
reports example 36 fingerprint `47ef98d84d4c31b3`; historical examples 09 and 29
retain `41472b38b65b60ed` and `54745c74a42665cf`. Physical listening, deadline
benchmarks and continuous stateful DSP migration remain unverified.

Slice 3 vibrato checkpoint adds an explicit audio/per-instance PitchModulate
adapter to example 36. Pitch = input.pitch + bipolar Control × depth_semitones;
no implicit sum, clamp or articulation. One adapter between input.pitch and
oscillator.pitch is supported; Plaits and chained pitch adapters reject.
The existing graph order/stack/LFO/voice worker handles amplitude/pitch fan-out.
Typed depth edits and keyboard drafts persist through the native serializer and
capture vocabulary. Sample references prove positive/zero/negative depth, exact
split rendering and isolated fresh-process reopen. Nine negative contracts reject
without source/history mutation. Playing worker/callback publication checks cover
adding the pitch graph. Full example 36 remains design; filter ports, variable-rate
phase, event cadence and broader scope remain open. Offline workspace tests,
the graph suite (eight parent tests plus isolated child runs), affected all-targets
Clippy with existing allowances, formatting/diff and VitePress checks pass.
Historical 09/29 fingerprints remain unchanged. Physical listening/deadlines and
continuous stateful DSP migration remain unverified.

Slice 3 filter checkpoint: ControlHertz explicitly converts unit-interval Control
to Hertz with authored gain/offset/clamps; low_pass.cutoff consumes that cable
instead of its configured fallback. Overrides cannot silently bypass a cutoff
cable. The existing dependency plan, worker and stateful filter remain authoritative.
Example 36's filter-patch draft and typed settings/capture vocabulary roundtrip;
independent coefficient/integrator references prove sound and tails at positive,
negative, zero and clamped gain, plus isolated full/split reopen. Eleven invalid
contracts retain source/history, and playing worker/callback checks cover combined
tremolo/vibrato/filter publication and invalid Hertz input continuity. Full example
stays design; modulatable depth/rate, broader scopes/cadence and normalization
remain open. Physical listening and continuous stateful DSP migration are unverified.
Legacy automated-filter lowering now rejects graph controls rather than ignoring
their cables, while its historical audio/automation ordering and tests stay intact.
Verification: offline workspace tests, the graph suite (ten parent tests plus
isolated reopen children), 24 render-example tests, affected all-targets Clippy
with existing allowances, formatting/diff checks and VitePress pass. Historical
09/29 fingerprints remain `41472b38b65b60ed` / `54745c74a42665cf`.

Slice 3 depth checkpoint adds ControlAttenuate to example 36, reusing the product
kernel with bipolar signal/unit-interval amount → bipolar output. The explicit
depth-patch draft routes ADSR into amount and fans out the attenuated LFO to pitch
and amplitude adapters; its phase is unchanged. Independent LFO/composition
arithmetic with existing oscillator/ADSR kernels checks attack/decay/release and
adjacent retrigger ordering, full/split rendering and isolated authored reopen.
Removing the separate envelope/VCA product leaves audible oscillation after depth
reaches zero. Typed capture reconstructs every draft/source fingerprint and PCM.
Nine invalid contracts preserve source/history, and the worker/callback checks
combined depth/filter/pitch publication and failed-unit continuity while playing.
Full example 36 stays design: variable-rate phase, broader cadence/scopes, feedback,
normalization and continuous DSP-state migration remain open. Physical listening
and deadline benchmarks remain unverified.
Verification: offline workspace tests, graph-LFO acceptance (twelve parent tests
plus isolated reopen children), affected all-targets Clippy with existing
allowances, formatting/diff and VitePress pass. Historical 09/29 fingerprints
remain `41472b38b65b60ed` / `54745c74a42665cf`.

Slice 3 rate checkpoint connects a second LFO through scale and ControlHertz to
the first LFO's typed rate input. The cable replaces its configured rate; declared
adapter clamps must stay within 0.001..100 Hz. Each sample reads the current phase
before advancing by Hertz/sample-rate, with explicit gate reset before evaluation.
The static-rate path and historical fingerprints remain unchanged. Independent
references cover all four waveforms, tempo/Hertz, reset, full/split and fresh-process
rendering; typed capture reconstructs the source and PCM. Nine negative contracts
preserve source/history. Playing tests combine rate, pitch, depth and filter cables,
and reject rate feedback without changing publication. Full example 36 stays
design: broader cadence/scopes, explicit delayed feedback, normalization and
continuous DSP-state migration remain open. Physical listening and deadlines
remain unverified.

Rate checkpoint verification: offline workspace tests, fourteen graph-LFO parent
tests plus isolated reopen children, affected all-targets Clippy with existing
allowances, formatting/diff checks and VitePress pass. Historical 09/29
fingerprints remain `41472b38b65b60ed` / `54745c74a42665cf`.

## 20. Instrument and pattern authoring (priority over section 19)

MicroLab mk3 checkpoint adds `midi-keyboard-check` and explicit native
`open-project --midi-keyboard MicroLab` entry through the existing decoder, queue,
typed note editor, capture, source compiler, worker and immutable publication.
All channels are accepted for step entry; messages resolve selection/cursor at
control-thread dispatch. Existing pitch/tick selects without overwriting velocity;
new notes use 480 ticks clipped at pattern end. Releases and non-note input do not
edit. Mono source overlaps reject with source/history/audio unchanged. Port match
must be unique; invalidated connection epochs and overflow remain inspectable.
MicroLab mk3 discovery and input connection were observed on the host. Physical
gestures/listening, key-held audition and timed recording remain pending; the latter
must introduce explicit runtime input bindings instead of rewriting patterns.

Keyboard verification: focused native/transport tests, offline workspace suite,
affected all-targets Clippy with existing allowances, formatting/diff and VitePress.
Headless tests preserve independent gate binding, note identities/velocity, source
and published audio on rejection, and reconstruct every captured fingerprint.

Live keyboard renderer foundation extracts MonoState from CompiledNotePatch's
existing mono sample path. LiveMonoRenderer owns the same oscillator, ADSR, filter
and control arrays across sequential blocks; it does not rerender held-note loops.
Fresh instances start at an explicit u64 origin with local modulation age zero.
Pitch and Gate events stay independent; duplicate low is idempotent, high retriggers,
and authored cables determine amplitude. Blocks prevalidate 1..4096 frames and
at most 128 ordered/in-range events before any state change. Tests compare varied
partitions to existing offline PCM, including cabled reset, rate/filter/release,
FM/harmonic Plaits, and check independent references, all invalid contracts,
polyphonic rejection, free amplitude wiring and origins near u64::MAX.
No app/source/capture/device code changes in this checkpoint; streaming publication,
explicit keyboard runtime bindings/lifecycle, live audition and timed capture
remain next. Worker DSP state is ephemeral; authored persistence is unchanged.
Verification: offline workspace suite and focused live renderer tests, affected
all-targets Clippy with existing allowances, formatting/diff checks and VitePress
pass. Historical 09/29 fingerprints remain `41472b38b65b60ed` / `54745c74a42665cf`.

Keyboard runtime checkpoint now adds app-owned BindKeyboard/KeyboardKey/
ResetKeyboard commands. Named active native mono instances, u64 frames and binding
epochs replace implicit inspector routing. Bounded 128-key last-pressed priority
keeps channel identity, emits independent Pitch/Gate and resumes held pitch without
retrigger; explicit disconnect/overflow/panic resets reject stale commands. Source
revisions invalidate bindings; cockpit/selection changes preserve them. Native
capture records typed runtime commands with unchanged authored fingerprints, and
headless reconstruction renders identical shared-worker PCM. Native saves retain
only authored definitions. Generic note lowering handles velocity-zero release;
velocity/sustain/bend need explicit future signal adapters. There is no production
live binding UI, overflow/disconnect reset integration, streaming output or device
timing yet: the current MicroLab host continues construction pattern entry.
Verification: focused lifecycle tests and final offline workspace suite pass;
affected all-targets Clippy with existing allowances, formatting/diff checks and
documentation build pass. Historical 09/29 fingerprints remain unchanged.

Stopped-transport audition checkpoint: native F9 binds/releases the selected
active mono instance. LiveKeyboardWorker compiles/renders the existing mono
kernel in 256-frame blocks, normally <=1024 frames ahead, into a fixed 4096-frame
timed PCM ring attached to the existing callback. No held-note PCM loops or DSP
in callbacks. Ingress binding epochs prevent stale retargeting; loss resets
discard old ingress/PCM while preserving ordered accepted events and room for a
release. Source/transport edits release audition; modes/selection preserve it.
Headless tests compare held PCM beyond one bar, retrigger/fallback/release, source
Arc/FP, loss/source/transport cleanup and complete typed capture reconstruction.
Ring tests prove finite atomic publication, stale/late/backpressure diagnostics,
u64 clock and zero callback allocations/deallocations. Saves still exclude holds.
Capture-write failure releases audition through typed pending cleanup and retries
without duplicating the accepted key or unbind; arrangement audio is unaffected.
Physical gestures/listening/deadlines, velocity/sustain/bend adapters, polyphonic
and simultaneous arranged/live routing, MIDI timestamps and timed replay are open.
Verification: focused keyboard/stream tests, final offline workspace suite,
affected all-targets Clippy with the existing allowances, formatting/diff checks
and documentation build pass. Historical 09/29 fingerprints remain
`41472b38b65b60ed` / `54745c74a42665cf`.

Keyboard velocity checkpoint: native patches may explicitly declare unit-interval
`input.velocity`. KeyboardKey captures MIDI-resolution velocity; older captures
missing the field default to unity. The shared mono kernel feeds only explicit
Control cables; legacy gain and unconnected patches stay unchanged. Existing
ControlMultiply combines envelope/LFO shape with velocity. Last-pressed priority
restores held pitch/velocity without Gate retrigger. Example 36 adds a readable
velocity project, atomic patch draft, timed trace and negative contract. Typed
capture reconstruction, soft/hard gain references, save/reopen, atomic input/unit
failures and real worker/callback PCM pass headlessly. Offline independent-pattern
velocity stays unity; pattern routing, input-declaration editing, sustain/bend,
polyphony/simultaneous routing, timed device replay and physical evidence remain
open. This bounded checkpoint does not complete example 36's broader contract.
Verification: focused keyboard acceptance and final offline workspace suite pass;
affected all-targets Clippy with established allowances, formatting/diff checks
and docs build pass. Historical 09/29 fingerprints remain
`41472b38b65b60ed` / `54745c74a42665cf`. Physical listening remains unverified.

Keyboard bend checkpoint: a declared bipolar `input.bend` Control feeds the
existing PitchModulate adapter with authored semitone depth. Typed KeyboardBend
retains raw 14-bit value, channel, binding epoch and u64 frame. Its fixed mono
owner is independent of keys; fallback/mode changes do not center or retarget it.
Exact center and ±1 endpoints, composed velocity/phase references, partitioned
PCM, invalid value/unit/epoch cases, typed capture and real atomic store reload
are checked. The production worker/callback exercises wheel motion, held fallback
and disconnect/overflow centering. Backpressure reserves two cleanup events before
acceptance. Default/unwired source semantics and old captures remain unchanged.
The explicit sustain gate processor checkpoint now passes headless acceptance:
`sustain.project.lua` plus `sustain-patch.txt` declares `input.sustain` Gate,
inserts one GateHold before ADSR and accepts an opt-in MIDI CC as typed runtime
pedal input. Held/retrigger/release and independent resolved-Gate/partition PCM,
atomic negative edits, typed capture, authored store reload and production
worker/callback loss release pass. Three pending-event slots are reserved for
Gate low, hold low and bend center. No pedal state enters authored saves.
Verification: focused sustain and CLI keyboard tests, offline workspace suite,
affected all-targets Clippy, formatting/diff checks and docs build pass.
Historical 09/29 fingerprints remain `41472b38b65b60ed` /
`54745c74a42665cf`. Physical listening remains unverified.
Smoothing, LFO/bend summation, Plaits adapters, polyphonic/simultaneous routing,
pattern control sources, input-declaration editing, timed device replay and
physical evidence and general Gate/Plaits routing remain open. Full example 36
status remains design.
Verification: focused bend/lifecycle/CLI keyboard tests and final offline
workspace suite pass; affected all-targets Clippy with established allowances,
formatting/diff checks and docs build pass. Historical 09/29 fingerprints remain
`41472b38b65b60ed` / `54745c74a42665cf`. Physical listening remains unverified.

Investigation and ownership decisions: [AUTHORING.md](AUTHORING.md). The first
implemented slice is [the instrument editor](INSTRUMENT_EDITOR.md): example 25
has automated headless acceptance; example 29 now accepts bounded
[composed musical authoring](MUSICAL_AUTHORING.md) headlessly. The
[Lua piano roll](PIANO_ROLL.md) now accepts examples 26/27 headlessly; examples
28 now accepts [editable faders and mixer](CONTROL_ASSIGNMENTS.md) headlessly;
30 now accepts typed patch composition and version-2 graph persistence headlessly. Example 32 adds the pinned Plaits crate subset;
see [PLAITS.md](PLAITS.md). Example 33 now expands selection to all 24 bundled
models with explicit source gates and unchanged historical FM/harmonic files.

- [x] Inventory actual synthesis/compiler, notes/commands, MIDI, Lua, mixer and
  publication boundaries; verify Plaits absence and audit the proposed port
- [x] Specify six initial authoring examples, invalid cases, observable criteria
  and required Lua persistence; retain all older unfinished work
- [x] Bind example 29's first parameter round-trip to headless tests;
  establish one editable Rust song source, minimum versioned Lua edit export/
  reload and confined base validation, without duplicating graph semantics
- [x] Example 25: inspect and edit existing oscillator, low-pass and ADSR through TUI;
  validate typed source edits, rerender publication and Lua persistence
- [x] Example 26: create/assign a synth note pattern through Lua APC grid policy,
  inspect through native TUI and persist musical positions/pitches/velocities/
  stable identities; two-bar fixture proves eight-step page crossing
- [x] Example 27: held-pad explicit note duration, release/lost-release cleanup,
  rewind/loop-safe interaction ordering and persistence beyond one step
- [x] Piano-roll usability: occupied taps toggle off, held endpoints retain notes,
  and the example renders eight independent voices (three-note chord sum tested)
- [x] Example 31: Lua drum rows/toggling/velocity/pages, explicit one-shot slot
  commands, shared TUI/publication and lossless Lua pattern persistence
- [x] Add an inspectable generated 909-inspired sample kit for the drum editor
- [ ] Implement and evaluate a faithful modular TR-909 voice/kit model;
  no native 909 engine, hat choke groups or live drum voice controls exist yet
- [x] Example 28: author/reassign/remove declared fader bindings to source-owned
  cutoff/resonance, track volume/pan and single-track delay mix/feedback;
  shared Lua/TUI inspection, pickup rearming, audible source edits and versioned
  Lua mapping/value save/reload with unchanged TOML and journals
- [x] Example 32: pinned crate FM/harmonic typed module, editor, Lua persistence,
  source/publication tests, direct port-reference blocks and allocation checks;
  authorized native f32/RNG exceptions, global omitted-rate default 48 kHz
- [x] Example 33: all 24 bundled Plaits choices, typed gate connections,
  keyboard/Lua selection, main/aux render/publication, lossless Lua reload,
  parameter boundary and worker allocation checks; physical listening pending
- [x] Example 30: compose the supported oscillator/ADSR/VCA/low-pass patch;
  inspect typed cables, reject invalid/unsupported graphs and export/reload;
  reuse SynthDefinition/typed port validation/compiler, bounded drafts, atomic
  Lua batches, main/aux Plaits replacement and continued transport/publication
Plaits catalog verification: all 24 models pass main/aux compilation/publication
and independent Lua graph/value reload; keyboard and Lua cycle the full catalog.
Ten nonrandom engines pass exact upstream block, split-range and reload checks.
All models process patched/unpatched gates and joint 0/0.5/1 parameter boundaries
without allocations after worker initialization. Example 32 source files and
historical Lua definitions remain compatible. Physical listening is pending.

- [x] Example 29: selected synth graph/pattern, declared mixer/fader bindings,
  unique composed Lua save, clean journal exit, fresh-process exact source/audio;
  invalid edits, interrupted gestures, stale references and save failure tested
- [x] Fix mixer swallowing Ctrl-E/Ctrl-N; show composed save and selected pickup
  hints; reject lossy standalone instrument exports
- [ ] Subsequent examples: multiple edited synths/patterns in one export;
  granular sync/commit fault injection, beyond tested filesystem failure
- [ ] Musician iteration: visible pitch/time page + modifier legend and fewer
  assignment-selection keystrokes; evaluate on physical APC first
- [x] Run relevant full crate/compatibility suites for the instrument slice
- [x] Default live device output to zero gain for four seconds, then a 100 ms
  rise to unity; device-frame clock independent of transport, unchanged authored
  values/offline renders, 44.1/48 kHz headless boundary tests
- [ ] Record physical APC pad/hold/fader/listening results separately from
  headless acceptance

Example 29 adds six headless integration tests plus a subprocess reload entry.
Composed checkpoint: 416 workspace tests pass, zero fail; the ignored child
entry runs explicitly under its parent. CLI reload/render, historical example
09 validation, formatting/diff checks, Clippy with existing allowances and
documentation build pass. F6 is portable save-all; performance shows its path.
Host APC input/LED connection and clean stopped TUI exit pass; physical gestures
and listening remain pending. See its results. Historical verification below
describes earlier checkpoints; bounded composed persistence is now implemented.

Current catalog/startup verification: all 408 workspace tests pass, including
historical compatibility. Formatting, diff checks, workspace Clippy with existing
lint allowances, documentation build, and example 33 validation pass. Startup
mute is headless verified; reported static's cause and physical listening remain
unverified. See [AUDIO_OUTPUT.md](AUDIO_OUTPUT.md).

Instrument-slice verification: `cargo test --workspace --offline` passed all
350 tests, including 11 instrument-authoring tests. The checked-in Lua export
validated and independently rendered through `render-instrument`. Formatting,
diff checks, the documentation build and changed-crate Clippy checks passed
(existing workspace lint allowances: collapsible_if, if_same_then_else,
manual_checked_ops, too_many_arguments). No physical controller/listening test
was performed. Wider example 29 persistence and example 30 remain open.

Piano-roll verification: `cargo test --workspace --offline` passed all 361
tests, including nine piano-roll acceptance tests. Independent `validate-song
--pattern` / `render-pattern` passed for the checked-in Lua definition, and the
historical example 09 still validates. Formatting, diff checks, documentation
build and changed-crate Clippy checks passed with the above existing allowances
plus `chunks_exact_to_as_chunks` for pre-existing DSP tests on the host compiler.
Physical APC gestures, orientation, LED states and listening remain untested.

Drum-slice verification: `cargo test --workspace --offline` passed all 366
tests, including four drum-editor and ten piano-roll tests. The checked-in
sample-slot Lua definition validated and independently rendered with
`render-pattern`. Changed-crate Clippy (the allowances above), formatting,
diff checks and the documentation build passed. The original generated kit
is a 909-inspired approximation; faithful 909 voices and physical acceptance
remain open.

Control/mixer-slice verification: `cargo test --workspace --offline` passed all
376 tests, including nine control assignment integration tests and a stereo
balance unit test. Physical MIDI observation/pickup, source edits, dirty ranges,
worker publication/callback consumption, projections, read-only Lua and unique
save/reload are tested headlessly. Each catalog parameter renders changed audio
and reloads exact source/audio at its range endpoints and midpoint. Historical
mono compatibility and first-pan stereo publication pass. The checked-in
control export independently validates and renders through `render-controls`;
example 09 still validates. Formatting, diff checks, the documentation build
and workspace Clippy pass with the existing allowances above. Physical
faders/LEDs/listening and composed example 29 persistence remain open.

Plaits verification: the full workspace suite passed (384 tests before the two
additional focused loader/keyboard tests, which also pass). Workspace Clippy,
formatting and diff checks pass with the existing allowances. Direct pinned-port
main/aux comparison and zero-allocation construction/processing are headless
checks, not independent firmware/hardware certification. Physical listening and
additional engines remain open.

Patch verification: all 398 workspace tests pass, including eleven patch-authoring
tests and the declared VCA/filter execution regression. Keyboard/Lua composition,
stale and invalid batches, continued callback audio, independent graph reload,
version-1 compatibility and zero-allocation Plaits reference tests pass headlessly.
Workspace Clippy with the existing allowances, formatting/diff checks and the
documentation build pass. The checked-in version-2 graph independently validates
and renders through render-instrument.
Filtered note PCM changes intentionally to follow the previously declared
VCA-before-filter cables; source fingerprints and file compatibility remain.
General DAG/DSP graph authoring, combined example 29 snapshots and physical
acceptance remain open.

Done when a musician can create/edit these example instruments, patterns,
patches, mappings and lane levels, save readable Lua, and reload equivalent
music without hardware or replaying a session. Existing-module authoring comes
before a generalized editor, bus routing, full runtime
replay or scheduler work. Necessary authoring clock/capture correctness remains
part of each slice. Physical acceptance stays pending until actually tested.

## 19. Song directories and `.ml*` formats

### 19.0 Example-first specification

The checked-in example corpus is the primary format specification and the
acceptance suite. Do not add schema fields, abstractions, loaders, or runtime
features merely because they may be useful later. Every implemented capability
must be justified by at least one concrete example that we expect a person to
author, perform, capture, or replay.

- [ ] Create a tree of minimal examples, with one focused capability per song
- [ ] Create composed examples that combine previously proven capabilities
- [ ] Give every example a short statement of intent and observable acceptance
  criteria: what must validate, compile, sound, expose, record, or replay
- [ ] Keep example files small enough to understand without reading Rust code
- [ ] Add invalid examples for each important diagnostic and boundary rule
- [ ] Turn each accepted example into an automated fixture or end-to-end test
- [ ] Maintain an example-to-capability matrix so unsupported and untested
  behavior is visible
- [ ] Require a new or changed example before expanding a `.ml*` schema
- [ ] Avoid speculative fields and abstractions not exercised by the corpus

Initial example tree:

```text
songs/examples/
  00-minimal-synth/
  01-synth-note-pattern/
  02-synth-parameter-pattern/
  03-dsp-chain/
  04-dsp-parameter-pattern/
  05-multiple-tracks/
  06-pattern-switching/
  07-pattern-duration-and-repeat/
  08-arrangement-and-scenes/
  09-curated-performance-controls/
  10-performance-and-all-params-modes/
  11-session-capture/
  12-exact-session-replay/
  13-quantized-session-replay/
  14-harvested-performance-cues/
  15-launch-control-xl-input/
  16-launch-control-xl-playground/
  composed-warehouse/
  invalid/
```

Done when the desired product behavior can be reviewed by reading example song
trees, and every supported format/runtime feature points back to an executable
example.

### 19.1 Format contract derived from examples

Synth and DSP definitions are Eurorack-inspired typed patch graphs. Files
declare modules, typed ports, normalized connections, and explicit cables.
Convenience integrations may be expressed as recipes only when their expansion
remains inspectable and compiles to the same patch model; opaque integrated
instruments, effects, and hidden modulation systems are not the foundation.

- [ ] Define versioned TOML schemas and reference rules for:
  - `.mlsynth`: synth modules, voice configuration, parameters, modulation
    targets, defaults, and stable parameter IDs
  - `.mldsp`: DSP modules/chains, routing, parameters, macros, modulation
    targets, latency/lookahead/tail declarations, and stable parameter IDs
  - `.mlpattern`: note/event patterns and DSP/control parameter patterns
  - `.mlperformance`: song assembly, synth/DSP/pattern assignments,
    arrangement, curated controls, bindings, and replayable interaction events
- [ ] Define a song as a directory of text files with one unambiguous entry
  `.mlperformance` file and relative references contained within the song root
- [ ] Specify reference syntax, namespaces, stable IDs, format versions,
  extension policy, path normalization, and cycle policy
- [ ] Derive the initial schema from the smallest examples instead of designing
  a comprehensive schema in advance
- [ ] Document which data is authored source truth and which data is generated
  session history

Done when the formats can describe a complete interactive song without relying
on hard-coded showcase construction.

### 19.2 Directory loading, validation, and compilation

- [ ] Add a song-directory loader outside realtime code
- [ ] Parse each file independently and retain file/path context for diagnostics
- [ ] Resolve relative references only within the song root by default
- [ ] Convert parsed names/references to typed IDs and typed parameter values
- [ ] Reject missing/duplicate IDs, missing files, incompatible parameter or
  port types, invalid ranges, illegal reference cycles, and ambiguous entry files
- [ ] Produce deterministic diagnostics with file and logical field context
- [ ] Fingerprint individual definitions and the fully resolved song model
- [ ] Compile the resolved song through the existing source, relation, render,
  automation, and realtime-publication boundaries
- [ ] Keep compatibility loading for existing `.toml` fixtures until their
  tests and demos have migrated

Done when a CLI validation command can load an example song directory, resolve
all cross-file references, print useful failures, and produce a deterministic
compiled fingerprint.

### 19.3 Pattern and parameter-pattern model

- [ ] Separate reusable pattern identity from a pattern's placement in a song
- [ ] Support note/event patterns targeting synth or sample tracks
- [ ] Support DSP and synth parameter patterns with typed targets, values,
  interpolation, duration, looping, phase, and launch quantization
- [ ] Define track selection, pattern selection/replacement, repeat count,
  duration, and arrangement/section semantics
- [ ] Preserve exact dirty-range invalidation for edited or switched patterns
- [ ] Include every referenced definition and placement in artifact fingerprints

Done when musical patterns and parameter patterns can be independently reused,
assigned, switched, and rendered deterministically from text definitions.

### 19.4 Curated performance controls

- [ ] Let `.mlperformance` declare the controls intentionally exposed to the
  performer: parameters, macros, toggles, choices, track/pattern launches, and
  quantized actions
- [x] Define control labels, value ranges/steps, bindings, and target mappings
  without duplicating underlying synth/DSP parameter semantics
- [ ] Add explicit control groups and defaults to the declared performance
  surface
- [ ] Validate duplicate or unreachable bindings and unsafe target mappings
- [x] Generate the default cockpit view from the declared controls
- [x] Route every interaction through typed commands rather than direct state
  mutation
- [x] Add a LaunchControl XL app-level input profile that maps faders to
  absolute normalized curated-control values and buttons to typed step nudges
- [x] Let `.mlperformance` declare MIDI devices and per-control MIDI CC
  bindings for absolute faders/knobs and step buttons
- [x] Open script-declared MIDI devices through the host MIDI stack and feed
  decoded CC messages into the typed control-surface profile
- [x] Add a script-aware hardware diagnostic path for MIDI control inputs
- [x] Rerender supported playground parameter controls for delay feedback and
  synth filter cutoff
- [x] Add live script-target support for filter resonance and delay mix so the
  LaunchControl playground can expose distinct tone/space rows instead of 32
  duplicate cutoff controls
- [x] Add script-declared MIDI action bindings for transport and performance
  buttons
- [x] Print raw note/unknown MIDI messages in the diagnostic path so specialized
  hardware buttons can be discovered
- [x] Add script-declared MIDI note action bindings and bind the discovered
  LaunchControl XL side-column note/CC buttons in the playground example
- [x] Add authored LaunchControl XL playground groove scenes/variations and
  rerender selected song patterns during `tui-song` playback
- [x] Make the LaunchControl XL playground scenes/fills musically differentiated
  enough to verify by ear instead of minor note-list variants
- [x] Add script-declared centered and overdrive MIDI curves so the LaunchControl
  playground has neutral center-detent knobs and faders with a normal/full-open
  value below the physical maximum
- [x] Show the LaunchControl XL playground's authored groove scenes/fills,
  actual note grid, and compact control telemetry in default performance mode so
  the playable instrument/pattern surface is visible without switching to
  all-parameters mode
- [x] Make `tui-song` autoplay by default and keep the published song loop
  sounding across live parameter rerenders; `--no-autoplay` is now the explicit
  stopped-start mode
- [x] Fix the LaunchControl playground note positions to use the loader's 960
  PPQ ticks per beat and build the default TUI grid from the authored initial
  `.mlpattern` instead of a dummy empty pattern
- [x] Route live `tui-song` output through a dedicated song-audio publication so
  the generic backing TUI coordinator cannot overwrite the initial rendered song
  with silence before the first controller movement
- [x] Add `tui-song --audio-debug` status telemetry for live output diagnosis:
  transport callbacks, playhead position, current sample peak, and upcoming
  song-publication peak
- [ ] Design `17-launch-control-xl-ensemble` as the next example-first target:
  a multi-lane LaunchControl XL performance with 4+ variations per lane:
  - Lane 1: one beat drum track
  - Lanes 2-3: rhythm/percussion drum tracks
  - Lane 4: polyphonic synth for chords/pads
  - Lanes 5-6: monophonic bass synth tracks
  - Lanes 7-9: sample-based tracks
  - Resolve the 9-lane/8-fader mismatch with script-declared banking: the beat
    drum can live on another page in this example, but the specific scene/
    performance declaration owns that distinction; Rust must only implement
    generic bank/page mapping
- [x] Add an `ACCEPTANCE.md` for the ensemble example before implementing
  runtime support, including exact LaunchControl row/side-button semantics
- [x] Add generic `.mlperformance` lane/page declarations with validation for
  scene-authored banked LaunchControl surfaces
- [x] Load generic lane/page declarations into app view state and render the
  active page in performance mode without hard-coded lane/page policy
- [x] Add script-declared `select_page` performance actions that switch active
  performance pages by page id
- [x] Add script-declared per-strip visible control lists and render only the
  active page's declared controls in performance-mode telemetry
- [x] Add page-scoped MIDI control bindings so active page selection can remap
  physical controls without hard-coded LaunchControl policy
- [x] Add a validating `17-launch-control-xl-ensemble` song skeleton with nine
  lanes, two scene-authored pages, four lane-specific variations per lane, and
  page-scoped fader controls
- [x] Add `.mlsamples` sample-bank metadata parsing and attach the ensemble
  sample lanes to a Raven voice placeholder bank
- [x] Define the script-level lane model for the ensemble: track IDs, lane
  roles, pattern banks, variation IDs, mute/solo behavior, launch quantization,
  and which controls are per-lane versus global
- [x] Load authored lane quantization, mute/solo defaults, active variation,
  and pattern banks into app/TUI performance-page view state
- [x] Add typed runtime commands for lane variation selection, lane pattern-bank
  selection, lane mute, and lane solo against generic performance-page state
- [x] Add script-declared actions that bind LaunchControl buttons to the typed
  lane variation/bank/mute/solo commands without hard-coded controller policy
- [x] Extend `.mlperformance` controls to support momentary modifiers/layers
  such as “hold button + move fader” without hard-coded Rust policy
- [x] Implement a first modifier example: while a declared modifier button is
  held, one or more faders send octave/transpose commands instead of their
  normal continuous parameter target
- [x] Apply script-declared lane octave/transpose state to the mixed song
  renderer so modifier-layer octave pushes affect audio, not only typed
  performance state
- [x] Make authored lane pattern-bank and variation selections rerender the
  supported song audio path
- [x] Extend authored lane pattern-bank and variation selections to true
  multi-track playback so the same controller can switch at least four
  variations per lane without requiring 32 dedicated buttons
- [x] Add render-layer multi-track note compilation/mixing for the ensemble
  synth tracks
- [x] Wire multi-track song compilation into `tui-song` playback and lane
  variation changes
- [x] Extend multi-track song playback for the subset needed by the ensemble
  example: drums/percussion first, then basses, then poly pad
- [x] Route live controls into mixed multi-track patches so script-authored
  per-lane/global parameter changes affect the currently selected ensemble
  tracks, not only the legacy single-track delayed-note patch
- [x] Author distinct ensemble synth/filter targets so each visible lane control
  changes the intended lane/instrument instead of the former shared placeholder
  synth filter
- [x] Author the `17-launch-control-xl-ensemble` main page with all 24
  LaunchControl XL knobs plus eight faders from the `.mlperformance` script:
  top row resonance, middle row full-range tone cutoff, bottom row darker
  cutoff range, and normalized fader overdrive curves
- [x] Add script-declared `lane:<id>/level` performance controls and mixed-song
  lane-level overrides so faders can silence or overdrive synth and sample
  lanes without depending on placeholder synth filters
- [ ] Replace sample-lane placeholder tone controls with real sampler
  pitch/slice targets so Raven sample tracks are directly playable from the
  LaunchControl surface
- [x] Add active page/lane pattern monitors to performance mode from authored
  `.mlpattern` data so each visible strip shows the selected variation and a
  compact step activity/playhead monitor
- [x] Add script-declared performance UI sections/key hints and let declared
  performance UIs omit the legacy single-pattern grid entirely
- [x] Add sample-track playback v1 for mixed songs: load text-authored sample
  references, trigger the first slot/first slice as deterministic one-shots from
  sample-bank track note patterns, and include them in range-identical mixes
- [x] Add `.mlsampler` voice definitions with explicit pattern targeting for
  sample slots/slices, note/root pitch tracking, per-event pitch offsets, and a
  sampler pitch envelope
- [ ] Extend sample-track playback with loop mode, live pitch/slice controls,
  and richer per-event sample selection/mapping
- [x] Add a polyphonic synth path for chord/pad note patterns in `.mlsynth`
  songs, with enough ADSR/filter control to be musically distinct from bass
- [x] Add TUI performance-mode lane overview for the ensemble: active page, 8
  visible controller strips, active variation per lane, mute/solo state, compact
  visible control values, and an explicit modifier-state placeholder
- [x] Replace the TUI modifier-state placeholder with real script-declared
  held/layer state after `.mlperformance` modifier controls exist
- [ ] Add session-capture tests proving normal controls and modifier-layer
  gestures record as typed inputs and can replay without the LaunchControl
  attached
- [x] Make lane mute/solo state affect mixed-song audio rerenders, not just
  app/TUI state
- [ ] Defer LED feedback until after the ensemble control semantics are stable,
  then map LEDs to active variation/mute/modifier state from script-authored
  output declarations
- [ ] Verify LaunchControl XL input with the diagnostic path on
  both Windows and Linux
- [ ] Add MIDI output/LED feedback support after confirming the LaunchControl XL
  output protocol for script-addressed LEDs
- [ ] Add quantized script-declared pattern-switching semantics and exact replay
  beyond the current immediate song-rerender scene path

Done when loading a song produces a small usable performance surface entirely
from its `.mlperformance` declaration.

### 19.5 Performance mode and all-parameters mode

- [x] Add `Performance` and `AllParameters` cockpit modes
- [x] Make `Performance` the default
- [x] Bind `Ctrl-Tab` to switch modes
- [x] In performance mode, show only controls curated by `.mlperformance`
- [ ] In all-parameters mode, expose the complete resolved synth/DSP/track/
  pattern parameter tree for inspection and editing
- [x] Preserve transport, playhead, selected track/pattern, queued launches,
  parameter state, and audio publication while switching modes
- [ ] Refine track, pattern, arrangement, duration, and parameter navigation so
  all-parameters mode remains deterministic even before it is polished

Done when `Ctrl-Tab` safely switches between a focused playable interface and
the complete parameter surface without disturbing playback.

### 19.6 Timestamped performance capture

- [x] Create a new datetime-stamped `.mlperformance` session file when an
  interactive song starts, using a collision-safe filename
- [x] Record song identity/version/fingerprint and the starting runtime state
- [ ] Record every accepted typed performer interaction with monotonic sequence,
  wall-clock offset, absolute frame, musical position, requested quantization,
  actual execution frame, previous value, resulting value, and provenance
- [ ] Record mode changes, track/pattern selection, launches, parameter edits,
  transport actions, cancellations, and fallback/cache decisions where relevant
  - [x] Classify mode, selection, transport, queue/cancel, transform,
    audio-source, synth-control, performance-FX, and parameter-edit events
    distinctly in captured sessions
- [x] Checkpoint session files outside the realtime callback with
  crash-tolerant writes
- [x] Add explicit bounded buffering for session event writes
- [x] Write a clean/unclean termination marker
- [x] Write the complete final runtime state
- [x] Never overwrite an authored performance definition or an earlier session

Done when every interactive run leaves a human-readable session artifact that
is sufficient to reconstruct what the performer did and when it took effect.

### 19.7 Replay and harvesting

- [ ] Load a captured session against the exact song fingerprint, with an
  explicit diagnostic or migration path when definitions differ
- [ ] Replay accepted actions deterministically at recorded execution frames
- [ ] Offer musical-phase/quantized replay as an explicit alternative to exact
  frame replay
- [ ] Distinguish authored automation, live performer input, learned/harvested
  cues, and replay provenance
- [ ] Extract recurring gestures into proposed macros, parameter patterns,
  pattern launches, or revised performance controls
- [ ] Keep harvesting non-destructive: write new proposed text files and retain
  the original session journal

Done when a saved performance can be replayed repeatably and mined into reusable
song material without opaque JSON-only state or mutation of the source session.

### 19.8 Verification and migration

- [ ] Schema/round-trip tests for every `.ml*` format
- [ ] Cross-file reference, path-boundary, duplicate-ID, cycle, and diagnostic tests
- [ ] Deterministic resolved-song and artifact fingerprint tests
- [ ] Full-buffer versus chunked rendering tests for parameter patterns
- [ ] TUI tests for default performance mode and `Ctrl-Tab` switching
- [ ] Session capture tests covering every accepted interaction category
- [ ] Exact replay and quantized replay tests
- [ ] End-to-end headless test: load song directory, compile, perform scripted
  interactions, save session, reload, and replay sample-identically
- [ ] Migrate at least one existing showcase to the new song directory format
- [ ] Update README commands and format documentation

Done when one existing showcase is driven by `.ml*` files, plays interactively,
records a session, and replays it deterministically under the standard checks.

### 19.9 Lua control-plane vertical slice

Progress checkpoint (2026-10-10): example 18 is accepted for its documented
headless scope. The implementation has 11 acceptance tests and 2 independent
compatibility tests; the workspace suite passed 312 test executions. The
headless runner demonstrates typed commands, hierarchical page changes,
cross-surface synchronization, reconnect restoration and timestamped session
writing. Production Lua song playback and physical controller support are not
complete. Exact commands, results and known limitations are in
[CONTROL_PLANE.md](CONTROL_PLANE.md#verification-record).

- [x] Audit existing app/DSL/render/audio/MIDI/TUI/capture ownership and document
  reusable boundaries and private CLI host integration gaps
- [x] Add constrained vendored Lua 5.4 composition through the existing typed
  song loader; preserve authored TOML entrypoints and historical fixtures
- [x] Add bounded deterministic dispatch, command acceptance/rejection tracing,
  causal follow-ups and a bounded lossless accepted-command outbox
- [x] Add a Lua-authored hierarchical presentation-page machine with guards,
  parent bubbling, ordered lifecycle actions and explicit rejection
- [x] Exercise one reusable mode widget on simulated APC, Stream Deck and TUI
  surfaces; test cross-surface coherence and disconnect/reconnect restoration
- [x] Add typed u64-frame journals, unique timestamped session writing and
  headless command replay; deterministic Lua-definition save/restore
- [x] Independently load/compile/render the pinned supported historical corpus
- [x] Add documented APC RGB/input codecs, generic expressive keyboard decoding
  and resettable soft-takeover foundation without claiming hardware acceptance
- [ ] Extract reusable song-host rerender/publication adapters from the CLI,
  then connect accepted Lua lane/control commands to the existing song worker
- [ ] Wire Lua surface projections and inputs into production song playback/TUI
  without adding a second app state or changing the LaunchControl path
- [ ] Unify CLI and Lua session capture through a reusable typed recorder;
  preserve historical session inputs and replace debug-string/u32 limitations
- [ ] Add automatic crash-tolerant checkpoints and exhaustive accepted-action
  capture, including page/lifecycle behavior and equivalent starting app state
- [ ] Add an authoritative pattern-launch widget with idle/queued/playing/
  unavailable presentations on multiple surfaces
- [ ] Add frame-scheduled and quantized musical replay with sample-identical
  acceptance tests, plus HSM runtime-state persistence
- [ ] Wire page-aware soft takeover to real faders and script bindings
- [ ] Add Stream Deck discovery/HID/image transport and real reconnect lifecycle
- [ ] Add live MicroLab voice input and authoritative APC feedback integration
  (example 19 now uses real APC input; example 20 sends static Lua LED frames)
- [ ] Hardware-test the new transports; retain LaunchControl support
- [ ] Add a Rust-owned hierarchical transport lifecycle when discovery,
  reconnect/backoff and device failures require it; test statig against those
  concrete cases before adopting a dependency
- [ ] Measure render throughput, worker utilization and deadline compliance
  before implementing independent track/voice parallelism or DSP checkpoints

Next implementation slice: extract the existing CLI song-host boundary and
prove one Lua lane/control gesture schedules a rerender and publishes changed
audio while all simulated surfaces remain coherent. Follow with shared typed
session capture and the authoritative pattern-launch widget before adding real
controller transports. Keep adding immutable compatibility fixtures as each
new persistence or runtime capability becomes supported.

Example 18's `accept` status covers only its explicit headless acceptance file.
Hardware protocols and transport gaps, statig evaluation, API details and actual
multicore bottlenecks are recorded in `CONTROL_PLANE.md`.

### 19.10 Connected APC playground and Lua colors

Progress checkpoint (2026-10-10): examples 19 and 20 are implemented. The APC
playground uses the existing TOML/MIDI song host and typed commands, not a second
application model. The Lua LED diagnostic shares the constrained VM and sends
only a fully validated, bounded presentation plan outside audio.

- [x] Four synth lanes, sixteen variations, nine audible fader targets,
  script-authored grid mute/solo and transport buttons; readable control map
- [x] Headless mapping, authoritative app-state/audio integration, render range
  equality, levels/filters/resonance/mute/solo and invalid-reference tests
- [x] Grow the independent compatibility corpus with example 19; retain old files
- [x] Recognize the existing `performance_session` kind in the fixture format
  check when running examples generates journals under `performances/`
- [x] Observe real APC Control-port input (105 diagnostic events, including CC
  and pad messages); open production audio/TUI, capture 226 accepted actions
  (223 curated controls, three lane mutes), and exit with clean session marker
- [x] Add Lua palette animation, fixed-color peripheral button behaviors,
  hardware-free dry run, bounds/diagnostics and deterministic wire tests
- [x] Send nine LED frames / 720 messages successfully to the connected APC
  Control output; retain final static playground hints
- [ ] Visually confirm all LED colors on hardware and exhaustively check every
  mapped input, audible response, transport action and unplug behavior
- [ ] Connect LED presentation to authoritative variation/mute/solo state;
  implement soft takeover, reconnect restoration and quantized launches

Verification: `cargo test --workspace` passed 320 test executions (the root and
CLI binaries duplicate CLI tests), including historical compatibility. The new
APC tests cover two CLI cases, one DSL validation/negative case, one render case
and two LED-plan cases. `cargo clippy -p meldritch-control --all-targets
--no-deps -- -D warnings`, `cargo fmt --all -- --check`, `git diff --check`,
`cargo run -q -- validate-song songs/examples/19-apc-mini-mk2-playground` and the
existing example 09 validation passed. The LED dry run and physical send passed.
Strict CLI Clippy reports three pre-existing `too_many_arguments` findings in
unchanged MIDI/TUI functions; the new control crate is lint-clean.
No audio callback or render scheduler implementation changed. Hardware input,
output submission and startup are observed; listening/visual acceptance remains
explicitly unverified. Local captured sessions are ignored user data.


### 19.11 VitePress documentation and GitHub Pages

- [x] Add VitePress site with getting started, controller/example guides, local
  search and a persistent “Nothing is stable yet” notice
- [x] Include architecture, invariants, formats, Lua API, capability matrix,
  testing strategy and this queue directly from their repository sources
- [x] Add reproducible public-registry npm lockfile, Node 24 scripts and a
  patched Vite 6.4.4 override for stable VitePress 1.6.4
- [x] Add PR build and main-branch Pages artifact/deployment workflow with
  read-only PR permissions and scoped Pages/OIDC deployment permissions
- [x] Verify clean `npm ci --offline --cache /tmp/meldritch-npm-cache`,
  `npm run docs:build` and `npm audit --offline --cache /tmp/meldritch-npm-cache`
  (zero advisories); inspect desktop and mobile headless-Chrome screenshots
- [x] Check all 19 generated HTML pages for the persistent notice, repository
  base links, local search index and public-registry lockfile URLs
- [ ] Enable repository Pages source **GitHub Actions** once in repository
  Settings → Pages, then confirm the first deployment and public URL

GitHub's configure-pages action explicitly requires a separate administrative
token to enable Pages automatically; this workflow uses only `GITHUB_TOKEN`.
The public Pages metadata endpoint currently returns 404. Local verification
does not claim the site is deployed. Setup and local commands are documented in
`docs/guide/contributing.md`.

First remote verification: [workflow run 38104433773](https://github.com/meleneth/meldritch/actions/runs/38104433773)
for `58cc7c8` passed checkout, Node setup, `npm ci`, `npm run docs:build` and
Pages artifact upload. Deployment failed at `configure-pages` with “Get Pages
site failed” / HTTP 404 because the repository has not enabled Pages. Enable
the GitHub Actions Pages source, then rerun the failed deployment job. This is
an outstanding repository setup step; no public deployment is claimed.

### 19.12 Production Lua APC performance integration (headless acceptance complete)

- [x] Audit existing host, MIDI, app, recording, Lua and TUI boundaries
- [x] Write example 21's executable acceptance contract before implementation
- [x] Extract compiled-song overrides, rerender worker and publication adapter;
  bound completion storage and report failures without touching audio callbacks
- [x] Extract the existing performance-session writer without changing its format
- [x] Use one authoritative-view song-host adapter for TOML and Lua commands;
  prove successive lane variations do not reset previously selected lanes
- [x] Add reusable Lua launch/mute/solo widgets on APC and simulated LCD/TUI
- [x] Wire Lua-owned APC pads exactly once, and state-driven output on the
  non-realtime control thread; preserve other authored MIDI bindings
- [x] Test input → accepted command → rendered publication → projections →
  shared captured session, plus rejections, stale work, cleanup and compatibility
- [x] Update status/API/site documentation and run complete verification
- [x] Commit extraction checkpoint (`120774b`); prepare verified integration
  checkpoint with remaining work explicit

Extraction checkpoint: `cargo test --workspace` passed 321 executions, including
the original CLI session/publication/MIDI checks and historical compatibility.
The shared render worker has one pending and one completion slot, reports invalid
overrides, and retains latest-request behavior. The session writer's legacy v1
format and checkpoint semantics are unchanged.

Integration checkpoint: example 21 is `accept` for its headless contract.
`tui-song --lua main.lua` uses the shared song host and recorder. Complete-view
compilation fixes successive lane changes resetting previous choices; the view
now starts at each track's authored initial pattern. The LaunchControl delayed
single-track variation and mute path have a direct audio regression test.
Lua-owned pad presses use a 64-slot observation mailbox and cannot also run
TOML bindings. Overflow is reported; accepted session facts are retained.
Lua launch/mute/solo widgets project coherent APC RGB/LCD/text descriptions.
The existing TUI continues to render the authoritative lane state.

Verification commands and actual results:

- `cargo test --workspace`: 328 executions passed, zero failures; includes
  historical compatibility, five new live-performance tests and CLI confinement
- `cargo test -p meldritch-control --test live_performance`: 5 passed
- `cargo clippy -p meldritch-control -p meldritch-render --lib --no-deps -- -D warnings`: passed
- `cargo clippy -p meldritch-cli -p meldritch-control --all-targets --no-deps -- -D warnings -A clippy::too_many_arguments`: passed; allows the existing CLI lint
- `cargo fmt --all -- --check` and `git diff --check`: passed
- `cargo run -q -- validate-song songs/examples/09-curated-performance-controls`
  and `cargo run -q -- validate-song songs/examples/21-lua-apc-performance`: passed
- `npm run docs:build`: passed, including the new Lua APC guide
- `cargo run -q -- tui-song songs/examples/21-lua-apc-performance --lua main.lua --no-autoplay --midi-debug`:
  exited 0 after `q`; real APC Control input and feedback output opened;
  stopped transport reported zero underruns/misses, and a clean session was saved

Strict render **all-targets** Clippy additionally reports two pre-existing
`chunks_exact_to_as_chunks` warnings in DSP tests; library Clippy passes. No
audio callback code or historical fixtures changed. No functional regressions
were found by the suites. Full-history rerender cost, legacy retained session
history and per-tick bounded Lua presentation evaluation remain performance
limitations; there is no new throughput or realtime deadline claim.

Remaining implementation and acceptance work:

- [ ] Human physical pad/fader interaction, color observation and listening
  acceptance of the Lua host; stopped-port smoke is not musical acceptance
- [x] Queued/callback-playing widget state tied to generation-tagged publication
  and callback consumption (19.15); DAC timing and physical acceptance remain open
- [x] Wire page/context fader soft takeover into the production MIDI path (19.13)
- [x] Production MIDI disconnect/reconnect and feedback restoration (19.14),
  accepted with simulated failures; actual cable acceptance remains open
- [ ] Real Stream Deck HID discovery/images and MicroLab live voice integration
- [ ] Shared typed, `u64` session migration, exhaustive capture, exact/quantized
  replay and crash recovery; legacy v1 still uses debug strings and retained events
- [ ] Render checkpointing/dependency scheduling justified by reproducible
  measurements; this slice deliberately preserves the existing renderer

Run the new playground with
`cargo run -- tui-song songs/examples/21-lua-apc-performance --lua main.lua --midi-debug`.
Nothing is stable yet. GitHub Pages still needs the setup step in 19.11.

### 19.13 MIDI fader takeover (headless acceptance complete)

- [x] Audit callback mapping, authoritative control values and existing pickup codec
- [x] Write example 22 and explicit acceptance criteria before implementation
- [x] Add failing headless tests for pickup, external edits, page changes and FIFO routing
- [x] Implement bounded control-thread input routing and physical pickup state
- [x] Connect typed commands, shared rendering, projections and recording
- [x] Preserve historical input defaults and run full compatibility/verification
- [x] Update API/site/capability status and prepare the verified checkpoint

The examples now prove distant observations produce no commands/audio/session
facts; crossing acquires control and publishes changed audio; Lua edits, page
away/back and modifier layers rearm it. Ownership acknowledgement preserves
quantized input from its controller while releasing other controllers. Typed
history gaps rearm safely. Both input and pickup storage have 64 slots, with
explicit overflow/invalid-target diagnostics. `--lua` enables pickup;
`--soft-takeover` opts TOML projects in while preserving historical defaults.
Unknown controls are blocked rather than assigned invented pickup values.

Verification commands and actual results:

- `cargo test --workspace`: 333 executions passed, zero failures; includes
  historical compatibility and the new immutable project in the pinned corpus
- `cargo test -p meldritch-control --test live_performance`: 8 passed,
  including real fader → command → audio publication → widget/session behavior
- `cargo test -p meldritch-cli control_thread_mapper`: 1 passed; actual CLI
  mapper/FIFO proves page ordering, absolute/centered/overdrive curves and layers
- `cargo clippy -p meldritch-control -p meldritch-render --lib --no-deps -- -D warnings`: passed
- `cargo clippy -p meldritch-cli -p meldritch-control --all-targets --no-deps -- -D warnings -A clippy::too_many_arguments`: passed; existing CLI lint allowance
- `cargo fmt --all -- --check`, `git diff --check`, and `npm run docs:build`: passed
- `cargo run -q -- validate-song songs/examples/09-curated-performance-controls`
  and `cargo run -q -- validate-song songs/examples/22-midi-soft-takeover`: passed
- `cargo run -q -- tui-song songs/examples/22-midi-soft-takeover --lua main.lua --no-autoplay --midi-debug`:
  real APC Control input/output opened, stopped transport reported zero
  underruns/misses, and `q` exited 0 with a clean captured session

No historical fixtures, audio callbacks or render scheduler were changed.
No functional regressions were observed. Enabled control-thread mapping uses
the existing TUI's 50ms poll cadence plus processing time; no throughput or
latency benchmark is claimed. Bursts beyond 64 observations are explicitly
dropped and counted. Unknown initial targets block pickup instead of guessing;
static projections are not audio-clock automation telemetry.

Remaining implementation and hardware acceptance:

- [ ] Physical fader pickup/page/layer interaction and listening acceptance
- [x] Automatic physical reconnect and fresh pickup ownership (19.14 headless);
  physical cable acceptance remains open
- [ ] Audio-clock automation telemetry and queued/audible widget states
- [ ] Typed session migration/replay and render scheduling work from 19.12

Nothing is stable yet. GitHub Pages setup remains open in 19.11.

### 19.14 MIDI connection recovery (headless acceptance complete)

- [x] Audit MIDI ownership, port discovery, feedback cache and pickup lifetimes
- [x] Write example 23's reconnect/failure acceptance before implementation
- [x] Add failing lifecycle and composed reconnect tests
- [x] Implement paced reconnect, stale-input rejection and device pickup reset
- [x] Connect physical input/output recovery and current-feedback restoration
- [x] Verify compatibility, workspace tests, lint and documentation build
- [x] Record results/remaining limitations and prepare the verified checkpoint

Production `tui-song` now retries missing/failed input and Lua APC output on a
one-second monotonic cadence. Connected port identity is probed; losing input
invalidates its queued observation lease, releases its pickup and uses typed,
captured commands to release held layers. Lua availability uses the existing
dispatcher and preserves the HSM; reconnection reconstructs the current RGB
frame. Write failures close output and delay retry. Lifecycle pumps retain the
render/capture observer for pending causal work. Availability still works after
a rewound playhead, without weakening musical-input ordering validation.

Verification commands and actual results:

- `cargo test --workspace`: 336 executions passed, zero failures, including
  the unchanged historical loader/compiler/range-identical render corpus
- `cargo test -p meldritch-control --test live_performance`: 11 passed;
  includes paced negative fixtures, cleanup, stale input, device-scoped pickup,
  unchanged musical facts/audio and real widget/observer recovery
- `cargo test -p meldritch-cli control_thread_mapper`: 1 passed; includes
  lost-note-off layer release and the historical direct absolute-input policy
- `cargo clippy -p meldritch-control -p meldritch-render --lib --no-deps -- -D warnings`: passed
- `cargo clippy -p meldritch-cli -p meldritch-control --all-targets --no-deps -- -D warnings -A clippy::too_many_arguments`: passed; existing CLI allowance
- `cargo fmt --all -- --check`, `git diff --check`, `npm run docs:build`: passed
- `cargo run -q -- validate-song songs/examples/09-curated-performance-controls`
  and `cargo run -q -- validate-song songs/examples/22-midi-soft-takeover`: passed
- `cargo run -q -- tui-song songs/examples/22-midi-soft-takeover --lua main.lua --no-autoplay --midi-debug`:
  APC Control input/output opened and remained connected through repeated probes;
  stopped transport reported zero underruns/misses, `q` exited 0 and saved
  `performances/session-20261011T034929Z.mlperformance`

No historical fixtures, audio callbacks, persistence format or render scheduler
changed. No functional compatibility regressions were observed. Legacy TOML
MIDI mapping now runs on the control owner with the same 64-observation FIFO and
50ms poll cadence as Lua; direct value semantics remain supported. Discovery,
open and transport destruction may delay UI processing, never the audio callback.
No latency benchmark or physical reconnect/listening acceptance is claimed.
This example adds a persistent transport scenario, not another authored format.

Remaining: physical cable/fader/color/listening acceptance, audio-clock widget
states, rewind/loop-aware Lua input clock epochs, typed replay, render scheduling
and GitHub Pages setup in 19.11. USB serial identity/event hotplug is deferred;
same-identity detach/reattach between polls may escape detection. Shared modifier
booleans lack per-device reference counts, so loss releases the declared shared
layer safely. Stream Deck HID and the standalone color demo do not acquire this
`tui-song` recovery lifecycle.
Nothing is stable yet.

### 19.15 Audio-aware widget presentation (headless acceptance complete)

- [x] Audit atomic publication, callback ownership and shared app/Lua projections
- [x] Write example 24 and acceptance before implementation
- [x] Add failing generation/presentation/failure acceptance tests
- [x] Add bounded callback generation telemetry and shared app projection
- [x] Connect queued/published/playing/error state to Lua and native TUI
- [x] Verify rapid supersession, stopped playback, reconnect, cleanup and compatibility
- [x] Update API/site/capabilities, record results and prepare verified checkpoint

Example 24's reusable Lua pattern widget now distinguishes queued, ready,
callback-playing, idle, unavailable and error states on APC/LCD/text projections.
Immutable snapshots carry their generation; the production callback acknowledges
present samples only after a completed, underrun-free buffer. Stopped/empty
callbacks do not advance acknowledgement and missing chunks clear it. The song
host maps bounded revision metadata to the shared app view outside realtime;
the new optional `song_audio` UI section renders those facts at 80×24 without
changing historical page layouts. UI fingerprint discriminants are explicit
and retain their old values; the ensemble hash is pinned to `2010d44585037a44`.

Verification commands and actual results:

- `cargo test --workspace`: 339 executions passed, zero failures; includes all
  historical load/compile/render tests, the new song and stable ensemble hash
- `cargo test -p meldritch-control --test live_performance`: 13 passed; actual
  dispatcher/commands/render worker/session capture/native 80×24 TUI and callback
  renderer prove queued vs published vs consumed, rapid supersession, error
  preservation, readonly Lua state, reconnect and restored Lua behavior
- `cargo test -p meldritch-control --test compatibility`: 2 passed
- `cargo test -p meldritch-audio --lib generation_ack`: 1 passed; stopped/empty
  buffers, present/missing samples, immutable tags and reserved generation
- `cargo clippy -p meldritch-audio -p meldritch-app -p meldritch-control --lib --no-deps -- -D warnings`: passed
- `cargo clippy -p meldritch-cli -p meldritch-control --all-targets --no-deps -- -D warnings -A clippy::too_many_arguments`: passed; existing CLI allowance
- `cargo clippy -p meldritch-tui --lib --no-deps -- -D warnings -A clippy::collapsible_if -A clippy::manual_checked_ops`: passed;
  strict TUI lib check without those allowances found two pre-existing warnings
  in key mapping and pattern-monitor division, not new audio-status code
- `cargo fmt --all -- --check`, `git diff --check`, `npm run docs:build`: passed
- `cargo run -q -- validate-song songs/examples/24-audio-aware-widgets`,
  `cargo run -q -- validate-song songs/examples/09-curated-performance-controls`
  and `cargo run -q -- validate-song songs/examples/17-launch-control-xl-ensemble`: passed
- `cargo run -q -- tui-song songs/examples/24-audio-aware-widgets --lua main.lua --no-autoplay --midi-debug`:
  real APC Control input/output opened; compact native song-audio panel showed
  both patterns ready, stopped transport reported zero underruns/misses, and
  `q` exited 0 with `performances/session-20261011T055245Z.mlperformance`

No functional compatibility regressions were observed. Historical fixtures,
session format and render scheduler are unchanged. Callback work adds scalar
bookkeeping and one atomic telemetry store on rendered/underrun buffers; it
adds no Lua, allocation, lock, parsing, logging or graph work. No throughput,
latency or DAC timing measurement is claimed. Four metadata slots retain
submitted/current/previous/last-identified revisions; a callback beyond retained
metadata is unknown rather than falsely identified. Opaque legacy scene patches
keep lane audio identity unavailable until an authoritative lane recompile.

Remaining: physical APC interaction/listening, DAC timing, quantized launch,
rewind/loop-aware input epochs, typed replay, Stream Deck HID and scheduler work.
Input clock epochs are next: existing Lua musical-input monotonic validation can
reject presses after loop wrap or rewind; connection observations are exempt,
but the musical timing/session migration has not been changed in this slice.
Older Lua fixtures keep their authored selection/global-transport color policy;
only scripts opting into the new audio projection claim callback-playing state.
Nothing is stable yet.

## Completed showcase milestones

The following work formed the previous implementation queue for the long-form
polyphonic demos. It remains here as completion history; broader phases live in
[`ROADMAP.md`](ROADMAP.md).

## 1. Deterministic polyphonic voice bank

- [x] Fixed-capacity voice bank around the existing stateful DSP voice
- [x] Deterministic idle/released/oldest voice allocation
- [x] Predictable voice stealing and note-off routing
- [x] Independent oscillator, ADSR, filter, glide, and modulation state per voice
- [x] Sample-identical full-buffer and split-chunk rendering
- [x] Headless chord and voice-stealing tests

Done when a chord can render deterministically through arbitrary chunk sizes and
voice allocation does not depend on worker order.

## 2. Polyphonic instrument rendering

- [x] Polyphonic pattern renderer and artifact fingerprint inputs
- [x] Worker-horizon chunk adapter with deterministic pre-roll
- [x] Mixed drum, monophonic bass, and chord-synth publication
- [x] Realtime host playback smoke test with zero underruns/misses
- [x] TUI diagnostics for active voices and stolen voices

Done when realtime playback can sustain chords while drums and bass continue.

## 3. Chord pattern model and editing

- [x] Represent chord tones as independent note lanes feeding one instrument
- [x] Typed note-lane and chord edit commands
- [x] TUI chord grid/inspector and selected-voice feedback
- [x] Chord inversion, transpose, velocity, gate, and probability controls
- [x] Exact dirty ranges for chord-tone edits

Done when chord tones remain individually editable and changes rebuild only
affected future artifacts.

## 4. Arrangement timeline

- [x] Ordered pattern instances extending beyond one grid loop
- [x] Repeat count, derived start time, transpose, track mute, and scene selection
- [x] Arrangement-aware event queries, sample rendering, and artifact keys
- [x] Transport position across sections and loopable arrangement ranges
- [x] TUI arrangement/section overview with active scene and loop highlighting

Done when a 24- or 32-bar piece can contain intro, variations, breakdown, chord
section, and full return without flattening everything into one pattern.

## 5. Parameter automation and scenes

- [x] Timestamped automation points with linear and stepped interpolation
- [x] Sample-accurate cutoff, resonance, envelope depth, drive, level, ducking, and modulation
- [x] Discrete waveform, voicing, mute, and scene changes
- [x] Automation segment fingerprints and exact point influence ranges
- [x] TUI automation inspector and active-scene explanation

Done when parameter changes happen over musical time and editing one segment
invalidates only intersecting chunks.

## 6. Long-form showcase

- [x] 32-bar fixture with multiple drum and bass variations
- [x] Chord progression and voicing changes
- [x] Filter/drive automation, kick ducking, and modulation
- [x] Accent scenes, breakdown, transitions, and full-pattern return
- [x] Offline WAV render, realtime demo command, manifest, and zero-underrun/miss smoke checks
- [x] Document controls and expected section sequence

Done when the demo plays repeatably, clearly changes over time, and exercises
arrangement, automation, polyphony, relational DSP, caching, and realtime output.

## 7. Event-aware effects

- [x] Typed delay and reverb send buses
- [x] Tag-matched send rules for accents and ghost notes
- [x] Deterministic delay and early-reflection reverb processing
- [x] Per-event explanation records for active sends
- [x] Effect-send fingerprints, finite tails, chunk equivalence, and semantic invalidation
- [x] Realtime coordinator integration and TUI send explanation panel

Done when tagged sends remain deterministic across chunk boundaries, participate
in cache invalidation, and can be inspected while the realtime demo plays.

## 8. Role-aware sidechain dynamics

- [x] Typed source roles and directional priority table
- [x] Deterministic attack/release envelope follower
- [x] Selectable low-band, high-band, or full-band ducking
- [x] Explanation model with roles, bands, envelope, and attenuation
- [x] Compile sidechain declarations into graph control edges for invalidation
- [x] Sidechain relation fingerprints, realtime coordinator, and TUI attenuation panel

Done when kick-role audio can duck selected bass bands through a declared
relation, edits invalidate dependent artifacts, and attenuation is visible.

## 9. Chunk transforms

- [x] Typed reverse, reslice, freeze, and smear specifications
- [x] Deterministic finite transforms with validation
- [x] Transform artifact keys covering source audio and parameters
- [x] Derived transformed sources in the relation graph and artifact cache
- [x] CLI capture, transform, provenance manifest, and optional playback
- [x] TUI capture, transform creation, cache status, and provenance controls
- [x] Atomic transformed-source audition and return-to-live controls

Done when a rendered bar can become a new cacheable, graph-connected playable
source and transforms can be created and inspected from the cockpit.

## 10. Speculative performance futures

- [x] Typed queued-scene, mute/unmute, and fill gestures
- [x] Deterministic recency, selection, and queue scoring
- [x] Candidate deduplication and bounded future plans
- [x] Gesture history with stable recency evidence
- [x] Renderable mute/fill/scene variants and artifact fingerprints
- [x] Score-ordered worker submission and future artifact cache
- [x] Sleep-on-clean behavior and worker diagnostics
- [x] TUI future-cache status and candidate inspection

Done when current and likely next performance states are pre-rendered in score
order, and workers sleep once every desired future artifact is clean.

## 11. Live performance execution

- [x] Beat/bar quantized gesture queue and cancellation
- [x] Deterministic active scene, mute, and fill state transitions
- [x] Speculative cache-hit selection with explicit live-render fallback
- [x] Atomic cached-artifact publication at the launch boundary
- [x] Temporary fill lifetime and automatic return to the base pattern
- [x] Scene queue, mute, fill, and cancel controls in the TUI
- [x] Queued/active launch state and fallback diagnostics

Done when scenes, mutes, and fills can be performed on musical boundaries,
switch cleanly to prepared audio when available, and remain safe on cache misses.

## 12. Self-performing live showcase

- [x] Installed `live-showcase` command with automatic transport start
- [x] Timed filter, waveform, chord, envelope, and drive performance score
- [x] Score reset and replay on transport loop boundaries
- [x] Manual cockpit input layered over the automatic baseline
- [x] Session history export with autopilot/performer provenance
- [x] Promote repeated performer edits into scored typed future evidence
- [x] Load and replay a saved performer-future library
- [x] Replace live invalidating autopilot edits with render-safe automation
- [x] Bound default workers and verify a prepared horizon before transport

Done when the default launch is musically dynamic without intervention and
performer deviations can be learned, ranked, and reused in later sessions.

## 13. Warehouse techno / big-beat showcase

- [x] Bounded hard-sync, phase-modulated, wavefolded oscillator mode
- [x] Oscillator automation and cockpit waveform-cycle integration
- [x] Phrase-bank model with quantized phrase changes and variations
- [x] 140 BPM breakbeat, acid bass, chord stab, and texture phrase set
- [x] Long resonant sweeps with pre/post-filter distortion staging
- [x] Performance-safe self-playing `warehouse-showcase` command
- [x] Deterministic phrase-choice evidence, ranking, and phase-aware replay selection
- [x] Realtime layout-safe phrase replacement through the render coordinator
- [x] Interactive warehouse cockpit and phrase-future learning

Done when an original high-tempo set cycles through recognizable phrases,
evolves without intervention, and accepts safe learned performance variations.

## 14. Warehouse session fidelity

- [x] Persist exact phrase/scene identity in performer future evidence
- [x] Replay learned phrase choices near their learned musical phase
- [x] Show learned and queued phrase identity in the cockpit status panel

Done when a saved warehouse performance reconstructs the performer's phrase
choices at musically similar moments and makes those decisions visible.

## 15. Expressive phrase performance

- [x] Direct `F1`–`F4` phrase pads with bar-quantized launching
- [x] Phrase-pad variation selection without changing the base scene
- [x] Performer override grace window for learned phrase cues

Done when specific phrases and their variations can be played deliberately
without learned automation immediately fighting a live decision.

## 16. Flashy DSP and production effects

- [x] Tempo-synced stereo ping-pong delay with filtered feedback
- [x] General tempo-aware LFO and modulation routing
- [x] Phaser and per-voice stereo spread
- [x] Master compressor, soft clipper, and limiter
- [x] Modulated reverb with damping, predelay, and freeze

Done when the live set has broad rhythmic modulation, animated stereo depth,
large controllable spaces, and a performance-safe master bus.

## 17. Learnable live DSP macros

- [x] Typed deterministic performance FX rack with bounded macro settings
- [x] Cockpit keys and realtime publication for all five DSP macros
- [x] Persist, rank, and phase-replay performer DSP gestures

Done when delay, phaser, reverb freeze, modulation, and master drive can be
performed safely and reconstructed from saved performer sessions.

## 18. Realtime performance hardening

- [x] Bound warehouse cockpit rendering to one phrase-length loop
- [x] Hold immutable audio during scene invalidation and preparation
- [x] Move DSP macro recomputation off the TUI input thread
- [x] Add host-side clean-playback soak diagnostics

Done when rapid scene and DSP changes remain audible without underruns on a
representative release build and expose enough diagnostics to catch regressions.
