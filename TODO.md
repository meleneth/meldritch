# Active TODO

The showcase milestones below are complete. The active work is to replace the
fixture-oriented, monolithic project input with a composable song-directory
format and a curated, recordable performance runtime.

## 19. Song directories and `.ml*` formats

### 19.0 Example-first specification

The checked-in example corpus is the primary format specification and the
acceptance suite. Do not add schema fields, abstractions, loaders, or runtime
features merely because they may be useful later. Every implemented capability
must be justified by at least one concrete example that we expect a person to
author, perform, capture, or replay.

- [ ] Create a tree of minimal examples, with one focused capability per song
- [ ] Create composed examples that combine previously proven capabilities
- [ ] Give every example a short statement of intent and observable acceptance
  criteria: what must validate, compile, sound, expose, record, or replay
- [ ] Keep example files small enough to understand without reading Rust code
- [ ] Add invalid examples for each important diagnostic and boundary rule
- [ ] Turn each accepted example into an automated fixture or end-to-end test
- [ ] Maintain an example-to-capability matrix so unsupported and untested
  behavior is visible
- [ ] Require a new or changed example before expanding a `.ml*` schema
- [ ] Avoid speculative fields and abstractions not exercised by the corpus

Initial example tree:

```text
songs/examples/
  00-minimal-synth/
  01-synth-note-pattern/
  02-synth-parameter-pattern/
  03-dsp-chain/
  04-dsp-parameter-pattern/
  05-multiple-tracks/
  06-pattern-switching/
  07-pattern-duration-and-repeat/
  08-arrangement-and-scenes/
  09-curated-performance-controls/
  10-performance-and-all-params-modes/
  11-session-capture/
  12-exact-session-replay/
  13-quantized-session-replay/
  14-harvested-performance-cues/
  15-launch-control-xl-input/
  16-launch-control-xl-playground/
  composed-warehouse/
  invalid/
```

Done when the desired product behavior can be reviewed by reading example song
trees, and every supported format/runtime feature points back to an executable
example.

### 19.1 Format contract derived from examples

Synth and DSP definitions are Eurorack-inspired typed patch graphs. Files
declare modules, typed ports, normalized connections, and explicit cables.
Convenience integrations may be expressed as recipes only when their expansion
remains inspectable and compiles to the same patch model; opaque integrated
instruments, effects, and hidden modulation systems are not the foundation.

- [ ] Define versioned TOML schemas and reference rules for:
  - `.mlsynth`: synth modules, voice configuration, parameters, modulation
    targets, defaults, and stable parameter IDs
  - `.mldsp`: DSP modules/chains, routing, parameters, macros, modulation
    targets, latency/lookahead/tail declarations, and stable parameter IDs
  - `.mlpattern`: note/event patterns and DSP/control parameter patterns
  - `.mlperformance`: song assembly, synth/DSP/pattern assignments,
    arrangement, curated controls, bindings, and replayable interaction events
- [ ] Define a song as a directory of text files with one unambiguous entry
  `.mlperformance` file and relative references contained within the song root
- [ ] Specify reference syntax, namespaces, stable IDs, format versions,
  extension policy, path normalization, and cycle policy
- [ ] Derive the initial schema from the smallest examples instead of designing
  a comprehensive schema in advance
- [ ] Document which data is authored source truth and which data is generated
  session history

Done when the formats can describe a complete interactive song without relying
on hard-coded showcase construction.

### 19.2 Directory loading, validation, and compilation

- [ ] Add a song-directory loader outside realtime code
- [ ] Parse each file independently and retain file/path context for diagnostics
- [ ] Resolve relative references only within the song root by default
- [ ] Convert parsed names/references to typed IDs and typed parameter values
- [ ] Reject missing/duplicate IDs, missing files, incompatible parameter or
  port types, invalid ranges, illegal reference cycles, and ambiguous entry files
- [ ] Produce deterministic diagnostics with file and logical field context
- [ ] Fingerprint individual definitions and the fully resolved song model
- [ ] Compile the resolved song through the existing source, relation, render,
  automation, and realtime-publication boundaries
