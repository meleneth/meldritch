# Relational TUI Groovebox

## Install

From the repository root:

```sh
cargo install --path . --locked
meldritch --help
```

This installs the `meldritch` executable into Cargo's normal binary directory
(usually `~/.cargo/bin`). Rust 1.97 or newer is required. On Linux, CPAL also
requires the system ALSA development package (commonly `libasound2-dev` on
Debian/Ubuntu or `alsa-lib-devel` on Fedora).

## Live showcase

Start the self-performing drums, bass, chords, automation, effects, sidechain,
and realtime-rendering cockpit:

```sh
meldritch live-showcase
```

Playback starts automatically. Across each cycle the render-safe automation
changes filter, waveform, voicing, mute, modulation, ducking, level, scenes, and
drive without invalidating the live timeline. Normal
cockpit controls remain active, so these are a baseline rather than a fixed
render. Press `q` to finish; typed supported actions are written to
`artifacts/live_showcase.futures.json` with musical phase and
`autopilot`/`performer` provenance. Performer actions accumulate occurrence and
recency scores across sessions. On the next run, the four highest-ranked learned
actions are applied while transport is stopped, then the prepared horizon is
verified before playback begins. Select another library with `--futures PATH`.

The live preset defaults to two render workers, 16,384-frame chunks, and sixteen
warm chunks. Override these with `--workers`, `--chunk-frames`, and
`--warm-chunks`, but the bounded defaults are intended to avoid starving the
audio callback or saturating every CPU core.

The synth engine also includes `SyncFold`, an aggressive bounded oscillator that
combines synchronized phase resets, phase modulation, and wavefolding. It is
available through waveform automation and the cockpit's `w` waveform cycle.

Phrase banks group compatible pattern variations into ordered musical sections
with per-phrase repeat counts. The phrase cycler changes only at complete pattern
boundaries, selects variations deterministically, and emits every intervening
transition if a frontend polls late.

Render the original 142 BPM warehouse set with four breakbeat phrases, sync-fold
acid bass, chord stabs, octave jumps, glide, and long automation builds:

```sh
meldritch render-warehouse --normalize
meldritch play-showcase artifacts/warehouse.wav
```

Or prepare and play the complete set with one performance-safe command:

```sh
meldritch warehouse-showcase --require-clean
```

For the interactive version, run `meldritch warehouse-cockpit`. It starts the
142 BPM SyncFold setup automatically; press `Q` to queue the next phrase at the
next bar boundary and `C` to cancel a queued launch. Performer actions are
saved to `artifacts/warehouse.futures.json` when the cockpit exits. On later
runs, learned phrase choices are queued again near their original musical
phase while leaving any pending manual launch untouched. `F1`–`F4` queue a
specific warehouse phrase directly, using the same bar quantization and future
learning as `Q`.

Warehouse DSP macros are also live and learnable: `{`/`}` adjusts delay
feedback, `e`/`f` adjusts phaser mix, `V` toggles reverb freeze, `K`/`O`
adjusts modulation depth, and `X`/`W` adjusts master drive. Manual macro moves
temporarily suppress learned cues and are replayed near their learned phase in
later sessions.

It renders and normalizes the complete set before opening the audio device, so
the callback only reads immutable prepared audio. Subsequent runs can skip the
render with `meldritch warehouse-showcase --reuse`.

The default render is 32 bars (about 54 seconds) and writes
`artifacts/warehouse.manifest.json` alongside the WAV.

Warehouse synth voices use two independent normalized saturation stages:
pre-filter drive excites the resonant filter input, while post-filter drive
compresses and hardens its output. Both stages are bounded and remain finite
through the complete high-resonance sweep render.

This project is not “another groovebox with a terminal UI.”

The thesis:

> Build a TUI-first groovebox that treats sources, relationships, futures, and mix rules as first-class compositional material.

Most grooveboxes sequence tracks. This one sequences relationships.

