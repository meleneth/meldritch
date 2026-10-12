# Shared palette, independent sources and named compositions

Status: **accept for the bounded headless Slice 2 contract**. The native keyboard
workflow starts in an empty directory, authors one shared patch, three independent
patterns and two instances, constructs two compositions, plays, edits the shared
patch during playback, saves and reopens each selection in a fresh process.
The audit below maps requirements to executable checks. Original v2 material stays
unchanged; `compositions.project.lua` demonstrates the v3 variant.

Accepted scope: mono independent pitch/gate instances, fixed stereo one-bar native
playback, static named membership and stopped composition selection. The existing
background worker publishes immutable finite PCM; this proves callback consumption,
not physical listening, deadlines or continuous DSP-state migration. Source traces
are inspectable bindings plus authored tick/frame event sequences, not a live CV
scope. Instance IDs are unique within the project; compositions reference them.
Deleting referenced palette material means removing it from an external saved
candidate and validating that candidate. An interactive palette-delete command is
not provided by this slice. General clocks/polyphonic inputs/event combiners,
quantized activation, metapattern scheduling, hardware mappings/pickup, timed replay
and physical acceptance remain future requirements, not restrictions on the
intended architecture.

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

## Named compositions

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
tests pass, including native instance/membership authoring. Physical listening,
quantized activation and continuous DSP migration require separate evidence.

## Completed acceptance audit

Test names are under `crates/`; commands execute their full files, including the
isolated children invoked by parent tests.

| Requirement | Executable evidence |
| --- | --- |
| Empty-root construction → independent melody/rhythm → two instances/compositions → play/edit/save/reopen | CLI `project_playback::tests::native_two_composition_keyboard_workflow_publishes_and_reopens_each_selection` executes `keyboard-workflow.toml`, compares published stereo to source PCM, consumes audio with the production callback and invokes fresh-process reopen for both selections. |
| Independent pitch/gate timing, fan-out and authored/runtime levels | Control `palette_routing::shared_patch_fanout_keeps_pitch_gate_and_instance_gain_independent` compares every frame with an oscillator reference, edits each source separately and checks independent gain sums. |
| Native pattern selection, cursor/visibility/identity and capture; no rebinding | Control `palette_routing::native_keys_route_two_patterns_and_selection_survives_typed_capture`, its track-owner/clock negatives, TUI `native_pattern_selection_is_visible_without_rebinding`, and CLI `native_palette_selection_preserves_playing_audio_and_runtime_gain`. |
| Named membership/default selection, canonical IDs, all palette persistence, exact full/split PCM | Control `compositions::both_compositions_reopen_in_fresh_processes_without_copying_the_palette` checks both selections and edited membership through isolated child processes and direct member-only compilation. |
| Shared definition edit reaches every referencing instance/composition, overlays remain independent | Control `compositions::one_shared_patch_edit_changes_each_composition_and_preserves_instance_overlays`; composed CLI workflow edits the shared oscillator while playing and checks replacement PCM. |
| Instance selection independent of inspectors; exact-ID bindings/levels; idempotent a/d; no inactive retarget | Control `compositions::native_shared_instance_keys_membership_and_controls_reconstruct_without_retargeting` and TUI `shared_instance_surface_highlights_one_id_and_guards_overview_gestures`. |
| Failed membership/selection preserves accepted state; pending transport/running activation rejected | Control `compositions::invalid_instance_selection_and_membership_preserve_accepted_state`, `malformed_or_inactive_composition_material_and_pending_play_reject`, and CLI `native_composition_transition_requires_stopped_transport_and_prepared_audio`. |
| Prior audio retained until prepared; inactive gains retained/excluded; failed bound edits preserve playing publication | CLI composition-transition and composed keyboard tests assert source/worker/publication/history continuity and exact active-member PCM. |
| Missing/multiple selection, duplicate composition/member IDs, inactive malformed definitions and bounds | Control `compositions::malformed_or_inactive_composition_material_and_pending_play_reject`; `audited_invalid_candidates_reject_referenced_deletion_chords_combination_and_clocks` executes every `invalid/audit.toml` case. Native/legacy entry ambiguity remains covered by control `native_project` tests. |
| Conflicting instance IDs, chords/overlapping gates, undeclared combination, referenced patch/pattern removal and incompatible clocks | Control audited-invalid-candidates test checks diagnostics and unchanged source/selection/runtime/history; duplicate-instance/chord typed requests also reject. Inactive referenced material validates too. |
| Typed capture reconstruction, source fingerprints and excluded runtime defaults | Control native-key composition tests reconstruct from the embedded baseline; fresh-process CLI/control children restore stopped authored defaults without replay or new capture/files. |
| Historical compatibility | Workspace offline suite includes examples 25–34, legacy formats, constrained Lua, Plaits, pickup, patch transactions, save failure/retry and capture baselines. Read-only CLI validation retains example 09/29 fingerprints `41472b38b65b60ed` / `54745c74a42665cf`. |

Run `cargo test -p meldritch-control --test compositions --test palette_routing`
and `cargo test -p meldritch-cli native_`, plus TUI and workspace suites. The root
binary includes the CLI tests too, so workspace verification protects both entry
points. Formatting, affected all-targets Clippy with existing allowances, diff
checks and the VitePress build complete documentation/compatibility validation.
