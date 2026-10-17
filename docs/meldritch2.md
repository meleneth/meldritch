# Meldritch 2: getting started

Meldritch 2 is the active project in this repository. Its Rust modules own
their contracts and DSP; Lua declares graph instances, values, controls, and
explicit cables. Its formats and behavior are independent of the
[Meldritch 1 archive](./archive/meldritch1).

Rust 1.97 or newer is required. From the repository root:

```sh
cargo build
cargo test
cargo run -- crates/meldritch2/examples/01-basic.lua
```

For terminal note performance, run:

```sh
cargo run -- perform crates/meldritch2/examples/53-tui-keys.lua
```

`cargo run -- devices` lists audio outputs; `cargo run -- midi-ports` lists
MIDI inputs. Add `--device=INDEX` and `--midi=INDEX` to `perform` when needed.
The executable [examples](./meldritch2/examples) define accepted behavior;
the [architecture](./meldritch2/architecture) describes current boundaries.

Meldritch 2 has no backward-compatibility obligation to Meldritch 1. Old
source, examples, and documents are retained for study, not as active tests or
design requirements.
