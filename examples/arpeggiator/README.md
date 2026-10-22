# Arpeggiator Playground

Three independent `meldritch.arp` instances traverse evolving chord snapshots
over 15-, 20-, and 28-step loops. The four-section arrangement lasts 155.56
seconds at its authored 108 BPM.

```sh
cargo run -- perform examples/arpeggiator/arpeggiator.lua
cargo run -- render examples/arpeggiator/arpeggiator.lua arpeggiator.wav 156
```

The arpeggiator writes normal pitched events into `meldritch.musical`
patterns; the established Rust sequencer remains the only playback clock and
owns note-off timing. Saving captures those editable events as Lua. Reloading
the authored program reconstructs the generators before applying saved edits.
Pitch, velocity, and gate are therefore editable through the existing shared
semantic commands and persistence path. Traversal mode, octave count, and
clock division are authoring-time generator settings today: changing them
rebuilds the prepared pattern rather than masquerading as a real-time scalar
DSP control. The exposed performance controls manipulate each resulting
voice's tone, shape, release, master level, and transport tempo without graph
recompilation.

`meldritch.arp.live(snapshot)` accepts the same note-set snapshot shape as
authored and generative sources. Meldritch does not yet expose a retained
control-side MIDI chord snapshot, so that adapter cannot currently be wired to
live MIDI without adding such a source. It deliberately never calls Lua from
the audio callback.
