# MELDRITCH Agent Guide

Read this file before changing the repository. Then read the project Markdown
documents relevant to the task. `TODO.md` is the active implementation queue;
do not infer that work is finished from the older completed showcase milestones.

## Product direction

MELDRITCH is a Rust, TUI-first relational groovebox inspired by Eurorack.
Songs are directories of human-readable TOML files that reference each other:

- `.mlsynth` — modular synth patch graphs
- `.mldsp` — modular DSP patch graphs
- `.mlpattern` — note/event or parameter patterns
- `.mlsamples` — sample-bank metadata: slots, relative asset paths, playback
  modes, levels, pitch offsets, and named slices
- `.mlperformance` — song assembly, curated controls, and recorded sessions

An authored song entry point is `main.mlperformance`. Timestamped captured
sessions belong under the song's `performances/` directory and also use the
`.mlperformance` extension with a distinct session document kind.

Synth and DSP definitions are modular all the way down. They declare modules,
typed ports, and explicit cables. Convenience recipes are allowed only when
they expand into the same inspectable patch graph. Do not introduce opaque
integrated instruments, effects, modulation systems, or hidden routing.

The default cockpit is performance mode: it shows only controls curated by the
loaded `.mlperformance`. `Ctrl-Tab` switches to the dense all-parameters mode.
Mode changes must preserve transport, playhead, selection, queued launches,
parameter state, and published audio.

Every accepted interactive action must eventually be written to a unique,
datetime-stamped, human-readable performance session for exact replay,
musical/quantized replay, and non-destructive harvesting.

## Example-first development rule

The trees under `songs/examples/` are the primary specification and acceptance
suite. `ML_FORMATS.md` is an index and explanation; it does not override the
examples.

For each capability:

1. Write the smallest readable example that demonstrates desired behavior.
2. Add `ACCEPTANCE.md` stating what must validate, compile, sound, display,
   record, or replay.
3. Add invalid examples for required diagnostics and boundary behavior.
4. Derive only the schema and runtime machinery required by those examples.
5. Turn the example into automated parsing, compilation, and/or end-to-end tests.
6. Update `songs/examples/CAPABILITIES.md` honestly.

Do not add speculative schema fields or abstractions without an example. Status
means:

- `design`: desired files and behavior exist only as a contract
- `parse`: files load, resolve, type-check, and fingerprint
- `compile`: definitions compile into executable/runtime structures
- `play`: the example audibly renders or plays as specified
- `accept`: automated acceptance tests prove all stated observable behavior

Never mark an example `accept` while part of its `ACCEPTANCE.md` remains
unimplemented.

## Non-negotiable architecture

Preserve `INVARIANTS.md`. In particular:

- Internal audio, parameters, and coefficients use `f64` by default.
- Absolute frame positions use `u64`.
- Source, relation, artifact, and realtime graphs remain distinct.
- Audio and control edges both participate in invalidation.
- Every mutation goes through typed commands.
- The realtime callback must not allocate, block, parse, access files, compile
  graphs, traverse unbounded structures, perform UI work, or log in hot paths.
- Parsed strings become typed IDs, targets, ports, values, and enums.
- Artifacts are deterministic and fingerprint every semantic input.
- Dirty ranges are precise and account for latency, lookahead, and tails.
- Relational and performance behavior must remain inspectable and explainable.
- External crates may provide bounded infrastructure; MELDRITCH owns graph
  semantics, invalidation, scheduling, fingerprints, recipes, and relationship
  DSP behavior.

## Current `.ml*` implementation state

The active milestone is section 19 of `TODO.md`.

Implemented foundations include:

- safe song-root loading through `main.mlperformance`
- path-aware TOML diagnostics and song-root reference confinement
- typed synth and DSP modules, inputs, cables, and signal compatibility
- note-name, musical-position, and duration conversion to `u64` frames
- note patterns and synth/DSP parameter patterns
- typed synth and DSP parameter targets
- curated performance control declarations
- deterministic semantic song fingerprints
- `meldritch validate-song SONG_DIRECTORY`
- executable oscillator, note/ADSR/VCA, low-pass automation, tempo-delay, and
  stepped DSP-feedback render paths
