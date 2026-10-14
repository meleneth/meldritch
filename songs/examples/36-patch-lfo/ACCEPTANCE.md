# Explicit graph LFO

Status: **design for full Slice 3**; the mono tremolo checkpoint compiles, renders,
edits and persists headlessly through the existing engine.
Existing note synthesis, independent pitch/gate, atomic typed patch commands,
worker PCM publication and authored project persistence must remain authoritative.

## First checkpoint: tremolo

From example 34/35's native mono patch, atomically disconnect env.control from
amp.level. Add slow:lfo, unipolar:control_scale, shape:control_multiply. Connect
slow.control → unipolar.control; env.control → shape.left;
unipolar.control → shape.right; shape.control → amp.level. One LFO may fan out
to multiple adapters/combiners. An LFO contains no destination policy.

The LFO explicitly declares shape, Hertz or quarter-note Beats period, phase,
audio-rate evaluation, per-instance scope and reset policy. Static period and
deterministic frame-origin phase reuse the existing Lfo kernel. Gate-rise reset
requires an explicit input.gate → slow.reset cable; no cable means no implicit
note reset. Reset is applied before evaluating that frame; adjacent gate events
are rises even without an intervening low sample. Pattern loops do not reset a
free-running LFO implicitly. Finite native playback repeats published one-bar PCM;
it does not promise a continuously running modulator across publication loops.

LFO output is bipolar dimensionless Control. The affine adapter declares gain,
offset and clamp endpoints and produces unit-interval dimensionless Control.
The multiplier requires two unit-interval Control inputs and declares multiplication
explicitly. ADSR outputs unit-interval Control. VCA level requires unit-interval
Control. Type family, unit and rate compatibility validate separately; direct
bipolar LFO → VCA level rejects rather than summing/clamping invisibly.
This checkpoint adds no implicit destination-specific modulation matrix.

Tests must prove tremolo against independent oscillator/envelope/LFO evaluation,
shared fan-out, reset at exact gate frames, no implicit reset, tempo/Hz timing,
full/split rendering, typed transaction failure with unchanged source/publication,
fresh-process authored save/reopen and explicit inspectable ports/cables/settings.
Negative tests: invalid/nonfinite period/phase/scale, missing inputs or reset cable,
unsupported evaluation/scope, unit/family mismatch, duplicate inputs, unused modules,
cycles, unsupported polyphonic lowering and palette/graph bounds. No unused cable
or module may silently disappear during compilation.

Checkpoint evidence: `cargo test -p meldritch-control --test graph_lfo` checks an
independent oscillator/phase/gate reference at every frame, 60/120 BPM, Hertz and
fractional beat periods, adjacent gate reset, shared fan-out, per-instance reset
origins, explicit VCA control without implicit envelope/gate multiplication,
every invalid case, atomic native drafts, typed capture reconstruction and
isolated fresh-process full/split PCM. `cargo test -p meldritch-cli native_graph_lfo`
checks playing worker/callback rate edits, exact stereo publication, retained
publication/history on invalid units, authored save and device-free execution.
Ctrl-E/Ctrl-G displays settings and Control unit/audio-rate metadata; the existing
scrollable inspector handles these added modules. LFO shape/rate/phase/reset and
scale/clamp edit through the ordinary typed draft commands in keyboard-patch.txt.
New controls are not automatically exposed as performance parameters.

## Next checkpoint: explicit vibrato

Apply vibrato-patch.txt to the tremolo patch. One shared LFO drives amplitude and
an explicit pitch_modulate adapter. Its pitch input is absolute semitone Pitch;
its control input is bipolar dimensionless Control. Output is Pitch:
input pitch + control × authored depth_semitones. Depth is finite in -24..24;
there is no implicit summing, gate coupling or pitch clamp. The adapter evaluates
at audio rate per instance and shares the existing dependency order and LFO phase.
The first lowering supports one adapter between input.pitch and oscillator.pitch;
other pitch chains and Plaits pitch modulation must reject until implemented.

Required tests: independent sample reference, zero/negative depth, fan-out,
exact split rendering, typed draft edits, rejection of missing/duplicate inputs,
wrong units/families/nonfinite depth/unsupported chains, unchanged source/history
on failure, and authored save/reopen with unchanged fingerprint and PCM.

Checkpoint evidence: explicit_vibrato_draft_fanout_depth_and_fresh_process_match_sample_reference
and invalid_pitch_adapters_leave_source_and_history_unchanged execute the draft,
sample references, typed capture vocabulary and isolated full/split reopen.
The native_graph_lfo worker test adds the adapter while playing and verifies
publication against authored PCM through the production callback without devices.

## Filter checkpoint: explicit Hertz adapter

