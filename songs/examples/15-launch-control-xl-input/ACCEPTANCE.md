# LaunchControl XL Input Acceptance

This example defines a small performance surface intended for a Novation
LaunchControl XL used as an external MIDI control surface.

It must:

- load as a normal delayed-note song with one curated `Echo Feedback` control
- use the loaded performance control as the target for LaunchControl XL fader 1
- map LaunchControl XL fader values as absolute normalized values, not repeated
  step nudges
- clamp and snap fader values to the control's authored range and step
- map LaunchControl XL button presses to typed increment/decrement steps
- ignore button releases so they do not create duplicate performer actions
- route every resulting hardware interaction through `AppInput` /
  `AppCommand`, so session capture can record it like keyboard input
- eventually open the physical LaunchControl XL through the host MIDI stack on
  both Windows and Linux

Current implementation status: the app-level LaunchControl XL fader/button
profile and absolute curated-control command are tested headlessly. Physical
MIDI device discovery/opening is still pending.
