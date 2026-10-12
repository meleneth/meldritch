# Live output startup

Native F9 MIDI audition now adds one bounded timed mono PCM lane to the existing
device callback. The shared mono DSP kernel runs on a named worker in 256-frame
blocks, normally at most 1024 frames ahead; the ring holds at most 4096 frames.
The callback copies atomic PCM slots, advances a u64 device-frame clock through
silence/stopped transport, rejects stale epochs/late frames and applies the same
startup gain, safety clamp and sample-format conversion. It never executes DSP,
locks the worker or drops PCM heap owners. Stale-slot examination is bounded to
the ring capacity per callback. Missing data yields silence with gap telemetry;
the worker advances DSP through missed frames rather than replaying old samples.

Audition currently requires stopped arrangement transport. Play/Stop/Rewind
disable buffered audition before the next callback samples; F9/source changes
also invalidate it. This is fresh-voice audition and continuous DSP within a fixed
graph, not graph migration or simultaneous arranged/live routing. Reset may
introduce a brief silence while stale buffered PCM is discarded. Frame scheduling
uses the locked render frontier, not MIDI timestamps or a claimed DAC clock.
Run `cargo test -p meldritch-audio --test streaming_audio` for timed publication,
stale/late/backpressure negatives and zero callback allocation/deallocation.
CLI keyboard audition tests compare production worker/callback PCM with the
shared renderer past a full authored bar and cover input loss and capture facts.
Physical listening/deadlines and timed replay remain unverified.
Capture checkpoint failure releases audition through typed cleanup retained for
retry, so filesystem failure cannot indefinitely sustain a keyboard gate.

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
