# First vertical slice

`01-basic.lua` renders 0.125 per sample. `02-two-gains.lua` renders 0.0625.
`03-reordered.lua` renders the same samples as `01-basic.lua` despite declaration
order. `04-cycle.lua` is rejected with a cycle diagnostic and cannot replace an
accepted machine. Tests also verify invalid ports, parameter updates at a
processing boundary, and an accepted machine surviving candidate rejection.

Example B uses `01-basic.lua` with a live override, retaining the compiled graph:
`cargo run -p meldritch2 -- crates/meldritch2/examples/01-basic.lua --set=gain.gain=0.75`.
The first buffer is 0.125; the next is 0.1875.

## Two-input Mix milestone

`05-mix.lua` connects independent 0.25 and 0.125 Constants to the named
`mix.left` and `mix.right` inputs, then passes their sum through Gain(0.5).
Every sample must be 0.1875. Declaration order must not determine execution
order; `07-mix-reordered.lua` proves this. The compiler must use
contract-ordered input connections, with no Mix topology case.
`06-mix-missing.lua` must reject the disconnected right input.
Tests also reject two cables to one Mix input, prove source fan-out, and keep
an accepted machine usable after a rejected Mix candidate.

## Control publication milestone

Use `05-mix.lua` as the authored graph. Resolve the stable `gain.gain` parameter
once on the control side and retain a handle. A control thread can publish 0.75
without borrowing the machine or changing the authored 0.5. The next complete
`process` buffer must render 0.28125 throughout; clearing the override returns
to 0.1875. Invalid values must be rejected. A handle from a replaced graph
revision must not control the replacement, even when the parameter ID is the
same. Processing performs only bounded atomic reads and preallocated writes.

## Bounded graph activation milestone

Compile `05-mix.lua` off the processing path while `01-basic.lua` is active.
Queue the completed candidate and activate it at the start of a later buffer:
the old buffer is all 0.125 and the new buffer is all 0.1875. The processing
path must neither compile nor drop the retired machine. The control side
reclaims it. A rejected cycle candidate leaves the active machine and any
queued valid candidate intact. Only one replacement may be pending at a time;
additional submissions receive a useful diagnostic. Activation may be delayed
when the short handoff lock is busy, without blocking audio processing.

## Stateful sine milestone

`08-sine.lua` compiles at an explicit 48 kHz sample rate. Its 12 kHz Sine feeds
Gain(0.25), yielding the repeating sequence approximately `[0, 0.25, 0,
-0.25]`. Splitting rendering across buffers must produce the same sequence
as one buffer; the oscillator phase belongs to its compiled instance. A live
frequency update must take effect at the next buffer boundary without resetting
phase. Invalid sample rates are rejected before a machine is accepted.

## Device output milestone

`meldritch2 play crates/meldritch2/examples/08-sine.lua 3` opens the default
output device only when explicitly requested. The device's sample rate is
passed to graph compilation. The callback renders bounded mono chunks into
preallocated `f64` scratch space, clamps to the device range, and copies each
sample to every output channel. Headless tests verify chunk boundaries,
interleaving, and sample conversion. Device opening or playback failure reports
a diagnostic; actual hardware sound remains a physical check.

## Instrument-declared control milestone

`09-controls.lua` names `level` and binds it to `gain.gain` over 0..1. Rust
must resolve that target from the same module contract used by graph validation.
Initial output remains 0.1875; publishing 0.75 through the named control makes
the next buffer 0.28125. Out-of-range values, unknown parameters, duplicate
control IDs, and invalid ranges are rejected. A retained control handle from a
replaced graph revision cannot change the new machine. Controls are presentation
and performance bindings; their ranges do not redefine DSP parameter validity.

## Terminal performance milestone

`meldritch2 perform crates/meldritch2/examples/09-controls.lua` opens the
declared graph and shows its named controls. Up/Down selects a control;
Left/Right changes it by one percent of its declared range; Home/End choose
its bounds; Q exits. The view renders only instrument-declared controls and
publishes through `ControlHandle`. A headless panel test verifies selection,
value changes, bounds, and the resulting graph samples. Physical key gestures
and listening remain unverified on this host.