- [ ] Keep compatibility loading for existing `.toml` fixtures until their
  tests and demos have migrated

Done when a CLI validation command can load an example song directory, resolve
all cross-file references, print useful failures, and produce a deterministic
compiled fingerprint.

### 19.3 Pattern and parameter-pattern model

- [ ] Separate reusable pattern identity from a pattern's placement in a song
- [ ] Support note/event patterns targeting synth or sample tracks
- [ ] Support DSP and synth parameter patterns with typed targets, values,
  interpolation, duration, looping, phase, and launch quantization
- [ ] Define track selection, pattern selection/replacement, repeat count,
  duration, and arrangement/section semantics
- [ ] Preserve exact dirty-range invalidation for edited or switched patterns
- [ ] Include every referenced definition and placement in artifact fingerprints

Done when musical patterns and parameter patterns can be independently reused,
assigned, switched, and rendered deterministically from text definitions.

### 19.4 Curated performance controls

- [ ] Let `.mlperformance` declare the controls intentionally exposed to the
  performer: parameters, macros, toggles, choices, track/pattern launches, and
  quantized actions
- [x] Define control labels, value ranges/steps, bindings, and target mappings
  without duplicating underlying synth/DSP parameter semantics
- [ ] Add explicit control groups and defaults to the declared performance
  surface
- [ ] Validate duplicate or unreachable bindings and unsafe target mappings
- [x] Generate the default cockpit view from the declared controls
- [x] Route every interaction through typed commands rather than direct state
  mutation
- [x] Add a LaunchControl XL app-level input profile that maps faders to
  absolute normalized curated-control values and buttons to typed step nudges
- [x] Let `.mlperformance` declare MIDI devices and per-control MIDI CC
  bindings for absolute faders/knobs and step buttons
- [x] Open script-declared MIDI devices through the host MIDI stack and feed
  decoded CC messages into the typed control-surface profile
- [x] Add a script-aware hardware diagnostic path for MIDI control inputs
- [x] Rerender supported playground parameter controls for delay feedback and
  synth filter cutoff
- [x] Add live script-target support for filter resonance and delay mix so the
  LaunchControl playground can expose distinct tone/space rows instead of 32
  duplicate cutoff controls
- [x] Add script-declared MIDI action bindings for transport and performance
  buttons
- [x] Print raw note/unknown MIDI messages in the diagnostic path so specialized
  hardware buttons can be discovered
- [x] Add script-declared MIDI note action bindings and bind the discovered
  LaunchControl XL side-column note/CC buttons in the playground example
- [x] Add authored LaunchControl XL playground groove scenes/variations and
  rerender selected song patterns during `tui-song` playback
- [x] Make the LaunchControl XL playground scenes/fills musically differentiated
  enough to verify by ear instead of minor note-list variants
- [x] Add script-declared centered and overdrive MIDI curves so the LaunchControl
  playground has neutral center-detent knobs and faders with a normal/full-open
  value below the physical maximum
- [x] Show the LaunchControl XL playground's authored groove scenes/fills,
  actual note grid, and compact control telemetry in default performance mode so
  the playable instrument/pattern surface is visible without switching to
  all-parameters mode
- [x] Make `tui-song` autoplay by default and keep the published song loop
  sounding across live parameter rerenders; `--no-autoplay` is now the explicit
  stopped-start mode
- [x] Fix the LaunchControl playground note positions to use the loader's 960
  PPQ ticks per beat and build the default TUI grid from the authored initial
  `.mlpattern` instead of a dummy empty pattern
- [x] Route live `tui-song` output through a dedicated song-audio publication so
  the generic backing TUI coordinator cannot overwrite the initial rendered song
  with silence before the first controller movement
- [x] Add `tui-song --audio-debug` status telemetry for live output diagnosis:
  transport callbacks, playhead position, current sample peak, and upcoming
  song-publication peak
