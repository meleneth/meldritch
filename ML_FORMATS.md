# MELDRITCH `.ml*` Formats

This document is an index, not an independent specification. The executable
song trees under `songs/examples/` define the behavior we want to support. The
format grows only when an accepted example requires it.

Project construction is the immediate priority (TODO section 21). FEATURES.md
and ARCHITECTURE.md specify the direction without adding a format in this
planning update. Existing entrypoints and explicit Lua load flags keep their meaning.

Example 28 implements [Lua fader assignments and mixer authoring](CONTROL_ASSIGNMENTS.md).
Track `volume` defaults to unity and optional `pan` explicitly opts into stereo
balance. Their source values and supported synth/DSP assignments persist in a
separate version-1 `kind="controls"` Lua snapshot over the unchanged TOML base.
[AUTHORING.md](AUTHORING.md) specifies the source ownership and minimum Lua
persistence direction. Example 25 accepts the initial instrument subset;
example 29 accepts a bounded composed round-trip headlessly. The separate version-1
authored instrument Lua representation is defined in
[INSTRUMENT_EDITOR.md](INSTRUMENT_EDITOR.md), with a complete checked-in example
and independent validation/render commands. Existing TOML entry points and
Lua composition v1 remain compatible; exports do not rewrite them.

Examples 26/27 now accept the first note-pattern authoring subset. Its separate
version-1 Lua pattern representation, stable IDs, musical tick/duration semantics,
ownership limits and independent reload/render commands are documented in
[PIANO_ROLL.md](PIANO_ROLL.md). The `.mlpattern` TOML importer remains compatible;
implicit historical note identities derive from source event order. New identity
allocation state is included in fingerprints without changing historical imports.

Example 31 adds explicit one-shot sampler-slot notes to that same version-1 Lua
definition using optional `slot` metadata and a typed `insert_sample` command.
Synth exports are unchanged; unsupported slice/root/transpose edits reject.
See [DRUM_EDITOR.md](DRUM_EDITOR.md) for its generated kit and persistence limits.

Example 32 adds the [Plaits typed module](PLAITS.md): `type="plaits"`, named
`model` and `harmonics`, `timbre`, `morph` in 0..1.
It uses the existing instrument Lua export and explicit pitch/main/aux ports.
Example 33 expands selection to all 24 bundled upstream model choices and adds
the optional typed gate input for trigger/excitation semantics.
Omitted performance/project sample rates now default to 48 kHz; explicit rates
remain authored, while the Plaits module requires 48 kHz.

Example 30 adds [typed patch composition](PATCH_EDITOR.md) with the existing
SynthDefinition and ports; changed topology uses version-2 `kind="instrument"`
Lua graph snapshots. Parameter-only exports retain version 1. No alternate graph
language/compiler or automatic source-file migration is introduced.

## Example-first rule

For every capability:

1. Write the smallest human-readable song that demonstrates it.
2. State what a user should hear, see, control, capture, or replay.
3. Add invalid neighbors that demonstrate required diagnostics.
4. Implement only enough schema and runtime behavior to satisfy those examples.
5. Keep the examples as automated acceptance fixtures.

Schema fields without an example are unsupported proposals, not features.

## File roles

- `.mlsynth` describes an instrument as modules and typed cables.
- `.mldsp` describes signal processing as modules and typed cables.
- `.mlpattern` describes note events or typed parameter changes over musical
  time.
- `.mlperformance` assembles a song and defines its curated playable surface.
  Generated session journals use the same extension with `kind = "performance_session"`.

All formats are TOML and begin with a versioned `[meldritch]` table.

## Modular doctrine

Synth and DSP files are Eurorack-inspired patch graphs. A module exposes typed
ports. A cable connects `module.port` to `module.port`. Audio, gate, pitch, and
control signals are distinct. Future normalled connections belong to inspectable
authored patch policy or explicit recipe expansion, separating policy, resolved
routes and current values. They never imply hidden blending. Current formats
do not implement authored normalization.

Convenience recipes are acceptable only when their complete module-and-cable
expansion can be inspected and fingerprinted like a handwritten patch.

## References

References are paths relative to the file containing them. Loaders normalize
the path and reject escape from the song root. Stable IDs name definitions
inside resolved files; filenames are organization, not identity.

The current song loader's entry point is `main.mlperformance`. Future projects
may be empty or contain multiple compositions; their layout requires an example.
Captured sessions are written
under `performances/` and are never considered entry-point candidates.

Example 34's native capture keeps the `performance_session` version-1 envelope
and explicitly declares `session.native_command_version = 1`. Its events add
an `events.native` table with a typed command and source fingerprints before/after.
See that example's `capture.txt` and `capture-command.toml`; this is captured
accepted state, not an authored project save or a claim of timed DSP activation.
Legacy hosts retain their historical event representation. Native checkpoint
storage is bounded and failure/retry behavior is tested. Native writers also embed
`session.native_source`, a quoted self-contained project-v2/v3 Lua opening baseline,
before events. `load_native_capture_baseline` bounds canonical header scanning and
parsing, restores through the existing importer/compiler and checks the opening
fingerprint. The original root is provenance; callers supply a destination root.
Later saves cannot change the captured baseline. Older captures without this field
require their original source explicitly; the loader never infers an empty baseline.
This additive field changes neither legacy writers nor the event vocabulary and
does not implement timed replay or execute events during project opening.
Example 36 adds ordinary synth module kinds `lfo`, `control_scale` and
`control_multiply` to the same graph snapshots and TOML importer. Required `lfo`
settings declare shape, tagged Hertz/Beats rate, phase, audio evaluation,
per-instance scope and none/gate-rise reset. Required `scale` settings declare
gain/offset and unit-interval clamp endpoints. Units belong to typed port metadata;
LFO/scale input is bipolar and scale/multiplier/ADSR output is unit interval.
Native v2/v3 saves retain these settings and reject invalid/unused/cyclic graphs.
Typed ConfigureLfo/ConfigureControlScale operations reuse EditPatch and capture;
historical graphs with absent settings retain their fingerprints. The first
executable control DAG ends at VCA level. The vibrato checkpoint adds
`pitch_modulate` with required `pitch_modulation = { depth_semitones = ... }`
settings (finite -24..24). Its independent Pitch input/output and bipolar Control
input explicitly add a semitone offset. ConfigurePitchModulation reuses EditPatch.
One adapter between input.pitch and oscillator.pitch is lowered at audio rate;
Plaits and chained pitch adapters reject. These settings fingerprint and survive
native save/reopen. Broader modulation remains proposed.

