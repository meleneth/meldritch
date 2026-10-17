# Meldritch 1 archive

**Status: completed problem-space research.** Meldritch 1 established the
capabilities and constraints for a software-defined modular musical environment.
It is no longer an active development target. Meldritch 2 is a separate
architectural effort with **no obligation to load Meldritch 1 projects, preserve
its APIs, reproduce its sound, or maintain its behavior**. No compatibility shim
is introduced by this archive.

## What the investigation demonstrated

Meldritch 1 explored typed synth and DSP graphs, note and parameter patterns,
explicit audio/control relationships, deterministic rendering and fingerprints,
dirty-range invalidation, curated performance controls, controller input, a
TUI, Lua control surfaces, source editing, session capture, and project save and
reopen. Its later examples proved bounded native workflows: an empty project
becoming a playable patch, a shared palette across compositions, and explicit
mono LFO and keyboard-expression graph paths. The historical
[capability matrix](songs/examples/CAPABILITIES.md) and individual
`ACCEPTANCE.md` files record exactly which observations were automated and
which remained design or physical-listening work.

The useful findings include explicit typed cables and units, module-owned
behavior, distinguishable authored source and runtime overrides, atomic graph
acceptance, bounded callback work, deterministic semantic inputs, and example
contracts with negative cases. These are research outcomes, not requirements
that force Meldritch 2 to reuse any Meldritch 1 type or format.

## Why it remains an archive

Meldritch 1 accumulated several overlapping song, project, Lua, controller,
and playback paths while investigating the problem. Many headless checkpoints
were accepted within narrow scopes, while general graph execution, event
consumers, stateful revision migration, exact timed replay, general graph polyphony,
and independent physical deadline evidence remained open. Its original
[architecture](ARCHITECTURE.md), [roadmap](ROADMAP.md), [invariants](INVARIANTS.md),
and [active-at-the-time queue](TODO.md) document those choices and limitations.
They are historical records, not Meldritch 2's current design baseline or work
queue.

## Where the artifacts live

| Artifact | Historical location |
| --- | --- |
| Rust source and old CLI | Root `meldritch` package and `crates/meldritch-{app,audio,cli,core,control,dsl,render,tui}` |
| Authored song contracts and negative cases | `songs/examples/` |
| Audio, rendering, and parser fixtures | `fixtures/`, `assets/`, `artifacts/` |
| Original guides and plans | [original overview](MELDRITCH1_README.md), [agent guide](MELDRITCH1_AGENTS.md), root design Markdown and historical pages under `docs/` |
| Commit history | Existing Git commits and paths; no history was rewritten |

The sources remain in place so relative paths in examples and documents keep
working. To inspect or build the old package deliberately:

```sh
cargo build -p meldritch
cargo test -p meldritch
bash tools/meldritch1-check.sh  # full historical suite, opt in
```

`cargo test --workspace` also includes old crates; it is an explicit archive
plus active-project run, not the Meldritch 2 acceptance command. Ordinary root
`cargo build`, `cargo test`, `cargo run`, and `tools/check.sh` select Meldritch 2.

## Candidate material for later study

This is an inventory, **not a migration plan**. Reuse requires a new Meldritch
2 example, contract, and independent decision about ownership and semantics.

| Candidate | Location | Why it may be useful |
| --- | --- | --- |
| Oscillator, filter, delay and related DSP routines | `crates/meldritch-render/src/dsp.rs`, `effects.rs`, `stereo_fx.rs` | Algorithm references and sound comparisons |
| LFO, modulation, dynamics, mastering and Plaits adapters | `crates/meldritch-render/src/modulation.rs`, `song_control.rs`, `dynamics.rs`, `mastering.rs`, `plaits.rs` | Tested kernels, control-rate questions, upstream integration experience |
| Incremental mono voice and audio publication | `crates/meldritch-render/src/song_render/mono.rs`, `crates/meldritch-audio/src/` | State continuity and callback-boundary case studies |
| Typed editing, capture and controller recovery | `crates/meldritch-control/src/`, `crates/meldritch-app/src/` | Command, transaction, journal and device-lifecycle lessons |
| Audio and sample assets | `fixtures/audio/`, `assets/samples/`, `songs/examples/*/samples/` | Reproducible fixtures; check each asset's provenance before reuse |

No source, DSP routine, asset, API, or format is moved into Meldritch 2 by this
archive. Meldritch 2 does not depend on any old crate.
