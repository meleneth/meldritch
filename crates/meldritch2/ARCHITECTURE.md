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

Lua's `meldritch.graph()` API constructs a candidate graph. `graph:module(id,
type, values)` returns handles with contract-derived ports; `graph:connect(src,
dst)` records edges. Rust receives only graph data, validates names, values,
fan-in, and cycles, then derives a topological schedule and compiles slots and
module operations. `Host::submit` replaces the accepted machine only after full
compilation. A rejected candidate leaves it usable.

`Machine::process` fills a caller-owned buffer with no allocation, parsing,
validation, compilation, Lua, UI, or file access. Compiled operations read
inputs in contract port order from preallocated scratch space. `Mix` sums two
named inputs; sources may fan out. Current ports are mono audio at one sample
per frame. Each input permits one cable. Cycles, disconnected required inputs,
and multiple Output instances are rejected. Feedback, multiple cables to one
input, other domains, block DSP, graph activation during audio playback, and
multi-parameter modules remain future work. Parameter publication uses one
atomic `u64` per parameter; validation and handle lookup happen off the process
path. Handles from replaced revisions refer only to their old cells. Graph
replacement itself still requires external coordination with any audio thread.
Contracts expose domain and fan-in explicitly so these restrictions are visible.

Run `cargo run -p meldritch2 -- crates/meldritch2/examples/01-basic.lua` and
`cargo test -p meldritch2`. See `examples/ACCEPTANCE.md` for the corpus.
