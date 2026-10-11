# Empty project → construct → pattern → independent signals → play → save → reopen

Status: **design for the complete workflow**. The read-only opening checkpoint
is implemented: DSL `project_directory`, CLI project-opening tests and native TUI
tests cover empty/legacy roots, diagnostics and presentation. `opening.txt` and
`invalid/malformed-entry.toml` specify that checkpoint. Construction, bindings,
playback and self-contained save/reopen remain unimplemented. An additional
ownership checkpoint moves accepted definitions/provenance into app-owned
`ProjectSource`. Headless commands create/edit a pattern and edit a patch without
an instrument panel; their views observe the same accepted revision, rejected
commands preserve it, and immutable provenance stays unchanged. Run
`cargo test -p meldritch-app project_source` for this checkpoint. It uses legacy
example 09 as import provenance, not as an empty-project seed. No new authored
format exists; add readable authored fixtures before extending its schema. See
[the roadmap](../../../ROADMAP.md) and
[implementation prompt](../../../AUTHORING.md#recommended-immediate-implementation-prompt).

Opening checks: `cargo test -p meldritch-dsl --test project_directory`,
`cargo test -p meldritch-cli project_opening` and
`cargo test -p meldritch-tui empty_project`. An actual no-subcommand 80×24 PTY
draw/quit and alternate-screen restoration smoke also passes without files.
This evidence accepts only opening; it does not accept construction or music.

## Scenario and observable behavior

1. In a temporary empty directory execute `meldritch` with no subcommand. Open
   a silent valid project in a usable TUI without writing files or initializing
   anything. Git may be required installed; `.git`, a manifest, seed files,
   network access and MIDI are unnecessary.
2. Enter construction. Create one instrument definition/instance using existing
   oscillator, ADSR, VCA, audio-output and optional low-pass modules. Typed patch
   drafts permit incomplete construction; only a supported complete topology
   commits. Show IDs, typed ports, authored defaults and every cable.
3. Create a one-bar monophonic pattern using native keyboard inputs dispatching
   typed note insertion/duration/velocity/delete. Create C3 at tick 0 for 480
   ticks and G3 at tick 960 for 480 ticks, velocity 0.8, at 960 PPQ, 120 BPM,
   48 kHz. At least one duration and parameter edit changes finite rendered
   samples; stable note identities/counter survive.
4. Explicit composition bindings route independent pattern pitch/gate outputs
   to patch pitch/gate inputs. Internal cables connect pitch→oscillator,
   gate→ADSR, oscillator→VCA audio, ADSR→VCA level, VCA→output (optionally
   through low-pass). No hidden note-to-instrument cable. Inspect pitch values
   and end-exclusive gate intervals separately. Pitch holds between starts;
   specify initial pitch and adjacent retrigger ordering in the executable fixture.
5. Play and switch construction/performance without stopping transport.
   Disconnect the composition gate binding: its explicit visible policy resolves
   that input to low, leaving pitch events unchanged. Finite rendered audio is
   silent after release/tail; reconnect restores notes. Internal patch cables
   remain supported. This is not a hidden general normalization system.
6. Expose one runtime level control. Performance moves change published audio
   and capture an accepted runtime fact without changing authored defaults or
   fingerprint. Construction edits change source and published audio. Navigation
   does neither; saving does not promote runtime level.
7. Save new human-readable authored files containing all supported definitions,
   note ticks/IDs/counter, bindings, defaults, exposed controls and composition
   identity, without an unchanged seed root. Preserve previous files/journals.
8. Exit, start a fresh process in that directory and reopen through documented
   deterministic selection. Without controller script or replay, source and
   deterministic full/split raw f64 audio equal saved authored state. Runtime
   level, transport, holds and pickup start from documented defaults.

Choose an additive versioned save subset using existing semantic definitions and
serializers. Necessary metadata may be created on save, never required on empty
open. Existing base-bound Lua project v1 is not silently reinterpreted. Remove
all dependency on a fixture seed before the fresh reload process starts.

## Negative acceptance required

- Duplicate definition/instance/note IDs, bad counter, missing/wrong-family
  endpoints, unsupported topology and invalid note time/type/value.
- Incomplete/stale graph commit, multiple sources without a declared combiner,
  invalid exposed target and omitted accepted authored edits during export.
- Unknown version/field, path escape, missing saved component and incompatible
  reference. A malformed project must not be mistaken for an empty root.
- Save failure/interrupted write preserves previous good files and dirty edits;
  retry works. No success notice before save commit.
- Rejected commands preserve source/runtime, accepted musical history, worker
  generation and published audio. Save failures cannot erase edits or captures.
- Empty open and optional absent MIDI produce no hidden seed or Git repository;
  original CLI commands retain behavior.

## Execution, compatibility and limits

Bind real native key mapping and `AppController::dispatch` to authoritative
projections, `SongAudioHost`/worker publication and separately production callback
consumption; spawn a fresh reload child. Bindings must actually drive ordinary
note lowering, not just be displayed. No source truth in Lua/TUI tables or the
legacy backing pattern. Use deterministic oscillator fixtures for exact PCM.

Preserve examples 25–33 and historical compatibility fixtures, hashes and
base-bound instrument v1/v2, pattern/controls/project v1. Preserve Plaits f32/RNG
exceptions. Run focused then affected full crate suites and format/diff/docs checks.

Completion requires the whole workflow and negatives/compatibility. The opening
checkpoint does not advance the complete example beyond `design`. Two-pattern routing, polyphonic signals,
LFOs, normalization, libraries, quantized graph changes and DSP-state migration
are deferred. PCM replacement proves rendered behavior, not continuous DSP state.
Physical sound/gestures and deadlines require separate evidence.
