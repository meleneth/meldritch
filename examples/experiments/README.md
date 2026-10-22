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

The delay experiment starts its 16-step bassline immediately. Stream Deck
START/STOP/RESTART and the keyboard (`s`, `x`, `r`, or Space) share the normal
sequencer command path. Its parameter buttons and F1-F7 select the outlined
TUI parameter widget; `h`/`l` or any configured MicroLab knob then edits that
selection. The default MicroLab profile listens to channel 1 CC 74, 71, 76,
and 77 and is intentionally authored beside the experiment. The APC mini mk2
channel faders address the source and delay controls; its far-right fader is
wired to a linked stereo master gain after the delay, as required for every
APC-authored performance surface.

There is no fake delay clear operation: the stereo-delay contract does not
currently expose state reset. IMPULSE adds a transient over the running bass,
while DECAY/BUILD and the PING/MOD toggles make stable and accumulating feedback
easy to compare.
