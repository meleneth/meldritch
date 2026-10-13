# Example Capability Matrix

Status values:

- `design` — desired behavior is expressed by readable files
- `parse` — files deserialize and references resolve
- `compile` — files compile to typed runtime models
- `play` — the example renders or plays as specified
- `accept` — automated acceptance coverage proves the observable behavior

Example 36 also has a headless runtime keyboard checkpoint: explicit active-mono
binding, bounded channel/key priority, independent Pitch/Gate, reset epochs,
atomic negatives, typed capture and identical reconstructed worker PCM. Runtime
bindings/held keys remain excluded from authored saves. Native F9 stopped-transport
audition now uses the production shared-DSP worker and timed PCM callback lane;
held-key PCM past a bar, ingress epochs/loss cleanup, typed capture reconstruction,
atomic ring negatives and zero callback allocation/deallocation pass headlessly.
Explicit keyboard velocity now has headless unit-interval port/multiplier wiring,
soft/hard/fallback gain, capture, negative and save/reopen acceptance. The real
worker/callback test uses velocity; unconnected patches retain fixed gain. Pattern
velocity routing and interactive external-port declaration editing remain open.
Explicit bipolar bend now composes with velocity through PitchModulate: independent
phase/partition references, raw typed capture, atomic store reload, rejection and
worker/callback loss centering pass. No implicit pitch range or articulation.
The explicit GateHold checkpoint now also passes held/retrigger/release sound,
partition/reference, capture/store reload, atomic negative and production
worker/callback cleanup tests with opt-in CC mapping. Physical listening/deadlines,
general gate routing, simultaneous arranged/live
routing and timed device replay remain open; full status stays `design`.

