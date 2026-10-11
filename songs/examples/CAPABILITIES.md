# Example Capability Matrix

Status values:

- `design` — desired behavior is expressed by readable files
- `parse` — files deserialize and references resolve
- `compile` — files compile to typed runtime models
- `play` — the example renders or plays as specified
- `accept` — automated acceptance coverage proves the observable behavior

| Example | Capability | Status |
| --- | --- | --- |
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

An implementation may advance a row only when all earlier statuses for that row
are satisfied. New schema or runtime behavior requires a new row or an explicit
extension to an existing example.
