# Codex Dependency Addendum

Read this after the main project docs.

The goal is not to collect crates. The goal is to keep MELDRITCH’s architecture sovereign.

Use crates for well-bounded jobs:
- terminal rendering
- audio device I/O
- MIDI I/O
- audio file decoding
- WAV fixture import/export
- resampling
- FFT kernels
- serialization
- CLI parsing
- diagnostics
- tests

Do not let a crate define:
- the source graph
- the relation graph
- the artifact graph
- patch recipe semantics
- dirty invalidation
- render scheduling
- cache fingerprints
- realtime safety policy
- relationship DSP behavior

MELDRITCH owns those.

## Immediate dependency posture

For the first milestone, keep dependencies small.

Suggested early dependencies:

```toml
[workspace.dependencies]
thiserror = "2"
serde = { version = "1", features = ["derive"] }
toml = "0.9"
clap = { version = "4", features = ["derive"] }
tracing = "0.1"
tracing-subscriber = "0.3"
proptest = "1"
```

First milestone does not need audio hardware I/O, MIDI, TUI rendering, FFT, or resampling unless the approved plan explicitly includes them.

## Candidate dependencies by area

### TUI

Use later:

```toml
ratatui = "0.30"
crossterm = "0.29"
```

Policy:
- Ratatui owns terminal drawing.
- Crossterm owns terminal input/backend glue.
- They must not own app state, command semantics, graph state, or engine state.

The TUI dispatches typed commands. It does not mutate engine state directly.

### Audio device I/O

Use later:

```toml
cpal = "0.17"
```

Policy:
- CPAL belongs behind an `audio_device` boundary.
- Device sample formats are boundary details.
- Internal audio remains `f64`.
- Convert to `f32`/integer/device format only at explicit output boundaries.
- CPAL types should not leak into core DSP modules.

The realtime callback must not:
- parse
- allocate casually
- block on locks
- perform file I/O
- compile graphs
- call into the TUI
- spawn threads

### MIDI I/O

Use later:

```toml
midir = "0.10"
```

Policy:
- MIDI is an I/O adapter, not the internal event model.
- Convert incoming MIDI to typed engine events.
- Convert outgoing engine events to MIDI at the boundary.
- Do not let MIDI channel/note quirks infect the core model.

### WAV fixtures and simple import/export

Use:

```toml
hound = "3"
```

Policy:
- Good for tests, fixtures, simple WAV import/export.
- Convert loaded audio into internal `f64` samples.
- Convert from internal `f64` to target file format only in export modules.

### General audio decoding

Use later:

```toml
symphonia = "0.6"
```

Policy:
- Import pipeline only.
- Decode outside realtime path.
- Convert decoded samples into internal `f64`.
- Do not put decoder logic in audio callback or render hot path.

### Resampling

Use later:

```toml
rubato = "2"
```

Policy:
- Prefer offline/import/render-farm resampling.
- Realtime resampling only when explicitly designed and bounded.
- Internal artifacts should be normalized to project sample rate when practical.

### FFT / spectral processing

Use later:

```toml
rustfft = "6"
```

Policy:
- Use for spectral analysis, spectral freeze, masking prototypes, and visualization.
- Do not lead the first milestone with FFT.
- Start relational DSP with event-aware sends and envelope-following sidechains first.

### Concurrency and state publication

Potential later dependencies:

```toml
arc-swap = "1"
ringbuf = "0.4"
crossbeam = "0.8"
```

Policy:
- `arc-swap` may publish immutable compiled state to realtime readers.
- `ringbuf` may be used for SPSC realtime-safe-ish queues.
- `crossbeam` may be used for worker/control channels outside the callback.
- Do not use ordinary blocking queues inside the audio callback.
- Do not wait indefinitely for workers from the callback.

### Offline parallelism

Potential later dependency:

```toml
rayon = "1"
```

Policy:
- Good for offline analysis, cold render jobs, import processing, and batch work.
- Be cautious with Rayon in or near realtime audio.
- Hot render scheduling may need a dedicated worker pool with explicit priorities.