| Example | Capability | Status |
| --- | --- | --- |
| `36-patch-lfo` | Bounded mono graph LFO tremolo/vibrato/filter/depth/rate checkpoints: explicit adapters/combination/fan-out/reset, integrated phase/all waveforms, independent references/split PCM, negatives, typed drafts/capture, playing publication and fresh-process persistence pass headlessly. MIDI/MicroLab pattern step entry checks velocity, epochs/loss, capture/save/reopen. Incremental shared mono DSP checks u64 independent signals, FM/harmonic Plaits, partitions and atomic recovery. Native F9 stopped-transport audition checks production worker/callback PCM past a bar, ingress epochs/loss and source/transport cleanup, mode continuity, typed reconstruction, timed PCM ring negatives and zero callback allocation/deallocation. Explicit velocity Control/multiplier and bipolar bend/PitchModulate compose with headless phase/partition, raw capture/store reload, negatives and worker/callback loss centering. Explicit mono GateHold additionally accepts pedal CC mapping, held/retrigger/release references, partition/capture/save/reopen, atomic negatives and worker/callback release after loss. Host MicroLab discovery/connection observed; physical interaction/listening/deadlines, pattern velocity/input-declaration editing, general Gate routing/Plaits GateHold, simultaneous/polyphonic routes, timed device replay, Plaits control chains and broader event/control scopes remain open | design |
| `35-project-palette-routing` | Bounded headless empty-root shared-patch/native multi-instance/independent-source workflow; named project-v3 composition membership, stopped selection, worker/callback publication, play/edit/save/fresh-process reopen for both selections; typed capture, inactive identity/gain scope, shared edits and all negative-contract cases checked. Interactive palette deletion, broader clocks, timed replay, physical listening and DSP migration remain deferred | accept |
| `34-empty-project-patch` | Bounded headless empty-root keyboard construction/pattern/independent pitch-gate/play/modify/save/fresh-process reopen; worker and production callback changes, exact mono-to-stereo publication, runtime/default separation, failures/retry and full/split oscillator PCM/identity equality checked. Embedded capture baselines survive later saves/root removal. Timed replay, physical listening and continuous DSP-state migration remain deferred | accept |
| `00-minimal-synth` | Explicit oscillator-to-output patch | accept |
| `01-synth-note-pattern` | Note pattern drives pitch and gate | accept |
| `02-synth-parameter-pattern` | Pattern drives a synth parameter port | accept |
| `03-dsp-chain` | Performance patches a synth through a DSP graph | accept |
| `04-dsp-parameter-pattern` | Pattern drives a DSP parameter port | compile |
| `09-curated-performance-controls` | Default curated mode and Ctrl-Tab all-parameters mode | play |
| `11-session-capture` | Timestamped `.mlperformance` session capture | compile |
| `15-launch-control-xl-input` | LaunchControl XL faders/buttons drive curated controls | compile |
| `16-launch-control-xl-playground` | Full LaunchControl XL MIDI surface with playable groove scenes | play |
| `17-launch-control-xl-ensemble` | Banked multi-lane LaunchControl XL groovebox ensemble with lane/pattern-bank/sample metadata | parse |
| `18-lua-control-plane` | Constrained Lua composition, hierarchical pages, reusable mode widget, coherent simulated surfaces, typed journal and historical compatibility (headless scope in ACCEPTANCE.md) | accept |
| `19-apc-mini-mk2-playground` | Four synth lanes, sixteen variations, nine faders and script-authored APC Session input; Linux discovery/input observed, full hardware/audio smoke pending | play |
| `20-apc-lua-colors` | Bounded Lua-authored palette animation, MIDI output and static playground hints; headless protocol tests, visual hardware confirmation pending | compile |
| `21-lua-apc-performance` | Shared production song host, reusable Lua launch/mute/solo widgets, bounded APC input and RGB feedback; real commands/audio publication/session capture and APC/LCD/TUI coherence accepted headlessly; hardware/listening acceptance pending | accept |
| `22-midi-soft-takeover` | Control-thread MIDI FIFO, page/context-aware fader pickup, cross-controller rearming and shared audio/session behavior; curves/modifiers/negative paths accepted headlessly, physical interaction pending | accept |
| `23-midi-reconnect` | Paced production MIDI input/output recovery, stale-observation rejection, pickup/layer release and RGB reconstruction; failure composition accepted headlessly using immutable example 22, physical cable acceptance pending | accept |
| `24-audio-aware-widgets` | Generation-tagged song publication, production callback acknowledgement, reusable queued/ready/playing/error Lua widgets and shared native TUI projection; rapid supersession, failure, readonly API, reconnect and compatibility accepted headlessly, hardware/listening pending | accept |
| `25-edit-existing-synth` | Typed oscillator/ADSR/low-pass inspection and keyboard/Lua source edits, worker publication, unique Lua export and independent reload/render; automated headless acceptance, physical listening untested | accept |
| `26-apc-note-pattern` | Lua grid policy creates/assigns and toggles typed synth notes, held endpoints, eight-voice chords, velocity/delete modifiers, time/pitch navigation, shared native inspection, LED projections and versioned Lua persistence; automated headless acceptance, physical gestures/listening pending | accept |
| `27-held-pad-duration` | Release-aware held-start inclusive endpoint/exclusive end, cross-page duration, interruption/duplicate/rewind handling and deterministic reload/render via example 26; automated headless acceptance, physical holds/LEDs/listening pending | accept |
| `28-author-fader-mappings` | Lua/TUI fader assignments, source-owned cutoff/resonance, track volume/stereo balance and single-track delay mix/feedback; pickup, typed edits, dirty ranges, publication, projections and deterministic Lua reload/render accepted headlessly; physical faders/LEDs/listening pending | accept |
| `29-lua-authored-roundtrip` | Selected synth graph + note pattern + declared mixer/fader bindings; keyboard/MIDI/Lua/publication/projections, unique Lua save, clean exit, fresh-process exact source/f64 reload and negative tests accepted headlessly; APC connection smoke passes, physical gestures/listening and real Stream Deck transport pending | accept |
| `30-compose-typed-patch` | Keyboard command drafts and atomic Lua batches compose existing oscillator/ADSR/VCA/low-pass modules; typed ports/cycles/reference validation, Plaits replacement, publication and version-2 graph save/reload accepted headlessly; physical listening pending | accept |
| `33-plaits-models` | All 24 bundled crate model choices, explicit source gate, keyboard/Lua selection, finite main/aux publication, exact authored reload and allocation checks accepted headlessly; physical listening pending | accept |
| `32-plaits-module` | Pinned crate FM/harmonic module, typed editor/Lua parameters, worker publication, exact Lua round-trip, main/aux port-reference and allocation tests; physical listening pending | accept |
| `31-apc-drum-editor` | Lua drum rows, one-shot slot note commands, tap toggles/velocity/pages, generated 909-inspired kit, TUI/publication and independent Lua reload/render accepted headlessly; native TR-909 engine and physical acceptance pending | accept |

An implementation may advance a row only when all earlier statuses for that row
are satisfied. New schema or runtime behavior requires a new row or an explicit
extension to an existing example.