Apply filter-patch.txt to the tremolo or vibrato patch. Insert the existing
low_pass in the explicit audio chain. control_hertz takes unit-interval Control
and emits Hertz Control via input × gain_hz + offset_hz, clamped to authored
clamp_min_hz/clamp_max_hz. Gain/offset are finite in -20000..20000;
clamps are ordered within 0.001..20000 Hz (defaults remain 20..20000).
The typed low_pass.cutoff input accepts
Hertz only. A cable replaces the filter's configured cutoff default; cutoff
overrides must reject on this cabled path. Uncabled historical defaults/overrides
retain their behavior. Existing filter DSP applies its documented physical
1 Hz..0.49 × sample-rate safety range after the authored adapter clamp.

One adapter may feed the one filter; no hidden sums, cross-family conversions,
additional filter chains or feedback. LFO → scale fans out to amplitude and
cutoff; vibrato may also share the same LFO. Evaluation remains audio/per-instance.
Required evidence: independent coefficient/filter recurrence reference across
silence/tails, full/split and fresh-process PCM, negative/zero gain and clamp
behavior, typed atomic drafts/settings/capture, worker publication while playing,
unit/range/missing/duplicate/unused adapter rejection with unchanged source/history.
Checkpoint evidence: filter_hertz_draft_matches_independent_recurrence_and_fresh_process
and invalid_filter_adapters_leave_source_and_history_unchanged check the draft,
settings/capture vocabulary, filter recurrence/tails, all negatives and isolated
full/split reopen. native_graph_lfo checks combined tremolo/vibrato/filter worker
publication while playing and failed-unit publication/history continuity.
Normalization remains a future authored policy; this first parameter port declares
its cable/default precedence explicitly rather than claiming configurable normals.
Historical automated-filter lowering retains its fixed audio/automation ordering
and rejects graph controls/cutoff cables rather than silently discarding them.
legacy_filter_automation_rejects_new_graph_controls_instead_of_ignoring_them
checks this compatibility boundary; it does not introduce a second graph engine.

## Depth checkpoint: explicit bipolar attenuation

Apply depth-patch.txt after vibrato-patch.txt, optionally with filter-patch.txt.
control_attenuate multiplies bipolar Control at its control input by unit-interval
Control at amount, producing bipolar Control. No implicit sum, offset or clamp;
it reuses the existing product kernel with independently validated port units.
It has no LFO or destination knowledge. Audio/per-instance evaluation is intrinsic
to this bounded primitive. LFO → depth.control and ADSR → depth.amount make an
envelope-controlled modulation source. depth.control fans out to the amplitude
scale and pitch adapter; the filter can share the scale output. ADSR settings
produce gradual attack, decay and release of depth. LFO phase/reset is unchanged.

Required tests: explicit source/unit metadata, atomic keyboard draft, full/split
sample references using the existing oscillator/ADSR kernels and independent LFO/
composition arithmetic, attack/decay/release/retrigger boundaries, fan-out and
fresh-process authored save/reopen. Removing the separate envelope → VCA product
must leave audible oscillation while envelope depth reaches zero: ADSR does not
inherently own amplitude. Missing/duplicate inputs, wrong units, unused modules
and signal/amount feedback reject with unchanged source/history/publication.
Typed capture reconstruction and playing worker publication must reuse existing
boundaries. Historical unit-interval multiplication and fingerprints remain intact.
Checkpoint evidence: envelope_depth_draft_fanout_capture_and_reopen_match_explicit_signal_reference
checks both amplitude-wiring variants, draft atomicity, attack/decay/release,
adjacent retrigger, units, isolated full/split reopen and full typed capture/source
reconstruction. invalid_depth_connections_reject_without_source_or_history_changes
executes every negative fixture. native_graph_lfo checks the combined depth/pitch/
filter worker publication and failed-amount-unit continuity while playing.

## Explicit bipolar sum and clamp checkpoint

Apply `sum-clamp-patch.txt` to the base tremolo graph. Two independent bipolar
LFO outputs enter `control_add.left/right`. Its output is dimensionless bipolar
wide Control in -2..2: addition does not clamp, scale or reset either source.
`control_clamp.control` accepts only that wide unit and emits bipolar Control
clamped to -1..1, which the existing scale maps to unit interval. Sum and clamp
are separate named modules and cables; neither has a destination policy.
Each input accepts one cable, while either source may fan out to other compatible
consumers. No implicit direct wide-to-bipolar or wide-to-unit-interval conversion
is allowed. The first lowering supports exactly two bipolar inputs and one
explicit clamp before existing consumers; chained wide sums remain future work.

