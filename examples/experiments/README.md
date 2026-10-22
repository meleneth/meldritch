# Numbered experiments

Run either experiment through the normal performance path:

```sh
cargo run -p meldritch2 -- perform examples/experiments/01_stream_deck.lua
cargo run -p meldritch2 -- perform examples/experiments/02_delay_feedback.lua
```

The instrument starts when no Stream Deck is connected. The HID worker keeps
trying discovery in the background and restores the current page after an XL
is attached or reattached. XL revision 1, XL revision 2, and the 32-key XL
module are accepted through library-provided device discovery.

The simulated adapter used by `tests/stream_deck.rs` exercises the same page,
edge, semantic-action, state feedback, and image-invalidation logic without
physical hardware.

The delay experiment intentionally has no fake clear operation: the current
stereo delay contract does not expose state reset. `IMPULSE` generates a short
enveloped pulse; `DECAY` and `BUILD` make decaying versus accumulating feedback
easy to compare. `PINGPONG` and `MOD` are stateful toggles. The `TIMES` page
selects independent left/right times, feedback filtering, and feedback drive.
