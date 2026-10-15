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
