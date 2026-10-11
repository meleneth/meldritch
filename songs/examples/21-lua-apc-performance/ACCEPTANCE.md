# Lua APC performance surface

Status: `design` until executable host acceptance is complete. Nothing is stable.

This example composes a supported four-lane TOML song with Lua launch/mute/solo
widgets. Rust owns commands, authoritative app state, rendering, recording and
MIDI. Lua owns reusable widget interactions and semantic presentation. Launches
are immediate after rerender publication, not quantized; there is no fabricated
queued/playing state before an audio generation is actually available.

## Required behavior

- Load the confined TOML song through the existing loader/compiler. Historical
  files remain unchanged and the LaunchControl/TOML host retains its behavior.
- A simulated APC pad runs the real Lua dispatcher, applies a real typed lane
  command, schedules the shared song worker and publishes changed audio through
  the existing atomic snapshot boundary. TUI lane state and simulated APC/LCD
  widget projections agree with the authoritative view.
- Selecting a variation in a second lane preserves the first lane's variation
  in both the app and published audio. Mute/solo and level changes compose.
- Lua-owned grid pads are dispatched once; releases and unbound notes cannot
  duplicate commands. Other authored MIDI bindings still use the established
  command path. No Lua runs in MIDI or audio callbacks.
- Accepted Lua and TUI/MIDI actions use the same timestamped performance-session
  writer. Rejected Lua commands produce diagnostics and no accepted session row.
- The APC projection emits state-driven palette colors, clears stale cells and
  suppresses identical refreshes. Another interface's accepted action refreshes
  every connected projection without independently stored musical state.
- Request and completion mailboxes each have one slot. Pending/obsolete audio
  work is superseded explicitly; accepted command/session facts are retained.
  Render/publication errors are reported while the previous audio stays usable.
- Tests exercise the actual worker, publication, app, Lua and recorder without
  requiring hardware. Include invalid command references and unknown colors.

## Run (once implemented)

```sh
cargo run -- tui-song songs/examples/21-lua-apc-performance --lua main.lua --midi-debug
```

The APC Session-mode Control port is configured by the TOML device declaration;
no device is required for validation. Grid columns 1–4 are drums/perc/bass/pad,
bottom four rows choose A/B/C/D, row 5 mutes and row 6 solos. Nine faders and
transport buttons keep example 19's map. Use a terminal at least 34 lines tall.

Physical LED/color and listening acceptance, fader takeover, automatic physical
reconnection, quantized launching and exact audio-clock replay remain separate
work. Full shared typed session migration is not implied by reusing the existing
writer; legacy debug-string fields and its retained event history remain.
