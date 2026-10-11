# Edit duration with a held pad

Status: `accept` for headless behavior using example 26's source/controller.
`interrupted.pads` and `scripts/compose.pads` are executed by
`cargo test -p meldritch-control --test piano_roll`. Run this script directly:

```sh
cargo run -p meldritch-control --example piano_roll -- songs/examples/27-held-pad-duration/interrupted.pads
```

Hold an occupied onset pad; pressing another pad on the same pitch row chooses
an inclusive endpoint. With an eighth-note grid, holding C3 step 0 and pressing
step 3 sets a half-note duration (four steps). Commit on the endpoint press,
once; endpoint releases only clear the transient hold. This gesture changes one event,
not four retriggers and not the legacy step gate. Plain short press selects;
a tap with no second pad deletes the start on release. Both release orders, note-on-zero releases and duplicate
note-ons are verified. Logical TUI pad events invoke the same Lua policy;
native keyboard inspection reads the same source and exclusive end.

Consume earlier/cross-row endpoints without edits or accidental insertion;
typed out-of-pattern ends reject. No implicit wrap/tie. Time page navigation
retains a valid hold for cross-page extension. Pitch-range/pattern/selection
changes cancel it until its original start is released. Disconnect/overflow
clear gesture/modifier state and discard queued pad observations; stale releases
cannot edit a different note. Deleted selection and rewind/loop ordering are
tested. Overlap events keep independent IDs and use existing voice semantics.

Persist onset, pitch and explicit musical duration only. Reload gates for the
chosen duration with ADSR release afterward; transport/held buttons are absent.
Headless tests independently reload equal source/fingerprint/audio, check split
renders and published samples. Physical acceptance of hold/second press/release,
LED orientation/colors, sustained sound and listening remains pending.