Tests compare every sample against two independent LFO phases, arithmetic,
clamp, envelope and oscillator, including saturation, full/split PCM, typed
native draft/capture and fresh-process save/reopen. Invalid neighbors reject
unclamped destinations, missing/duplicate/wrong-unit inputs, unused clamps and
cycles without accepted-source or publication changes. Historical graphs and
fingerprints remain unchanged.

Apply `sum-fanout-patch.txt` after the sum/clamp, vibrato and filter drafts.
The one clamped sum fans out to the amplitude scale and PitchModulate; the
amplitude scale also feeds the Hertz cutoff adapter. An independent oscillator,
pitch-offset and filter-recurrence reference, split PCM, typed capture and
fresh-process reopen must agree. The clamped source is evaluated once per
instance/sample regardless of its destinations.

## Fixed control cadence checkpoint

Apply `cadence-patch.txt` to the base graph. `slow` still advances phase at the
audio sample rate, but its bipolar output is sampled and held every 64 frames,
anchored at the instance's frame zero. The first sample is at frame zero.
An explicitly cabled Gate rise samples immediately after phase reset even when
it falls between cadence ticks; the next scheduled tick remains on the original
64-frame grid. A free-running LFO ignores Gate. For a cabled Hertz rate, every
sample's rate advances phase, even while the output is held. Downstream scale,
multiply, pitch and cutoff adapters continue evaluating at audio rate with
the latest held LFO value. Other instances have independent phase and latch
state. Existing `evaluation = "audio"` remains samplewise and unchanged.

The accepted control period is 1..1024 frames; zero and larger periods reject
before source/history/publication changes. Tests compare every frame to an
independent phase/latch reference, including off-grid reset, rate steps,
full/split rendering and fresh-process save/reopen. Native edits and typed
capture retain the cadence setting. This checkpoint does not introduce Event
sampling, control-rate downstream nodes, smoothing or shared/global scope.

## Gate-event sampled LFO checkpoint

Apply `event-sample-patch.txt` to the base graph. `input.gate` fans out to the
ADSR and `slow.sample`. The LFO runs its phase at audio rate, but its bipolar
output remains zero until the first Gate rise, then holds the value sampled at
each Gate rise. Adjacent starts each count as an event, even without a low audio
sample between them. Gate ends, pattern loops without a start, and MIDI loss
cleanup do not sample. A separate `slow.reset` cable may reset phase before a
same-frame sample; without it, sampling observes free-running phase. The sample
source may be the bounded resolved GateHold output, which can also feed ADSR
and reset. This does not make Gate and Event the same signal family.

No sample cable may be present for audio or fixed-frame cadence, and event
cadence requires exactly one explicit Gate sample cable. Unsupported sources,
duplicate drivers and unused control nodes reject atomically. Tests compare
each frame to an independent free-running phase/event latch reference, including
off-grid adjacent starts, full/split and live partitions, typed capture and
authored save/reopen. A cabled Hertz rate still integrates every sample.
General event transforms, variable event cadence and shared/global scopes
remain open.

## Rate checkpoint: one modulator changes another's speed

Apply rate-patch.txt to the base patch or its pitch/filter/depth extensions.
clock LFO → unit-interval scale → explicit Hertz adapter → slow.rate. Hertz
adapters now permit authored clamps within 0.001..20000 Hz; defaults remain
20..20000. An LFO rate cable must originate at an adapter whose declared clamps
are within 0.001..100 Hz. Bipolar/unit-interval sources never become Hz implicitly.
The cable replaces the configured Hertz/Beats rate, which remains an inactive
default. The clock may be beat-synchronized or free-running; the cabled target
integrates instantaneous Hertz. Uncabled LFO evaluation stays exactly compatible.

At frame zero, or an explicitly cabled gate rise, target phase equals authored
phase. Evaluate the waveform at the current phase, then advance by that frame's
Hertz/sample_rate and wrap into [0,1). Thus rate at frame N affects phase N+1;
changing rate does not retroactively multiply elapsed frames or reset phase.
Reset precedes evaluation and increment; resetting the target never resets its
rate source. Pattern loops do not reset free-running phase. Dependency order
includes rate cables; sources and state remain per instance, evaluated once per
sample in bounded worker arrays. Finite published PCM still loops; this does not
promise uninterrupted phase across realtime graph revisions/publication loops.

Required evidence: independent discrete phase-integration/sample references,
tempo versus Hz rate-source timing, all four waveforms, cabled gate reset and
fan-out, full/split/fresh-process render equality, typed drafts/capture and playing
publication. Wrong units/ranges, missing/duplicate/unused inputs and rate feedback
reject with source/history/audio unchanged. Broader cadence/scopes remain open.

