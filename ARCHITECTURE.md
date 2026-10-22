# Architecture

Meldritch separates authored instrument semantics from preparation, real-time
execution, and presentation. This document describes the desired boundaries,
then states separately what is implemented, what currently limits the system,
and what remains intended. Aspirations are not implementation claims.

## Architectural boundaries

### Authoring and composition

Lua is the language in which an author describes a musical machine: modules,
instances, ports, connections, normalization, exposed parameters and controls,
performance affordances, and eventually transformations over structure.

Authored objects express instrument meaning. They must not expose executor
slots, callback buffers, device handles, or scheduler accidents as their
identity. A composition of modules must be capable of acquiring a stable,
deliberately narrow module interface and being reused like any other module.

### Validation and preparation

Candidate instruments cross a validation boundary before they may execute.
Contracts define primitive ports, parameters, state requirements, and process
functions. Validation resolves names, domains, connection policy, parameter
bounds, controls, and interface references. Preparation lowers accepted
meaning into an execution form with known storage and costs.

Invalid candidates do not damage the accepted instrument. Multiple related
structural changes should ultimately be prepared and committed as a coherent
operation rather than becoming visible piecemeal.

### Real-time execution

Rust owns the real-time machine: schedules, sample storage, DSP state, bounded
publication mechanisms, and device callbacks. The process path must not parse
Lua, resolve names, validate graphs, access files, perform unbounded allocation,
or block on authoring and control work.

The executor is an implementation of instrument semantics, not the source of
those semantics. New authored compositions should reuse general lowering and
execution machinery rather than require orchestration cases for each musical
shape.

### Control and structural publication

Parameter gestures and structural changes have different costs, but both need
explicit transaction boundaries. A control publication becomes visible at a
defined processing boundary. A structural publication prepares a complete
candidate away from the callback and activates it coherently. Ownership of the
old and new machines, including retirement, must be explicit.

Future structural operations may be authored and automated, but the audio
callback remains a bounded consumer of prepared state. Runtime dynamism is not
permission to make the callback a graph editor.

### Presentation and interoperability

An instrument exposes semantic affordances. TUI views and hardware adapters
project those affordances onto interaction surfaces. Controller-specific
layout, transport, and lifecycle policy live outside DSP modules and must not
constrain the instrument model.

## Core invariants

- Authored instrument semantics are independent of a particular UI, controller,
  audio device, and callback size.
- Primitive contracts are the shared authority for validation, authoring
  discovery, and lowering.
- Connections and normalizations are explicit authored relationships.
- Compositions preserve encapsulation while exposing intentional interfaces.
- A candidate is accepted only after complete validation and preparation;
  rejection preserves the last accepted machine.
- Real-time processing uses prepared, owned storage and bounded operations.
- Parameter and graph revisions become visible only at defined boundaries.
- Presentation reads and publishes through compiled semantic handles; it does
  not execute Lua or reach into DSP implementation state.
- Convenient current restrictions do not silently become permanent product
  laws. In particular, feedback requires future explicit semantics rather than
  a permanent assumption that all useful graphs are acyclic.

## Implemented and verified

The following statements are supported by the current Meldritch 2 source and
automated tests:

- Lua constructs graph data containing primitive module instances, explicit
  named connections, authored normalizations, controls, MIDI bindings, screens,
  and reusable subpatch definitions.
- Rust owns an explicit primitive contract registry. Compilation validates
  module types, ports, parameters, fan-in, required inputs, output selection,
  normalization, controls, screens, and MIDI targets.
- Nested subpatch instances expand to namespaced primitive graphs. Subpatches
  can export inputs, outputs, and parameters; each instance receives independent
  primitive DSP state.
- Accepted acyclic graphs compile to a stable operation schedule and
  preallocated sample/state storage. The mono `f64` process path performs no
  Lua evaluation, graph validation, file access, or graph compilation.
- Parameter handles publish finite, validated values through revision-scoped
  cells. Processing snapshots values at buffer or host-callback boundaries.
- A prepared machine can be handed to a live runtime and activated at a buffer
  boundary with nonblocking processing-side coordination; rejected or busy
  submissions preserve the current machine.
- Instrument-declared controls can be projected into a generic terminal panel
  or Lua-declared screens. Links and fixed-value buttons operate through the
  same compiled control handles.
