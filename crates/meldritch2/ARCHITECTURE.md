# Meldritch 2

Meldritch 2 composes musical machines from independently defined Rust modules.
Its first slices are an offline mono `f64` graph executor, isolated from the
prototype.

`contract.rs` owns each module type's ports, parameters, and processing function.
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

Lua's `meldritch.graph()` API constructs a candidate graph. `graph:module(id,
type, values)` returns handles with contract-derived ports; `graph:connect(src,
dst)` records edges. Rust receives only graph data, validates names, values,
fan-in, and cycles, then derives a topological schedule and compiles slots and
module operations. `Host::submit` replaces the accepted machine only after full
compilation. A rejected candidate leaves it usable.

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
Contracts expose domain and fan-in explicitly so these restrictions are visible.

`compile_at_rate` validates the sample rate; `compile` uses 48 kHz. Live graph
revisions inherit the active runtime's sample rate and start with fresh DSP
state. `DeviceOutput` opens CPAL's default device on explicit `play`, compiles
at its sample rate, and copies bounded mono `f64` chunks into interleaved device
channels. `devices` lists output endpoints and `--device=INDEX` selects one for
`play` or `perform`; this is host policy, outside the graph. It reports the
first stream error outside the audio callback. Run
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
