# Persist and reload musical edits as Lua

Status: `compile` for separate instrument and pattern foundations; the full composed contract
below remains unimplemented. Example 25's `edited-instrument.lua` and
`cargo test -p meldritch-control --test instrument_authoring` prove seven-parameter
exports, independent reload/render, deterministic text, base validation, atomic
unique saves and failure recovery. Examples 26/27 now independently accept
selected note-pattern exports, stable IDs/counters, musical ticks, assignment,
gesture cleanup and reload/render via `cargo test -p meldritch-control --test piano_roll`.
Instrument and pattern exports remain separate definitions; unrelated source
edits reject selected-pattern export rather than disappear silently. Composed
exports and graph/mapping/mixer authoring remain unimplemented.
Use [shared rules](../../../AUTHORING.md#acceptance-execution-contract).

The first subset uses example 25's bass with authored cutoff/release edits. Export
human-readable Lua referencing the confined immutable base plus deterministic
absolute source edits. The implemented minimum versioned syntax is documented
in [INSTRUMENT_EDITOR.md](../../../INSTRUMENT_EDITOR.md). The base identity/fingerprint
is required so a changed external source cannot silently change the saved music.

Then compose examples 25–28 and 30. Save reconstructs synth defaults, note
events/durations/assignment, mappings, lane settings and explicit patch graph.
Fresh reload, with hardware disconnected and no performance journal, must
validate/compile into equivalent source and deterministic rendered audio.
Saved text exposes readable IDs, note names, musical times, parameters and
cables; it contains no VM bytecode, rendered samples or toggles depending on
prior runtime history. Stable source exports identically. Do not rewrite an
arbitrary hand-authored script in place; write an explicit authored export.

The host owns atomic filesystem save and reports failure while retaining dirty
state and the previous good file. Confined paths, missing/changed bases, duplicate
identities, unknown versions and unsupported edits/content must diagnose.
No session replay, HSM snapshot, full TOML migration or default entrypoint
precedence change is implied. Partial exports must reject rather than drop music.