## MIDI keyboard entry checkpoint (user-prioritized)

midi-keyboard.txt demonstrates MicroLab mk3 input through the generic keyboard
decoder and existing typed note editor. Port selection is explicit, nonempty and
must match exactly one input; ambiguity rejects instead of choosing a device.
No MIDI opens without the flag, or during --check. A standalone diagnostic lists
ports without project files/audio and monitors decoded notes, velocity, releases,
14-bit bend, modulation CC1 and sustain CC64; configurable CC flags must not collide.

Native MIDI callbacks enqueue bounded observations only. Control-thread dispatch
inserts note-on into the visible construction palette editor at the current cursor,
with 480-tick duration clipped to the pattern end and velocity / 127. Existing
pitch/tick selects without overwriting it. Release, note-on-zero, malformed messages,
CC/bend, hidden editors and performance mode never author notes. Selection/cursor
are evaluated at dispatch, not hardware arrival. Pitch and gate bindings remain
independent; input does not silently connect a pattern to an instrument.

Disconnect invalidates queued observations from the old connection. Reconnect
requires the same unique name match; queue overflow is reported, never silently
claimed as lossless recording. Accepted commands use native capture, whole-project
save/reopen and the existing worker/publication path. Failure preserves source and
published audio. Headless acceptance must exercise decoded input, boundaries,
mode gating, velocity, capture/reopen and publication. Physical MicroLab gestures,
key-held audition, sustain/bend performance routing and timed recording remain open.

Evidence: native_midi_keyboard_entry_publishes_captures_and_reopens_without_live_voice_claims
checks velocity/channel, editor and mode gating, clipping, overlap rejection,
existing-note identity, every invalid/keyboard.toml case, stale connection events,
playing callback publication, typed reconstruction and authored save/reopen.
Keyboard transport tests cover unique matching and bounded overflow/epochs.
MicroLab mk3 port discovery and a real input connection were observed; physical
gestures/listening and reconnect interactions remain unverified.

## Live keyboard renderer checkpoint

live-keyboard.toml specifies independent timed Gate events at a u64 origin beyond
u32, matching the existing fixture's rhythm without consuming or changing its
patterns. Extract one shared mono DSP state/kernel from CompiledNotePatch; the
offline renderer and incremental worker renderer must execute that same kernel.
The live renderer starts a fresh instance at its declared origin, holding initial
Pitch and low Gate. Modulator age starts at zero there. Pitch changes do not
articulate; repeated high Gate explicitly retriggers; duplicate low Gate is
idempotent. Neither Gate nor keyboard release implicitly owns VCA amplitude:
the compiled cables determine it. This checkpoint has no velocity adapter yet.

Live blocks are sequential, at most 4096 frames, with at most 128 prevalidated
events. Events stay inside the block and in nondecreasing absolute frame order;
same-frame order is stable. Validate the complete block before state mutation.
Invalid range/order/pitch/budget must preserve the next cursor and all DSP state.
No re-preroll, looped held-note PCM or DSP in device callbacks is acceptable.
Retain oscillator/ADSR/filter/control phase and history across blocks. Mono only;
reject polyphonic live lowering. Existing worker-side Plaits resources/kernels
are reused, but physical listening and continuous graph-state migration stay open.

Required evidence: exact existing offline PCM, varied block partitions, pitch
under held Gate, explicit retrigger/reset, envelope release/filter tail and variable
LFO phase continuity, invalid-block recovery, u64 timeline and unchanged source.
The renderer foundation alone does not provide device-facing routing/publication.
The app lifecycle and stopped-transport audition extensions below now supply
bounded typed key ownership, streaming and capture. Sustain/bend adapters and
device-clock timed capture/replay remain unimplemented.

Evidence: live_mono_worker_blocks_preserve_existing_pcm_and_u64_event_timing checks
varied partitions against existing offline PCM for tremolo, cabled reset,
modulated rate/filter/release and FM/harmonic Plaits. Source/history/files remain
unchanged. live_pitch_gate_and_atomic_invalid_block_recovery_follow_explicit_cables
checks independent pitch/gate references, release/retrigger, free amplitude wiring,
all invalid/live-keyboard.toml cases, polyphonic rejection and origins near u64::MAX.
DSP state/cursor are ephemeral worker state; authored saves retain their existing
meaning. DSP state serialization and graph revision continuity remain open.

## Keyboard runtime lifecycle checkpoint

