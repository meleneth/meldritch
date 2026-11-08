# Self-Reproducing Lua Performance Playground

Run the hardware-free native fallback:

```sh
MELDRITCH_PLAYGROUND_NATIVE_ONLY=1 cargo run -p meldritch2 -- perform \
  crates/meldritch2/examples/81-self-reproducing-playground.lua
```

The browser is served at `http://127.0.0.1:8081`. `SAVE SUCCESSOR LUA` writes
`81-self-reproducing-playground.gN.lua` beside the currently executing program,
or beneath `MELDRITCH_PLAYGROUND_OUTPUT_DIR`. The successor is the complete,
readable editor program with the current source-owned state embedded between
`BEGIN SELF STATE` and `END SELF STATE`.
If a successor is moved before execution, set `MELDRITCH_PLAYGROUND_SOURCE` to
its new path for that launch; the following successor records the new location.

Set `MELDRITCH_SERUM_BANK` to a small user-local subset of the extracted Serum
factory library. The bank must contain at least two captured patches. The
playground never stores those proprietary states; it embeds only its own Lua
configuration. Set `PULSE_STATE`, `PULSE_VST3_PATH`, `MICROLAB_MIDI`, and
`APC_MINI_MIDI` as needed for local hardware. Without Serum or PULSE it remains
fully executable with native Plaits voices.

Lua owns scale-locked entry, transpose, rotation, inversion, quantization,
two-pattern interpolation, pattern-driven arpeggiation, editor variants, and
successor generation. APC and Stream Deck use existing control surfaces; the
browser uses the existing Vue/Pinia/Phaser projection.

## Persistent-state ownership

Browser/Phaser note commands and APC mini grid edits converge on the
sequencer's retained logical-note representation. At Save, Lua reads that
authoritative representation through `sequence:logical_notes(...)`; it does
not infer the edit source or rely on a controller-specific shadow store. Pitch,
step, duration, velocity, logical ID, pattern length, and the supported legato
flag are emitted as readable Lua and reconstructed before playback starts.

The browser piano roll follows the active pattern length and supports the
playground's 16-, 20-, 24-, and 32-step patterns, including note edits after
step 16. APC and browser edits to the same pattern therefore reproduce in the
same successor.

## Remaining host gaps

- MicroLab can audition the independent monophonic live voice, but Lua cannot observe its
  retained held-note set. Live held-chord arpeggiation and record-style MIDI
  note entry are therefore unsupported. No fixed chord is substituted.
- Visual arrangement authoring is absent. Pattern selection and transport use
  the existing Lua-authored selector and hardware mappings.

Transport position, held MIDI notes, pending boundary transitions, and the
instantaneous arpeggiator cursor are performance state rather than persistent
composition state and are intentionally not serialized. The quine does not use
the older project-save format; it owns its readable source and mutable state.

Run the focused three-generation regression with:

```sh
tools/check-self-reproducing-playground.sh
```
