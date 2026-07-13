# 03 — DSP Chain

This patches a synth track through a separate tempo-aware delay graph.

It should:

- resolve synth, note-pattern, and DSP files from the performance
- keep synth voice generation and downstream DSP as separate definitions
- connect the track output to `input.audio` and publish `output.audio`
- validate audio and control cables inside the DSP graph
- derive delay time from the performance tempo
- expose no DSP implementation detail through the realtime callback