The machine should:
- Use 64-bit floating-point audio internally by default.
- Prefer Eurorack-like patchable primitives over opaque integrated blobs.
- Automate integrations as inspectable patch recipes when convenience matters.
- Declare musical sources, roles, relationships, and performance rules.
- Compile those declarations into audio/control dependency graphs.
- Render deterministic futures across all available CPU cores.
- Cache rendered artifacts by semantic fingerprint.
- Pull from cache in the realtime audio path.
- Keep the realtime path tiny, predictable, and boring.
- Expose enough graph state that the performer can understand why the sound changed.

The first public demo should not be “look, a step sequencer.”
It should be:

> A dense groove that becomes clearer, stranger, and more alive when relationship rules are enabled.

## Suggested document order

1. `INVARIANTS.md`
2. `NUMERICS_AND_64BIT.md`
3. `EURORACK_PRINCIPLES.md`
4. `FEATURES.md`
5. `ARCHITECTURE.md`
6. `DSP_ALGORITHMS.md`
7. `ROADMAP.md`
8. `TESTING_STRATEGY.md`
9. `CODEX_PLAN_PROMPT.md`

## Initial implementation posture

Build the engine in layers.

Start headless. Prove:
- clock
- event scheduling
- DSL parse/validate
- graph compilation
- deterministic offline render
- cache keys
- invalidation

Then add:
- audio device output
- TUI
- worker pool
- speculative rendering
- relational DSP

Do not start by making a pretty grid. The grid is the cockpit. The engine is the dragon.

## Fixture Workflow

Generate the Csound sample fixtures:

```sh
bash tools/csound/generate-fixtures.sh
```

Render the sample-backed drum fixture through the Rust CLI:

```sh
bash tools/render-fixture.sh
```

Layer the native saw/envelope/low-pass bass voice onto that beat:

```sh
cargo run -p meldritch-cli -- render-bassline fixtures/basic_drums.toml --normalize
```

This writes `artifacts/bassline.wav` by default.

Render the first polyphonic showcase with drums, relational bass, and an
eight-voice chord progression:

```sh
cargo run -p meldritch-cli -- render-poly-demo fixtures/basic_drums.toml --normalize
```

The poly demo now defaults to 16 seconds and applies sample-accurate cutoff,
drive, level, modulation, and ducking automation plus stepped waveform,
voicing, mute, and scene changes to its bass and chord voices.

Render a longer four-section drum arrangement (intro, groove, breakdown, full
return) to `artifacts/arrangement.wav`:

```sh
cargo run -p meldritch-cli -- render-arrangement fixtures/basic_drums.toml --normalize
```

Play the whole arrangement, or loop a half-open section range such as the
groove and breakdown (`1..3`):

```sh
cargo run -p meldritch-cli -- play-arrangement fixtures/basic_drums.toml --from-section 1 --to-section 3 --loops 2 --normalize
```

Arrangement-enabled TUI sessions show a section strip above the pattern grid:
the active section is yellow, sections inside the selected loop are cyan, and
the label reports scene, repeat count, and current repeat.

This writes `artifacts/poly_demo.wav` by default.

Run the drum, monophonic bass, and polyphonic chord layers through the realtime
worker horizon and interactive cockpit:

```sh
cargo run -p meldritch-cli -- tui-poly-demo fixtures/basic_drums.toml
```

The polyphonic TUI includes a realtime automation inspector showing each
lane's current value, interpolation mode, next point, and active scene. The
same lanes are included in worker artifact keys and rendered DSP chunks.

Render the 32-bar showcase and its JSON manifest:

```sh
cargo run -p meldritch-cli -- render-showcase --normalize
```

The deterministic section sequence is intro → groove → full → variation →
breakdown → build → climax → outro. It uses four drum patterns, a four-bar bass
phrase, Cm–Ab–Eb–Bb chords, automated voicing and waveform changes, filter and
drive movement, kick ducking, a muted chord breakdown, and a full return. The
outputs are `artifacts/showcase.wav` and `artifacts/showcase.manifest.json`.

Play the full showcase through the default device, or run a one-second strict
render-delivery smoke check:

```sh
cargo run -p meldritch-cli -- play-showcase
cargo run -p meldritch-cli -- play-showcase --frames 48000 --require-clean
```

