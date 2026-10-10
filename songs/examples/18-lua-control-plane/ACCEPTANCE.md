# Headless Lua control-plane acceptance

The scope of this example is a constrained composition adapter, not a format
migration or hardware acceptance. `main.lua` composes `main.mlperformance`.

- Load the song through the existing typed loader and render its oscillator.
- Save and restore Lua source in a versioned TOML envelope; preserve source,
  project/widget identities and song fingerprint. Reject changed song identity.
- One mode widget drives real `AppController` commands from APC, Stream Deck
  and TUI logical surfaces. All connected projections agree with app state.
- `home` and `detail` are children of `online`. An unhandled child event bubbles
  to `online`; guards reject detail entry until all-parameters mode is active.
  Transitions exit child before entering sibling, preserve shared parent, and
  change the displayed page. Unhandled events are explicitly rejected.
- Disconnect ignores input without changing musical state; reconnect publishes
  the current page and widget values, clearing stale surface cells.
- A bounded FIFO distinguishes inputs, requested commands, accepted/rejected
  commands and domain facts. Follow-up domain events are queued, not recursive.
  Capacity rejection happens before applying commands; traces include source,
  frame, handlers, lifecycle, commands, outcomes and surface updates.
- Accepted commands enter a lossless bounded journal with `u64` frames and
  replay through real typed dispatch on a fresh controller. Rejected commands
  never become accepted journal facts.
- Unknown commands, malformed/out-of-range bindings, duplicate identities,
  invalid transitions/cycles and escaped project references are diagnosed.
- Lua OS/files/process/module loading, bytecode and infinite execution are
  unavailable or bounded. Each project load owns a fresh VM; dropping a plane
  releases all callbacks and subscriptions. Projection failure is reported
  without rolling back already accepted musical commands.
- Historical fixtures remain unchanged. An independent pinned compatibility
  manifest loads each supported directory and compiles/renders its previously
  supported path, with finite audio and split-range equality.

Real MIDI/HID connections, bitmap encoding, automatic CLI session checkpointing,
pattern-launch widgets, live keyboard voices and audio rerender integration are
outside this example's acceptance. Existing paths retain their own tests.
