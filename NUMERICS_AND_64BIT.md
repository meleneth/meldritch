# Numerics and 64-Bit Audio Policy

## Corrected stance

When this project says “64-bit native audio,” it means:

> The internal audio engine should use 64-bit floating-point samples by default.

Recommended baseline:

```rust
pub type Sample = f64;       // internal audio buffers and artifacts
pub type Param = f64;        // DSP parameters and automation values
pub type Coeff = f64;        // filter/effect coefficients
pub type Frame = u64;        // absolute frame positions
pub type Frames = u32;       // block/chunk lengths
pub type SampleRate = u32;
```

The hardware/device boundary may still require `f32`, `i16`, `i24`, or another format.
Conversion belongs at the boundary, not in the middle of the engine.

## Is floating point better for audio?

For internal DSP: yes, usually.

Floating point is better than fixed-point/integer for this engine because it provides:
- enormous internal level range
- safer summing of many sources
- fewer accidental clipping problems
- simpler filters and nonlinear processors
- cleaner parameter automation
- easier feedback/delay/reverb math
- easier analysis/resynthesis paths
- better behavior through deep effect chains

For final playback/export: not always.

The DAC, driver, or file format may want `f32`, 24-bit PCM, 16-bit PCM, or something else.
That is fine. The engine should compute internally in `f64`, then convert at the boundary.

## Why use f64 internally?

This project is not optimizing for tiny embedded DSP hardware.

The thesis is:
- use the actual computer
- exploit all cores
- render futures
- cache artifacts
- support dense relationship graphs
- keep transformations stable through deep chains

`f64` helps because it provides:
- more mantissa precision
- cleaner accumulation across large mixes
- safer repeated rendering/transformation passes
- better numerical stability for filters and analysis
- less roundoff error in long feedback/recursive structures
- better precision when summing many sources and buses
- safer offline/cache artifact generation

This matters more here than in a simple groovebox because the engine wants:
- many sources
- symbolic combinations
- nonlinear barriers
- pre-rendered stems
- chunk transforms
- speculative variants
- relational DSP
- analysis/control streams derived from audio

A deep graph can chew through precision. Give it tungsten teeth.

## Headroom clarification

Amplitude headroom is not mainly about `f32` versus `f64`.
Both are floating-point formats with enormous numeric range.

In this project, “headroom” means:
- internal buses may exceed `[-1.0, 1.0]`
- do not clamp between every processor
- preserve precision while summing many sources
- avoid accumulating avoidable rounding damage
- manage gain explicitly
- apply safety limiting near output/export only

Practical rule:

```text
f64 gives precision headroom.
floating-point buses give level headroom.
gain staging gives musical headroom.
```

## Default engine policy

Use `f64` for:
- internal audio blocks
- bus buffers
- render artifacts
- cached chunks
- offline render output before final export conversion
- parameter smoothing
- automation curves
- filter coefficients
- analysis inputs/outputs where useful
- mix accumulation

Use `u64` for:
- absolute frame positions
- timeline positions
- artifact ranges
- deterministic scheduling

Use `f32` only for:
- device APIs that require it
- optional low-memory/performance build profiles
- SIMD experiments if profiling proves a need
- import/export conversion when the file/device format is f32

## Device I/O boundary

The realtime callback may need to fill an output buffer in the device format.

Recommended shape:

```text
internal f64 render/mix buffer
  -> final limiter / soft clipper
  -> convert to device sample format
  -> audio backend
```

No device-format assumptions should leak into DSP modules.

## Artifact cache policy

Default cached audio artifacts should be `f64`.

Rationale:
- artifacts may be reused many times
- artifacts may feed downstream chunk transforms
- artifacts may become sources
- artifacts may be mixed into many variants
- this avoids repeated f32 quantization in the render graph

Later, cache storage may support quality tiers:

```text
cache.quality = "f64"
cache.quality = "f32"
cache.quality = "compressed_preview"
```

But the first serious engine should be `f64` internally.

## Performance tradeoff

`f64` costs more:
- twice the memory bandwidth versus `f32`
- twice the cache footprint
- fewer SIMD lanes per vector
- larger artifacts
- larger bus buffers

This is acceptable for the project identity.

The machine is allowed to work hard.
The program should sleep only when useful work is done.

Optimization rule:

```text
Start with f64 correctness.
Profile.
Then add targeted f32/SIMD modes only where they are proven valuable.
```

Do not design the engine around fear of memory bandwidth before the relational audio model exists.

## SIMD and f64

SIMD still matters with `f64`; it just has fewer lanes than `f32`.

Design for SIMD anyway:
- contiguous buffers
- planar channel layout where useful
- block processing
- minimal branching in hot loops
- no dynamic dispatch per sample
- compile graph chains into render plans

If later profiling says certain preview/analysis paths can be `f32`, make those paths explicit and typed.

## Type strategy

Initial simple strategy:

```rust
pub type Sample = f64;
```

Later, if needed:

```rust
pub trait SampleFormat:
    Copy
    + Send
    + Sync
    + 'static
{
    // intentionally narrow at first
}
```

Do not prematurely genericize the whole engine unless there is a concrete reason.
A type alias keeps the early code clean.

## No hidden narrowing

Avoid accidental narrowing:

```rust
let x: f32 = some_internal_sample as f32; // only at explicit boundary
```

Any conversion from `f64` to another format should occur in named modules:
- `device_output`
- `export_wav`
- `preview_downsample`
- `cache_compression`

## Final output

At final output/export:
- apply final gain staging
- apply limiter or soft clipper
- convert to target format
- dither when exporting to integer PCM if needed
- report clipping/limiter activity

## Denormals/subnormals

DSP can produce very tiny floating-point values that are expensive on some CPUs.

Mitigations:
- flush-to-zero / denormals-are-zero where available
- add tiny controlled noise only if necessary
- zero filter state below a very small threshold
- test long reverb/delay/filter tails for denormal behavior

## Project doctrine

This project is not trying to recreate old hardware scarcity.

Use:
- 64-bit floating-point internal audio
- explicit boundary conversion
- all-core rendering
- cacheable artifacts
- high precision by default

Then optimize only where profiling proves the engine is wasting work.
