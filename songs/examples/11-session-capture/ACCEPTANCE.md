# 11 - Timestamped Session Capture

This records an interactive `tui-song` performance as a human-readable
`.mlperformance` session file.

It should:

- load as a normal delayed-note song with one curated `Echo Feedback` control
- create a collision-safe file under `performances/` when `tui-song` starts
- use `performance_session` as the session document kind
- record the source performance id, title, song fingerprint, and timeline length
- checkpoint the file outside the realtime callback
- keep an explicit bounded buffer of uncheckpointed accepted actions before
  forcing a checkpoint
- record accepted typed inputs with sequence, wall offset, absolute frame,
  musical beat, quantization, execution frame, command/result text, and
  performer provenance
- record structured previous/current values for curated controls and mode changes
- write a final runtime state including cockpit mode, transport, selection,
  history length, and curated control values
- mark clean termination when the TUI exits normally
- never overwrite `main.mlperformance` or an earlier session file
