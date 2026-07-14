# LaunchControl XL Ensemble Acceptance

This demo script is the next example-first target after the single-synth
LaunchControl XL playground. It describes a fuller groovebox/performance
surface authored entirely in `.ml*` files.

It must:

- define a nine-lane musical ensemble with four or more variations per lane:
  - Lane 1: beat drum / bass drum
  - Lanes 2-3: rhythm/percussion drums
  - Lane 4: polyphonic chord/pad synth
  - Lanes 5-6: monophonic bass synths
  - Lanes 7-9: sample-based tracks
- expose eight physical LaunchControl XL strips at a time and support
  script-declared banking/page switching; the active scene/performance decides
  which lanes appear on which page, and Rust only applies that declared mapping
- keep every oscillator, sampler, filter, DSP, pattern, variation, lane,
  controller binding, modifier, and bank/page definition in text-authored
  `.ml*` files; Rust must implement generic behavior only
- treat the LaunchControl XL as a modular controller surface, not as hard-coded
  groovebox policy:
  - knobs and faders bind to script-declared lane/global targets
  - launch buttons bind to script-declared scene, variation, mute, solo, and
    bank/page actions
  - side-column buttons bind to script-declared transport, modifier, and
    navigation actions
- support at least four variations per lane without requiring a dedicated
  physical button for every variation/lane combination
- support momentary modifier/layer controls, including the first required
  example: while a declared button is held, a fader sends octave/transpose
  commands instead of its normal continuous parameter target
- record modifier/layer gestures as typed performance inputs so the session can
  replay without the LaunchControl XL attached
- render multiple tracks continuously while live controls mutate parameters,
  patterns, variation selection, and bank/page state
- show a performance-mode TUI overview centered on musical state, not raw
  parameter telemetry:
  - active visible bank/page
  - eight visible controller strips
  - lane role and active variation per visible strip
  - mute/solo state
  - modifier/layer state
  - compact current values for the relevant knobs/faders
- keep all-parameters mode available behind `Ctrl-Tab` but do not make it the
  default interaction path
- defer LaunchControl XL LED output until after the ensemble control semantics
  are stable, then define LED state through script-authored output declarations

Initial example scene page model:

- The example scene may put rhythm/percussion, pad, bass, and sample lanes
  across the first eight visible strips.
- The example scene may put the beat drum/bass drum on another page plus enough
  companion lanes to edit it musically in context.
- Switching pages must not stop playback or lose current variation/mute/control
  state.
- The runtime must not special-case the beat drum, the page names, or which
  lanes appear together; those are scene-authored declarations.

Current implementation status: design only for this example. The existing
LaunchControl XL playground proves script-authored LaunchControl input, typed
actions, live rerendered parameters, authored groove variations, default
performance mode, and continuous audio publication for a single-synth
playground. `.mlperformance` can now declare generic lanes and visible pages,
but this example still requires new schema/runtime support for multi-lane
songs, sample tracks, polyphonic pad rendering, pattern banks, momentary
modifier layers, and replayable modifier gestures.