## Explicit output selection checkpoint

`meldritch2 devices` lists output devices by index. `play` and `perform` accept
`--device=INDEX` to select one explicitly. The selection is resolved before
stream creation; indices reflect the current host enumeration, and an invalid
index reports a diagnostic. This keeps the DSP
graph independent of host device policy. The host default route reported
buffer underruns in both debug and release smoke tests, so the selectable
devices must be checked separately before claiming physical playback. On this
host, the discard endpoint ran without stream errors, PulseAudio still
underran, and the Audio Expansion Card endpoint was busy. No listening result
is claimed.

## Declared MIDI CC routing milestone

`10-midi-control.lua` binds channel 1 CC 7 to the authored `level` control.
Rust validates channel, CC number, target control, and duplicate assignments.
Raw `[0xB0, 7, 127]` publishes level 1 and the next graph buffer renders
0.375; `[0xB0, 7, 0]` publishes level 0 and renders silence. Unbound CCs,
other channels, malformed messages, and note messages leave it unchanged.
The MIDI adapter uses the same `ControlHandle` as the terminal view; no DSP
module interprets MIDI bytes. Hardware connection is a later checkpoint.

## MIDI input checkpoint

`meldritch2 midi-ports` lists host input ports. `perform` accepts
`--midi=INDEX` alongside optional `--device=INDEX`; it opens that port and
routes incoming bytes through the graph's declared CC mapping. Port selection
is explicit. Invalid indices and connection failures report diagnostics.
The connection is held for the performance session and closed on exit.
The terminal view displays the selected port and reflects published CC values;
subsequent keyboard gestures start from the current value.
Headless routing acceptance above remains the authority for signal behavior;
physical CC gestures require a separate device check.

## Audio multiplication milestone

`11-audio-multiply.lua` connects an independently instantiated Constant and
Sine to the two inputs of Multiply. At 48 kHz its output repeats approximately
`[0, 0.25, 0, -0.25]`. Modules are declared out of dependency order, and the
compiler must derive the schedule without a Multiply-specific topology case.
The same signal must render across split buffers. `12-audio-multiply-missing.lua`
must reject a disconnected right input without replacing an accepted machine.
`13-two-sines-multiply.lua` connects two time-varying signals and must render
`[0, sqrt(1/2), 0, -sqrt(1/2)]` for its first four frames at 48 kHz.
Current ports are mono audio at sample cadence; this demonstrates audio-rate
signal multiplication, not a new control-rate domain or implicit parameter
modulation rule.

## Declared input fan-in milestone

`14-gain-fan-in.lua` sends two Constants to the same `gain.input`. That port's
contract explicitly sums incoming samples before applying Gain(0.5), yielding
0.1875 per sample. The two cable declarations and module declarations are out
of dependency order. Reversing either order must produce the same schedule and
samples. `15-output-fan-in-invalid.lua` must reject a second cable to
`output.input`, whose contract still permits one, without replacing an accepted
machine. Fan-in aggregation belongs to the port contract and generic executor;
there is no graph topology case for these three module types.

## Multiple parameters per module milestone

`16-affine.lua` applies `input * scale + offset` with independently authored
`transform.scale = 0.5` and `transform.offset = 0.125`, rendering 0.25 per
sample. The contract exposes both parameters through Lua and inspection. A
control-side update to `transform.scale = 0.75` must render 0.3125 at the next
buffer boundary while `transform.offset` remains authored at 0.125. Clearing
the override restores 0.25. The executor must use contract parameter order
without recognizing Affine as a special topology or allocating in processing.

## Optional audio-rate modulation input milestone

`17-gain-modulated.lua` connects a 0.25 Constant to `gain.input` and a 12 kHz
Sine to `gain.mod`. With authored `gain.gain = 0.5`, its output at 48 kHz
repeats approximately `[0, 0.125, 0, -0.125]`; overriding the parameter to
0.75 changes the next buffer to `[0, 0.1875, 0, -0.1875]` without changing
the graph. The modulation cable does not change the authored or effective
parameter value. Existing Gain graphs leave `mod` disconnected and receive its
contract neutral value of 1. `18-gain-mod-missing-input.lua` must still reject
a disconnected required `gain.input` and preserve an accepted machine.

