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

Current implementation status: placeholder mixed playback. This directory now
contains a validating song skeleton with nine declared tracks/lanes, two
scene-authored pages, 36 lane-owned note patterns/four variations per lane, nine
distinct renderer-compatible synth patches, lane-authored launch quantization,
default mute/solo state, per-lane control IDs, nested pattern banks,
page-visible controls, page-scoped MIDI fader bindings, and a `.mlsamples`
Raven voice sample-bank metadata file attached to the three sample lanes. The
referenced WAV is currently a deterministic generated placeholder until the
final usable Raven voice asset is committed. The sample lanes use an
`.mlsampler` voice definition with pitch tracking, level, polyphony, a pitch
envelope, and explicit slice events in their patterns.
`tui-song` now carries that lane metadata into app view state, and performance
mode renders an ensemble page overview centered on the active page, eight
visible controller strips, lane role, mute/solo state, active variation,
launch quantization, pattern-bank names/counts, compact visible control values,
and real held modifier state. The example declares an `octave-layer` momentary
modifier in `.mlperformance`; while that modifier is held, main-page fader 1 is
mapped to a typed `set_lane_octave` command for the `rhythm-drum-a` lane instead
of its normal cutoff control; that lane octave state is applied as semitone
transpose in the mixed-song renderer, so the held-layer fader affects audio.
Generic typed app commands can
select lane variations, select lane pattern banks, toggle lane mute, and toggle
lane solo in that performance-page state, and the results are classified for
session capture. Lane mute/solo state is applied to mixed-song rerenders:
muted lanes are excluded, and any soloed lane limits the mix to the solo set.
`.mlperformance` actions can bind LaunchControl MIDI buttons and CCs to those
lane commands without hard-coded controller policy. Lane variation and
pattern-bank selection now rerender the current song audio through the song
rerender worker. Single-track songs keep the legacy
delayed-note patch path with live override support; this multi-track ensemble
uses a compiled mixed-note patch, and lane variation selection changes one
track's selected placeholder pattern inside that mixed audio. Mixed-note
rendering can compile sample-bank tracks as deterministic one-shot sample
playback from authored `.mlsampler` voices; sample events can select slot/slice
while `note`, `root`, and `pitch_semitones` control playback pitch, with smooth
sampler pitch-envelope modulation applied per voice. Mixed-note rendering also
accepts script-targeted synth filter overrides, so `tui-song` controls can
change audio for synth-backed mixed patches. The main page now declares all 24
LaunchControl XL knobs plus eight faders across the visible strips: top-row
knobs map resonance, middle-row knobs map normal tone cutoff, bottom-row knobs
map a darker cutoff range, and faders target script-declared `lane:<id>/level`
with normalized unity/overdrive behavior. Lane level overrides apply to both
synth and sample tracks, so fader minimum silences the lane regardless of
instrument type. The drums page currently maps faders only. Sample lanes still
need explicit pitch/slice live targets before Raven slicing is fully playable
from the control surface. The existing
LaunchControl XL playground proves script-authored LaunchControl input, typed
actions, live rerendered parameters, authored groove variations, default
performance mode, and continuous audio publication for a single-synth
playground. Synths with `polyphony > 1` now render overlapping notes as
independent voices, and the ensemble pad uses four voices. This ensemble
skeleton still needs loop-mode sample playback, live sample pitch/slice
controls, full pattern-bank runtime semantics, and replayable modifier gestures.
