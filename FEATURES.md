# Feature Spine

This document separates table-stakes groovebox features from the features that make this project worth existing.

## Sonic thesis

The draw is not:
- another step sequencer
- another sampler
- another subtractive synth
- another grid of mutes
- another clone of old hardware constraints

The draw is:

> A groovebox where sources, roles, relationships, and future performance states are compiled into adaptive audio behavior.

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
