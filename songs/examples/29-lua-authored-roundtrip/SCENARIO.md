# End-to-end musical authoring contract

This integration uses one existing deterministic oscillator/ADSR/VCA/filter
synth, one new two-bar note pattern, and three Lua-declared APC faders.
No new synthesis, controller framework, renderer or routing semantics.

Start the song, edit oscillator waveform and envelope/filter, compose notes using
real MIDI byte press/release decoding and Lua grid policy, hold a start and choose
an inclusive endpoint (Rust duration ends exclusively), visit time page 1, assign
cutoff/volume/pan through Ctrl-M, acquire pickup, and change the audible mix.
Visit performance, instrument, pattern and assignment/mixer views without
interrupting transport. APC and simulated terminal/Stream Deck presentations
must reflect the same authoritative source.

F6 / Lua save_project writes one atomic, unique kind="project",
version=1 Lua definition containing existing instrument (v1/v2), pattern (v1)
and controls (v1) definitions over one unchanged base fingerprint. It must
restore the exact selected synth graph, pattern IDs/notes/durations/counter,
track ownership, mixer values and assignments in a fresh process. Repeated
serialization is byte-identical. Full and split f64 renders and fresh-process
f64 sample bytes must agree. The envelope rejects uncaptured source changes.

Invalid IDs/types/ranges, unsupported targets, stale draft/base/references,
interrupted pad gestures and failed saves preserve accepted source and prior
good artifacts. Clean exit checkpoints a separate performance session; reload
must succeed without reading it. Transient pickup, held pads, UI page and transport
are not authored musical state.

Automate this contract headlessly. Physical gestures, audible changes, LED
orientation/feedback and Stream Deck device transport remain separate, pending
checks. Record concrete navigation friction and defer broader ergonomic changes.
