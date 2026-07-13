# 02 — Synth Parameter Pattern

This adds a filter and a separate pattern that controls its cutoff.

It should:

- attach both a note pattern and a parameter pattern to one track
- resolve `filter.cutoff` against the synth's typed parameter input
- linearly sweep cutoff from 180 Hz to 2400 Hz over one bar
- loop note and parameter patterns independently at the same boundary
- include both patterns and the target synth definition in artifact fingerprints