- [ ] Design `17-launch-control-xl-ensemble` as the next example-first target:
  a multi-lane LaunchControl XL performance with 4+ variations per lane:
  - Lane 1: one beat drum track
  - Lanes 2-3: rhythm/percussion drum tracks
  - Lane 4: polyphonic synth for chords/pads
  - Lanes 5-6: monophonic bass synth tracks
  - Lanes 7-9: sample-based tracks
  - Resolve the 9-lane/8-fader mismatch with script-declared banking: the beat
    drum can live on another page in this example, but the specific scene/
    performance declaration owns that distinction; Rust must only implement
    generic bank/page mapping
- [x] Add an `ACCEPTANCE.md` for the ensemble example before implementing
  runtime support, including exact LaunchControl row/side-button semantics
- [x] Add generic `.mlperformance` lane/page declarations with validation for
  scene-authored banked LaunchControl surfaces
- [x] Load generic lane/page declarations into app view state and render the
  active page in performance mode without hard-coded lane/page policy
- [x] Add script-declared `select_page` performance actions that switch active
  performance pages by page id
- [x] Add script-declared per-strip visible control lists and render only the
  active page's declared controls in performance-mode telemetry
- [x] Add page-scoped MIDI control bindings so active page selection can remap
  physical controls without hard-coded LaunchControl policy
- [x] Add a validating `17-launch-control-xl-ensemble` song skeleton with nine
  lanes, two scene-authored pages, four placeholder variations per lane, and
  page-scoped fader controls
- [x] Add `.mlsamples` sample-bank metadata parsing and attach the ensemble
  sample lanes to a Raven voice placeholder bank
- [x] Define the script-level lane model for the ensemble: track IDs, lane
  roles, pattern banks, variation IDs, mute/solo behavior, launch quantization,
  and which controls are per-lane versus global
- [ ] Extend `.mlperformance` controls to support momentary modifiers/layers
  such as “hold button + move fader” without hard-coded Rust policy
- [ ] Implement a first modifier example: while a declared modifier button is
  held, one or more faders send octave/transpose commands instead of their
  normal continuous parameter target
- [ ] Add runtime pattern-bank selection for each lane so the same controller
  can switch at least four variations per lane without requiring 32 dedicated
  buttons
- [ ] Add multi-track song compilation/playback for the subset needed by the
  ensemble example: drums/percussion first, then basses, then poly pad, then
  samples
- [ ] Add sample-track playback support for text-authored sample references,
  sample slots, one-shots/loops, start/end slices, level, pitch, and per-pattern
  triggering
- [ ] Add a polyphonic synth path for chord/pad note patterns in `.mlsynth`
  songs, with enough ADSR/filter control to be musically distinct from bass
- [ ] Add TUI performance-mode lane overview for the ensemble: 8 visible
  controller strips, active variation per lane, mute/solo state, modifier state,
  and compact values
- [ ] Add session-capture tests proving normal controls and modifier-layer
  gestures record as typed inputs and can replay without the LaunchControl
  attached
- [ ] Defer LED feedback until after the ensemble control semantics are stable,
  then map LEDs to active variation/mute/modifier state from script-authored
  output declarations
- [ ] Verify LaunchControl XL input with the diagnostic path on
  both Windows and Linux
- [ ] Add MIDI output/LED feedback support after confirming the LaunchControl XL
  output protocol for script-addressed LEDs
- [ ] Add quantized script-declared pattern-switching semantics and exact replay
  beyond the current immediate song-rerender scene path

Done when loading a song produces a small usable performance surface entirely
from its `.mlperformance` declaration.

### 19.5 Performance mode and all-parameters mode

- [x] Add `Performance` and `AllParameters` cockpit modes
- [x] Make `Performance` the default
- [x] Bind `Ctrl-Tab` to switch modes
- [x] In performance mode, show only controls curated by `.mlperformance`
- [ ] In all-parameters mode, expose the complete resolved synth/DSP/track/
  pattern parameter tree for inspection and editing
- [x] Preserve transport, playhead, selected track/pattern, queued launches,
  parameter state, and audio publication while switching modes
- [ ] Refine track, pattern, arrangement, duration, and parameter navigation so
  all-parameters mode remains deterministic even before it is polished

