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

## Remaining full Slice 3 contract

Broader explicit addition/clamping,
event/control-rate latching and global/per-voice scopes
remain requirements. Reset/stop/rewind and composition clocks need their full scope
contracts. General feedback requires an explicit executable delay. Native controls
and capture must retain stable semantic identities. Complete status must wait for
the full contract. Physical listening/deadlines and seamless DSP-state migration
require separate evidence; immutable PCM replacement alone does not prove them.