- chunk-identical pre-roll tests for oscillator, envelope, filter, and delay state
- typed `Performance` / `AllParameters` cockpit mode state
- default performance mode and `Ctrl-Tab` typed mode command
- a focused curated-control rendering layout that hides dense editor panels
- `meldritch tui-song SONG_DIRECTORY` for delayed-note `.ml*` songs
- loaded `.mlperformance` controls routed to typed curated-control commands
- script-declared MIDI devices and per-control MIDI CC bindings for absolute
  faders/knobs and step buttons
- `tui-song` script-declared MIDI input via the host MIDI stack (`midir`), with
  generic CC decoding routed through typed app inputs
- `meldritch midi-controls-check SONG_DIRECTORY` for listing MIDI ports and
  printing script-mapped CC events plus raw note/unknown MIDI messages without
  starting audio/TUI playback
- live `tui-song` rerendering for script-authored curated delay-feedback,
  delay-mix, synth low-pass cutoff, and synth low-pass resonance controls
- script-declared MIDI action bindings for transport and performance buttons
- latest-wins background rerender for live delay-feedback overrides
- completed song rerenders published through the existing atomic audio snapshot
- `tui-song` timestamped `.mlperformance` session files under `performances/`
- session checkpoints for accepted typed inputs, source fingerprints, and clean
  termination markers
- bounded session event checkpoint buffers and final runtime state snapshots
- structured session categories for mode, selection, transport, queue/cancel,
  transform, audio-source, synth-control, performance-FX, and parameter edits

Check `songs/examples/CAPABILITIES.md` for the exact current status. At the time
this guide was written:

- examples `00` through `03` are `accept`
- example `04` is `compile` because live curated-control override is unfinished
- example `09` is `play` because loaded controls now connect to a `tui-song`
  runtime and atomically published rerenders, but all-parameters inspection
  remains unfinished
- example `11` is `compile` because session files are produced and tested
  headlessly, but exhaustive action coverage and replay remain unfinished
- example `15` is `compile` because LaunchControl XL fader/button mapping and
  `tui-song` MIDI input wiring are tested headlessly from script-authored
  bindings, but real Windows/Linux hardware smoke testing has not been
  performed yet
- example `16` is `play` because the full LaunchControl XL default MIDI surface
  is declared in `.mlperformance`; supported feedback, mix, cutoff, and
  resonance controls rerender audio; launch buttons and the discovered
  side-column note/CC buttons can trigger typed transport/performance actions;
  the B row selects authored groove scenes/variations that rerender through the
  song synth and delay; and the physical strip controls use script-declared
  centered/overdrive curves so centered knobs are neutral and faders reach
  normal full-open before the physical maximum. The authored note patterns use
  the loader's 960 PPQ ticks per beat, and default performance mode shows the
  scene/fill map, actual authored note grid, and compact LaunchControl value
  telemetry so the playable surface is visible without opening all-parameters
  mode. `tui-song` autoplays by default for this hardware playground, and the
  live device output uses a dedicated song-audio publication so the generic
  backing TUI coordinator cannot overwrite the song with silence. Quantized
  launch timing, exact replay, and MIDI output/LED feedback are not yet
  implemented.

