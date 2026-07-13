# MELDRITCH `.ml*` Formats

This document is an index, not an independent specification. The executable
song trees under `songs/examples/` define the behavior we want to support. The
format grows only when an accepted example requires it.

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
control signals are distinct. Normalled connections belong to module
definitions or explicit recipes; they must never become a second hidden graph.

Convenience recipes are acceptable only when their complete module-and-cable
expansion can be inspected and fingerprinted like a handwritten patch.

## References

References are paths relative to the file containing them. Loaders normalize
the path and reject escape from the song root. Stable IDs name definitions
inside resolved files; filenames are organization, not identity.

The authored entry point is `main.mlperformance`. Captured sessions are written
under `performances/` and are never considered entry-point candidates.

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
