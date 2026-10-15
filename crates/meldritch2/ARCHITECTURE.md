# Meldritch 2

Meldritch 2 composes musical machines from independently defined Rust modules.
Its first slices are an offline mono `f64` graph executor, isolated from the
prototype.

Each `src/units/*.rs` file owns one module type's ports, parameters, and
processing function. `contract.rs` defines their shared shape and explicit
registry. A module contract declares its Output role; compilation selects that
role rather than recognizing a module type string.
Input ports declare fan-in and a required or neutral disconnected value;
output ports declare only a name and signal domain. Their distinct Rust types
keep input-only policy out of source-port contracts.
Graph instances have unique string IDs. A port is an instance ID plus a named,
directional endpoint. Connections are explicit edges. Parameters are identified
by `(instance ID, parameter name)` from the same contract used by Lua discovery,
validation, and lowering. Authored values live in `Graph`; live overrides are
published through per-revision `ParameterHandle`s. `Machine::process` snapshots
all overrides once at buffer entry, so each buffer uses one effective value per
parameter. Clearing an override restores the authored value.
Lua can declare named controls bound to these same parameter IDs with a
presentation range. Rust validates the binding against the module contract;
`ControlHandle` publishes through the existing parameter cell. A control's
range limits that control's gestures, not DSP parameter validity. Handles are
scoped to one compiled revision.
`Panel` turns declared controls into selectable rows and bounded gestures.
`perform` renders those rows in a small terminal view while CPAL runs; it has
no fixed instrument knobs. This first view tracks the opened graph revision
and does not refresh itself after a structural graph replacement.
Lua may also bind channel 1..16 MIDI CC numbers to a named control. Graph
validation checks those declarations; `MidiRouter` precomputes a bounded raw
CC lookup and publishes through `ControlHandle`. DSP modules never see MIDI
bytes. The router belongs to one compiled graph revision. MIDI device opening
is explicit through `midi-ports` and `perform --midi=INDEX`. The input callback
only decodes and publishes CC values; the terminal view reads those values from
the same parameter cells. Physical CC gestures remain unverified.
Lua may also map a MIDI channel's note messages to named pitch and gate
controls. The MIDI adapter keeps a bounded last-pressed held-key stack per
mapped channel, converts the active note to Hertz, and publishes 0/1 gate
through those same control cells. Note-on with zero velocity releases a key;
releasing the top key restores the prior held pitch without dropping gate.
The controls must cover all MIDI note frequencies and 0..1 gate. This is a
buffer-boundary mono route. Each note channel owns a shared publication epoch;
the audio callback makes one bounded pair read at buffer entry. If a note
write overlaps that read, it reuses the prior complete pitch/gate pair for
that buffer. Note writers serialize on their bounded held-key state lock; the
audio path reads only atomics and its own cached pair. Other control gestures
on those parameters remain independent publications. Timed input and
velocity remain open.

