# Meldritch

Meldritch is a programmable environment for creating musical machines. It is
not a fixed synthesizer with a scripting layer. The instrument itself—its
behaviors, signals, modules, relationships, controls, and performance
surface—is the authored work.

Eurorack is an inspiration because it makes constructing an instrument part of
making music. It is not the ceiling. Software can create, clone, replace, and
destroy modules; change complete networks coherently; capture topology as
state; automate structure; and project the same instrument onto very different
interfaces. Meldritch exists to make those acts composable and performable.

The question guiding the project is:

> What becomes musically possible when constructing the synthesizer is itself
> a programmable act of composition?

## The division of responsibility

- **Lua is the composition and authoring language.** Authored instruments
  describe modules, connections, normalization, reusable compositions,
  exposed controls, performance surfaces, and eventually operations over
  structure.
- **Rust is the execution substrate.** It supplies DSP primitives, validation,
  lowering, scheduling, bounded real-time execution, state ownership, and
  device interoperability.
- **Interfaces are projections of instrument intent.** A TUI or hardware
  controller presents capabilities exposed by an instrument. It does not
  define what the instrument is or limit which instruments can exist.

This boundary is essential. Lua must become more than configuration for a
fixed Rust synthesizer, Rust must remain a coherent substrate rather than a
bag of unrelated plugins, and no particular UI or controller may become the
implicit instrument model.

## What works today

Meldritch 2 is the active implementation. It currently supports Lua-authored
graphs built from contract-defined Rust modules; named ports and parameters;
explicit connections and authored fallback normalizations; reusable nested
subpatches with exported ports and parameters; validation and compilation to a
bounded mono sample executor; stateful processing; control publication at
buffer boundaries; prepared graph replacement; instrument-declared controls
and terminal screens; and MIDI mappings, timed notes, and bounded polyphonic
voice assignment.

Those capabilities are foundations, not the product definition. Current
graphs are compiled as directed acyclic graphs, signal processing is narrowly
mono and sample-oriented, normalizations are selected during compilation, and
structural performance operations are not yet authored in Lua. Feedback,
general signal domains, coherent structural automation, topology capture, and
adaptive controller projection remain intended capabilities rather than
implemented claims.

The executable Lua corpus lives in `crates/meldritch2/examples/`. Its focused
acceptance guide is in [examples/ACCEPTANCE.md](examples/ACCEPTANCE.md).

## Try the current system

```sh
cargo run -p meldritch2 -- crates/meldritch2/examples/01-basic.lua
cargo run -p meldritch2 -- perform crates/meldritch2/examples/66-screens.lua
cargo test -p meldritch2
```

Audio and MIDI device commands depend on available host hardware. The active
repository check is:

```sh
bash tools/check.sh
```

Read [PHILOSOPHY.md](PHILOSOPHY.md) for the design constitution and
[ARCHITECTURE.md](ARCHITECTURE.md) for the boundary between verified reality,
current limitations, and intended architecture.
