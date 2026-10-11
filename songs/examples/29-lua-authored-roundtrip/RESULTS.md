# Execution record — 2026-10-11

Headless composed workflow **passes**. A fresh OS process reconstructs the same
authored source and assignment definition, encodes identical Lua and renders
sample-identical f64 PCM. Hardware acceptance is limited to connection/save/exit
smoke; physical gestures and listening remain pending.

## Verified commands

Executed from the repository root (offline dependencies already available):

```sh
cargo check --workspace --offline
cargo test -p meldritch-cli --no-run --offline
cargo test -p meldritch-control --test project_authoring --offline
cargo run -p meldritch-control --example authoring_acceptance --offline -- --output /tmp/meldritch-authoring-acceptance-verified
cmp /tmp/meldritch-authoring-acceptance-verified/saved.f64le /tmp/meldritch-authoring-acceptance-verified/reloaded.f64le
cmp /tmp/meldritch-authoring-acceptance-verified/saved-state.json /tmp/meldritch-authoring-acceptance-verified/reloaded-state.json
cargo run -q --offline -- validate-song songs/examples/29-lua-authored-roundtrip --authored edited-project.lua
cargo run -q --offline -- render-authored songs/examples/29-lua-authored-roundtrip --authored edited-project.lua --output /tmp/meldritch-composed-reference.wav
cargo run -q --offline -- validate-song songs/examples/09-curated-performance-controls
cargo test --workspace --offline > /tmp/meldritch-composed-tests.log 2>&1
cargo test -p meldritch-cli --offline > /tmp/meldritch-composed-cli-tests.log 2>&1
cargo fmt --all -- --check
git diff --check
cargo clippy --workspace --all-targets --offline -- -D warnings -A clippy::collapsible_if -A clippy::if_same_then_else -A clippy::manual_checked_ops -A clippy::too_many_arguments -A clippy::chunks_exact_to_as_chunks
npm run docs:build
```

Workspace: **416 passing, zero failing**, one intentionally ignored subprocess
entry (explicitly invoked by its passing parent). Composed suite: six passing
parent tests; fresh-process child also passes. Existing compatibility and separate
instrument, pattern, patch, assignment/mixer and Plaits suites pass. Formatting,
diff check, documentation build and Clippy pass; lint allowances are the existing
workspace baseline. No physical acceptance is inferred from these results.

Exploratory runner failures exposed mixer navigation swallowing Ctrl-E/Ctrl-N
and waiting for a rerender after a no-op pickup observation. Navigation is fixed;
the script now distinguishes changed edits from correctly non-rendering no-ops.
A new read-only simulated widget initially omitted its required interaction
function; fixed to return nil. Clippy caught a needless borrow; corrected.
Documentation initially failed dead links; corrected. Final automated checks
have no outstanding failures. Negative-test rejections are expected passes.

## Restored music and artifacts

| Authored item | Verified state |
| --- | --- |
| Base fingerprint | 54745c74a42665cf |
| Edited fingerprint | b083eb1f96df891e |
| Instrument | bass, eight voices, source oscillator triangle |
| Envelope/filter | release 0.09999999999999999 seconds, cutoff 6000 Hz; other supported values retained |
| Graph | explicit pitch → source → VCA → filter → output; gate → ADSR → VCA level |
| Pattern | roll, bass owner, looped 7680 ticks, next ID 5 |
| note-1 | C3, start 0, duration 1920 |
| note-2 | E3, start 1920, duration 480 |
| note-3 | G3, start 3840, duration 480 |
| note-4 | C3, start 4800, duration 1440 |
| Velocities | all 0.8 |
| Mixer | volume 0.5, pan +1; left PCM silent, right nonzero |
| Assignments | APC CC48 cutoff, CC49 volume, CC50 pan, main page |

Durations use 960 PPQ ticks; ends are exclusive. Four seconds at 48 kHz render
192000 stereo frames, 384000 f64 samples / 3072000 bytes. Each parameter movement
changes PCM, publication replaces the audio snapshot and callback consumption
remains nonzero. Full/split render and fresh-process PCM comparisons pass.

The runner retains artifacts under
`/tmp/meldritch-authoring-acceptance-verified/`: song/edited-project.lua,
unique song/authored/project-*.lua, saved-state.json, reloaded-state.json,
saved.f64le, reloaded.f64le, views.txt, separate clean song/performances journals
and reloaded-session.mlperformance. Host smoke adds its own unique export/journal.
The durable checked-in reference is [edited-project.lua](edited-project.lua).
The independent production render is `/tmp/meldritch-composed-reference.wav`.
WAV conversion uses f32; exact equivalence is tested before conversion at f64.

Invalid typed edits preserve source/history/publication. Invalid/stale saved
references, values, versions, durations/cables and changed base reject. Disconnect
consumes late releases without stray notes. A blocked authored directory reports
unsaved music while preserving previous Lua and journal. Export refuses musical
changes outside its supported selected scope rather than dropping them.

## Host smoke versus physical acceptance

Executed with host device access outside the sandbox:

```sh
cargo run -q --offline -- midi-controls-check songs/examples/29-lua-authored-roundtrip --seconds 1
cargo run -q --offline -- tui-song /tmp/meldritch-authoring-acceptance-verified/song --lua main.lua --authored edited-project.lua --no-autoplay
```

MIDI discovery reports APC Mini MK2 Control and Notes ports. Production TUI
renders the saved source (peak 0.188), connects APC input and Lua LED feedback,
opens the host audio stream, reports stopped callbacks with zero underruns/misses
in observed frames, accepts terminal F6 (ESC [17~), displays its new saved Lua path,
and exits via q with a clean separate journal. Its Lua export equals the reference.
Sandbox-only attempts failed because ALSA/default audio and /dev/snd/seq were
unavailable; host-access retries pass. Earlier run-8 and final output directories
were also used for initialization smoke before the verified run.

No physical pads/faders were operated and no listening result was observed.
APC gestures, LED distinguishability/orientation, pickup feel and musical audio
acceptance remain **pending** in [the physical checklist](PHYSICAL_ACCEPTANCE.md).
Stream Deck transport is **simulated only**; its Lua play/stop/rewind/save and
LCD musical-state projections pass headlessly. Real-device transport is absent.

## Friction and next iteration

Corrected direct editor navigation, silent lossy instrument export, missing
composed persistence, truncated selected pickup telemetry and missing performance
save-path confirmation. F6 avoids terminals collapsing Ctrl-Shift-S into Ctrl-S.

Remaining friction: textual patch commands, linear target selection, controller
pitch/time page and modifier policy poorly visible together in native TUI, and
the compact performance panel cannot show all three targets at 80×24. Save/resume
requires a reported relative filename plus the immutable base directory.
Occupied taps delete on release; evaluate accidental deletion physically.

Next musician-facing example: make page/pitch/modifier context obvious and shorten
supported assignment selection based on actual APC composition sessions. Keep
broader routing/catalog changes separate. Multiple edited synths/patterns in one
export, granular sync/commit fault injection and exact session replay remain
unfinished. Random Plaits retains its authorized deterministic exception; this
acceptance deliberately uses the deterministic existing synth.
