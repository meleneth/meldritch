# Composable control boards (first slice)

A Board is currently authored using the existing `meldritch.subpatch` API,
not a second parallel graph. It owns primitive instances, connections,
normalizations and explicitly exported inputs, outputs, and parameters.

## Compiled output ports

`Machine::output_slot(module, port)` performs one-time name resolution for a
primitive output or an exported Board output. Its result is a small `OutputSlot`
pointing directly into the prepared machine's contiguous sample storage.
`Machine::read_output(slot)` performs exactly one indexed storage read.
No string lookup, recursive traversal, callback or intermediate output copy
occurs on read. Exports of exports alias the same physical storage location.

Output slots are *revision-local*. Resolve again when the accepted Machine
changes. Control/UI threads must not concurrently read the audio thread's
mutable sample storage; use the established published snapshot machinery
for cross-thread observations.

## Stateful composition

The `counter` primitive is a small, sampled-edge counter, not a sequencer:
- `advance` and `reset` detect rising 0.5 crossings, with reset priority.
- `length` defaults to 8; `looped` defaults to true (1.0).
- `index` exposes zero-based position; `position` exposes 0..1 normalized position.
- Neither length nor looping is a special sequencer setting. Both are ordinary
  numeric inputs, compatible with normalled `constant` sources or other components.
- This initial primitive is *sampled edge* based, not a general lossless trigger
  delivery engine. Two events at the same sample cannot be represented here.

A board can normalize independent constant output components to these inputs,
expose the constants as editable parameters, and optionally expose the inputs
for an external cable to override their normalled source. Existing graph
expansion selects explicit wiring over authored normalizations.

The next necessary mechanisms for the complete ValueSequencer are an indexed,
runtime-length value table backed by prepared immutable storage, and proper
timestamped trigger routing. Neither should be hard-coded into a sequencer.
