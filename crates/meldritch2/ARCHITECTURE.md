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
`Mix` sums two named inputs; sources may fan out. Current ports are mono audio
at one sample per frame. Each input permits one cable. Cycles, disconnected required inputs,
and multiple Output instances are rejected. Feedback, multiple cables to one
input, other domains, block DSP, physical audio integration, and multi-parameter
modules remain future work. Parameter publication uses one
atomic `u64` per parameter; validation and handle lookup happen off the process
path. Handles from replaced revisions refer only to their old cells. The live
handoff uses a nonblocking lock attempt on the processing side; a busy handoff
delays activation to a later buffer.
Contracts expose domain and fan-in explicitly so these restrictions are visible.

`compile_at_rate` validates the sample rate; `compile` uses 48 kHz. Live graph
revisions inherit the active runtime's sample rate and start with fresh DSP
state. `DeviceOutput` opens CPAL's default device on explicit `play`, compiles
at its sample rate, and copies bounded mono `f64` chunks into interleaved device
channels. It reports the first stream error outside the audio callback. Run
`cargo run -p meldritch2 -- crates/meldritch2/examples/08-sine.lua`,
`cargo run -p meldritch2 -- play crates/meldritch2/examples/08-sine.lua 3`, and
`cargo test -p meldritch2`. The host device smoke opened a 48 kHz stereo stream
but reported buffer underruns; physical listening and deadline reliability are
unverified. See `examples/ACCEPTANCE.md` for the corpus.
