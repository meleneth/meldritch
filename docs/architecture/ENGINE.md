# Meldritch engine architecture

This repository owns the Rust engine and runtime only. Public cross-repository
behavior is defined by the immutable `meldritch-contracts` revision in
`contracts.lock`; Vue/Phaser source belongs to `meldritch-ui`, and authored Lua
applications belong to `meldritch-lua`.

## Crate direction

```text
meldritch-interface  <-  meldritch-arp
        ^             <-  meldritch-midi-utils
        |
    meldritch2  -> vendored vst3-host
        |
        +------ test-only meldritch-vst3-fixture
```

`meldritch-interface` owns host-neutral audio descriptors, processing traits,
named ports, and timestamped MIDI types. `meldritch-arp` and
`meldritch-midi-utils` are host-neutral processors. `meldritch2` owns runtime
assembly, graph compilation, the parallel DSP scheduler, devices, Lua VM,
sequencer, persistence, VST3 lifecycle, and HTTP/WebSocket adapters.

The detailed supported surfaces and explicit non-responsibilities are in the
pinned contract repository's `crates/` documents. Rust types remain the
authoritative in-process interface; YAML does not duplicate them.

## Realtime boundary

Audio callbacks execute only prepared Rust processors and bounded queues. They
perform no filesystem or network I/O, Lua execution, contract validation,
plugin construction, unbounded allocation, or unexpected locking. VST3
construction and destruction remain host-wide serialized outside the callback.
The dependency-level parallel scheduler is unchanged by repository fracture.

Contract and manifest validation happens before application evaluation and
before audio starts. UI assets are read from a configured independent release
directory on the control side before the network thread starts.

## Development

```sh
cargo check --workspace --all-targets
cargo test -p meldritch-interface -p meldritch-arp -p meldritch-midi-utils
cargo test -p meldritch2 --lib --tests
```

These commands need neither Node nor a Lua application checkout. The native
VST3 lifecycle suite is deliberately barred from default test commands and
requires the explicit `vst3-integration-tests` feature. Run it only when VST3
host or fixture behavior changes and the physical/plugin gate is intended.

An application directory can be anywhere on disk:

```sh
target/debug/meldritch2 /unrelated/path/to/application
```

The directory's `application.json` is validated before its entry script runs.
`MELDRITCH_UI_DIST=/unrelated/path/to/meldritch-ui/dist` configures an
independently built static bundle.
