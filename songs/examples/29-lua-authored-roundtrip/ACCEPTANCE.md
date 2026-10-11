# Create, change, hear, save and return

Status: **accept, headless bounded subset**. Physical gestures/listening pending.
Real APC input/LED connection and clean stopped production TUI exit smoke pass.
See [scenario](SCENARIO.md), [results](RESULTS.md), [physical checklist](PHYSICAL_ACCEPTANCE.md)
and [guide](../../../MUSICAL_AUTHORING.md).

Executable specifications:

```sh
cargo run -p meldritch-control --example authoring_acceptance --offline -- --output /tmp/meldritch-authoring-acceptance
cargo test -p meldritch-control --test project_authoring --offline
```

Output directory must be new. The runner uses actual keyboard mapping, MIDI
press/release decoding, Lua policy, typed commands, worker publication and capture.
It launches a fresh process for reload; the integration test also spawns a fresh
test executable. Its ignored child entry is explicitly executed by the parent.
`acceptance.feature` names the executable Rust scenarios, with no Cucumber runner.
Examples 25–28/30 retain their detailed boundary acceptance.

Observable pass criteria:

- Load unchanged TOML with existing eight-voice oscillator/ADSR/VCA/filter.
  Ctrl-E edits waveform/release and changes published samples. Ctrl-G executes
  `compose.patch`, replacing osc with typed source and explicit cables.
- Ctrl-N plus `compose.pads` creates roll, four stable IDs, durations
  1920/480/480/1440 ticks and eight-step page navigation. Both release orders and
  velocity-zero release preserve intended notes. Rust ends are exclusive; the
  pressed endpoint includes its eighth-note step.
- Ctrl-M binds three Lua-declared faders to cutoff/volume/pan. Real CC bytes
  observe, wait for pickup, acquire, then edit to 6000 Hz / 0.5 / +1.
  Each changes compiled/published audio. Hard-right pan silences left output.
- Native 80×30 TUI shows authoritative parameters, note/end times, bindings,
  values/observation/pickup. APC and simulated terminal grid labels/colors agree;
  simulated Stream Deck transport/LCD musical telemetry tracks authoritative
  state. View changes and supported edits preserve playing transport.
- F6 saves a unique readable kind="project", version=1 Lua envelope
  containing instrument v2, pattern v1 and controls v1. Repeated text is identical;
  base TOML, prior exports and journals remain intact. q finishes a separate
  clean performance session.
- Fresh load without old journals restores exact graph, parameters, note IDs/
  counter, track assignment, mixer values and bindings. Transport/gestures/pickup
  reset. Same-source encode is identical. Full/split renders and saved/reloaded
  raw f64 samples match exactly: 192000 stereo frames.
- Invalid IDs/types/ranges, unsupported targets, dependent/stale graphs, dangling
  references, invalid durations, conflicting component values, unknown versions,
  changed base and uncaptured edits reject. Rejected commands preserve source,
  history/publication. Disconnect cancels holds; late releases do not edit music.
  Filesystem save failure reports unsaved state and preserves prior Lua/journal.

The envelope supports one selected existing synth and one selected pattern plus
declared control/mixer values and bindings. It requires the unchanged base root.
Multiple edited synths/patterns, arbitrary scripts, general DSP topology and
session replay are outside this accepted contract. Random Plaits is excluded from
exact PCM comparison. Granular sync/commit fault injection remains subsequent work.
