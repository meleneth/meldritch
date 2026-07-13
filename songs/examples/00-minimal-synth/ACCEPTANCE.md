# 00 — Minimal Synth

This is the smallest complete song directory.

It should:

- load `main.mlperformance` as the entry point
- resolve `synths/drone.mlsynth` within the song root
- validate an audio cable from oscillator output to song output
- compile a mono saw oscillator fixed at A3
- render a finite, non-silent signal

There is no sequencer, envelope, DSP chain, or exposed control yet.
