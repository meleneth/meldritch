# Plaits module

> **Meldritch 1 archive:** This document records completed research and its
> historical plans. Meldritch 2 is the active project; this is not
> its current architecture, acceptance queue, or compatibility contract.

Start the model explorer:

```sh
cargo run -- tui-song songs/examples/33-plaits-models --lua main.lua
```

Ctrl-E opens the instrument editor. Select `osc.model` with Up/Down; Left/Right
cycles all 24 model choices. APC grid cell 0 (or logical terminal cell 0) also
cycles models through the example's Lua-authored policy. Numeric `harmonics`,
`timbre`, `morph` use the same typed edits and accept finite f64 values in 0..1.
Ctrl-S saves a unique readable Lua instrument definition under `authored/`.
Reload with `--instrument authored/FILE.lua`; independent `render-instrument`
uses the same validation/compiler. Session journals remain in `performances/`.

The dependency is the actual unmodified `mi-plaits-dsp` crate, version 0.1.0,
pinned to [effe9864bb67d23aaf5724722ac1ce165d4e7c9f](https://github.com/sourcebox/mi-plaits-dsp-rs/commit/effe9864bb67d23aaf5724722ac1ce165d4e7c9f).
No Plaits implementation existed in MELDRITCH before example 32.
Upstream is MIT, with Oliver Rockstedt and original Emilie/Olivier Gillet
notices retained in the dependency. dyn-clone is MIT/Apache-2.0, num-traits is
MIT/Apache-2.0, libm is MIT, spin is MIT. Cargo.lock pins transitive versions.
The upstream README describes firmware 1.2 algorithms, not hardware emulation.

All 24 upstream Voice model choices now have typed standalone adapters:

| Authored model | Engine |
| --- | --- |
| `fm` | Two-operator FM |
| `harmonic` | Additive harmonics |
| `virtual_analog` | Virtual analog |
| `waveshaping` | Waveshaping |
| `grain` | Granular formants |
| `wavetable` | Wavetable |
| `chord` | Chord |
| `speech` | Vowels and LPC speech |
| `swarm` | Swarm |
| `noise` | Filtered noise |
| `particle` | Particle noise |
| `string` | Inharmonic string |
| `modal` | Modal resonator |
| `bass_drum` | Analog/synthetic bass drum |
| `snare_drum` | Analog/synthetic snare |
| `hihat` | Hi-hat |
| `virtual_analog_vcf` | Virtual analog with upstream VCF |
| `phase_distortion` | Phase distortion |
| `six_op_a` | Six-operator FM, bundled SYX_BANK_0 |
| `six_op_b` | Six-operator FM, bundled SYX_BANK_1 |
| `six_op_c` | Six-operator FM, bundled SYX_BANK_2 |
| `wave_terrain` | Wave terrain, bundled terrains |
| `string_machine` | String machine |
| `chiptune` | Chiptune |

Model names are stable typed identities rather than upstream numeric indices.
The old default remains `fm` with three values of 0.5; existing FM/harmonic TOML
and version-1 Lua definitions remain compatible. Changes of topology use the
existing version-2 Lua graph snapshot. Unknown names and incompatible cables
are rejected through the same semantic boundaries.

Harmonics/timbre/morph preserve each engine's upstream meaning. For example FM
uses ratio/index/feedback; string/modal use material/excitation/decay; six-op
uses preset/modulator level/envelope stretching; wave terrain uses terrain/path
radius/path offset. These are not generic cutoff/resonance controls.
Bundled wavetables and all three six-op banks are initialized before rendering.
User-supplied terrains, wavetables, SYX banks, and the Voice wrapper's internal
LPG, modulation knobs and output normalization are not exposed. The sound source
remains one inspectable typed module with external ADSR, VCA and optional filter.

Ports are pitch input, optional gate input, and main `audio` / `aux` outputs.
Example 33 explicitly connects `input.gate -> osc.gate` and
`input.gate -> env.gate`. The source consumes note starts as RisingEdge,
held gates as High, and ends as Low; velocity supplies the upstream accent.
A start that is released before the next block remains latched for that block.
Several starts within one native block coalesce into one trigger per voice.
An unconnected source gate uses upstream Unpatched (continuous excitation),
so older example 32 and saved graphs keep working. To trigger drums or strings
in that graph, use a Ctrl-G draft `connect input.gate osc.gate`, then `commit`.
Polyphonic voices have independent engines.

48 kHz is the global preferred rate for omitted song/project rates; explicit
rates remain compatible. Plaits rejects other rates at compilation.
Native blocks are 24 samples: mono pitch/source gate changes apply at the next
block (0..23 frames); external ADSR timing remains sample accurate. Polyphonic
voices start at the note frame. There is no resampler. Native f32 DSP is an
authorized exception, widened to f64 before external envelope/filter/mixing.
Source f64 values retain their precision even when they round to the same f32.

Constructors allocate fixed upstream engine buffers on the render worker;
FM/harmonic retain allocation-free construction. All 24 choices pass the checked
zero-allocation processing test, including patched/unpatched gates and joint
parameter values 0, 0.5, 1. Six-op's spin::Once algorithm initialization happens
during worker setup; it is not called from the device callback.
Parameter/model changes construct a new render revision and publish immutable
PCM through the normal audio host. Engine code never runs in the device callback.

The global upstream RNG uses separate relaxed AtomicU32 loads/stores; random
models can vary with worker/voice ordering. Plaits RNG is explicitly exempt
from exact sample determinism; authored graph/values/fingerprints remain exact.
Ten nonrandom engines have direct separately initialized upstream block
comparisons and full/split/reload equivalence tests. All models have finite,
non-silent main/aux preset renders, keyboard/Lua selection, publication and
independent Lua reload tests. These checks verify the port integration, not
official C++ golden equivalence or physical MI/APC listening. CPU deadline
benchmarks and physical listening remain pending.
