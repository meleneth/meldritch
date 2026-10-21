# Meldritch Groovebox

This is an eight-track, Lua-authored performance instrument. It starts at 128
BPM with the **Foundation** pattern playing. The six drum tracks use the cached
Strudel `RolandTR909` pack; the percussion track deliberately uses the pack's
low tom (`lt`). Bass and Lead are ordinary Plaits-based graph compositions.

Launch the audio engine and TUI from the repository root:

```sh
cargo run -- perform examples/groovebox/groovebox.lua
```

The first run may populate the sample cache. Later launches use that cache and
do not download the kit again. To render the deterministic 20-bar arrangement
(Foundation, Broken, Acid, Atmosphere, then Foundation) as stereo 24-bit WAV:

```sh
cargo run -- render examples/groovebox/render.lua groovebox-demo.wav 37.5
```

## Keyboard

`F1`-`F4` queue Foundation, Broken, Acid, and Atmosphere for the next pattern
boundary. `Space` toggles transport, `s` starts, `x` stops, and `r` restarts. Up/Down
select a track, `[`/`]` select steps 1-8 or 9-16, and `1`-`8` toggle steps in
that bank. `m` and `o` toggle mute and solo. `+`/`-` change the selected event's
velocity, `p`/`P` transpose it by a semitone, and `g`/`G` change its gate.
`c` copies the active pattern over the following pattern; `C` clears the
selected track. `S` deliberately saves the editable project to
`groovebox-project.json`; shipped Lua is never overwritten. `j`/`k` move
through the declared track, voice, tempo, and master controls; `h`/`l` adjust
the selected control. `q` exits.

The screen shows active and queued patterns, transport, playhead, mute/solo
flags, track selection, status messages, the step grid, and all declared voice,
track-volume, and master controls.

## APC mini mk2

The mapping is authored in `groovebox_core.lua`; Rust only supplies generic APC
input, RGB output, and semantic command/control dispatch.

- Page A (MIDI note 100) is the step page. Grid rows are tracks and columns are
  the selected eight-step bank. Left/Right select the bank. LEDs distinguish
  empty, active, selected, playhead, and active-under-playhead cells.
- Page B (MIDI note 101) is the performance page. From the top, rows select
  patterns, toggle track mutes, toggle track solos, select tracks, and restart.
  Active and queued patterns have distinct LED colors.
- Play/Stop (112), Restart (113), and Clear (114) invoke the same semantic
  operations as the TUI. Clear acts on the selected track.
- The eight channel faders (CC 48-55) control track volumes. The master fader
  (CC 56) controls master output level.

Switching pages does not alter transport or pattern contents. Solo masks other
tracks at the sequencer output and never destroys their authored mute state.

## Authored data and persistence

`patterns.lua` contains four human-readable patterns. Drum events carry
velocity; Bass and Lead events independently carry MIDI pitch, fractional gate
duration, and velocity. Runtime edits affect the current performance state.
Saving writes a versioned JSON project beside the launch working directory.
After saving, restart that exact composition with:

```sh
cargo run -- perform examples/groovebox/project.lua
```

`project.lua` calls `sequence:load_project(path)` before starting transport.
Filesystem work and Lua evaluation stay outside the audio callback.