Done when `Ctrl-Tab` safely switches between a focused playable interface and
the complete parameter surface without disturbing playback.

### 19.6 Timestamped performance capture

- [x] Create a new datetime-stamped `.mlperformance` session file when an
  interactive song starts, using a collision-safe filename
- [x] Record song identity/version/fingerprint and the starting runtime state
- [ ] Record every accepted typed performer interaction with monotonic sequence,
  wall-clock offset, absolute frame, musical position, requested quantization,
  actual execution frame, previous value, resulting value, and provenance
- [ ] Record mode changes, track/pattern selection, launches, parameter edits,
  transport actions, cancellations, and fallback/cache decisions where relevant
  - [x] Classify mode, selection, transport, queue/cancel, transform,
    audio-source, synth-control, performance-FX, and parameter-edit events
    distinctly in captured sessions
- [x] Checkpoint session files outside the realtime callback with
  crash-tolerant writes
- [x] Add explicit bounded buffering for session event writes
- [x] Write a clean/unclean termination marker
- [x] Write the complete final runtime state
- [x] Never overwrite an authored performance definition or an earlier session

Done when every interactive run leaves a human-readable session artifact that
is sufficient to reconstruct what the performer did and when it took effect.

### 19.7 Replay and harvesting

- [ ] Load a captured session against the exact song fingerprint, with an
  explicit diagnostic or migration path when definitions differ
- [ ] Replay accepted actions deterministically at recorded execution frames
- [ ] Offer musical-phase/quantized replay as an explicit alternative to exact
  frame replay
- [ ] Distinguish authored automation, live performer input, learned/harvested
  cues, and replay provenance
- [ ] Extract recurring gestures into proposed macros, parameter patterns,
  pattern launches, or revised performance controls
- [ ] Keep harvesting non-destructive: write new proposed text files and retain
  the original session journal

Done when a saved performance can be replayed repeatably and mined into reusable
song material without opaque JSON-only state or mutation of the source session.

### 19.8 Verification and migration

- [ ] Schema/round-trip tests for every `.ml*` format
- [ ] Cross-file reference, path-boundary, duplicate-ID, cycle, and diagnostic tests
- [ ] Deterministic resolved-song and artifact fingerprint tests
- [ ] Full-buffer versus chunked rendering tests for parameter patterns
- [ ] TUI tests for default performance mode and `Ctrl-Tab` switching
- [ ] Session capture tests covering every accepted interaction category
- [ ] Exact replay and quantized replay tests
- [ ] End-to-end headless test: load song directory, compile, perform scripted
  interactions, save session, reload, and replay sample-identically
- [ ] Migrate at least one existing showcase to the new song directory format
- [ ] Update README commands and format documentation

Done when one existing showcase is driven by `.ml*` files, plays interactively,
records a session, and replays it deterministically under the standard checks.

## Completed showcase milestones

The following work formed the previous implementation queue for the long-form
polyphonic demos. It remains here as completion history; broader phases live in
[`ROADMAP.md`](ROADMAP.md).

## 1. Deterministic polyphonic voice bank

- [x] Fixed-capacity voice bank around the existing stateful DSP voice
- [x] Deterministic idle/released/oldest voice allocation
- [x] Predictable voice stealing and note-off routing
- [x] Independent oscillator, ADSR, filter, glide, and modulation state per voice
- [x] Sample-identical full-buffer and split-chunk rendering
- [x] Headless chord and voice-stealing tests

Done when a chord can render deterministically through arbitrary chunk sizes and
voice allocation does not depend on worker order.

## 2. Polyphonic instrument rendering

- [x] Polyphonic pattern renderer and artifact fingerprint inputs
- [x] Worker-horizon chunk adapter with deterministic pre-roll
- [x] Mixed drum, monophonic bass, and chord-synth publication
- [x] Realtime host playback smoke test with zero underruns/misses
- [x] TUI diagnostics for active voices and stolen voices

Done when realtime playback can sustain chords while drums and bass continue.