The next implementation direction is `17-launch-control-xl-ensemble`: an
example-first multi-lane LaunchControl XL performance. The requested musical
surface is one beat drum lane, two rhythm/percussion lanes, one polyphonic
chord/pad lane, two monophonic bass lanes, and three sample-based lanes. That
adds up to nine musical lanes against eight physical faders; the resolved
direction is nine musical lanes with eight visible strips and script-declared
bank/page switching. The example scene may put the beat drum on another page,
but that layout is scene/performance-authored data; Rust must only implement
generic bank/page mapping and keep the selected page reachable without stopping
playback. Generic `.mlperformance` lane/page declarations are parsed and
validated, loaded into app view state, and rendered by the performance-mode TUI
as generic pages/strips. Script-declared `select_page` actions can switch the
active page by page id. Page strips can declare visible control IDs, and
performance-mode telemetry renders the active page's declared controls. MIDI CC
control bindings may be scoped to a declared page, so the selected page can
remap physical controls without hard-coded LaunchControl policy.
`17-launch-control-xl-ensemble` now parses as a skeleton: nine tracks/lanes,
two scene-authored pages, 36 lane-owned note patterns/four variations per lane,
nine distinct renderer-compatible synth patches, and page-scoped fader
controls. Its lane
declarations now carry authored launch
quantization, default mute/solo state, per-lane control IDs, and nested pattern
banks that group four variations into selectable banks. `tui-song` loads that
lane metadata into generic app performance-page view state, and performance
mode renders visible strips with lane status, active variation, launch
quantization, and pattern-bank names/counts. Generic typed app commands can now
select a lane variation, select a lane pattern bank, toggle lane mute, and
toggle lane solo against that performance-page state; those results are
classified for session capture. `.mlperformance` actions can now bind
LaunchControl MIDI buttons/CCs to those lane commands without hard-coded
controller policy. Lane variation and pattern-bank selection now rerender the
currently selected song audio through the song rerender worker. Single-track
songs keep the delayed-note patch path with live override support; multi-track
placeholder songs use a compiled mixed-note patch. It also parses
`.mlsamples` sample-bank metadata and attaches the three sample lanes to a Raven
voice placeholder bank. Mixed-note rendering accepts script-targeted synth
filter overrides, so `tui-song` controls can change audio for mixed patches.
The main and drums pages now map faders to distinct lane synth/filter targets,
giving the hardware a simple but real palette before sample/poly engines land.
Audio sample decoding, sample triggering, sample rendering, audio-affecting
mute/solo behavior, and real pattern-bank runtime semantics beyond choosing a
lane variation are not implemented yet. The skeleton still uses placeholder
synth stand-ins for the sample lanes until sample-track playback, poly-synth,
and full multi-track audio support land.
Build this from examples first, then implement only the required multi-track,
sample-track playback, poly-synth, pattern-bank runtime, and modifier control
support.
Modifier/layer behavior such as “hold button + fader becomes octave pusher”
must be script-declared, typed, captured, and replayable; do not hard-code
LaunchControl policy in Rust. LED feedback remains deferred until the ensemble
control semantics are stable. Full all-parameters inspection remains open after
that.

## Important files

- `TODO.md` — active and completed implementation queue
- `ML_FORMATS.md` — `.ml*` format doctrine and example-first policy
- `songs/examples/CAPABILITIES.md` — honest capability status
- `INVARIANTS.md` — non-negotiable engine constraints
- `EURORACK_PRINCIPLES.md` — modular graph doctrine
- `ARCHITECTURE.md` — graph and crate boundaries
- `NUMERICS_AND_64BIT.md` — numerical policy
- `TESTING_STRATEGY.md` — required test layers
- `crates/meldritch-dsl/src/song.rs` — directory song loader and typed formats
- `crates/meldritch-render/src/song_render.rs` — initial executable `.ml*` plans
- `crates/meldritch-app/src/lib.rs` — typed application commands and mode state
- `crates/meldritch-tui/src/lib.rs` — key mapping and cockpit rendering

## Verification

Rust 1.97 or newer is required. Run focused tests while developing and the
relevant full crate suites before checkpointing:

```sh
cargo fmt --all -- --check
cargo test -p meldritch-dsl
cargo test -p meldritch-render --test song_examples
cargo test -p meldritch-render --lib
cargo test -p meldritch-app
cargo test -p meldritch-tui
cargo run -q -- validate-song songs/examples/09-curated-performance-controls
git diff --check
```

Do not claim device/realtime acceptance from headless render tests alone.

## Working and commit discipline

- Preserve user changes and unrelated dirty work.
- Use `apply_patch` for file edits.
- Keep changes example-driven and test the smallest affected boundary first.
- Update `TODO.md`, acceptance files, and the capability matrix in the same
  slice as implementation status changes.
- Make a focused git commit after each completed, verified implementation slice
  when committing is authorized. Do not leave a long chain of completed
  milestones only in an uncommitted working tree.
- Before committing, inspect `git status`, review the diff, and ensure the commit
  includes only the intended slice.
- Never discard or rewrite user work with destructive Git commands.
