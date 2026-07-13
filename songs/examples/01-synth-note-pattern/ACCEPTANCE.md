# 01 — Synth Note Pattern

This adds pitch/gate inputs, an envelope, a VCA, and a reusable note pattern.

It should:

- resolve the synth and pattern from the performance
- validate pitch, gate, envelope, control, and audio port types
- play C3, E3, G3, and B-flat3 as quarter notes over one looping bar
- route pattern pitch to the oscillator and pattern gate to the envelope
- produce deterministic note timing at 120 BPM and 48 kHz
