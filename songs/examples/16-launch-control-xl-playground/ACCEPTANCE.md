# LaunchControl XL Playground Acceptance

This demo script maps the LaunchControl XL default MIDI surface into a small
script-authored performance playground.

It must:

- define the oscillator, filter, delay, note pattern, curated controls, MIDI
  device match, and every MIDI CC binding in `.ml*` files
- avoid Rust-defined control order, CC maps, or “fader 1 means X” policy
- expose authored controls for all default LaunchControl XL performance inputs:
  - 24 rotary knobs: CC 13-20, 29-36, and 49-56
  - 8 faders: CC 77-84
  - 16 launch buttons: CC 41-48 and 57-64
- route all MIDI input through typed `AppInput` / `AppCommand` results so a
  captured performance can replay without the controller attached
- rerender and publish audible delay-feedback and filter-cutoff changes from
  supported curated controls
- validate as a normal song directory
- support `meldritch midi-controls-check` as the hardware smoke path for
  listing visible MIDI ports and printing script-mapped CC events

Current implementation status: the format can declare every MIDI CC binding, the
runtime derives MIDI routing from the script, and supported feedback/cutoff
parameter controls rerender audio. The playground is intentionally limited to
currently supported curated parameter controls; richer pattern and launch action
controls are still future schema/runtime work.
