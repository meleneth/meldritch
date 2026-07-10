# Codex Plan-Mode Prompt

Paste this into Codex after placing these docs in the repository root.

```text
You are working on a Rust project for a relational TUI groovebox.

Before coding, read these project documents in this order:

1. README.md
2. INVARIANTS.md
3. NUMERICS_AND_64BIT.md
4. EURORACK_PRINCIPLES.md
5. FEATURES.md
6. ARCHITECTURE.md
7. DSP_ALGORITHMS.md
8. ROADMAP.md
9. TESTING_STRATEGY.md

Enter plan mode.

Do not implement yet.

Produce a detailed implementation plan for the first milestone only.

The first milestone should establish:
- Rust workspace skeleton
- core typed IDs and timeline types
- command model
- minimal source graph
- Eurorack-style typed port model
- minimal relation graph with audio/control edge types
- node/module properties
- DSL fixture loading if reasonable
- tests for graph construction and invalidation basics

Preserve all invariants in INVARIANTS.md.

Important constraints:
- Target 64-bit desktop platforms first: x86_64/aarch64.
- Use f64 audio samples internally by default; convert to f32 or device format only at explicit boundaries.
- Use u64 absolute frame positions.
- Keep the realtime audio path separate from parsing, UI, graph compilation, and file IO.
- No arbitrary scripting in audio callback.
- No plugin hosting.
- No web UI.
- No premature full synth engine.
- Build testable headless foundations before TUI.

Your plan should include:
1. proposed crate/module structure
2. key structs/enums/traits to define first
3. tests to write first
4. implementation order
5. risks and how to avoid them
6. explicit non-goals for the milestone
7. commands I should run to verify the milestone

After the plan, wait for approval before making code changes.
```

## Follow-up implementation prompt

After approving the plan, use something like:

```text
Implement the first milestone from the approved plan.

Keep changes small and test-driven.

After implementation:
- summarize files changed
- list tests added
- show command output from cargo test
- call out any deviations from the plan
- do not start the next milestone
```

## Good first concrete task

```text
Create the Rust workspace and implement:
- typed IDs
- FrameRange
- EdgeKind::{Audio, Control}
- Linearity
- NodeProperties
- SourceGraph
- RelationGraph
- DirtyRange
- a minimal invalidation traversal that follows both audio and control edges
- tests proving audio-edge invalidation and control-edge invalidation
```

This gives the whole project a spine before the sound toys arrive.
