# Create a note pattern on an APC Mini MK2

Status: `design`; authoring and feature bindings are unimplemented. Baseline:
`../01-synth-note-pattern`, using its existing synth and a newly created blank
pattern. Apply [shared rules](../../../AUTHORING.md#acceptance-execution-contract).

The authored interaction maps the Session grid's eight columns to eighth-note
steps in one bar and rows from bottom upward to C3..G3 chromatically. This is
example policy, not hard-coded Rust controller behavior. Bottom-left note 0
creates C3 at step 0; note 9 creates C#3 at step 1. TUI equivalent inputs produce
the same notes. Initial duration is 1/8 and velocity 0.8. A press toggles a note;
its release never toggles it again. Display pitch, onset, duration and assignment
in the TUI and derive APC feedback from authoritative notes.

Persist new pattern identity, bar length, looping, notes with musical positions/
durations/velocity and track assignment in Lua. At 120 BPM/48 kHz the first
onset is frame 0, second 12000, each duration 12000. Reload has equal source/audio.
These are note events, not lane variation launch buttons. Physical tests must
confirm Session orientation, press/release, displayed notes and listening.