## 3. Chord pattern model and editing

- [x] Represent chord tones as independent note lanes feeding one instrument
- [x] Typed note-lane and chord edit commands
- [x] TUI chord grid/inspector and selected-voice feedback
- [x] Chord inversion, transpose, velocity, gate, and probability controls
- [x] Exact dirty ranges for chord-tone edits

Done when chord tones remain individually editable and changes rebuild only
affected future artifacts.

## 4. Arrangement timeline

- [x] Ordered pattern instances extending beyond one grid loop
- [x] Repeat count, derived start time, transpose, track mute, and scene selection
- [x] Arrangement-aware event queries, sample rendering, and artifact keys
- [x] Transport position across sections and loopable arrangement ranges
- [x] TUI arrangement/section overview with active scene and loop highlighting

Done when a 24- or 32-bar piece can contain intro, variations, breakdown, chord
section, and full return without flattening everything into one pattern.

## 5. Parameter automation and scenes

- [x] Timestamped automation points with linear and stepped interpolation
- [x] Sample-accurate cutoff, resonance, envelope depth, drive, level, ducking, and modulation
- [x] Discrete waveform, voicing, mute, and scene changes
- [x] Automation segment fingerprints and exact point influence ranges
- [x] TUI automation inspector and active-scene explanation

Done when parameter changes happen over musical time and editing one segment
invalidates only intersecting chunks.

## 6. Long-form showcase

- [x] 32-bar fixture with multiple drum and bass variations
- [x] Chord progression and voicing changes
- [x] Filter/drive automation, kick ducking, and modulation
- [x] Accent scenes, breakdown, transitions, and full-pattern return
- [x] Offline WAV render, realtime demo command, manifest, and zero-underrun/miss smoke checks
- [x] Document controls and expected section sequence

Done when the demo plays repeatably, clearly changes over time, and exercises
arrangement, automation, polyphony, relational DSP, caching, and realtime output.

## 7. Event-aware effects

- [x] Typed delay and reverb send buses
- [x] Tag-matched send rules for accents and ghost notes
- [x] Deterministic delay and early-reflection reverb processing
- [x] Per-event explanation records for active sends
- [x] Effect-send fingerprints, finite tails, chunk equivalence, and semantic invalidation
- [x] Realtime coordinator integration and TUI send explanation panel

Done when tagged sends remain deterministic across chunk boundaries, participate
in cache invalidation, and can be inspected while the realtime demo plays.

## 8. Role-aware sidechain dynamics

- [x] Typed source roles and directional priority table
- [x] Deterministic attack/release envelope follower
- [x] Selectable low-band, high-band, or full-band ducking
- [x] Explanation model with roles, bands, envelope, and attenuation
- [x] Compile sidechain declarations into graph control edges for invalidation
- [x] Sidechain relation fingerprints, realtime coordinator, and TUI attenuation panel

Done when kick-role audio can duck selected bass bands through a declared
relation, edits invalidate dependent artifacts, and attenuation is visible.

## 9. Chunk transforms

- [x] Typed reverse, reslice, freeze, and smear specifications
- [x] Deterministic finite transforms with validation
- [x] Transform artifact keys covering source audio and parameters
- [x] Derived transformed sources in the relation graph and artifact cache
- [x] CLI capture, transform, provenance manifest, and optional playback
- [x] TUI capture, transform creation, cache status, and provenance controls
- [x] Atomic transformed-source audition and return-to-live controls

Done when a rendered bar can become a new cacheable, graph-connected playable
source and transforms can be created and inspected from the cockpit.

## 10. Speculative performance futures

- [x] Typed queued-scene, mute/unmute, and fill gestures
- [x] Deterministic recency, selection, and queue scoring
- [x] Candidate deduplication and bounded future plans
- [x] Gesture history with stable recency evidence
- [x] Renderable mute/fill/scene variants and artifact fingerprints
- [x] Score-ordered worker submission and future artifact cache
- [x] Sleep-on-clean behavior and worker diagnostics
- [x] TUI future-cache status and candidate inspection