## Independent controls on one module

`19-affine-controls.lua` exposes `transform.scale` and `transform.offset` as
separate named controls with distinct channel-1 CC assignments. It begins at
0.25 per sample. CC 7 value 127 sets scale to 1 and renders 0.375; CC 8 value
0 sets offset to -0.5 and renders -0.25. Each gesture must leave the other
parameter unchanged. The TUI and MIDI adapter resolve both names through the
same compiled contract and parameter IDs; no per-module control catalogue is
added.

## Explicit audio-rate frequency modulation

`20-sine-fm.lua` connects a 12 kHz Sine through Gain(6000) to the optional
`carrier.fm_hz` port of a 6 kHz Sine. The input sample is a Hertz offset to
the authored frequency for that sample's phase advance; the output sample is
read before that advance. At 48 kHz the first four frames are approximately
`[0, sqrt(1/2), sqrt(1/2), 0]`, and splitting processing across buffers must
match one complete buffer. Publishing `depth.gain = 0` between buffers removes
modulation on the next buffer without reconstructing the graph or resetting
carrier phase. Existing Sine graphs leave `fm_hz` unconnected and receive 0.
`21-sine-fm-cycle.lua` must reject self-feedback as an unsupported cycle while
leaving an accepted graph usable.

## Shared modulation composition

`22-shared-modulator.lua` fans one Sine into two ordinary processing paths:
Gain scales it to a Hertz offset for `carrier.fm_hz`, while Affine shapes it
into a 0..1 factor for `level.mod`. The carrier's output passes through level
to Output. At 48 kHz the first four frames are approximately
`[0, sqrt(1/2), sqrt(1/2)/2, 0]`. The graph is declared out of dependency
order; reversing module and cable declaration order must preserve the schedule
and samples. A split buffer must match a continuous buffer. The compiler adds
no case for the combined FM/AM shape.

## Stateful Lag and parameter bounds

`23-lag.lua` connects Constant(1) to Lag(response=0.5) and Output. Lag starts
at zero and applies `state += response * (input - state)` each sample, yielding
`[0.5, 0.75, 0.875, 0.9375]` for the first four frames. Split buffers must
match a continuous buffer. Publishing response=1 at a buffer boundary makes
the next buffer all 1 without resetting state. The contract owns the inclusive
0..1 response bound; authored values, live overrides, and declared control
ranges use it. `24-lag-invalid.lua` rejects response=1.5 and
`25-lag-control-invalid.lua` rejects a control range extending below zero.
Both rejected candidates leave an accepted machine usable.

## Smoothed instrument control composition

`26-smoothed-control.lua` binds channel-1 CC 7 to a Constant named `motion`.
That Constant feeds Lag(response=0.5), whose output feeds `gain.mod` while an
independent Constant(0.25) supplies `gain.input`. The authored graph starts
silent. CC value 127 publishes motion=1; the next four samples must be
`[0.125, 0.1875, 0.21875, 0.234375]`. CC value 0 then publishes motion=0;
the next four samples must decay to `[0.1171875, 0.05859375, 0.029296875,
0.0146484375]`. Neither gesture reconstructs the graph. The MIDI adapter,
parameter cell, Lag state, and Gain port compose without a special route.

## Audio-rate Switch composition

`27-switch.lua` selects between Constant(0.25) and Constant(-0.25) using a
12 kHz Sine at 48 kHz. `Switch` chooses its right input when the selector
sample is at least the threshold; the threshold starts at 0.5. Its first four samples are
`[0.25, -0.25, 0.25, 0.25]`. Module and cable declarations are out of
dependency order, and splitting an eight-sample render across buffers must
preserve the result. A live override of `switch.threshold` to 2 makes the
next buffer all 0.25, with the authored value and topology intact. Clearing
the override restores audio-rate selection. `28-switch-missing.lua` omits the
required selector cable and must reject the candidate while the accepted
machine continues to process.
