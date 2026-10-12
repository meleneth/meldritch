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

## Remaining full Slice 3 contract

Filter unit adapters, broader explicit addition/clamping, modulatable LFO rate,
envelope-controlled depth, event/control-rate latching and global/per-voice scopes
remain requirements. Reset/stop/rewind and composition clocks need their full scope
contracts. General feedback requires an explicit executable delay. Native controls
and capture must retain stable semantic identities. Complete status must wait for
the full contract. Physical listening/deadlines and seamless DSP-state migration
require separate evidence; immutable PCM replacement alone does not prove them.