`--require-clean` fails on render underruns or missed artifacts. Backend stream
notifications are reported separately because virtual/default host devices may
emit them even when every source frame is delivered.

The render layer also supports event-aware effect rules: an `Accent` tag can
feed the delay bus while a `Ghost` tag feeds reverb. Each matched send produces
an explanation record containing its source pattern, track, step, frame, tag,
bus, and gain.

In `tui-poly-demo`, these buses are rendered by the worker coordinator and the
“Effect Sends · why” panel shows recent routed events with their track, step,
bus, matched tag, and gain.

Role-aware dynamics provide directional priority (kick over bass by default),
attack/release envelope following, selectable low/high/full-band ducking, and
an explanation containing the source and target roles, active bands, detector
peak, and maximum attenuation.

`tui-poly-demo` declares a kick-to-bass low-band sidechain. The coordinator
renders a kick-only detector signal, fingerprints the relation and settings,
and the “Sidechain · attenuation” panel shows roles, selected bands, and live
attenuation.

Projects can declare the dependency directly:

```toml
[[relations]]
from = { pattern = 7 }
to = { pattern = 8 }
kind = "sidechain"
```

The DSL validates pattern endpoints and compiles this to a distinct
`PatternSidechainsPattern` binding backed by a control edge, so dirty ranges
from the source propagate to the ducked target. See
`fixtures/sidechain_relations.toml`.

Finite audio chunks can be transformed deterministically with reverse, slice
reordering, single-frame freeze, or moving-window smear. Transform artifact
keys cover both source samples and the complete validated transform spec.

Capture and reorder a rendered bar into a new WAV and provenance manifest:

```sh
cargo run -p meldritch-cli -- transform-chunk artifacts/basic_drums.wav \
  --kind reslice --frames 96000 --order 3,2,1,0 \
  --output artifacts/resliced_drums.wav \
  --manifest artifacts/resliced_drums.manifest.json
```

Use `--kind reverse`, `--kind freeze --freeze-frame N`, or
`--kind smear --smear-radius N`; add `--play` to audition through the host.

In the TUI, uppercase `R/S/F/E` captures the currently published timeline and
creates reverse, four-way reslice, playhead freeze, or smear artifacts. The
derived-transform panel shows the specification, cache hit/miss, layout, and
provenance fingerprint.

Press `A` to atomically audition the derived timeline through the existing
audio stream and `D` to return to the latest worker-assembled live snapshot.
Transport position and device state are preserved across both swaps.

The speculative-futures planner models queued scenes, track mutes/unmutes, and
fills. Candidates are deduplicated and ranked deterministically: queued first,
then selected, then recent, with a configurable plan capacity.

Play the sample-backed drum fixture on the default audio device:

```sh
cargo run -p meldritch-cli -- play-samples fixtures/basic_drums.toml --loops 4 --normalize
```

Playback renders one pattern cycle ahead of time by default, then loops it in
the realtime callback. Use `--frames` to choose a different loop length.

Render future chunks on workers while playback is running:

```sh
cargo run -p meldritch-cli -- play-realtime-samples fixtures/basic_drums.toml --loops 4
```

Use `--chunk-frames`, `--warm-chunks`, and `--workers` to tune the render
horizon. Missing chunks fall back to silence and appear in playback diagnostics.

Open the interactive terminal cockpit (`p` play/pause, arrows or `hjkl` move,
space toggles a step, `r` rewinds, and `q` quits):

```sh
cargo run -p meldritch-cli -- tui-samples fixtures/basic_drums.toml
```

Open the same realtime cockpit with a native synthesized bass track selected:

```sh
cargo run -p meldritch-cli -- tui-bassline fixtures/basic_drums.toml
```

The bass track uses the same live velocity, gate, probability, invalidation,
worker rendering, and atomic chunk publication path as the drums.

Render the control-relation fixture and manifest:

```sh
bash tools/render-control-fixture.sh
```

Render the control-relation fixture with active controller velocity scaling:

```sh
bash tools/render-controlled-fixture.sh
```

