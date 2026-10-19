# Design Philosophy

Meldritch is a software-defined environment for inventing, composing,
modifying, and performing musical instruments. Its fundamental unit of
creativity is the composition of behaviors, signals, modules, relationships,
and controls into an instrument.

This document is the design constitution. Current implementation choices are
evidence about what has been built, not authority over what Meldritch is
allowed to become.

## Eurorack is an inspiration, not a software specification

Eurorack makes construction immediate and musical. A performer can choose
modules, establish relationships with cables, exploit normalization, and
change the machine while it is sounding. The resulting instrument is not just
a preset for a synthesizer; it is the patch and the system that hosts it.

Meldritch should preserve that creative immediacy while removing constraints
that exist only because hardware is physical. A software-defined instrument
can potentially:

- create, replace, clone, and destroy modules dynamically;
- rewire complete networks as one coherent performance operation;
- define conditional, programmable, and context-sensitive normalization;
- turn a composition of modules into a reusable module;
- expose an arbitrary performance surface suited to its musical intent;
- adapt the same intent to radically different physical controllers;
- capture and reproduce complete state, including topology;
- automate structure as well as parameter values; and
- exceed practical limits of rack space, panels, cables, and module counts.

These are architectural ambitions. They must not be represented as features
that already work, but foundational decisions must preserve a credible route
to them.

## Authoring should have generative leverage

Mobilis is the architectural reference for the authoring philosophy: authors
compose reusable capabilities to describe systems, while the framework
provides the machinery that realizes those descriptions. Meldritch should
provide the same leverage for musical machines.

Lua describes instrument meaning and composition. Rust implements validated,
efficient execution. Interfaces interpret the capabilities the instrument
chooses to expose. None of these layers substitutes for another.

The quality of the environment is measured by how much musical territory an
author can reach through composition without changing the Rust execution
substrate. An abstraction that multiplies possible instruments is more
valuable than a feature that adds one familiar sound.

## Composition is the central abstraction

Modules are first-class composable objects. A useful module boundary may
contain DSP primitives, other modules, private state, ports, parameters, and
control semantics. A composition must be able to become a new module with a
deliberately selected interface.

Wiring is authored meaning, not merely an implementation pointer. Connections
relate compatible signals. Pitch and Gate are separate signals. Audio,
continuous control, events, and triggers may need different semantics, but the
system should introduce those distinctions through concrete instruments rather
than attempt to design an elaborate universal type system in advance.

Encapsulation must hide implementation without destroying composability.
Normalization must remain explicit and inspectable: a patch defines its
normalled behavior, a composition can override it, and future instruments
should be able to transform it dynamically.

Polyphony is principally a strategy for composing and instantiating behavior,
not a personality that every primitive must implement. The foundation must not
assume a subtractive voice, piano keyboard, sequencer track, or groovebox.

## Dynamism is part of the instrument

A graph compiled once and played forever is a useful first execution model,
not a permanent definition of an instrument. Topology can be performance
state.

The architecture must support an evolutionary path toward runtime graph
construction, reconnection, module instantiation, hierarchical composition,
programmable normalization, reusable graph transformations, structural
automation, and coherent multi-change commits.

Real-time safety remains non-negotiable. Dynamism does not mean evaluating
general-purpose Lua, allocating arbitrary structures, or blocking in the audio
callback. Authoring, control, graph preparation, publication, and real-time
execution are distinct phases with explicit ownership and lifecycle rules.

Likewise, a directed acyclic graph is a convenient currently implemented
schedule, not a philosophical ban on feedback. Feedback requires explicit
delay and execution semantics. The project should confront those semantics
through examples instead of silently defining feedback out of existence.

## Interfaces are generated from instrument intent

An instrument declares the affordances it wants to expose: continuous values,
discrete choices, actions, status, routing, and eventually structural
operations. Interfaces decide how to present those meanings.

A terminal, LaunchControl XL, APC mini mk2, MicroLab mk3, or future controller
is an interaction surface, not the source of instrument identity. An
instrument must remain meaningful when a particular controller is absent. The
system should adapt exposed intent to available surfaces rather than force all
instruments into one controller layout.

Interface work is architectural progress when it strengthens this projection
model. Counting widgets or supported devices is not, by itself, a measure of
Meldritch's expressive power.

## Examples are the specification

Concrete authored instruments are the primary acceptance criteria. Every
substantial architectural addition must enable something a musician can
construct, hear, inspect, manipulate, perform, capture, or replay.

Examples should demonstrate qualitative increases in expressiveness and expose
limitations honestly. A new primitive can be useful, but a second instrument
composed differently from existing primitives is stronger evidence that the
environment has grown. Avoid speculative frameworks without motivating
instruments, while refusing to let the first simple example impose permanent
limits on the language.

## Engineering discipline serves expression

Meldritch values explicit invariants, composition over special cases, clear
ownership, deterministic behavior where feasible, diagnosable failures,
test-driven implementation, bounded real-time costs, and small executable
vertical slices. Broad recovery must not hide broken invariants. Existing
exploratory compatibility is not a reason to preserve a wrong foundation.

Meldritch 1 was problem-space exploration. Meldritch 2 may reject its designs.
Past implementation effort does not outrank architectural coherence.

## How development priorities are decided

Evaluate proposed work in this order:

1. Does it increase the expressive power of authored instruments?
2. Does it strengthen the coherence of the composition model?
3. Is it demonstrated by a concrete, musically meaningful example?
4. Does it preserve a credible path to runtime structural dynamism?
5. Does it maintain clear separation between authoring, control, and real-time execution?
6. Can it be implemented and validated as a focused vertical slice?

DSP primitives, hardware integration, latency work, and interface components
remain important supporting work. They are not the project's organizing
purpose. The next milestone follows the philosophy and the most consequential
missing expressive capability—not the most recent subsystem or the longest
list of unchecked implementation details.

Success means that instruments are programmable, composable, inspectable, and
performable, and that authors can explore new musical territory without first
modifying the execution substrate.
