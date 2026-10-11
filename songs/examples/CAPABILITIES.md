# Example Capability Matrix

Status values:

- `design` — desired behavior is expressed by readable files
- `parse` — files deserialize and references resolve
- `compile` — files compile to typed runtime models
- `play` — the example renders or plays as specified
- `accept` — automated acceptance coverage proves the observable behavior

| Example | Capability | Status |
| --- | --- | --- |
| `34-empty-project-patch` | Read-only empty/legacy opening and app-owned source independent of instrument panel checked headlessly; construction, independent pitch/gate bindings and self-contained save/reopen unfinished | design |
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
