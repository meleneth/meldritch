# MIDI reconnect

Status: `accept` for the headless contract below. Nothing is stable yet.

This scenario uses the immutable song and Lua widgets in example 22. It adds
transport lifecycle acceptance without changing that project's musical source.

- A missing device or transport initialization/open error is recoverable.
  Discovery/open attempts are separated by at least one second, including
  repeated errors. A connected port is probed on that cadence.
- Port disappearance drops the connection. Input queued by that connection is
  invalidated; fresh connections cannot replay stale presses or fader movements.
- Disconnect/reconnect changes surface availability through the real dispatcher,
  preserving transport, selected page, Lua control state, controls, captured
  musical facts and the published audio snapshot.
- Reconnection reconstructs current semantic feedback, even when unchanged;
  failed writes retain no partially acknowledged frame and trigger retry.
- Pickup ownership is released only for the disconnected input device. Its
  first distant movement after reconnection cannot edit the song.
- Declared held modifiers are released via typed commands, avoiding a stuck
  layer after a lost note-off. The current shared boolean has no per-source
  reference count; a disconnect safely releases that declared shared layer.
- Simulated failures exercise the production lifecycle, dispatcher and feedback
  implementation. Negative cases cover initialization errors, missing ports,
  probe errors and output write errors; ordering and retry bounds are asserted.

Run:

```sh
cargo run -- tui-song songs/examples/22-midi-soft-takeover --lua main.lua --midi-debug
```

Unplug/replug the APC: current colors should return and fader 1
must reacquire pickup. Actual cable, button, fader, color and listening checks
are separate hardware acceptance, not implied by simulated tests.

Port discovery uses the configured name match; exact connected port identity
is checked. USB serial identity and detach/reattach between two polls are not
guaranteed. No discovery, Lua or MIDI work enters the audio callback.

Verify with `cargo test -p meldritch-control --test live_performance` and
`cargo test -p meldritch-cli control_thread_mapper`. The production lifecycle
is exercised with deterministic probe/open/write failures; its real dispatcher,
app commands, song render/publication and session capture are used directly.
This transport scenario has no new authored song format to round-trip; the
referenced example 22 remains in the historical load/compile/render corpus.