### Errors and diagnostics

Use:

```toml
thiserror = "2"
tracing = "0.1"
tracing-subscriber = "0.3"
```

Policy:
- Use `thiserror` for typed library/domain errors.
- Use `tracing` outside realtime paths.
- Do not log from the audio callback except through bounded diagnostic counters or explicitly safe telemetry.

### Serialization / DSL

Use:

```toml
serde = { version = "1", features = ["derive"] }
toml = "0.9"
```

Policy:
- The external project format may use TOML initially.
- Parsed DSL strings must become typed IDs/enums after validation.
- No long-lived stringly-typed routing in the validated model.

### CLI

Use:

```toml
clap = { version = "4", features = ["derive"] }
```

Policy:
- CLI should support fixture validation, graph inspection, and offline render tests before TUI exists.

### Property testing

Use:

```toml
proptest = "1"
```

Policy:
- Use for graph invalidation invariants, timeline conversion, dirty range behavior, and fingerprint stability where useful.

## Crates to inspect, not blindly adopt

### FunDSP / `fundsp`

FunDSP is worth studying because it has an audio graph / DSP orientation.

Do not adopt it as the core engine without a spike.

Reasons:
- MELDRITCH needs source/relation/artifact graph separation.
- MELDRITCH needs audio and control dependency tracking.
- MELDRITCH needs dirty range invalidation.
- MELDRITCH needs semantic artifact fingerprints.
- MELDRITCH needs speculative future rendering.
- MELDRITCH needs inspectable patch recipes.
- MELDRITCH uses internal `f64` doctrine.

It may still be useful for:
- prototype DSP nodes
- inspiration
- comparison
- isolated utility patterns

### Rodio / Kira

Useful to know about, probably not core.

Policy:
- These are convenient playback/game-audio style tools.
- MELDRITCH needs lower-level control over graph, cache, scheduling, and render artifacts.
- Do not use high-level playback crates to avoid designing the actual engine.

### FSM crates

Possible crates:
- rust-fsm
- statig
- sm
- sfsm

Policy:
- Start with Rust enums + exhaustive `match`.
- Use typestate for lifecycle gates where it helps.
- Avoid macro-heavy FSM crates in the first milestone unless a clear benefit appears.
- The FSMs are domain-shaped and should remain readable.

## Explicit dependency anti-goals

Do not add dependencies for:
- VST/CLAP hosting
- plugin systems
- web UI
- scripting engines
- arbitrary runtime graph language
- database storage
- cloud sync
- network collaboration
- GUI frameworks
- AI/ML inference
- complex ECS frameworks

These are not first-milestone concerns.

## Version policy

When creating `Cargo.toml`, verify current compatible versions.

Do not pin ancient versions from memory.
Do not add crates that are not used by the milestone.
Prefer workspace dependencies so crate versions stay centralized.

## First milestone dependency rule

For the first milestone, prefer only:

```toml
thiserror = "2"
serde = { version = "1", features = ["derive"] }
toml = "0.9"
clap = { version = "4", features = ["derive"] }
tracing = "0.1"
tracing-subscriber = "0.3"
proptest = "1"
```

If you believe another dependency is needed in milestone one, explain why in plan mode before adding it.

## Architecture reminder

Dependency ownership boundaries:

```text
ratatui/crossterm:
  terminal rendering/input only

cpal:
  device I/O only

midir:
  MIDI boundary only

hound/symphonia:
  file import/export only

rubato:
  resampling only

rustfft:
  FFT kernels only

rayon/crossbeam/ringbuf/arc-swap:
  scheduling/state plumbing only

MELDRITCH:
  source graph
  relation graph
  artifact graph
  patch recipes
  dirty invalidation
  render plans
  artifact cache
  relationship DSP semantics
  realtime safety policy
```

If a dependency starts pulling architecture across these boundaries, stop and ask for plan review.
