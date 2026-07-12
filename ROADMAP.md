# Roadmap

The current implementation queue for arrangement, automation, polyphony, chord
editing, and the long-form showcase lives in [`TODO.md`](TODO.md).

This roadmap is ordered to reduce risk. Do not build the shiny cockpit before the engine has bones.

## Phase 0: Repository skeleton

Deliverables:
- Rust workspace
- crates split by boundary
- basic CI
- formatting/linting
- no audio hardware requirement in tests
- sample project fixtures
- command-line offline test runner

Done when:
- `cargo test` runs
- crates compile
- fixtures can be loaded as raw text

## Phase 1: Core timeline and event model

Deliverables:
- typed IDs
- `FrameRange`
- tempo/BPM conversion
- pattern length
- steps
- events
- tags
- probability seed model
- event scheduling into frame ranges

Done when:
- tests prove step-to-frame conversion
- events can be queried for a frame range
- probability can be deterministic when seeded

## Phase 2: DSL parse and validation

Deliverables:
- project format, likely TOML or RON
- instruments
- samples
- tracks
- patterns
- roles
- buses
- simple relations
- validation diagnostics

Done when:
- fixture project parses into typed model
- missing references produce useful diagnostics
- parsed strings are replaced by typed IDs in validated model

## Phase 3: Headless single-thread offline render

Deliverables:
- `AudioBlock`
- sample loader stub or WAV loader
- sample playback
- gain/pan
- basic mixer
- offline render command
- WAV output or raw buffer test output

Done when:
- a pattern renders to deterministic audio
- golden/fingerprint tests pass
- no TUI/audio device required

## Phase 4: Realtime audio output

Deliverables:
- device output backend
- realtime callback skeleton
- preallocated buffers
- event queue
- basic playback transport
- underrun/miss counters

Done when:
- audio plays from a simple pattern
- callback avoids forbidden operations
- tests still run headless

## Phase 5: TUI command cockpit

Deliverables:
- transport panel
- pattern grid
- selected step inspector
- command dispatch
- logs/diagnostics panel
- basic edit commands

Done when:
- user can start/stop playback
- toggle steps
- edit velocity/gate
- see diagnostics
- TUI edits go through command model only

## Phase 6: Source and relation graph

Deliverables:
- source graph
- relation graph
- audio/control edges
- node properties
- dirty range calculation
- graph inspector data

Done when:
- tests prove invalidation through audio edges
- tests prove invalidation through control edges
- nonlinear barriers are represented

## Phase 7: Render cache and artifact keys

Deliverables:
- `ArtifactKey`
- fingerprinting
- artifact cache
- chunk ranges
- cache lookup
- dirty invalidation
- render plan fingerprints

Done when:
- repeated renders hit cache
- source edits invalidate only affected artifacts
- tail/lookahead expansion is tested

## Phase 8: All-core worker pool

Deliverables:
- long-lived worker threads
- priority queue
- render horizon
- wanted artifact discovery
- hot/warm/cold priorities
- worker diagnostics

Done when:
- dirty future work saturates worker cores
- clean state sleeps
- realtime callback does not spawn workers
- missed hot artifacts degrade gracefully

## Phase 9: Event-aware effects

Deliverables:
- event tags
- tag predicates
- effect send rules
- delay/reverb placeholder bus
- explanation data for active sends

Done when:
- accents can feed delay while normal notes do not
- ghost notes can feed reverb while main hits stay dry
- TUI can show why a send happened

## Phase 10: Role-aware sidechain and dynamic ducking

Deliverables:
- source roles
- role priority table
- envelope follower
- sidechain relation
- simple ducking
- multiband ducking prototype
- explanation model

Done when:
- kick can duck bass
- ducking can be limited to selected bands
- changing kick invalidates bass ducked artifacts through control edge
- TUI can show active relation and attenuation amount

## Phase 11: Chunk transforms

Deliverables:
- chunk transform source type
- reverse
- reslice
- freeze/smear prototype
- transformed chunks become artifacts/sources
- TUI command to create transform

Done when:
- user can transform a bar into a new playable source
- transformed artifact participates in cache/fingerprint system

## Phase 12: Speculative performance futures

Deliverables:
- likely gesture model
- queued scene pre-rendering
- mute/fill variants
- cache prioritization by recency/selection
- visible status panel

Done when:
- current/queued patterns render ahead
- likely mutes/fills are precomputed
- worker pool sleeps when all desired futures are clean

## Phase 13: Polish and hardening

Deliverables:
- better diagnostics
- graph visualization/export
- audio safety limiter
- CPU/memory profiling
- artifact cache bounds
- project save/load stability
- crash-resistant fixtures
- docs

Done when:
- demo can run repeatably
- missed artifacts are visible
- glitches are diagnosable
- project can be understood from TUI panels

## Explicitly later

Not in the first major milestone:
- arbitrary graph UI
- plugin hosting
- full synth workstation
- DAW arrangement editor
- cloud/library features
- arbitrary scripting in realtime
- maximal spectral processing