- MIDI CC, mono note, timed note, velocity, reset, and bounded two-to-four voice
  assignment routes are implemented without placing MIDI parsing in DSP
  primitives.
- CPAL output compiles at the selected device rate and renders bounded chunks
  into interleaved host buffers.
- Sequencers advertise source-scoped asynchronous event contracts. Lua can
  subscribe persistently with `sequence:on(...)` or for a screen lifetime with
  `screen:on(...)`. Per-kind atomic interest counts gate fixed-capacity SPSC
  publication, and callbacks run only when the UI-thread owner drains events.
  With no relevant subscriber, the producer does not construct a payload or
  touch the queue. These lossy observations never drive musical correctness.

The executable evidence is concentrated in the Meldritch 2 unit tests and the
Lua corpus under `crates/meldritch2/examples/`.

### Sequencer event contracts

Every delivered event contains `kind`, numeric `source_id`, authored `source`
name, and the zero-based absolute `sample_position` at occurrence time. Lua
indices remain one-based: `step_advanced` adds `step`, `pattern`, and monotonic
`position`; `note_on`, `note_off`, and `voice_triggered` add `track`, `pitch`
in Hz, and `velocity`; `pattern_changed` adds the newly committed `pattern`.
Events preserve production order within one sequencer source. No global order
between independent producers is promised.

Each source owns a fixed-capacity SPSC queue. A full queue drops the newest
observation and increments its diagnostic counter; it never blocks audio.
Registration and removal are serialized by the Lua owner and become visible at
the producer's next atomic interest check; a lifetime epoch prevents queued
events from an earlier mount reaching a later subscription.
Callbacks execute later on the retained Lua program's UI thread. Callback
errors are recorded with source and event kind, do not stop other subscribers,
and are reported when the TUI exits. Disconnect is idempotent; a disconnected
callback does not receive already queued events, dead sources are pruned, and
runtime shutdown removes remaining interest. Current authoritative state can
always be queried after an observational drop.

## Current limitations

These are implementation facts, not desired permanent boundaries:

- Graph scheduling rejects cycles. Feedback and its delay semantics are not
  represented.
- Runtime structural replacement exists as a Rust preparation/publication
  mechanism, but authored Lua operations cannot yet create, destroy, reconnect,
  or atomically transform a running instrument.
- Subpatches are expanded during compilation rather than retained as runtime
  hierarchical objects.
- Authored normalization is static candidate data selected at compilation. It
  is not conditional or dynamically programmable.
- The primary signal path is mono, `f64`, and sample-oriented. The contract has
  a domain field, but the implementation does not yet provide a general set of
  audio, control, event, and trigger execution semantics.
- Controls principally target scalar primitive parameters. Structural controls,
  semantic discrete choices, status publication, and general actions are not
  yet a unified interface model.
- Screens are a terminal projection tied to one compiled revision. Adaptive
  mapping of instrument intent onto arbitrary controllers is not implemented.
- Graph replacement starts fresh DSP state, and complete topology/state capture
  and replay are not implemented.
- Device integration exists, but hardware behavior is environment-dependent and
  is not evidence by itself of architectural expressiveness.
- The event bridge currently has sequencer producers only. Its 255-event usable
  per-source queue drops newest observations on overflow and reports the count;
  Lua dispatch latency follows the non-real-time UI polling cadence. MIDI input
  and scalar controls retain their existing authoritative paths rather than
  producing redundant event traffic.

## Intended capabilities not yet implemented

Meldritch intends to make topology a programmable part of the instrument while
preserving a strict preparation/execution boundary. The architecture must grow
toward authored graph transformations, coherent multi-change commits, dynamic
instantiation and reconnection, programmable normalization, structural
automation, topology-aware capture and replay, and principled feedback.

Signal distinctions should grow from musical examples. Audio, continuous
control, pitch, Gate, triggers, and events may require distinct rates,
combination rules, storage, and scheduling, but the project will not impose a
universal type hierarchy without demonstrated need.

The interface model should grow from scalar controls into semantic continuous
values, discrete choices, actions, status, routing, and structural operations.
Those affordances should admit multiple projections, including interfaces and
controllers not known when an instrument was authored.

These directions are evaluated through the authored-instrument challenges in
[examples/ACCEPTANCE.md](examples/ACCEPTANCE.md), not through a sprawling
feature roadmap.
