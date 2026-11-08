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

## Demonstrated host gaps

- MicroLab can audition the independent monophonic live voice, but Lua cannot observe its
  retained held-note set. Live held-chord arpeggiation and record-style MIDI
  note entry are therefore unsupported. No fixed chord is substituted.
- The host plays 16-, 20-, 24-, and 32-step melodic patterns, but the browser
  piano-roll projection is fixed at 16 steps. The tail remains audible and
  editable through Lua/APC actions but is not a complete browser viewport.
- Direct browser piano-roll and drum-grid edits mutate authoritative host
  patterns without a Lua-readable note snapshot. Reproducing edits must use
  the Lua-owned entry/transform controls. The quine does not use the older
  project-save format and preserves its logical note IDs in its source state.
- Visual arrangement authoring is absent. Pattern selection and transport use
  the existing Lua-authored selector and hardware mappings.

Run the focused three-generation regression with:

```sh
tools/check-self-reproducing-playground.sh
```
