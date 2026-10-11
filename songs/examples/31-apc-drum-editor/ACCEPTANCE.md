# APC drum editor

Status: `accept` for the automated headless contract below. Run
`cargo test -p meldritch-control --test drum_roll` or
`cargo run -p meldritch-control --example drum_roll`. Physical acceptance remains
pending and is not included in this status.

Initial contract: eight Lua-authored rows select kick, snare, clap, closed hat,
open hat, low tom, high tom and ride; eight columns are sixteenth notes. Tapping
an empty cell inserts an explicit sample-slot hit; tapping an occupied start
removes only that hit. Simultaneous rows layer. Releases and duplicate note-ons
never insert or delete another hit. Velocity modifiers and time paging follow
the authored controls, while pitch paging and held duration are absent.

The kit is an original generated 909-inspired approximation, not a TR-909
emulator or licensed Roland recordings. There is no 909 synthesis engine in
the repository. Sample formulas/assets remain inspectable and replaceable.
The ordinary sampler/compiler/publication owns playback; Lua owns the pad map.

Typed edits validate sampler ownership, explicit one-shot slot IDs, time/pitch/
velocity and reject unsupported slices/root/transpose edits. Lua export retains
slot IDs, stable note IDs/counter, note/pitch, ticks, velocity and track assignment.
Load/edit/save/reload must preserve source fingerprint and rendered samples.
The TUI must expose slot identity; APC LEDs must distinguish hits/selection/playhead.
Save must preserve source files and performance journals. Invalid slots and
sample edits on synth tracks must reject without mutation or publication.

Run the deterministic script through the production Lua/MIDI/typed-command/
worker/capture path and compare independently reloaded audio. Also prove each
row renders a distinct, finite, non-silent hit, simultaneous rows sum and edits
leave the transport playing. Physical pad mapping, LEDs and sound remain pending.
