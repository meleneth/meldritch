# LaunchControl XL Playground Acceptance

This demo script maps the LaunchControl XL default MIDI surface into a small
script-authored performance playground.

It must:

- define the oscillator, filter, delay, note patterns, curated controls, MIDI
  device match, and every MIDI CC binding in `.ml*` files
- avoid Rust-defined control order, CC maps, or “fader 1 means X” policy
- expose authored controls/actions for all default LaunchControl XL performance
  inputs:
  - 24 rotary knobs: ch 9 CC 13-20, 29-36, and 49-56
  - 8 faders: ch 9 CC 77-84
  - 16 launch buttons: ch 9 notes 41-48 and 57-64
  - right-side specialized buttons discovered bottom-up, right-to-left with the
    raw MIDI diagnostic path:
    - Record Arm, Solo, Mute, Device: ch 9 notes 108, 107, 106, 105
    - Track Select Next, Track Select Prev, Send Select Up, Send Select Down:
      ch 9 CCs 107, 106, 104, 105
    - Template User and Template Factory currently emit observed SysEx messages
      `F0 00 20 29 02 11 77 00 F7` and `F0 00 20 29 02 11 77 08 F7`;
      they are left diagnostic-only until an example needs raw/SysEx bindings
- map the 16 launch buttons and the 8 discovered side-column buttons to
  script-declared typed performance actions, not hard-coded Rust behavior
- provide four authored groove scenes and one authored variation/fill per scene;
  the B row selects scenes 1-4 and variation 1 for scenes 1-4
- make those scenes musically distinct enough to verify by ear:
  - Scene 1: simple ascending line
  - Scene 2: low stab pattern
  - Scene 3: syncopated bounce
  - Scene 4: climbing/tension pattern
  - fills: denser one-bar variations for each scene
- author pattern event positions with the loader's 960 PPQ ticks per beat
  (`240` ticks per sixteenth), so the visible grid and rendered groove agree
- normalize the physical surface so centered knobs are neutral and faders use
  the authored cutoff curve: MIDI 0 -> 100 Hz, MIDI 108 -> 4350 Hz “full open”,
  MIDI 127 -> 5000 Hz “overdrive full open”
- map the LaunchControl rows to distinct live musical targets instead of 32
  duplicate cutoff controls:
  - top knobs `K01-K08`: synth filter resonance, centered at `0.2`
  - middle knobs `K09-K16`: delay feedback, centered at `0.35`
  - bottom knobs `K17-K24`: delay mix, centered at `0.25`
  - faders `F01-F08`: synth filter cutoff with the full-open/overdrive curve
- route all MIDI input through typed `AppInput` / `AppCommand` results so a
  captured performance can replay without the controller attached
- rerender and publish selected groove scenes from authored `.mlpattern` files
  during `tui-song` playback
- rerender and publish audible delay-feedback, delay-mix, filter-cutoff, and
  filter-resonance changes from supported curated controls
- autoplay in `tui-song` by default so the authored pattern keeps sounding while
  LaunchControl inputs only change scene selection, transport, and parameters;
  `--no-autoplay` is the explicit stopped-start smoke-test mode
- keep the live `tui-song` audio device connected to a dedicated song-audio
  publication seeded from the initial rendered pattern, so the generic backing
  TUI coordinator cannot overwrite the song with silence before the first
  controller movement
- validate as a normal song directory
- support `meldritch midi-controls-check` as the hardware smoke path for
  listing visible MIDI ports and printing raw MIDI event details plus authored
  labels for mapped action buttons; unmapped controls still print raw note or
  other MIDI messages for discovery
- support `tui-song --midi-debug` as the live playground smoke path so mapped
  button/control labels and unmapped raw MIDI events are visible in the status
  line while testing the controller
- support `tui-song --audio-debug` as the live audio smoke path so the status
  line reports transport callbacks, playhead position, current sample peak, and
  upcoming song-publication peak while debugging device/output silence
- show the authored groovebox surface in default performance mode: B-row scene
  and fill mapping, queued/active state, the actual authored note pattern grid,
  and compact LaunchControl value telemetry

Current implementation status: the format can declare every MIDI CC binding and
script action bindings for MIDI CCs or MIDI notes. The runtime derives MIDI
routing from the script; supported feedback, mix, cutoff, and resonance
parameter controls rerender audio; and launch/side-column buttons can trigger
typed transport/performance actions. The LaunchControl B row now selects
authored groove scenes/variations that rerender through the song synth and
delay. Pattern positions use real 960 PPQ ticks, so the default TUI performance
mode can expose the authored note grid instead of a dummy or collapsed pattern.
`tui-song` autoplays by default and the realtime output loop is fed from a
dedicated song-audio publication that keeps playing across parameter rerender
publications. Quantized launch timing and exact replay remain future
schema/runtime work. The two observed SysEx messages are intentionally left as
diagnostic output until an example needs raw/SysEx output or binding support.