The controlled render script also writes a manifest with the active scale and controller activity summary.

Summarize a render manifest:

```sh
bash tools/manifest-summary.sh artifacts/control_relations.manifest.json
```

Assert expected manifest graph contents:

```sh
bash tools/manifest-check.sh artifacts/control_relations.manifest.json
```

The render script writes local WAVs and render manifests under `artifacts/`, which is intentionally ignored.

Emit a machine-readable project summary for scripts:

```sh
cargo run -p meldritch-cli -- summary-json fixtures/basic_drums.toml
```

Emit the compiled source/relation graph skeleton:

```sh
cargo run -p meldritch-cli -- graph-json fixtures/basic_drums.toml
```

Emit declared and compiled relation diagnostics:

```sh
bash tools/relations.sh fixtures/explicit_relations.toml
```

Declare explicit sample-to-pattern audio relations in project TOML:

```toml
[[relations]]
from = { sample_note = 36 }
to = { pattern = 1 }
kind = "audio"
```

Declare pattern-to-pattern control relations the same way:

```toml
[[relations]]
from = { pattern = 7 }
to = { pattern = 8 }
kind = "control"
```

Emit loaded sample diagnostics:

```sh
cargo run -p meldritch-cli -- samples-json fixtures/basic_drums.toml
```

Emit scheduled events as JSON for a render range:

```sh
cargo run -p meldritch-cli -- events-json fixtures/basic_drums.toml --pattern-id 1 --frames 48000
```

Emit target events with incoming controller activity:

```sh
bash tools/control-events.sh fixtures/control_relations.toml
```

Assert expected controller activity:

```sh
bash tools/control-events-check.sh fixtures/control_relations.toml
```

Emit graph invalidation from a pattern source:

```sh
bash tools/dirty-pattern.sh fixtures/control_relations.toml
```

Emit graph invalidation from a compiled source:

```sh
cargo run -p meldritch-cli -- dirty-json fixtures/basic_drums.toml --source-id 1036 --start 0 --end 48000
```

Emit graph invalidation from a sample note:

```sh
cargo run -p meldritch-cli -- dirty-note-json fixtures/basic_drums.toml --note 36 --start 0 --end 48000
```

Run the standard local check suite:

```sh
bash tools/check.sh
```

Speculative rendering resolves scored performance gestures into complete scene,
track-mix, or fill recipes before worker submission. Future artifact keys cover
the render range, sample rate, sorted mute state, and current content
fingerprints for every participating pattern; missing mappings or content are
reported as unresolved candidates.

The speculative worker pool consumes those recipes in score order, deduplicates
in-flight artifacts, and reports desired, clean, queued, active, and completed
counts. Re-submitting an entirely clean plan is cache-only and leaves its worker
threads asleep.

Frontends can snapshot each desired candidate as missing, queued, rendering, or
clean. The TUI performance-futures panel shows those ranked candidates beside
aggregate cache health and whether speculative workers are working or asleep.

The performance launcher queues gestures to deterministic beat or bar
boundaries. When a gesture becomes due it updates the active scene, mute, or
fill state and selects its clean speculative artifact; if that artifact is not
ready, the launch is retained as a live-render fallback rather than delayed or
dropped.

At execution time, a clean speculative launch atomically replaces the realtime
immutable audio snapshot. A cache miss atomically restores the coordinator's
live chunk snapshot, keeping publication non-blocking on both paths.

Fill launches carry a tempo-derived end boundary. Their configurable beat
lifetime is fixed when queued; at expiry the overlay state clears and realtime
publication atomically returns to the live base-pattern snapshot.

Live performance controls are `Q` for the next arrangement scene, `Z` for the
selected track's mute/unmute, `P` for the configured fill, and `C` to cancel the
queued gesture. Commands use the current realtime playhead, and the application
performance tick executes due gestures through the future cache and realtime
publisher.

The conditional Live Performance cockpit panel reports the queued gesture and
boundary, active scene, muted tracks, fill expiry, cancellations, prepared and
fallback launch totals, automatic fill returns, and the most recent audio-source
decision.