keyboard-lifecycle.toml is an executable runtime trace, not new project schema.
BindKeyboard explicitly chooses an active native mono instance and an absolute
u64 worker frame. Binding changes advance an epoch and clear held keys; emitted
release targets the previous instance through KeyboardUpdated.output_voice.
KeyboardKey carries that epoch, frame, channel and note. Pitch and Gate remain
separate outputs: press emits Pitch then high Gate, including repeated-key
retrigger. Last-pressed key wins. Releasing the winner resumes a still-held key
with Pitch alone; releasing the final key emits low Gate while pitch holds.
Releasing other/unheld keys never articulates. Channel is part of key identity.

At most 128 distinct keys are held. Reject invalid pitches/channels, backwards
frames, stale epochs, unbound input/reset and unknown/inactive targets before
changing runtime, source or history. Native polyphonic definitions already reject
at import; legacy sources cannot be bound. All invalid/keyboard-lifecycle.toml
cases require automated evidence. Equal frames preserve dispatch order.
Explicit ResetKeyboard reasons Disconnect, Overflow and Panic clear held keys,
release once and advance the epoch. Old queued commands cannot affect new state.
The audition adapter below now performs these resets and discards pre-reset
queued observations after overflow/disconnect.

Mode and inspector selection preserve binding. Accepted authored revision changes
invalidate it, clear held keys and require explicit rebinding; this is a fresh
voice policy, not seamless graph-state migration. Rejected source changes must
retain it. Runtime commands do not alter authored source/fingerprints or save
defaults. Native capture records their full typed arguments, including u64 frame
and reset reason. Fresh authored reload restores no held-key/binding state.
This checkpoint proves command reconstruction and identical worker PCM; it does
not implement wall-clock MIDI scheduling or timed device playback/replay.

Evidence: control/tests/keyboard_lifecycle.rs executes the trace into the shared
LiveMonoRenderer, checks nonzero PCM before final release and silence afterwards
for this envelope-wired patch, reconstructs the capture into identical signals/PCM,
and checks unchanged native source plus fresh authored reload. It tests all
negative cases, reset reasons, bound-capacity retrigger, mode and selection
preservation, authored invalidation and same-pitch/channel identity.
devices::keyboard_key_command lowers valid note packets with caller-supplied
epoch/frame; velocity-zero releases, channel 15 and u64 frames are retained.
Malformed packets, sustain/modulation/bend are not lowered to implicit controls.
This lifecycle checkpoint is extended by the production audition contract below.

## Stopped-transport keyboard audition checkpoint

keyboard-audition.txt demonstrates F9 binding of the selected active native mono
instance. Explicit --midi-keyboard selection is required; no-keyboard/no-selection,
inactive target and pending/playing transport reject before device/binding edits.
F9 unbinds a current target; selection and Ctrl-Tab preserve it. MIDI keys while
bound never author notes, regardless of editor/mode. Authored revisions invalidate
binding and require fresh F9; Play/Stop/Rewind release it. Saves/reopen retain only
authored definitions. Audition is mono while arrangement is stopped in this subset;
simultaneous pattern/live routing remains a future requirement.

Use the existing mono compiler/LiveMonoRenderer on a worker. Render 256-frame
blocks, normally at most 1024 frames ahead, into a fixed 4096-frame timed PCM ring.
Device clock and event frames are u64. Acceptance and scheduling share one locked
frontier on control/worker threads; the callback never takes that lock. MIDI
observations retain connection validity plus ingress binding epoch. Queued input
must not retarget after binding/reset. The callback copies finite PCM and rejects
stale/late data, without allocation/deallocation, DSP, files or logging. Advance
clock through silence and reject late samples instead of replaying them later.
On underflow, worker DSP catches up through skipped frames. Preserve state across
normal blocks and modes; never loop a held-note PCM clip. Existing device startup
mute/fade and safety conversion apply. Display held target/count and gap telemetry.

At most 128 events enter a live block; ordinary dispatch stops at 126 pending
events, preserving two slots for Gate low and bend center. Overflow/disconnect reset typed
app state, retain already accepted ordered events, invalidate buffered PCM and
drop pre-reset ingress. Unsupported CC/bend input must not trigger a reset or
implicit parameter change. Reset may briefly silence output; this contract does
not claim click-free transition or DSP-state migration.

Evidence: native_keyboard_audition_streams_shared_dsp_and_cleans_lost_input compares
real worker and production callback PCM with the shared direct renderer beyond
one bar, including retrigger/fallback/final release. It verifies source Arc/FP,
mode continuity, stale ingress, overflow/disconnect, authored/transport invalidation,
unbind and typed capture reconstruction with headless transport consumption.
pending_keyboard_budget_reserves_release_and_rejects_before_acceptance executes
deterministic backpressure at a u64 origin above u32. audio/tests/streaming_audio.rs
checks timed ring negatives, stale epochs/gaps/late data, transport cleanup,
conversion/clamp and zero callback allocation/deallocation. Negative cases are
listed in invalid/keyboard-audition.toml. Existing authored reload tests still
exclude live state. Device-opening failure reports unavailable audio with edits
retained; physical gestures/listening/latency are not headless acceptance.

