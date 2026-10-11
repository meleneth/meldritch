# Persist and reload musical edits as Lua

Status: `design`; the existing Lua save envelope does not satisfy this example.
Use [shared rules](../../../AUTHORING.md#acceptance-execution-contract).

Start minimally with a copied example 02 and one authored cutoff edit. Export
human-readable Lua referencing the confined immutable base plus deterministic
absolute source edits. The implementation must define its minimum versioned
syntax alongside failing tests; this contract intentionally specifies no new
schema fields or unimplemented Lua command names. The base identity/fingerprint
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
