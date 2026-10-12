# Shared palette, independent sources and named compositions

Status: **design for the full Slice 2 contract**. Native palette-pattern selection
and independent source routing are implemented;
`cargo test -p meldritch-control --test palette_routing` proves real native keys,
shared-patch source fan-out/independent gains, save/reopen and typed capture
reconstruction. TUI screen checks show selected IDs; the CLI host test proves
selection preserves source, runtime gains, worker generation and playing callback
audio. Named composition ownership, stopped selection and project-v3 persistence
now implement `compositions.txt` and `compositions.project.lua`. Control tests
restore both selections in isolated fresh processes and reconstruct typed capture;
CLI worker/callback tests prove stop acknowledgement, prepared publication and
inactive runtime-gain exclusion. `instance-keys.txt` now covers native creation and
stable-ID selection/bindings/levels for multiple shared-patch instances, stopped
membership add/remove, unchanged unrelated controls, typed capture reconstruction,
edited membership publication and isolated fresh-process persistence. Full Slice 2
status remains design pending its complete negative-contract audit.

## Pattern-selection checkpoint

`main.project.lua` is an executable project-v2 checkpoint: one palette patch is
shared by two independent instances. Melody supplies pitch and rhythm supplies
gate; rhythm's pitches/velocities do not change oscillator frequency/level.
The melody changes from C3 to G3 while the rhythm holds a gate. Each instance has
its own authored initial pitch/level and runtime gain identity.

Native PageUp/PageDown select the previous/next palette note pattern in stable
ID order, wrapping at either end. They work in the construction overview and
native pattern editor, preserve panel visibility, and leave source, runtime gain,
transport and audio generation/publication unchanged. A changed selection resets
the cursor to tick 0/C3 and clears note selection; selecting the same ID preserves
the cursor/note. Selection is captured as a typed command, not saved as music.
Performance mode, instrument/patch drafts, repeats/releases and modified keys
must not select a pattern. Track-owned and parameter patterns are excluded.

From an empty native project: n, c, Enter, Right twice, Up seven times, Enter,
Esc, v, p creates a pitch-bound instance. c, Enter, ] twice, Esc creates another
pattern with a longer C3 gate; g binds that pattern independently. PageUp selects
pattern-1 without changing either binding. Ctrl-N opens it for melody editing.
Ctrl-S/F6 saves both patterns and the independent bindings. Reopen restores
the same source/full/split oscillator audio and defaults; selecting a pattern
never silently rebinds a cable or launches material.

Negative fixtures in `invalid/selection.toml` require unknown/empty/track-owned
selection and incompatible bound clocks to reject without source/runtime/history
or selection changes. Failed bindings leave publication unchanged. Preserve
example 34's keys, source hashes and capture/importer contracts.

## Named-composition checkpoint and remaining Slice 2 contract

Two explicitly named compositions now use different subsets of the same palette,
persist both plus explicit selection, and reopen independently. Selection
prepares the new composition after acknowledged stop and before restart; retain
the prior active material on failure. Composition instances require stable scoped
identities and inspectable pitch/gate traces; one source may fan out to both.
Shared definition edits affect every referencing instance; instance runtime gains
remain independent. Prove rhythm edits move gates without transposition and melody
edits move pitch without moving gate boundaries. Reject missing/ambiguous
composition selection, conflicting instance IDs, unsupported chords/combination,
deletion of referenced palette material and incompatible clocks.

Use the existing authoritative Rust source, independent voice compiler,
SongAudioHost/worker/publication, typed capture and constrained Lua serializer.
Do not build another event engine or silently reinterpret project-v2 files.
Fresh-process composition persistence and composed native selection/publication
tests now pass, including native instance/membership authoring. Full acceptance
still needs the composed negative-contract and compatibility audit. Physical listening, quantized
activation and continuous DSP migration require separate evidence.