Timestamped capture retains accepted scheduled frames, resets and fingerprints.
If checkpoint failure prevents input processing, release audition with a typed
unbind retained in bounded history for retry. Do not leave a sounding held gate
dependent on filesystem recovery. native_keyboard_capture_failure_releases_audition_and_retries_typed_cleanup
proves repair records the accepted key and cleanup exactly once. Ordinary
arrangement playback is unaffected by disabling this audition lane.
It does not yet record exact device-consumption gaps/activation or implement timed
replay. MIDI timestamp mapping, sustain adapters, polyphonic input,
simultaneous arranged/live routes and deadline measurements remain deferred.

## Explicit live velocity checkpoint

Copy `velocity.project.lua` to an empty project's `main.project.lua`. Open with
`--midi-keyboard MicroLab`, select bass in construction and enter Ctrl-G; execute
`velocity-patch.txt` line by line, committing the complete graph. Ctrl-S persists
the declared external input and explicit multiplier/cables. Stop arrangement and
F9-bind bass-instance: soft/hard strikes now scale amplitude. An atomic draft that
removes touch and reconnects shape.control to amp.level restores original gain.
Do not rewrite defaults implicitly during performance.

`input.velocity` is a declared Control port with fixed unit-interval units.
KeyboardKey carries MIDI-resolution 0..127, normalized by /127 once at this
input. Pitch, Velocity, Gate are ordered independent same-frame outputs. Repeated
press replaces a held key's velocity; releasing the winner restores prior pitch
and velocity without retrigger. Final release/reset lowers Gate and holds velocity
for explicit release-tail modulation. Fresh DSP initializes velocity to unity.
An unwired velocity has no effect. Unit mismatches and undeclared inputs reject
before source/audio replacement; arbitrary cable addition remains unsupported.

`keyboard-velocity.toml` specifies soft/hard/repeated/fallback gestures at a u64
origin above u32. `invalid/keyboard-velocity.toml` enumerates atomic input and graph
negatives. Evidence: `explicit_velocity_patch_restores_held_expression_captures_and_reopens`
checks the typed command-pane draft, gain against unscaled PCM, zero unwired
velocity, typed capture/identical reconstruction and authored fresh reload.
`invalid_velocity_inputs_and_graphs_are_atomic` proves all listed negatives,
including DSP cursor/state and source/history preservation after rejection.
The production worker/callback audition test now uses this wired velocity graph
with differing velocities and held fallback; allocation-safety contracts persist.
Old native v1 captures omitting velocity deserialize to 127 and retain fixed gain.

This checkpoint adds live keyboard velocity only. The later independent pattern
velocity checkpoint supplies the same Control from a separately bound pattern;
older unbound voices still initialize it to unity. Existing legacy coupled-note
velocity semantics remain unchanged. The later input-port checkpoint allows the
three supported expression declarations in Ctrl-G. Physical gestures/listening
and timed device replay still require separate evidence.

## Explicit live bend checkpoint

Copy `bend.project.lua` to `main.project.lua`; its bend/velocity ports are explicit
Control declarations. In Ctrl-G, apply velocity-patch.txt and bend-patch.txt and
save. Stop arrangement, F9-bind bass-instance, hold a key and move the wheel/strip.
`wheel` PitchModulate applies authored ±2 semitones through its explicit cables.
Changing pitch-depth changes range; unplugging/removing the adapter and restoring
input.pitch → osc.pitch removes bend's pitch effect. There is no default range.

MIDI values 0..16383 normalize to bipolar Control around exact 8192 center.
Lower-half offset divides by 8192, upper-half by 8191; endpoints are exactly ±1.
PitchModulate evaluates this held Control at audio rate on the existing worker;
wheel changes preserve oscillator phase. Fresh/offline independent-pattern input
is zero. Motion alone cannot articulate Gate or create notes. Unwired bend has
no frequency effect. The current bound mono instance owns one shared bend value
across MIDI key channels; channel remains captured, and key/fallback/mode/selection
never retarget or implicitly center it. This does not claim per-channel allocation.

Reset emits Gate low if needed and centers a noncentered bend even when Gate is
already low. Bind/unbind cleans the previous owner; new DSP starts centered.
Source/transport invalidation releases the instance and clears wheel ownership.
Ingress epochs prevent old wheel motion reviving it. Worker acceptance reserves
two of 128 event slots for cleanup; rejected capacity does not mutate app/history.
Sustain/other CCs and malformed wheel packets remain unmapped.

