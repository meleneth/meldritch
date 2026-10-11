# Live output startup

Every newly opened audio-device stream starts at zero output gain for four
seconds, then rises linearly to unity over 100 ms. Timing uses the actual
configured device sample rate and counts output frames, including silence while
transport is stopped. Transport can continue advancing during the muted period.
Play/stop/rewind and published render replacements do not restart this timer;
opening a new stream does.

This is a device output policy. Authored track volume/pan, instrument definitions,
Lua exports, session journals, published PCM and offline WAV renders retain their
values. It adds no new musical parameter or assignment target.
The callback applies one common gain to every channel before sample-format
conversion, after the existing safety clamp. Its counters and ramp use fixed
inline state, with no allocation, lock or wall-clock call.

Headless acceptance is automated with:

```sh
cargo test -p meldritch-audio device_output::tests
```

At 44.1 and 48 kHz, a stereo constant source must emit exactly four seconds of
zeros across irregular callback boundaries, reach unity after another 100 ms,
retain stereo balance, and leave source samples unchanged. Waiting while stopped
must consume the startup interval; playing and rewinding afterward must retain
full gain. Existing callback publication, transport, underrun and format
behavior must remain compatible.

These tests exercise the same callback implementation with the startup policy
enabled. Raw headless publication tests and offline rendering omit device startup.
Physical listening remains pending. The startup mute does not diagnose or repair
the cause of reported startup static.
