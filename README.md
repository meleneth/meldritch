# MELDRITCH: a patchable musical environment

**In massive development: nothing is stable yet.** APIs, formats, commands,
controller mappings and behavior may change. See the
[documentation](https://meleneth.github.io/meldritch/),
[capability matrix](songs/examples/CAPABILITIES.md) and [active queue](TODO.md).

The intended model is project-oriented: reusable definitions and compositions
built from explicit typed relationships, with the machine itself as a playable
compositional instrument. **Coherence under composition** governs design. See
[requirements](FEATURES.md), [architecture/code audit](ARCHITECTURE.md) and
[next roadmap](ROADMAP.md). `meldritch` now opens the current directory: an empty
root shows a project overview, while `main.mlperformance` roots use the
existing song cockpit. `meldritch open-project PATH --check` validates opening
without terminal/audio/MIDI. In an empty root, **n** creates a silent mono-voice
palette definition with inspectable pitch/gate inputs, modules and cables.
**Ctrl-E** opens parameters and **Ctrl-G** opens typed patch drafts. **c** creates
an independent one-bar pattern; arrows move its cursor, Enter inserts/selects
notes, **[ ]** change duration, **- +** change velocity, and Delete removes notes.
Esc closes an editor; **Ctrl-N** reopens the pattern. With a patch and pattern
selected, **v** creates an unbound mono instance; **p** and **g** separately
connect/disconnect pattern pitch and gate. Its inspector shows pitch's C3 fallback
and gate's low fallback. **Space** starts/stops native one-bar playback once the
worker has prepared audio; the output device opens only on the first play request.
**Ctrl-Tab** switches construction/performance while playback continues;
performance hides internal editing and blocks construction gestures. In construction,
**x** explicitly exposes the selected patch's output level. In performance,
Up/Down select an exposed patch, Left/Right change runtime gain, Home silences and
End restores the authored instance default. These gestures leave source unchanged.
This path is checked headlessly through the production callback; physical
listening remains unverified. Accepted gestures now create a timestamped typed
capture under `performances/`; opening alone creates no files. Capture preserves
construction/runtime commands but does not replace authored saving or automatically
reopen the project. **Ctrl-S/F6** saves the complete authored project as
`main.project.lua`; starting `meldritch` there restores patches, patterns, bindings
and exposed controls. Transport starts stopped and runtime gain starts at authored
defaults. Save works in either mode and editors; it excludes staged drafts.
The screen distinguishes saved/unsaved authored revisions from capture. Conflicting
legacy/native entries, symlinks and external entry edits diagnose explicitly.
The composed native-key workflow is accepted headlessly, including fresh-process
reopen and exact deterministic oscillator rendering. Native captures embed their
opening source so subsequent saves cannot erase the reconstruction baseline.
For MicroLab mk3 or another MIDI keyboard, `meldritch midi-keyboard-check` lists
inputs without a song or audio. `meldritch midi-keyboard-check --port MicroLab
--seconds 20` monitors decoded notes, velocity, bend and CCs. Start a native project
with `meldritch open-project . --midi-keyboard MicroLab`; select a pattern and open
**Ctrl-N** in construction mode. Notes insert at the cursor with MIDI velocity;
arrows move the cursor. Close the editor and use Space to play the independently
bound pattern, then Ctrl-S to save. With transport stopped, **F9** binds the selected
active mono instance for held-key audition; F9 again releases it. Wait through the
device's four-second startup mute and 100 ms fade. Mode/selection changes preserve
the binding; starting transport or editing authored source releases it. Held keys
do not create pattern notes, even with the pattern editor open. The same worker
DSP continues across short streaming blocks. A declared `input.velocity` Control
input now carries strike velocity / 127; its cables decide what velocity changes.
Copy example 36's `velocity.project.lua` as `main.project.lua`, then apply
`velocity-patch.txt` in Ctrl-G to multiply the envelope by strike velocity and
Ctrl-S to save. The inspector shows unit-interval units. Unwired velocity has no
effect; held-key fallback restores pitch and velocity without retriggering.
For bend, start with `bend.project.lua` and apply `bend-patch.txt` as well: the
declared bipolar `input.bend` Control passes through an explicit PitchModulate
adapter, with authored ±2-semitone depth. Wheel motion changes neither key
ownership nor articulation; disconnect/overflow centers it. Sustain,
simultaneous pattern/live routing and timed recording remain queued.
Port matching is explicit and rejects ambiguity; reconnect retries
without accepting stale messages. Headless entry/publication/persistence tests
pass, including shared worker/callback held-key audition. MicroLab port
discovery/connection has been observed; physical gestures/listening have not.
In construction, **PageUp/PageDown** selects palette patterns at the overview or
pattern editor. Selection keeps bindings unchanged; **p/g** explicitly binds the
selected pattern's pitch/gate. This lets melody and rhythm come from different
patterns. In the construction overview, **b** names a composition containing all
declared instances; **F7/F8** selects named compositions after stopping transport.
Wait for prepared audio before restarting. Named projects save as project-v3 Lua;
unnamed version-2 projects keep their current meaning. Composition selection saves
the authored default and preserves instance runtime gains without saving them.
At the overview, **v** creates and selects another instance of the chosen patch;
**Up/Down** selects instance IDs independently of patch/pattern selection.
**a/d** adds/removes the selected instance in a named composition while stopped.
Performance arrows address the selected active instance's level, so shared-patch
instances remain independently controllable. Inactive selection never retargets.
Example 35 now accepts this bounded workflow headlessly, including both composition
selections, shared edits, negative diagnostics and fresh-process reopen.
Example 36 adds a working tremolo graph: LFO → explicit scale → envelope multiplier
→ VCA. Open `songs/examples/36-patch-lfo`; Ctrl-E/Ctrl-G inspects and edits the same
typed graph. See [patch commands](PATCH_EDITOR.md). Static Hertz/beat timing,
explicit gate reset and authored save/reopen are checked headlessly; broader LFO
modulation remains in progress.
Timed replay and stateful graph transitions remain proposed. Existing commands
remain valid.
Git is allowed as a required installed dependency; opening needs no repository
or initialization. Empty opening creates no files or audio/MIDI streams.

Create, change and return to music with [composed authoring](MUSICAL_AUTHORING.md):

```sh
cargo run -- tui-song songs/examples/29-lua-authored-roundtrip --lua main.lua
```

**Ctrl-E** edits synths, **Ctrl-N** inspects APC notes, **Ctrl-M** assigns faders
and mixer values. **F6** saves the composed instrument, patch, pattern,
mixer and bindings as readable Lua; restart with `--authored PATH`. Fresh-process
source/audio equivalence passes headlessly. APC connection/clean exit were
smoke-tested; physical gestures and listening remain pending.

Edit an existing synth with **Ctrl-E** in `tui-song`, then save a readable Lua
definition with **Ctrl-S**. See [the instrument editor guide](INSTRUMENT_EDITOR.md)
for the supported parameters, independent reload/render commands and test scope.

Explore all 24 bundled [Plaits model choices](PLAITS.md):
`cargo run -- tui-song songs/examples/33-plaits-models --lua main.lua`.
In Ctrl-E, select the model and cycle it with Left/Right; Ctrl-S saves the patch.
Physical listening acceptance remains pending.
Live device output starts muted for four seconds, then fades up over 100 ms.
See [audio output startup](AUDIO_OUTPUT.md); authored mixer values are preserved.

Compose notes with the [Lua APC piano roll](PIANO_ROLL.md):
`cargo run -- tui-song songs/examples/26-apc-note-pattern --lua main.lua`.
Physical gesture/LED/listening acceptance is still pending.

Compose drum hits with the [APC drum editor](DRUM_EDITOR.md):
`cargo run -- tui-song songs/examples/31-apc-drum-editor --lua main.lua`.
Its generated kit is 909-inspired; a native TR-909 engine is not implemented.

Assign APC faders interactively to synth, delay and mixer parameters with
[the control assignment editor](CONTROL_ASSIGNMENTS.md):
`cargo run -- tui-song songs/examples/28-author-fader-mappings --lua main.lua`.
**Ctrl-M** opens assignments; **Ctrl-S** saves readable Lua mappings and values.

For the connected APC Mini MK2, start the four-lane playground:

```sh
cargo run -- tui-song songs/examples/19-apc-mini-mk2-playground --midi-debug
```

See its [control map and acceptance scope](songs/examples/19-apc-mini-mk2-playground/ACCEPTANCE.md).

Run a Lua-authored LED animation and leave static playground color hints:

```sh
cargo run -- apc-lua-colors songs/examples/20-apc-lua-colors/colors.lua
```

Add `--dry-run` to inspect the bounded MIDI plan without hardware. See the
[Lua color API and remaining hardware checks](songs/examples/20-apc-lua-colors/ACCEPTANCE.md).

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

The realtime warehouse cockpit renders only its current phrase-length loop.
During a scene change it keeps the previous immutable audio playing until the
replacement loop is prepared, avoiding publication gaps during invalidation.
DSP macro renders run on a latest-wins background worker, so rapid control
changes replace obsolete pending work instead of blocking the TUI/audio path.

To verify a host under concurrent audio and DSP load, first render the
warehouse WAV and then run:

```bash
meldritch warehouse-soak --seconds 120 --require-clean
```

The soak report includes callbacks, underruns, missed artifacts, backend stream
errors, completed DSP stress renders, and worst DSP render latency.

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

> Build a TUI-first musical environment that treats sources, relationships, futures, and mix rules as first-class compositional material.

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

This is historical engine-layering advice. Current vertical slices in
[ROADMAP.md](ROADMAP.md) supersede its sequencing recommendations.

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