`keyboard-bend.toml` is the executable u64 gesture contract. Evidence:
`explicit_bend_composes_with_velocity_and_matches_independent_phase_reference`
checks both typed drafts, endpoint/midpoint frequency references with continuous
oscillator phase, exact partition PCM, unwired neutrality, native capture and
real atomic store reload with no saved wheel/key state.
`invalid_bend_commands_blocks_and_cables_preserve_accepted_state` covers every
invalid/keyboard-bend.toml case and state/cursor/source/history atomicity.
`wheel_ownership_survives_keys_and_resets_center_without_implicit_articulation`
checks keys/fallback, all reset reasons, no-gate centering, stale events, unbind
and source cleanup. Production worker/callback audition now composes bend/velocity
and checks loss centering; paused-worker backpressure checks exact reserved bounds.

Smoothing, combined LFO/bend pitch, Plaits pitch-adapter lowering, per-channel or
polyphonic allocation, simultaneous arrangement and timed device replay are open.
Physical wheel/pedal gestures, listening and deadline measurements are unverified.
Sustain requires an inspectable gate processor, not hidden MIDI note-off suppression.

## Remaining full Slice 3 contract

## Explicit live sustain checkpoint

`sustain.project.lua` declares a Gate input consumed
only through sustain-patch.txt's explicit gate_hold module. Physical Gate highs
retrigger; lows defer while hold high. Pedal cannot resurrect released Gate, and
pedal up releases only when physical Gate is low. Pitch/velocity remain independent
physical-key-owned signals. Configure the adapter explicitly with --midi-sustain-cc;
unsupported/malformed CCs and construction-only input stay unmapped. Capture typed
pedal events; saves retain declarations/wiring, never pedal or DSP state. Reset
must clear hold even when physical Gate is already low, reserving three cleanup
slots for Gate low/hold low/bend center. The first lowering supports one mono
oscillator GateHold to ADSR; Plaits and general gate routing require later contracts.
`keyboard-sustain.toml` and `invalid/keyboard-sustain.toml` specify the trace and
negative cases. `keyboard_sustain.rs` executes the typed Ctrl-G draft, compares
full/split PCM to an independent resolved-Gate reference, checks held sound,
retrigger and release tails, typed capture/reconstruction, unwired neutrality,
atomic rejection and authored store reload. The production worker/callback test
composes hold, velocity and bend across more than one bar and loss cleanup;
backpressure reserves the three cleanup slots. Physical pedal gestures and
listening remain unverified. `--midi-sustain-cc 64` is an explicit example mapping,
not a claim about the MicroLab mk3's configured pedal message.

## Resolved Gate fan-out checkpoint

Apply `fanout-gate.txt` after the velocity, bend and sustain drafts. The one
resolved `hold.gate` output feeds both `env.gate` and `slow.reset`. Physical
Gate rises, including retriggers while sustain is high, reset the LFO before
that frame is evaluated. Sustain can extend the resolved Gate without creating
another rise. A direct `input.gate → slow.reset` route remains valid. Each reset
input accepts one cable; duplicate drivers and unsupported Gate destinations
reject atomically. The direct ADSR path remains audible, and full/split PCM and
authored save/reopen agree. This is bounded Gate fan-out, not a general Gate
processor or a Pitch/Event mutation filter.

## Editable external-input checkpoint

`ports.project.lua` starts with only independent pitch/gate inputs. In Ctrl-G,
`input-ports.txt` stages typed declarations of velocity/bend Control and sustain
Gate, their explicit modules/cables, and one complete commit. The accepted patch
must render live strike gain, wheel pitch and pedal hold through the same mono
kernel as the earlier checkpoints. Save/reopen must retain declarations/cables
and reset runtime expression. A declaration alone does not create a controller
mapping or change sound. `input remove ID reject|disconnect` must respect cable
dependencies; removing a consumed input without a complete replacement graph
rejects atomically. `invalid/input-ports.toml` enumerates duplicate, name/type,
protected pitch/gate, dependency, missing and stale-draft cases. Legacy authored
patches and historical fingerprints must remain unchanged. This is a bounded
editor contract for the three currently executable expression inputs, not
arbitrary event/audio external-input lowering.

Evidence: `typed_input_draft_builds_expression_from_plain_pitch_gate_patch_and_reopens`
executes the command draft, checks staged-source isolation, audible
velocity/bend/pedal changes and exact authored save/reopen PCM. The paired
`invalid_input_drafts_and_dependent_removal_preserve_accepted_graph` executes
all invalid cases, a complete removal/reconnect transaction and typed capture
reconstruction of an input declaration.

## Independent pattern velocity checkpoint

