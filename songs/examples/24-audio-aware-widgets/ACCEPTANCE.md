# Audio-aware Lua pattern widgets

Status: `accept` for the headless contract below. Nothing is stable yet.

The lead has two patterns. The same Lua widget is bound to APC pads 0/8,
simulated Stream Deck keys 0/1 and TUI text cells 0/1.

- Accepted launch commands immediately select authoritative state, record once
  and display amber `queued` while the song render is pending.
- Publishing a complete snapshot alone cannot display green `playing`.
  A real production playback renderer must consume a present frame from that
  exact immutable generation in a playing callback before green appears.
- The older callback-consumed pattern remains identifiable while a newer one
  is queued. Rapid launches supersede older work; stale completions cannot
  overwrite the requested generation or falsely acknowledge a newer pattern.
- Stopped transport shows `ready` after publication; muted/excluded lanes and
  missing audio metadata show `unavailable`. Render/publication failure shows
  `error`, preserves the previous audio and records no invented command fact.
- All simulated surfaces and the native TUI read the same app projection.
  The demo declares a compact layout with readable audio status at 80×24.
  Feedback is semantic and unchanged refreshes do not produce redraw storms.
- Disconnect/reconnect restores current audio-aware presentation without
  resetting selection, song audio or the Lua hierarchy.
- Lua save/load retains source, IDs and semantic behavior. The new project joins
  the historical loader/compiler/range-identical render corpus.
- The callback adds only bounded scalar/atomic generation acknowledgement;
  Lua, metadata mapping, UI rendering, strings and trace work stay outside it.

Negative fixtures reject unknown audio dependencies and readonly projection
mutation, and exercise a publication layout failure without replacing audio.

```sh
cargo run -- tui-song songs/examples/24-audio-aware-widgets --lua main.lua
cargo test -p meldritch-control --test live_performance
```

Green means a complete callback rendered present samples from that generation.
It does not prove DAC output time, human listening, nonzero signal, quantized
launch scheduling or physical LED appearance. Muting/solo uses existing lane
inclusion semantics. Callback acknowledgement is refreshable telemetry, not a
lossless performance event. General rewind/loop-aware input epochs remain open.
