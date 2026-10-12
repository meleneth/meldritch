# Empty project → construct → pattern → independent signals → play → save → reopen

Status: **design for the complete workflow**. The read-only opening checkpoint
is implemented: DSL `project_directory`, CLI project-opening tests and native TUI
tests cover empty/legacy roots, diagnostics and presentation. `opening.txt` and
`invalid/malformed-entry.toml` specify that checkpoint. Native patch and independent
note authoring and independent mono binding checkpoints are implemented.
Bindings render through the existing worker and production callback headlessly;
native lazy-output playback and mode transitions are implemented with headless
host/callback checks. Explicit patch output-level declarations and instance runtime
overlays are implemented as specified by `keyboard-level.txt`; app/TUI/native-host
tests prove rejected identity/value commands preserve source/history, independent
instance values, mode/patch preservation, load reset, callback mute/restore with
advancing transport, and no generation for repeated identical settings. Accepted
runtime facts now persist through native typed disk capture as specified by
`capture.txt` and its valid/invalid payload fixtures. `cargo test -p
meldritch-control --test native_capture` checks lazy file creation, bounded pending
buffers, failure recovery without duplication, history/source-continuity diagnostics,
confined paths and reconstruction of source/runtime/PCM after history eviction.
The bounded self-contained serializer checkpoint is now implemented as specified
by `persistence.txt`, `authored-project.lua` and its invalid neighbor.
`cargo test -p meldritch-control --test native_project` proves authored-only
round trips, deleted-note counters, optional filter/defaults, independent bindings,
Plaits authored settings, bounds/diagnostics and fresh-process raw oscillator PCM
equality without the original root. Runtime overlays do not enter the format.
Native atomic disk save, entry discovery/TUI reopen and timed replay remain
unimplemented; the full workflow therefore retains design status.
Native 80×24 PTY checks also prove file-free empty open/quit, twelve typed
construction/runtime events and a clean final gain, and capture-path failure
deferring another gesture/quit until successful retry without duplicates. These
smoke checks open no audio device; they do not establish physical listening.
Physical listening remains unverified. Run `cargo test -p meldritch-app --test
pattern_voice`, `cargo test -p meldritch-tui native_level` and `cargo test -p
meldritch-cli project_playback` for this checkpoint.
An additional
ownership checkpoint moves accepted definitions/provenance into app-owned
`ProjectSource`. Headless commands create/edit a pattern and edit a patch without
an instrument panel; their views observe the same accepted revision, rejected
commands preserve it, and immutable provenance stays unchanged. Run
`cargo test -p meldritch-app project_source` for this checkpoint. It uses legacy
example 09 as import provenance, not as an empty-project seed. Typed silent palette
creation is also implemented, as specified by the construction checkpoint below.
Project-v2 Lua is additive; base-bound project-v1/instrument-v1/v2 exports retain
their existing contracts. The new serializer neither writes files nor changes
entry discovery. See
[the roadmap](../../../ROADMAP.md) and
[implementation prompt](../../../AUTHORING.md#recommended-immediate-implementation-prompt).

Opening checks: `cargo test -p meldritch-dsl --test project_directory`,
`cargo test -p meldritch-cli project_opening` and
`cargo test -p meldritch-tui empty_project`. An actual no-subcommand 80×24 PTY
draw/quit and alternate-screen restoration smoke also passes without files.
This evidence accepts only opening; it does not accept construction or music.

Palette checks: the same DSL suite plus `cargo test -p meldritch-app empty_project`
and `cargo test -p meldritch-tui native_creation`. They cover typed recipe equality,
invalid inputs/IDs, incomplete and unsupported wiring, deterministic in-memory
command replay, the construction bound, silence and full 80×24 inspection.
A no-subcommand PTY smoke at the palette checkpoint accepted two `n` presses and
clean quit/restoration without files. Native capture now deliberately writes
accepted gestures to sessions; opening/quit without gestures still creates no files.
Authored persistence, timed replay and physical-audio acceptance remain unfinished.

Native editor checks: `cargo test -p meldritch-tui native_` covers patch drafts
and independent pattern gestures. The DSL project-directory suite also rejects
unconnected sampler slots, missing note IDs and invalid values without inventing
ownership. Historical authoring/persistence/pickup suites remain required.

## Scenario and observable behavior

Construction checkpoint: `construction.toml` is a typed request fixture,
not a save format. From a file-free empty root, native `n` dispatches creation of
`voice-1` with the fixture's pitch/gate inputs, four modules and five cables.
Subsequent `n` creates a distinct definition; no track, pattern or composition
binding is created. The palette stays silent. The screen shows independent typed
inputs and every module/cable. No authored files or devices are opened; accepted
native gestures now create capture metadata lazily. Invalid/incomplete
requests, duplicate identities and unsupported executable wiring leave source,
history and publication unchanged. Existing patch edits act on the same source.
Definitions remain unsaved authored state; capture preserves typed facts but does
not automatically reopen them. The screen must distinguish capture from saving.
This partial checkpoint does not accept the complete save/reopen workflow.

Native patch-editing checkpoint: after `n`, Ctrl-E opens the accepted instrument
editor over the same project-owned source. Right changes its waveform; Ctrl-G
stages `keyboard-patch.txt` using existing patch operations. Disconnecting a
cable is draft-only. Incomplete commits and stale drafts leave accepted source
and history unchanged; completing the filter chain commits atomically. Esc exits
the draft panel, then parameters, without quitting. A `q` typed inside a draft is
text, while q at the project overview quits. Repeats/releases never edit. Native
save requests explicitly report unsupported new-project saving and retain edits.
Imported provenance stays empty; no baseline fixture, authored files or device are
added. Native accepted gesture captures remain separate from provenance/source.

Native pattern checkpoint: `keyboard-pattern.txt` creates a one-bar project-owned
pattern using `c`, independently of whether any instrument exists. It adds no
track, assignment or note. Its inspector shows separate pitch/gate source ports.
Arrow keys move a tick/pitch cursor; Enter inserts/selects notes and the existing
`NoteEdit` handles duration (`[`/`]`), velocity (`-`/`+`) and deletion. Stable note
IDs are not reused. Esc/Ctrl-N close/reopen without source edits. Invalid note
ranges/values and unknown IDs preserve accepted source/history. Native cursor
movement/repeats cannot accidentally duplicate notes. The existing version-1
track-owned pattern exports, Lua projections and tests retain their behavior;
new unconnected material must reject those legacy exports instead of inventing
an owner. All this material is still in-memory and silent, with bindings and
persistence acceptance unfinished. Independent composition binding acceptance
is described below.

Binding checkpoint: `independent-voice.toml` is a typed request fixture, not a save
format. `keyboard-bindings.txt` specifies native v/p/g. Instances are separate
from palette definitions; v creates neither binding, p/g author them individually.
Clock length/loop, initial C3 and output level 0.8 are explicit. Pitch holds until
another start, including across loop boundaries. Gate ends are exclusive; ends
and loop gate reset precede pitch updates, then gate rises. Gate carries no
implicit velocity or pitch. Unbound pitch holds initial pitch, unbound gate is low.
This bounded subset rejects overlapping gates, simultaneous pitch starts,
sample translations, clock mismatch and non-mono patches. App acceptance compiles
before replacing source; rejected edits preserve source/provenance/history. Note
edits invalidate instances that explicitly bind that source; adding an instance
invalidates all mixed lanes because existing mix normalization uses source count.

Run `cargo test -p meldritch-render --test independent_pattern_voice`,
`cargo test -p meldritch-app --test pattern_voice`,
`cargo test -p meldritch-tui native_composition`, and the DSL project-directory
suite. Tests prove pitch changes under a held gate, exact split/full rendering,
independent disconnect/reconnect, native inspection, typed in-memory replay and
worker publication consumed by the production callback without transport reset.
This evidence does not open a physical device or establish DSP-state migration.
The native cockpit reports unsupported save; device playback is specified below.

Native playback checkpoint: `keyboard-playback.txt` binds Space and Ctrl-Tab to
the actual CLI/TUI adapter over SongAudioHost. Opening starts construction through
existing AllParameters mode, creates no streams/files and remains silent. Space
requires an explicit instance and current prepared audio before opening default
48 kHz output; later Space toggles typed transport. Ctrl-Tab hides construction
internals and blocks its native gestures in performance; authored source,
publication and playhead are retained. This initial stream has fixed stereo
one-bar looping PCM. Unsupported clocks preserve publication/worker generation;
device errors preserve in-memory edits and report failure. Failed stream setup
consumes its engine, so retry in that cockpit is not yet supported.

`cargo test -p meldritch-cli native_host` and
`cargo test -p meldritch-cli unsupported_native_clock` check the real adapter
headlessly, including callback audio, mode/source/publication identity and gate
disconnect while the playhead advances. The TUI suite checks performance hides
graph internals and suppresses native editing. Device/listening and deadline
evidence remain separate; these checks do not accept the whole example.

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
checkpoint does not advance the complete example beyond `design`. General transformed/multi-composition routing, polyphonic signals,
LFOs, normalization, libraries, quantized graph changes and DSP-state migration
are deferred. PCM replacement proves rendered behavior, not continuous DSP state.
Physical sound/gestures and deadlines require separate evidence.
