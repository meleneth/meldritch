# Feature Spine

## Governing product requirements (2026-10-11)

MELDRITCH is a project-oriented, patchable, dynamically reconfigurable musical
environment. The machine itself is a playable compositional instrument.
**Coherence under composition** governs design: independently understandable
primitives retain explicit meaning as musical complexity grows. Maximal
expressive complexity is welcome; hidden semantics, ambiguous ownership and
unnecessary interaction friction are not. Eurorack is inspiration, not a limit
on digital capabilities or a requirement to reproduce a physical rack.

These are requirements, not implementation claims. Current support is recorded
in [the capability matrix](https://github.com/meleneth/meldritch/blob/main/songs/examples/CAPABILITIES.md); the code audit and
migration decisions are in [ARCHITECTURE.md](ARCHITECTURE.md), and accepted
vertical slices are planned in [ROADMAP.md](ROADMAP.md).

- A project is an ordinary directory containing a reusable palette and zero or
  more compositions, control declarations, captures and assets. An instrument
  is not a track; a pattern is not an instrument; a composition is not a
  performance; a project is not a composition. Relationships are explicit.
- `mkdir black-cathedral; cd black-cathedral; meldritch` must open an empty
  project without initialization, mandatory manifest/database/template or Git
  repository. Empty opening and silent mono-patch palette creation are implemented;
  pattern/routing/playback construction from empty and standalone
  save/reopen remain unimplemented. Git is allowed as an external
  dependency and may be required installed; a repository, commit or network
  operation is unnecessary merely to open/play local music.
- Construction edits authored definitions. Performance edits active instances
  and exposed controls, launches material and requests graph revisions. Both
  share Rust authority and continue audio across mode changes. Capture and
  explicit promotion into authored music are separate operations.
- Typed ports preserve Audio, Pitch, Gate, Trigger, Control/CV, Event, Feature
  and appropriate Metadata families. Numeric representation does not authorize
  conversion. Pitch and gate remain independently routable; envelopes have no
  inherent amplitude destination, and gates need not originate from notes.
- Patterns are independent timed sources. Supported transforms compose through
  inspectable ports with explicit timing, identity and probability commitments.
  There must be no second event engine to implement this direction.
- LFOs and other modulators are graph modules with destination-independent
  algorithms. Rate, units, phase/reset, scope, fan-out, scaling, combination,
  feedback and render-ahead semantics are explicit. Modulators are modulatable.
- Normalization is patch-authored policy, distinct from resolved connections and
  signal values. It selects/replaces routes; mixing/multiplication/selection
  uses explicit components. Dynamic routing remains a supported direction.
- Graph transactions validate the complete candidate and atomically publish
  coherent graph/control interfaces. Performance activation can be immediate
  or quantized. Stateful continuity needs explicit module-state migration;
  a PCM swap alone does not provide it.
- Patches declare separate signal, construction and performance interfaces.
  Performance controls include numeric, enum, toggle, trigger, gate, transformed
  macros, inspectable graph recipes and composite/quantized actions. Surfaces
  can change by revision; semantic identities and pickup prevent silent retargeting.
  Hardware and TUI are adapters. Lua composes on the control plane; Rust validates,
  schedules, accepts state and publishes audio.
- Compositions are metapatterns scheduling material, routing, modulation,
  controls, graph revisions and bounded conditional actions. A performance is
  an interpretation with live interventions. Captures must be harvestable into
  compositions without manually reconstructing gestures.
- Libraries support copy/import, pinned reference, local instantiation/override
  and deliberate update as distinct operations. Definition identities, pinned
  Git revisions, content/dependency locks and asset fingerprints must make
  resolution reproducible and report resolved/missing/incompatible dependencies.
  Submodules are optional, not the universal model. A package manager is deferred.

The feature categories below retain the original feature spine and showcase
history. Their initial non-goals are sequencing decisions, not ceilings on this
product model; the newer roadmap takes priority.

The original spine separates basic musical facilities from relational behavior.

## Sonic thesis

The draw is not:
- another step sequencer
- another sampler
- another subtractive synth
- another grid of mutes
- another clone of old hardware constraints

The draw is:

> A musical environment where sources, roles, relationships, and future performance states are compiled into adaptive audio behavior.

A second core identity:

> Eurorack-style modular pieces, with integrations automated as visible patch recipes rather than hidden monoliths.

## Core feature categories

### 1. Pattern and event engine

Required:
- BPM and transport
- frame-accurate event scheduling
- pattern length
- tracks
- steps
- notes
- velocity
- gate
- probability
- ratchets
- mutes/solos
- pattern switching
- scene snapshots

Stretch:
- polymeter
- per-track step length
- tempo automation
- probability commit/freezing
- conditional trigs
- transform history

### 2. Source model

Sources may be:
- sample players
- synth voices
- external MIDI tracks
- rendered chunks
- frozen stems
- buses
- effect returns
- control streams
- analysis streams

Each source may expose:
- audio output
- event stream
- role
- tags
- envelope follower
- transient detector
- spectral features
- density/intensity metrics

### 3. Relationship model

Relationships are first-class.

Examples:
- kick ducks bass
- snare accents enter delay
- lead carves pad spectrum
- hats modulate noise bus brightness
- fills bypass glue compressor
- ghost notes feed reverb only
- scene intensity controls saturation
- bass harmonics bias kick distortion

A relation can create audio dependencies, control dependencies, or both.

### 4. DSL/project format

Required:
- project metadata
- instruments
- samples
- patterns
- tracks
- roles
- buses
- effects
- relations
- scenes
- macros

Initial format:
- TOML or RON for project data
- compact pattern strings for step data
- typed validated model after parsing

### 5. TUI cockpit

Required:
- transport view
- pattern grid
- selected track/step inspector
- source/role inspector
- relation inspector
- cache/render status
- CPU worker status
- audio underrun/miss counter
- active rule explanation panel

Important:
The TUI should not impersonate hardware knobs in ASCII. It should be a command cockpit.

Visual palette:
- use the `lospec500` palette when TUI styling begins

### 6. Audio engine

Required:
- device output through `cpal` or equivalent
- fixed sample rate per project/session
- block-based processing
- sample playback
- gain/pan
- bus mixing
- simple filters
- envelopes
- delay/reverb placeholder
- final limiter/soft clipper
- diagnostics

Policy:
- `f64` internal buffers by default
- `u64` frame timeline
- `f64` coefficient/time calculations where appropriate

### 7. Render farm

Required:
- chunk renderer
- artifact cache
- dirty invalidation
- worker pool
- priorities
- render horizon
- cache hits/misses
- fallback behavior

Priority bands:
- P0: needed by realtime path
- P1: next few seconds
- P2: current pattern
- P3: queued scene/pattern
- P4: likely performance variants
- P5: previews/analysis

### 8. Relational DSP

Initial killer features:
1. event-aware sends
2. role-aware sidechain ducking
3. multiband dynamic ducking
4. chunk transforms
5. cross-source modulation/distortion

Later:
- adaptive spectral masking
- transient/body/noise split
- granular resynthesis
- spectral freeze
- role-priority mix compiler

### 9. Explanation and debug tools

Required:
- show active relation rules
- show which source controls which target
- show cache artifact lineage
- show dirty ranges after edits
- show worker queue by priority
- show fallback/missed artifact events
- show graph node properties

This is essential. Powerful relational DSP becomes unusable if it feels haunted.

## Non-goals for first implementation

Do not implement these first:
- VST/CLAP hosting
- plugin sandboxing
- full DAW timeline
- piano roll
- generic synth workstation
- arbitrary scripting inside audio callback
- sample library browser
- cloud sync
- network collaboration
- cross-platform polish beyond 64-bit desktop targets

## Minimum impressive demo

A first demo should show:
1. dense loop with kick, bass, snare, hats, pad/noise
2. relationship rules disabled: muddy but understandable
3. relationship rules enabled:
   - kick owns sub transient
   - bass ducks only in selected bands
   - snare accents feed delay
   - ghost notes feed reverb
   - pad carves around lead or bass role
4. TUI panel explains which rules are active
5. CPU workers pre-render next variants
6. audio callback stays boring