Lua's `meldritch.graph()` API constructs a candidate graph. `graph:module(id,
type, values)` returns handles with contract-derived ports; `graph:connect(src,
dst)` records edges. Rust receives only graph data, validates names, values,
fan-in, and cycles, then derives a topological schedule and compiles slots and
module operations. `Host::submit` replaces the accepted machine only after full
compilation. A rejected candidate leaves it usable.
Each declared output port has its own compiled sample slot. A unit's process
function writes its declared outputs in contract order; cables read the named
source port's slot. The terminal Output role has a private sample slot but no
connectable output port. `Mirror` demonstrates two distinct outputs from one
module instance.

`LiveControl::submit` compiles a candidate before acquiring a short handoff
lock. `LiveRuntime::process` uses a nonblocking `try_lock` at buffer entry,
swaps in at most one complete candidate, and places the old machine in a
retirement slot. The control side reclaims and drops that old machine. Only one
candidate may be pending; while the handoff is busy, processing continues with
the current graph. This is a bounded in-process publication boundary, not a
physical audio host.

`Machine::process` fills a caller-owned buffer with no allocation, parsing,
validation, compilation, Lua, UI, or file access. Compiled operations read
inputs in contract port order from preallocated scratch space. Each operation
owns a fixed, contract-declared state vector allocated at compile time. `Sine`
uses one state value for phase and the machine's explicit sample rate; it keeps
phase across buffers and uses a published frequency at the next buffer entry.
Its optional `fm_hz` port adds an audio-rate Hertz offset to each sample's phase
advance; the current output sample is read before advancing phase. Self-feedback
still rejects as a cycle.
Device callbacks may exceed the fixed 256-frame scratch chunk. The callback
activates at most one pending revision and snapshots parameters once before
rendering all its chunks, so publication during a callback waits for the next
callback boundary.
`Mix` sums two named inputs; sources may fan out. Current ports are mono audio
at one sample per frame. `Multiply` multiplies two independent audio signals
per sample, supporting explicit audio-rate modulation through ordinary cables.
`gain.input` currently declares summed fan-in; its connected sources execute
in stable module-ID order. `gain.mod` permits one optional cable, using 1 when
disconnected; its audio-rate signal multiplies the authored/live Gain value
without changing parameter identity. Other inputs permit one cable. Cycles,
disconnected required inputs, and multiple Output instances are rejected. Feedback,
other domains, and block DSP remain future work. `Affine` demonstrates two
independently identified parameters on one module; the executor passes values
in contract order from preallocated scratch space. Parameter publication uses one
atomic `u64` per parameter; validation and handle lookup happen off the process
path. Handles from replaced revisions refer only to their old cells. The live
handoff uses a nonblocking lock attempt on the processing side; a busy handoff
delays activation to a later buffer.
`Lag` is a stateful signal processor with a contract-owned response range of
0..1. Authored values, live overrides, and declared control ranges are checked
against that one bound before publication; its state continues across buffers.
`Switch` chooses between two signals per sample from an independent selector
signal and a live threshold parameter; it uses the same contract and schedule
mechanism as the other units.
`SampleHold` detects upward threshold crossings on an audio-rate trigger input,
captures its separate signal input, and retains the value in compiled instance
state across buffers. This is a signal processor, not a general event domain.
`Envelope` follows a thresholded audio-rate Gate with separate attack and
release responses in 0..1; its compiled state continues across buffers.
Contracts expose domain and fan-in explicitly so these restrictions are visible.

`compile_at_rate` validates the sample rate; `compile` uses 48 kHz. Live graph
revisions inherit the active runtime's sample rate and start with fresh DSP
state. `DeviceOutput` opens CPAL's default device on explicit `play`, compiles
at its sample rate, and copies bounded mono `f64` chunks into interleaved device
channels. `devices` lists output endpoints and `--device=INDEX` selects one for
`play` or `perform`; this is host policy, outside the graph. The stream error
handler atomically records a bounded error category and count without formatting
or allocation; the first category is described outside the callback. Run
`cargo run -p meldritch2 -- crates/meldritch2/examples/08-sine.lua`,
`cargo run -p meldritch2 -- play crates/meldritch2/examples/08-sine.lua 3`,
`cargo run -p meldritch2 -- perform crates/meldritch2/examples/09-controls.lua`,
`cargo run -p meldritch2 -- midi-ports`,
`cargo run -p meldritch2 -- perform crates/meldritch2/examples/10-midi-control.lua --midi=INDEX`,
and `cargo test -p meldritch2`. Host smoke tests found underruns on the default
and PulseAudio routes, a busy Audio Expansion Card endpoint, and no stream
errors on the discard endpoint. The APC mini mk2 Control input opened in a
terminal session with the discard output. Physical CC gestures, listening, and
deadline reliability remain unverified. See `examples/ACCEPTANCE.md` for the corpus.
