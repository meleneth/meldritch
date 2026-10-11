# APC Mini MK2 physical acceptance — pending

Record operator/date, OS, audio device, APC port/firmware and stereo monitoring
path with observed results. Host port discovery, input/LED connection and clean
stopped TUI exit smoke passed on 2026-10-11. No pad/fader or listening result has
been observed. MIDI injection does not prove physical usability.

- [ ] Start the guide command; verify four-second startup mute and audible seed.
  Confirm grid orientation, pitch range, time page and moving playhead.
- [ ] Ctrl-E change waveform/cutoff/release and hear each during playback.
- [ ] APC 116 create pattern; tap several rows. Starts, bodies, selection and
  playhead must remain distinguishable under stage lighting.
- [ ] Hold a start and press later same-row endpoint; release in both orders.
  Verify duration/no extra note; tap an occupied start again to delete.
- [ ] Hold across page 107 and press an endpoint; inspect exclusive end in Ctrl-N.
  Verify adjacent notes and overlapping-note projection.
- [ ] Try delete/velocity modifiers, invalid endpoints, another row, pitch change
  and disconnect during a hold. No stuck/unintended notes.
- [ ] Ctrl-M assign cutoff/volume/pan; acquire physical faders without jumps.
  Reassign/remove, reacquire and verify TUI observation/value/pickup.
- [ ] Hear filter changes, volume silence/unity, stereo left/center/right.
  Playback must survive navigation and supported edits.
- [ ] F6 or APC 115; record saved path. q, restart with --authored PATH.
  Verify graph, notes/durations, mixer and assignments. Pickup must rearm and
  transport/held gestures must not restore.
- [ ] Record navigation burden, accidental deletion/modifier confusion and
  visibility of pitch/time page while composing.

Stream Deck simulated play/stop/rewind/save and LCD state pass headlessly.
Real-device transport is unimplemented and has no acceptance claim.
