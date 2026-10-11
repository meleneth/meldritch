# APC Mini MK2 playground

Status: `play`. The existing host renders the four-lane song and routes authored
MIDI bindings through typed application inputs. Headless tests cover the mapping,
variation rendering, mute/solo and level changes. Physical discovery was checked
on Linux; real fader and pad input was observed and the MIDI/audio/TUI host
started and exited cleanly. Exhaustive interactive hardware/audio acceptance
remains pending.

## Run

From the repository root (Rust 1.97+, Linux ALSA development headers):

```sh
cargo run -- validate-song songs/examples/19-apc-mini-mk2-playground
cargo run -- midi-controls-check songs/examples/19-apc-mini-mk2-playground --seconds 30
cargo run -- tui-song songs/examples/19-apc-mini-mk2-playground --midi-debug
```

Use a terminal at least 34 lines tall to see all four lane monitors and controls.
Playback starts automatically. Start with your listening volume low. `p` toggles
playback, `r` rewinds, and `q` exits. Accepted interactions are captured under
`performances/` using the existing host session writer.

Use the APC's Session mode, not Note or Drum mode. Only the `APC mini mk2 Control`
port is matched. Authored MIDI channel 1 means wire channel 0. No controller is
required to validate or render this project.

## Map

Columns 1–4 are Beat Drum, Percussion, Bass and Pad. Grid note 0 is bottom left;
rows below are counted from the bottom. Other pads are intentionally unbound.

| Input | Behavior |
| --- | --- |
| Grid rows 1–4, columns 1–4 (notes 0–3, 8–11, 16–19, 24–27) | Select lane variation A/B/C/D |
| Grid row 5, columns 1–4 (32–35) | Toggle lane mute |
| Grid row 6, columns 1–4 (40–43) | Toggle lane solo |
| Bottom track buttons 1–4 (100–103) | Toggle lane mute |
| Right scene buttons 1–4, top to bottom (112–115) | Play/stop, stop, rewind, cockpit mode |
| Faders 1–4 (CC 48–51) | Lane levels: zero is silent, maximum is unity |
| Faders 5–8 (CC 52–55) | Corresponding lane low-pass cutoff, 80–6000 Hz |
| Fader 9 (CC 56) | Pad resonance, 0–0.8; this is not master volume |

Protocol reference: [Akai communication protocol v1.0](https://cdn.inmusicbrands.com/akai/attachments/APC%20mini%20mk2%20-%20Communication%20Protocol%20-%20v1.0.pdf).
The four synths and sixteen patterns are independent copies of supported ensemble
assets; historical fixtures are unchanged.

## Executable criteria

- Load and resolve all references with stable fingerprints; compile four tracks.
- Render finite, non-silent audio; split rendering equals contiguous rendering.
- All sixteen launch pads map to the correct typed lane/variation input.
- Note releases, wrong channels/devices and unmapped pads do not trigger actions.
- Nine faders map to the declared controls; zero and maximum normalize correctly.
- Mute/solo and transport buttons map to their typed inputs.
- A lane variation changes audio; muting all lanes or zeroing every lane level
  yields silence; solo selects one lane.
- Invalid lane/variation references fail validation with diagnostics.

## Remaining hardware acceptance

Observed on Linux: the short diagnostic received 105 events, including faders,
mute/solo pads and variation D. A production TUI/audio run captured 226 accepted
actions (223 curated controls and three lane mutes) and saved a clean session.
The displayed audio counters showed zero underruns/misses during the inspected
run. This does not establish deadline compliance or audible quality on other
devices. The full workspace suite passed; no historical sources were modified.

- Move every fader and press every mapped pad/button in `midi-controls-check`;
  confirm the table on the connected unit.
- Hear level/cutoff/resonance changes and variations in `tui-song`; inspect TUI
  lane state and the resulting captured session.
- Test clean exit and unplug behavior during playback.

Launches currently apply after background rerender publication, not on a quantized
beat. Faders are absolute and can jump on first movement. RGB LED feedback,
soft takeover, automatic reconnect and exact musical session replay are not
implemented in this playground. This project and its interface are unstable.
