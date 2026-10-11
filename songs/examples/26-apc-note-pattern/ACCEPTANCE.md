# Create a note pattern on an APC Mini MK2

Status: `accept` for the scoped automated headless contract. Physical gestures,
orientation, LEDs and listening remain untested. Run
`cargo test -p meldritch-control --test piano_roll` or
`cargo run -p meldritch-control --example piano_roll`. The readable feature file
indexes scenarios bound by these Rust tests; there is no separate Gherkin runner.

The Lua policy maps eight eighth-note columns and chromatic C3..G3 rows. It
starts with a blank two-bar seed; NEW creates/assigns `roll`. Two bars allow
cross-page endpoints. Pad 0 creates C3 at step 0; pad 9 creates C#3 at step 1,
both at velocity 0.8 with one-step duration. Existing starts select on press;
an unmodified tap deletes on release, while a second grid pad consumes deletion.
Bodies select without starting a duration gesture. Explicit held controls delete
and change velocity. Endpoint releases create no commands. These policies live in Lua;
Rust validates typed note/selection/create commands against the single song
source. Actual MIDI packet decoding, wrong-channel/malformed rejection, logical
terminal equivalence and native 80×24 inspection are verified headlessly.

`scripts/compose.pads` asserts creation, duration, velocity, time navigation,
cross-page extension, deletion and save. Every edit changes finite rendered audio;
rapid edits publish only the latest generation. Tests prove the unchanged prefix
before an affected onset, dirty ranges through voice/filter/tail/loop dependencies,
exact publication samples, ongoing transport, callback consumption and split
render identity. The APC and logical terminal project equal note labels/colors;
native TUI shows the same selected pattern, note IDs, pitches, starts and ends.

Persistence saves the entire selected pattern, length/loop, readable note names,
960-PPQ onset/duration/velocity, stable IDs/counter and track assignment over the
unchanged base fingerprint. Independent `edited-pattern.lua` reload compiles and
renders identical audio without hardware/journal. Unique atomic saves preserve
TOML, scripts, previous exports and journals; failures retain source. Invalid
versions, values, ranges, IDs/counters, paths and bases reject. Unsupported
sample/ownership/time translation and unrelated source edits refuse lossy export.
Full composed-song exports and native keyboard grid creation remain outside this
slice. See [PIANO_ROLL.md](../../../PIANO_ROLL.md) for controls, duration policy,
commands, persistence syntax, independent validation/render and physical limits.

The synth declares eight voices. A three-note same-onset chord must render as
the sum of its independently rendered voices; toggling one start leaves the
other notes intact. Tests cover velocity-zero release deletion and held gestures.
