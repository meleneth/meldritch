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
- open the physical LaunchControl XL through the host MIDI stack on Windows and
  Linux using the same typed input path as keyboard actions

Current implementation status: the app-level LaunchControl XL fader/button
profile, MIDI CC decoding, and `tui-song` MIDI input wiring are tested
headlessly. Actual hardware smoke testing on Windows and Linux is still
pending.
