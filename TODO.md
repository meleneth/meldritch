# Active TODO

This is the implementation queue for the long-form polyphonic demo. The broader
project phases remain in [`ROADMAP.md`](ROADMAP.md).

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
- [ ] Interactive warehouse cockpit and phrase-future learning

Done when an original high-tempo set cycles through recognizable phrases,
evolves without intervention, and accepts safe learned performance variations.