Done when current and likely next performance states are pre-rendered in score
order, and workers sleep once every desired future artifact is clean.

## 11. Live performance execution

- [x] Beat/bar quantized gesture queue and cancellation
- [x] Deterministic active scene, mute, and fill state transitions
- [x] Speculative cache-hit selection with explicit live-render fallback
- [x] Atomic cached-artifact publication at the launch boundary
- [x] Temporary fill lifetime and automatic return to the base pattern
- [x] Scene queue, mute, fill, and cancel controls in the TUI
- [x] Queued/active launch state and fallback diagnostics

Done when scenes, mutes, and fills can be performed on musical boundaries,
switch cleanly to prepared audio when available, and remain safe on cache misses.

## 12. Self-performing live showcase

- [x] Installed `live-showcase` command with automatic transport start
- [x] Timed filter, waveform, chord, envelope, and drive performance score
- [x] Score reset and replay on transport loop boundaries
- [x] Manual cockpit input layered over the automatic baseline
- [x] Session history export with autopilot/performer provenance
- [x] Promote repeated performer edits into scored typed future evidence
- [x] Load and replay a saved performer-future library
- [x] Replace live invalidating autopilot edits with render-safe automation
- [x] Bound default workers and verify a prepared horizon before transport

Done when the default launch is musically dynamic without intervention and
performer deviations can be learned, ranked, and reused in later sessions.

## 13. Warehouse techno / big-beat showcase

- [x] Bounded hard-sync, phase-modulated, wavefolded oscillator mode
- [x] Oscillator automation and cockpit waveform-cycle integration
- [x] Phrase-bank model with quantized phrase changes and variations
- [x] 140 BPM breakbeat, acid bass, chord stab, and texture phrase set
- [x] Long resonant sweeps with pre/post-filter distortion staging
- [x] Performance-safe self-playing `warehouse-showcase` command
- [x] Deterministic phrase-choice evidence, ranking, and phase-aware replay selection
- [x] Realtime layout-safe phrase replacement through the render coordinator
- [x] Interactive warehouse cockpit and phrase-future learning

Done when an original high-tempo set cycles through recognizable phrases,
evolves without intervention, and accepts safe learned performance variations.

## 14. Warehouse session fidelity

- [x] Persist exact phrase/scene identity in performer future evidence
- [x] Replay learned phrase choices near their learned musical phase
- [x] Show learned and queued phrase identity in the cockpit status panel

Done when a saved warehouse performance reconstructs the performer's phrase
choices at musically similar moments and makes those decisions visible.

## 15. Expressive phrase performance

- [x] Direct `F1`–`F4` phrase pads with bar-quantized launching
- [x] Phrase-pad variation selection without changing the base scene
- [x] Performer override grace window for learned phrase cues

Done when specific phrases and their variations can be played deliberately
without learned automation immediately fighting a live decision.

## 16. Flashy DSP and production effects

- [x] Tempo-synced stereo ping-pong delay with filtered feedback
- [x] General tempo-aware LFO and modulation routing
- [x] Phaser and per-voice stereo spread
- [x] Master compressor, soft clipper, and limiter
- [x] Modulated reverb with damping, predelay, and freeze

Done when the live set has broad rhythmic modulation, animated stereo depth,
large controllable spaces, and a performance-safe master bus.

## 17. Learnable live DSP macros

- [x] Typed deterministic performance FX rack with bounded macro settings
- [x] Cockpit keys and realtime publication for all five DSP macros
- [x] Persist, rank, and phase-replay performer DSP gestures

Done when delay, phaser, reverb freeze, modulation, and master drive can be
performed safely and reconstructed from saved performer sessions.

## 18. Realtime performance hardening

- [x] Bound warehouse cockpit rendering to one phrase-length loop
- [x] Hold immutable audio during scene invalidation and preparation
- [x] Move DSP macro recomputation off the TUI input thread
- [x] Add host-side clean-playback soak diagnostics

Done when rapid scene and DSP changes remain audible without underruns on a
representative release build and expose enough diagnostics to catch regressions.
