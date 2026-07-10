# Architecture

## Workspace shape

Initial Rust workspace:

```text
crates/
  gremlin-core/
    IDs, commands, source graph, relation graph, timeline types

  gremlin-dsl/
    TOML/RON parsing, validation, typed model construction

  gremlin-audio/
    audio buffers, nodes, effects, instruments, realtime engine

  gremlin-render/
    render jobs, artifact cache, invalidation, worker pool

  gremlin-tui/
    terminal UI, command cockpit, inspectors

  gremlin-cli/
    command-line entrypoint, offline render commands, diagnostics
```

Project names can change. Boundaries should not.



## Eurorack-style patch graph

The graph should prefer patchable modules with typed ports.

Convenience features should expand into patch recipes rather than hidden integrated behavior.

```text
recipe: kick_ducks_bass
  -> kick audio to envelope follower
  -> envelope follower to bass duck control
  -> bass audio to dynamic gain/EQ cell
  -> dynamic gain/EQ to bass bus
```

The expansion must remain visible to graph inspection and invalidation.

The compiler validates:
- port compatibility
- signal rate compatibility
- needed adapters
- audio dependencies
- control dependencies
- cache boundaries

## Data flow

```text
DSL / TUI commands
        |
        v
Command processor
        |
        v
Source graph + relation graph
        |
        v
Compiler passes
        |
        v
Render plans + realtime state
        |
        +------------------+
        |                  |
        v                  v
Worker render farm     Realtime callback
        |                  |
        v                  v
Artifact cache       Audio device output
```

## Graph layers

### Source graph

Truth about musical material.

Contains:
- samples
- instruments
- patterns
- tracks
- events
- scenes
- macros
- roles
- buses
- effects

### Relation graph

Truth about dependencies and interactions.

Contains:
- audio edges
- control edges
- sidechains
- modulation rules
- role-priority rules
- routing
- nonlinear/group barriers

### Artifact graph

Derived render products.

Contains:
- rendered audio chunks
- stems
- bus chunks
- previews
- waveform peaks
- spectra
- compiled event tables
- frozen probability outcomes

Artifacts are cacheable and disposable.

### Realtime graph

Small bounded state needed to emit audio now.

Contains:
- output device callback state
- live overlay voices
- current playhead
- current published compiled state
- realtime event queue consumer
- hot cache reader
- fallback renderer

## Commands

All mutations enter as typed commands.

```rust
pub enum Command {
    SetStep(SetStep),
    ClearStep(ClearStep),
    SetTrackRole(SetTrackRole),
    SetRelation(SetRelation),
    SetParam(SetParam),
    ToggleMute(ToggleMute),
    SelectScene(SelectScene),
    CommitProbability(CommitProbability),
}
```

Command result:

```rust
pub struct CommandResult {
    pub changed_entities: Vec<EntityId>,
    pub dirty_ranges: Vec<DirtyRange>,
    pub diagnostics: Vec<Diagnostic>,
}
```

## Node properties

Each graph node declares its behavior.

```rust
pub enum Linearity {
    Linear,
    Nonlinear,
    TimeVariant,
    Feedback,
}

pub struct NodeProperties {
    pub linearity: Linearity,
    pub latency_frames: u32,
    pub lookahead_frames: u32,
    pub tail_frames: u32,
    pub can_cache: bool,
    pub needs_group_signal: bool,
}
```

These properties guide:
- graph optimization
- cache boundaries
- invalidation range expansion
- render scheduling

## Edge types

```rust
pub enum EdgeKind {
    Audio,
    Control,
}
```

Audio edge:
- signal flows into another node

Control edge:
- signal/analysis/event metadata influences another node

Both affect invalidation.

## Audio buffer policy

Use block processing.

```rust
pub struct AudioBlock {
    pub channels: u16,
    pub frames: u32,
    pub samples: Vec<Sample>,
}
```

Planar buffers may be preferable for DSP:

```rust
pub struct StereoBlock {
    pub left: Vec<Sample>,
    pub right: Vec<Sample>,
}
```

Do not dynamic-dispatch per sample.
Dynamic dispatch per node per block is acceptable.

## Effects

```rust
pub trait Effect: Send {
    fn properties(&self) -> NodeProperties;
    fn process_block(&mut self, ctx: &ProcessContext, block: &mut AudioBlock);
}
```

## Instruments

```rust
pub trait Instrument: Send {
    fn note_on(&mut self, event: NoteOn);
    fn note_off(&mut self, event: NoteOff);
    fn process_block(&mut self, ctx: &ProcessContext, out: &mut AudioBlock);
}
```

## Event sources

Patterns compile to event sources.

```rust
pub trait EventSource {
    fn events_between(
        &self,
        start_frame: u64,
        end_frame: u64,
        out: &mut Vec<Event>,
    );
}
```

The realtime path should use preallocated event buffers.

## Render plans

User-facing graph composition compiles into flat render operations.

```rust
pub enum RenderOp {
    Clear { buffer: BufferId },
    RenderSource { source: SourceId, out: BufferId },
    ApplyEffect { effect: EffectId, buffer: BufferId },
    Mix { src: BufferId, dst: BufferId, gain: f32 },
    Analyze { src: BufferId, feature: FeatureId, out: ControlId },
}
```

Benefits:
- easier testing
- easier fingerprinting
- easier scheduling
- less runtime graph wandering
- cleaner cache keys

## Artifact keys

```rust
pub struct ArtifactKey {
    pub node: NodeId,
    pub range: FrameRange,
    pub sample_rate: u32,
    pub fingerprint: Fingerprint,
}
```

Fingerprint includes:
- source revision
- input artifact fingerprints
- effect parameters
- automation slice
- routing/rule state
- random/probability seed
- render mode

## Scheduler

The scheduler operates over wanted artifacts.

```text
wanted artifact
  -> discover dependencies
  -> check cache
  -> enqueue missing/dirty jobs
  -> execute in priority order
  -> publish artifacts
```

Do not generate the entire combination space.

## Worker pool

Use long-lived worker threads.

Workers are for:
- rendering future chunks
- resampling
- analysis
- waveform/spectrum previews
- frozen stems
- likely scene variants

The audio callback must not spawn workers.

## Realtime callback

Expected shape:

```rust
fn audio_callback(output: &mut [f32]) {
    // 1. Determine frame range.
    // 2. Pull hot cached artifact if available.
    // 3. Apply live overlays.
    // 4. Apply final safety limiter.
    // 5. Write output.
    // 6. Increment diagnostic counters.
}
```

No graph compilation. No parsing. No file IO.

## Publishing state

Use immutable compiled state with atomic/lock-free swap.

Possible crates:
- `arc-swap`
- `triple_buffer`
- SPSC ring buffers for realtime events

## First fixed topology

Do not start with fully arbitrary graphs.

Start with:

```text
sources/tracks -> buses -> master
```

Then add explicit relation/control edges.

Only generalize once the fixed topology proves too tight.
