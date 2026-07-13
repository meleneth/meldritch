# 04 — DSP Parameter Pattern

This drives delay feedback with a pattern while notes continue independently.

It should:

- resolve a parameter target in a referenced DSP graph
- step delay feedback at each beat without rebuilding the static patch
- keep all values within the delay module's declared safe range
- loop the note and parameter patterns independently
- expose delay feedback as a curated performance control
- treat live control changes and authored pattern changes as distinct provenance
