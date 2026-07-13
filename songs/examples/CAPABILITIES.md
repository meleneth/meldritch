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
| `16-launch-control-xl-playground` | Full LaunchControl XL MIDI surface declared in scripts | compile |

An implementation may advance a row only when all earlier statuses for that row
are satisfied. New schema or runtime behavior requires a new row or an explicit
extension to an existing example.