`pattern-velocity.project.lua` binds three note patterns independently to one
mono instance: pitch, gate and `input.velocity` Control. The dynamics pattern's
note-start velocity is a unit-interval value held until its next start, including
across note ends. Its note pitch and duration do not articulate Gate or change
oscillator pitch. At the same frame, gate end/loop reset precede pitch and velocity
updates, which precede Gate rise. A dynamics start while Gate remains high must
change amplitude through the authored multiplier without retrigger. Unbound
velocity remains unity; pitch/gate patterns' own velocity values do not enter
this input implicitly. The pattern's note IDs remain stable; binding is typed,
captured and saved in native project Lua. Editing its notes must invalidate the
bound voice. `invalid/pattern-velocity.toml` specifies missing/wrong port,
unknown/incompatible pattern, simultaneous start, sample-event and invalid
note-edit rejection.
Exact full/split PCM, same-frame order, loop behavior, save/reopen, historical
fingerprints and failed-command source/publication continuity are required.
This is one per-instance event-to-Control bridge in the existing scheduler;
general control/event processors and realtime cadence remain future work.

Evidence: `pattern_velocity.rs` compares two loop cycles against an independent
unity render scaled at each authored velocity start, proves exact split PCM,
uncabled neutrality, TUI `u` binding, typed capture reconstruction, bound-voice
dirty range and edited audio, and authored store reload. All `invalid/pattern-velocity.toml`
cases reject before accepted source/history or render behavior changes.

## Independent pattern sustain checkpoint

`pattern-sustain.project.lua` binds a third note pattern to declared
`input.sustain` Gate. The pattern's timed intervals set hold high/low; note pitch
and velocity do not affect it. Its output changes sound only through the
inspectable `gate_hold` processor and cables. An unbound hold is low. Pattern
length and loop policy match the voice; overlapping hold intervals reject.

At a shared frame, an expiring hold falls before a new hold rises; hold rise
precedes physical gate fall, then pitch/velocity updates precede physical gate
rise. A hold rise coinciding with a key-gate fall defers release; a hold fall
after the key gate is low releases the envelope. At loop boundaries old hold
falls, the new hold start is applied, then the old physical gate falls before
new pitch/velocity and gate starts. No pedal state or authored defaults
are changed by this pattern relationship. Editing the bound hold pattern dirties
the voice. The native `h` key toggles its typed binding; project save/reopen
retains the relation without saving runtime envelope state.

`pattern_sustain.rs` must prove exact PCM against an independently extended
physical gate, full/split two-loop rendering, audible hold/release, typed
binding, source fingerprint and save/reopen. `invalid/pattern-sustain.toml`
must reject undeclared/wrong ports, bad pattern references/clocks, overlapping
events, sample events and invalid note edits without changing accepted source.
This is a bounded mono event-to-Gate bridge in the existing scheduler. General
gate routing, quantized event processors, Plaits lowering and simultaneous
arrangement/live input are future work.

## Pattern-controlled LFO rate checkpoint

`pattern-rate.project.lua` binds a third pattern to `input.velocity` Control,
then explicitly cables it through a Hertz adapter to `slow.rate`. Its note-start
velocity holds across note ends and loop boundaries until the next start. The
note pitch/duration never alters oscillator pitch or Gate. The LFO samples the
current Hertz value at audio rate: the value at frame N advances phase N+1,
without resetting phase on pattern steps or loops. The static LFO rate is an
inactive fallback while the cable is present. The same typed `u` binding,
note editor, source fingerprint and native project save/reopen are reused.

`pattern_rate.rs` must compare two-loop PCM sample-by-sample against an
independent oscillator and phase integrator, including exact step boundaries,
held values and split renders. Editing the bound pattern must dirty the voice
and change sound. The invalid fixture rejects an omitted adapter, wrong units,
missing input, invalid Hertz bounds, unknown pattern, simultaneous starts and
invalid edits without changing accepted source/history. Fresh reopen must
retain the graph and audio. No event-rate LFO sampling, arbitrary source sum,
general event processors or runtime DSP-state migration is claimed.
The headless live-keyboard path uses the same declared input and cables: MIDI
velocity 0/64/127 changes LFO speed at exact scheduled frames without an
implicit gate or phase reset. Device listening remains a separate check.

Broader explicit addition/clamping,
event/control-rate latching and global/per-voice scopes
remain requirements. Reset/stop/rewind and composition clocks need their full scope
contracts. General feedback requires an explicit executable delay. Native controls
and capture must retain stable semantic identities. Complete status must wait for
the full contract. Physical listening/deadlines and seamless DSP-state migration
require separate evidence; immutable PCM replacement alone does not prove them.
