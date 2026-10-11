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
  Generated session journals use the same extension with `kind = "session"`.

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

Generated sessions additionally record absolute `u64` frames and wall-clock
offsets so exact replay does not depend on reparsing musical notation.

## Current support target

The current examples are design fixtures. They intentionally precede parser and
runtime support. See `songs/examples/CAPABILITIES.md` for implementation state.
