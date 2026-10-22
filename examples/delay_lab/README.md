# Delay Laboratory

Two polymetric arpeggiators feed the reusable stereo delay for a four-minute
four-section arrangement. The left and right taps are synchronized to the
sequencer BPM at 3/4 and 5/4 beats, with slow additive time modulation,
cross-channel feedback, low-pass filtering, and bounded saturation.

```sh
cargo run -- perform examples/delay_lab/delay_lab.lua
cargo run -- render examples/delay_lab/delay_lab.lua delay-lab.wav 300
```

Tempo changes immediately retarget beat-derived delay times. Read heads glide
to their targets over 20 ms, including time modulation and tempo changes. Tap
times clamp to one sample through four seconds; modulation is additive seconds
after beat conversion. Feedback is clamped to ±0.95, filtered, softly
saturated, and owned entirely inside the primitive, so no graph cycle is
introduced. Parameter edits preserve the allocated lines and their contents.