Example 35 extends native command version 1 with `select_palette_pattern`, a
typed selection fact with an explicit pattern ID. Selection changes neither
authored source nor bindings; project-v2 saves exclude cursor/editor selection.
Its project-v2 fixture reuses existing independent voice bindings and one palette
definition across two instances. Its named variant adds native command version-1
`create_composition` / `select_composition` authored actions; running or pending-Play
composition changes reject. Selection is an authored default, not a runtime override.

Example 35's `compositions.project.lua` adds self-contained project **version 3**:
`compositions` contains `{ id, voices = { instance_id, ... } }` declarations and
`selected_composition` is mandatory and must resolve. `voices` remains one
project-owned, globally unique instance registry; compositions reference it rather
than copying the palette. Membership IDs are canonicalized; repeats/missing members
and more than 32 compositions reject. All definitions, including inactive ones,
validate/compile. The selected list alone controls mix inclusion/normalization.
Runtime gain keys remain instance-owned and are applied only when active.
Native b snapshots all declared instances into a new named composition; the first
is explicitly selected by that construction action. F7/F8 selects while stopped
in construction. Selection saves as the authored default; reopening starts stopped
with authored gains. Native a/d lowers to SetCompositionMember while stopped,
validating and compiling the full candidate before acceptance. It preserves the
global instance registry; v creates another instance without implicitly adding it.
Up/Down selects by instance ID; inspector selection and runtime gains remain outside
the authored document. Removing membership never silently retargets that selection.
Version 2 rejects named composition fields and preserves its implicit full voice
list. Version 3 requires at least one composition plus explicit selection. Native
export chooses v2 for unnamed projects and v3 for named ones, checks full restored
source equality and preserves all inactive definitions. The file/store boundaries
and historical base-bound project-v1/instrument formats remain unchanged.

Example 34 additionally defines self-contained `kind="project", version=2` Lua
in `authored-project.lua`. It stores project identity/clock, complete supported
mono synth graphs (including exposed output-level declarations), independent
tick-based pattern material and mono instance pitch/gate bindings. Rust restores
the existing source through graph/note/voice validators and the executable compiler.
The encoder rejects omitted/lossy source state by full restored-source equality.
Runtime gains, transport, editor selections, capture and drafts are excluded.
The existing constrained VM/source limit applies, with explicit palette/instance/
note bounds in `persistence.txt`. Headless fresh-process reload is accepted for
deterministic oscillator PCM; Plaits retains its documented RNG exception.
Native Ctrl-S/F6 now saves this subset at `main.project.lua`. The control-owned
store loads it only for project opening, after DSL root discovery and before
devices. Existing `main.lua` scripts are unrelated. Both musical entries present
(`main.project.lua` and `main.mlperformance`) diagnose ambiguity. Native entries
must be ordinary bounded files; symlinks/unknown snapshots reject instead of
becoming empty projects. Save checks original/last-saved bytes, rejects external
changes and foreign `.main.project.lua.tmp`, then syncs/atomically renames its own
temporary file. Capture files are never entry candidates. Legacy base-bound
project-v1 and instrument-v1/v2 formats remain unchanged.

Lua API v1 adds an explicit composition adapter, without changing the authored
TOML entrypoint or migrating historical songs. `main.lua` chooses a confined
existing song and defines headless control behavior. Saved Lua envelopes retain
source text, stable project identity and song fingerprint; they revalidate the
same typed song on restore. See [CONTROL_PLANE.md](CONTROL_PLANE.md).

Example 24 adds an optional `[ui]` section declaration with
`[[ui.sections]] kind = "song_audio"`, plus the existing title/height fields.
It displays shared publication/callback telemetry without changing historical
page layouts. Historical UI kind fingerprint tags remain stable. Lua's `audio`
projection is runtime telemetry and does not enter authored song fingerprints.

## Time

Authored patterns use musical time. Initial examples use:

- durations such as `1 bar`, `1/8`, and `1/16`
- positions such as `1:1:0`, meaning bar, beat, and PPQ tick within the pattern;
  the current parser uses 960 ticks per beat, so one sixteenth note is 240 ticks
- explicit looping and launch quantization

Typed Lua journals record `u64` frames. The legacy performance-session envelope
and native control-frame observations currently use `u32` positions plus wall-clock
offsets. Long-running `u64`/epoch sessions and sample-timed native replay remain
requirements; control observations must not be reported as DSP activation times.

## Current support target

The current examples are design fixtures. They intentionally precede parser and
runtime support. See `songs/examples/CAPABILITIES.md` for implementation state.
