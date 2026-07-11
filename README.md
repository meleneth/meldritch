# Relational TUI Groovebox

This project is not “another groovebox with a terminal UI.”

The thesis:

> Build a TUI-first groovebox that treats sources, relationships, futures, and mix rules as first-class compositional material.

Most grooveboxes sequence tracks. This one sequences relationships.

The machine should:
- Use 64-bit floating-point audio internally by default.
- Prefer Eurorack-like patchable primitives over opaque integrated blobs.
- Automate integrations as inspectable patch recipes when convenience matters.
- Declare musical sources, roles, relationships, and performance rules.
- Compile those declarations into audio/control dependency graphs.
- Render deterministic futures across all available CPU cores.
- Cache rendered artifacts by semantic fingerprint.
- Pull from cache in the realtime audio path.
- Keep the realtime path tiny, predictable, and boring.
- Expose enough graph state that the performer can understand why the sound changed.

The first public demo should not be “look, a step sequencer.”
It should be:

> A dense groove that becomes clearer, stranger, and more alive when relationship rules are enabled.

## Suggested document order

1. `INVARIANTS.md`
2. `NUMERICS_AND_64BIT.md`
3. `EURORACK_PRINCIPLES.md`
4. `FEATURES.md`
5. `ARCHITECTURE.md`
6. `DSP_ALGORITHMS.md`
7. `ROADMAP.md`
8. `TESTING_STRATEGY.md`
9. `CODEX_PLAN_PROMPT.md`

## Initial implementation posture

Build the engine in layers.

Start headless. Prove:
- clock
- event scheduling
- DSL parse/validate
- graph compilation
- deterministic offline render
- cache keys
- invalidation

Then add:
- audio device output
- TUI
- worker pool
- speculative rendering
- relational DSP

Do not start by making a pretty grid. The grid is the cockpit. The engine is the dragon.

## Fixture Workflow

Generate the Csound sample fixtures:

```sh
bash tools/csound/generate-fixtures.sh
```

Render the sample-backed drum fixture through the Rust CLI:

```sh
bash tools/render-fixture.sh
```

Render the control-relation fixture and manifest:

```sh
bash tools/render-control-fixture.sh
```

Summarize a render manifest:

```sh
bash tools/manifest-summary.sh artifacts/control_relations.manifest.json
```

Assert expected manifest graph contents:

```sh
bash tools/manifest-check.sh artifacts/control_relations.manifest.json
```

The render script writes local WAVs and render manifests under `artifacts/`, which is intentionally ignored.

Emit a machine-readable project summary for scripts:

```sh
cargo run -p meldritch-cli -- summary-json fixtures/basic_drums.toml
```

Emit the compiled source/relation graph skeleton:

```sh
cargo run -p meldritch-cli -- graph-json fixtures/basic_drums.toml
```

Emit declared and compiled relation diagnostics:

```sh
bash tools/relations.sh fixtures/explicit_relations.toml
```

Declare explicit sample-to-pattern audio relations in project TOML:

```toml
[[relations]]
from = { sample_note = 36 }
to = { pattern = 1 }
kind = "audio"
```

Declare pattern-to-pattern control relations the same way:

```toml
[[relations]]
from = { pattern = 7 }
to = { pattern = 8 }
kind = "control"
```

Emit loaded sample diagnostics:

```sh
cargo run -p meldritch-cli -- samples-json fixtures/basic_drums.toml
```

Emit scheduled events as JSON for a render range:

```sh
cargo run -p meldritch-cli -- events-json fixtures/basic_drums.toml --pattern-id 1 --frames 48000
```

Emit graph invalidation from a pattern source:

```sh
bash tools/dirty-pattern.sh fixtures/control_relations.toml
```

Emit graph invalidation from a compiled source:

```sh
cargo run -p meldritch-cli -- dirty-json fixtures/basic_drums.toml --source-id 1036 --start 0 --end 48000
```

Emit graph invalidation from a sample note:

```sh
cargo run -p meldritch-cli -- dirty-note-json fixtures/basic_drums.toml --note 36 --start 0 --end 48000
```

Run the standard local check suite:

```sh
bash tools/check.sh
```
