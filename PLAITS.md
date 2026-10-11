# Plaits module

Use `tui-song songs/examples/32-plaits-module --no-midi-controls`, Ctrl-E,
Up/Down, and Left/Right to inspect/edit the source. Lua `edit_instrument` uses
`model = "fm"` or `"harmonic"` as the typed value for parameter `model`;
`harmonics`, `timbre`, `morph` accept finite f64 values in 0..1.
The imported default is visible; omitted values default to FM and 0.5.
Ctrl-S and `--instrument authored/...lua` use the existing version-1 authoring
representation and unchanged base validation. This is a module with explicit
pitch input and main `audio` / `aux` outputs, external ADSR and VCA.

The dependency is the actual unmodified `mi-plaits-dsp` crate, version 0.1.0,
pinned to [effe9864bb67d23aaf5724722ac1ce165d4e7c9f](https://github.com/sourcebox/mi-plaits-dsp-rs/commit/effe9864bb67d23aaf5724722ac1ce165d4e7c9f).
No Plaits implementation existed in MELDRITCH before this milestone.
Upstream is MIT, with Oliver Rockstedt and original Emilie/Olivier Gillet
notices retained in the dependency. dyn-clone is MIT/Apache-2.0, num-traits is
MIT/Apache-2.0, libm is MIT, spin is MIT. Cargo.lock pins transitive versions.
The upstream README describes firmware 1.2 algorithms, not hardware emulation.

The crate contains 24 Voice engine choices: virtual analog VCF, phase distortion,
three six-operator FM presets, wave terrain, string machine, chiptune, virtual
analog, waveshaping, two-operator FM, additive/harmonic, wavetable, chord, speech,
swarm, noise, particle, string, modal, bass drum, snare and hi-hat.
Only standalone two-operator FM and harmonic engines are exposed here.
The Voice wrapper's internal LPG, trigger conditioning, resources and other
algorithms are excluded; importing the crate does not make them supported modules.

FM harmonics controls the frequency ratio, timbre the modulation index, morph
the feedback; aux is a sub-oscillator. Harmonic harmonics controls spectral
bumps, timbre the prominent harmonic, morph the bump shape; aux is the organ
harmonic subset. Parameter names preserve upstream semantics rather than
renaming them into a second normalization system.

48 kHz is the global preferred rate for omitted song/project rates; explicit
rates remain compatible. This module rejects other rates at compilation.
The adapter uses fixed 24-sample upstream blocks; mono pitch changes take
effect at the next block (at most 23 samples), while gate/envelope changes
remain sample accurate. Polyphonic voices start independently at the note frame.
There is no resampler. Native f32 state/block arithmetic is an authorized
external DSP exception, widened to f64 before envelope/filter/mix processing.
Source values remain f64 and distinguish even values that round to the same f32.

The upstream global AtomicU32 RNG uses separate relaxed loads/stores; random
algorithms can depend on worker/voice ordering. Plaits RNG has an authorized
exception to deterministic samples. FM and harmonic use no RNG, allocate
nothing in construction/processing in the checked test, and full/split renders
remain identical by existing pre-roll. Parameter/engine changes construct a
new worker render revision; engine code never runs in the device callback.
No lazy initialization or spin lock occurs in these two engine paths.

Tests compare adapter blocks directly to separately initialized pinned upstream
engines, cover main/aux, extremes, source edits, publication/callback consumption,
and exact Lua reload/audio equivalence. This verifies port adapter behavior;
there is no independent official C++ golden or physical MI/APC listening result.
Upstream WAV-producing tests alone are not a production certification. CPU
deadline/latency benchmarks and more engine acceptance remain future work.
