# Meldritch 2 development

This crate is the active project. It owns its Lua graph format, module
contracts, compiler, runtime, MIDI mapping, audio output, and TUI. It has no
dependency on Meldritch 1 crates and no backward-compatibility obligation to
their formats or behavior. Read [ARCHITECTURE.md](ARCHITECTURE.md) and the
[example acceptance corpus](examples/ACCEPTANCE.md) before extending it.

From the repository root:

```sh
cargo build
cargo test
cargo run -- crates/meldritch2/examples/01-basic.lua
bash tools/check.sh
```

The root workspace defaults to this crate. `cargo test --workspace` includes
the archived Meldritch 1 crates and is an intentional broader command. New
capabilities get a small Lua example, a behavior test, and an updated acceptance
contract. No archive format or interface needs a compatibility adapter.

Current boundaries are mono audio graphs and bounded per-channel note routes.
The module registry and explicit cables drive validation and execution; general
event-domain processing, graph-wide polyphonic voice allocation, and measured
physical callback deadlines are future work. The examples document exactly
which behavior is already accepted.
