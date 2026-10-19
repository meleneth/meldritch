# Authored-Instrument Acceptance

Examples are Meldritch's primary specification. A capability is meaningful
when it lets a musician construct, hear, inspect, manipulate, perform, capture,
or replay a musical machine. Tests of internal machinery support that claim;
they do not replace the authored instrument.

This document distinguishes reproducible evidence in the current tree from
future architectural challenges. The challenges are acceptance targets, not
claims that the current implementation already satisfies them.

## Running the current evidence

The active Lua corpus is in `crates/meldritch2/examples/`. The standard check
formats, lints, tests, and renders the basic example:

```sh
bash tools/check.sh
```

Focused commands include:

```sh
cargo run -p meldritch2 -- crates/meldritch2/examples/01-basic.lua
cargo run -p meldritch2 -- crates/meldritch2/examples/55-two-subpatch-voices.lua
cargo run -p meldritch2 -- perform crates/meldritch2/examples/66-screens.lua
cargo test -p meldritch2
```

Commands that open audio or MIDI devices require suitable host hardware and
must report failures rather than being treated as universally reproducible.

## Present executable evidence

The numbered examples and their tests currently demonstrate these foundations:

- **Authored topology:** primitive instances connect through named ports;
  declaration order does not determine execution order; invalid ports,
  fan-in, missing inputs, and cycles reject a candidate without replacing the
  accepted machine.
- **Compositional signal behavior:** shared sources, summing, multiplication,
  modulation, switching, sample-and-hold, envelopes, and stateful processing
  emerge from ordinary module connections rather than graph-specific executor
  cases.
- **Explicit normalization:** example 54 authors a fallback connection and
  demonstrates that an explicit cable overrides it during compilation.
- **Hierarchical foundations:** examples 55–62 define reusable subpatches,
  instantiate them independently, nest them, export selected ports and
  parameters, supply instance values, and fan an exported input into multiple
  internal destinations.
- **Authored performance intent:** examples 09, 19, 29, 66, and 67 expose named
  controls and project them into generic panels or authored screens with links
  and actions. The UI does not contain identities for those particular
  instruments.
- **Prepared and live execution boundaries:** tests cover parameter snapshots,
  candidate rejection, bounded graph handoff, timed events, reset, overflow,
  and callback chunking without moving Lua or compilation into sample
  processing.
- **Reusable voice construction:** examples 60 and 63–65 instantiate composed
  voices and route timed note state across multiple independent instances.
  This is useful substrate evidence, not a declaration that MIDI polyphony is
  the product goal.

These examples prove bounded slices of current behavior. They do not prove
general structural programmability, controller adaptation, or the full design
vision.

## Qualities required of new acceptance examples

A substantial new example should:

- create a musically meaningful capability unavailable through the preceding
  composition model;
- express the capability in Lua rather than hiding instrument orchestration in
  a one-off Rust path;
- state observable behavior and relevant failure behavior;
- identify invariants at authoring, preparation, publication, and real-time
  boundaries;
- preserve the accepted instrument when a candidate operation fails; and
- include focused automated tests while remaining understandable as an
  instrument a musician could use.

Parameter variants and additional primitive types can be supporting examples,
but they do not by themselves demonstrate increased authoring expressiveness.

## Challenge A: Compositional diversity

**Target:** Construct two meaningfully different playable instruments using
the same existing runtime and primitives without changing Rust orchestration
code.

Acceptance requires more than different constants or MIDI mappings. The two
instruments should organize behavior differently enough to demonstrate that
the authoring model, rather than a concealed fixed synthesizer architecture,
determines their identities. Their signal flow and exposed performance intent
must be inspectable from Lua, and both must execute through the same general
compiler and runtime.

**Status:** Accepted by `examples/instruments/subtractive.lua` and
`examples/instruments/drone.lua`. They use the same production Lua lowering,
contract registry, compiler, executor, control handles, and stereo output path
to construct a MIDI-played filtered polyphonic instrument and an ungated
self-running modular instrument. Integration tests load the authored files and
compare their observable audio and control behavior.

## Challenge B: Hierarchical composition

**Target:** Build a module out of existing modules, expose a deliberately
selected interface, and reuse it in another instrument.

Acceptance requires private internal structure, explicit exported ports and
parameters, independent state for multiple instances, and use from a parent
composition without parent knowledge of internal primitive names. Failure in
an invalid instance must preserve the accepted instrument.

**Status:** Future acceptance target. Current subpatch examples demonstrate
much of the mechanism, but this challenge remains open until a musically
deliberate reusable module and second instrument establish the architectural
claim as a whole.

## Challenge C: Programmable performance surface

**Target:** Have an instrument declare its controls and present those controls
through the TUI without hardcoding the instrument's identity into the UI
implementation.

Acceptance requires the Lua instrument to name and organize its affordances,
the compiled interface to validate references, and the generic TUI to render
and operate them through semantic handles. The instrument must remain valid
without a particular hardware controller.

**Status:** Accepted by the subtractive and drone instruments. Each declares
four named controls in Lua; the production control mechanism resolves their
authored targets, and the generic panel exposes exactly those public controls.
Allocator controls remain internal rather than becoming part of the presented
instrument.

## Challenge D: Structural dynamism

**Target:** Change an instrument's signal topology during performance through
an authored operation, with coherent graph transition semantics and without
unsafe real-time work.

Acceptance must identify the pre-change and post-change topology, define the
audible transition boundary and state lifecycle, prepare all structural work
off the audio callback, commit related changes atomically, and preserve the
running instrument if preparation fails.

**Status:** Not implemented. The current Rust machine handoff is supporting
infrastructure; Lua does not yet author a running structural operation.

## Challenge E: Programmable normalization

**Target:** Demonstrate an authored normalled connection that can be overridden
and restored, with explicit, predictable signal behavior.

Acceptance must make the fallback relationship inspectable, distinguish it
from an explicit cable, define override precedence, and demonstrate restoration
without an invisible module-specific default. To advance beyond current static
normalization, the operation should establish a credible path to conditional or
performance-time normalization.

**Status:** Future acceptance target. Static authored fallback selection is
implemented and tested, but programmable performance-time normalization is not.

## Interpreting progress

The challenges are not a linear feature checklist. Choose focused vertical
slices using the decision hierarchy in [PHILOSOPHY.md](../PHILOSOPHY.md).
Progress is the expansion and clarification of what instruments can be
authored—not the number of primitives, devices, widgets, or tests accumulated.
