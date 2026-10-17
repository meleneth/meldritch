# Meldritch 2

**Meldritch 2 is the active project.** It is a new Rust implementation of a
software-defined modular musical environment. Small Lua graphs compose
independently defined DSP modules through explicit ports and cables. The current
runtime supports mono `f64` graph execution, live controls, terminal performance,
and bounded timestamped MIDI notes. See the [Meldritch 2 architecture](crates/meldritch2/ARCHITECTURE.md)
and [executable examples](crates/meldritch2/examples/ACCEPTANCE.md) for exact scope.

From the repository root, ordinary Cargo commands select Meldritch 2:

```sh
cargo build
cargo test
cargo run -- crates/meldritch2/examples/01-basic.lua
```

For terminal note performance, run:

```sh
cargo run -- perform crates/meldritch2/examples/53-tui-keys.lua
```

`cargo run -- devices` and `cargo run -- midi-ports` list audio and MIDI
endpoints. Devices open only when requested. `tools/check.sh` is the active
crate check.
To install the active binary, use `cargo install --path crates/meldritch2`.
`cargo install --path .` still names the retained Meldritch 1 root package.

**Meldritch 1 is archived completed research.** Its source crates, fixtures,
song examples, original documentation, and Git history remain in this repository.
Read [the archive guide](MELDRITCH1_ARCHIVE.md) before inspecting or running it.
Its original overview is [preserved](MELDRITCH1_README.md). Meldritch 2 has
**no backward-compatibility obligation** to Meldritch 1 formats, interfaces,
sound, or behavior. The old implementation is not an active dependency or
acceptance gate for Meldritch 2.
