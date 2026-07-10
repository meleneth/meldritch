# DSP Algorithms

This document focuses on algorithms that support the project thesis: relational groove synthesis.

## Table-stakes DSP

Needed, but not differentiating:
- gain
- pan
- envelopes
- sample playback
- interpolation
- pitch shift basics
- filters
- delay
- reverb
- compressor
- saturation
- limiter
- resampling

Implement enough to support the relational features. Do not spend the first month building a generic synth museum.

## 1. Event-aware sends

Sequencer metadata controls effect routing.

Examples:
- only accents enter delay
- only ghost notes enter reverb
- only ratchets enter comb filter
- only fill notes enter spectral smear
- probability-generated notes enter a different bus

Event tags:

```rust
pub enum EventTag {
    Accent,
    Ghost,
    Fill,
    Ratchet,
    Probabilistic,
    Humanized,
    SceneTransition,
}
```

A send rule:

```rust
pub struct EventSendRule {
    pub source: SourceId,
    pub target_bus: BusId,
    pub tag_filter: TagPredicate,
    pub amount: f32,
}
```

Why this matters:
Effects become musically selective instead of dumb stream processors.

Difficulty: medium-low.
Demo value: high.

## 2. Envelope follower sidechain

Basic driver envelope:

```text
env[n] = attack_coeff  * env[n-1] + (1 - attack_coeff)  * x, if x > env
env[n] = release_coeff * env[n-1] + (1 - release_coeff) * x, otherwise
```

Use absolute value or RMS window as detector input.

Ducking gain:

```text
gain = 1.0 - amount * shaped(env)
```

Better:
- threshold
- ratio/curve
- attack/release
- lookahead optional
- per-band later

Relationship example:

```text
kick envelope -> bass gain
```

Difficulty: medium.
Demo value: high.

## 3. Multiband dynamic ducking

Split driver and target into bands.

Initial bands can be simple:
- sub
- low
- low-mid
- high-mid
- air

For each band:
1. band-pass or crossover driver
2. envelope-follow driver band
3. attenuate target band according to rule
4. recombine target bands

Example:

```text
kick ducks bass only in sub/low bands
lead ducks pad only in mids
snare ducks noise only around snap band
```

This avoids full-spectrum pumping.

Difficulty: medium-high.
Demo value: very high.

## 4. Role-aware mix rules

Sources declare roles.

Example roles:
- anchor.transient
- anchor.sustain
- lead
- texture.harmonic
- texture.noise
- ghost
- chaos
- glue
- air

Rules:

```text
anchor.transient wins low bands during attacks
lead wins mid bands while active
ghost never triggers master ducking
chaos bypasses polite mix rules
texture yields to lead
```

Role priority table:

```rust
pub struct RolePriority {
    pub role: RoleId,
    pub band: BandId,
    pub priority: i32,
}
```

Compile role rules into:
- sidechains
- dynamic EQ
- gain automation
- routing changes
- send levels

Difficulty: architecture-heavy.
Demo value: very high.

## 5. Adaptive spectral masking

A more advanced version of role-aware ducking.

For each time-frequency bin:
1. estimate energy per source
2. apply role priorities
3. attenuate lower-priority sources where conflict exceeds threshold
4. smooth over time/frequency
5. inverse transform or apply through multiband approximations

Implementation options:
- start with 4-8 band filterbank, not full STFT
- later add STFT for precision
- avoid phase nastiness until design is solid

Difficulty: high.
Demo value: high if audible and controllable.

## 6. Chunk transforms

Use the render farm for operations that rewrite future audio chunks.

Examples:
- reverse bar
- reverse only reverb tail
- spectral freeze
- transient smear
- reslice into grains
- turn previous bar into ghost texture
- render fill variant
- freeze bus into resequencable artifact

Model:

```rust
pub enum ChunkTransform {
    Reverse,
    FreezeSpectrum,
    Granulate(GranulateParams),
    Reslice(ResliceParams),
    Smear(SmearParams),
}
```

Chunk transform output is a new source/artifact.

Difficulty: medium to high.
Demo value: extremely high.

## 7. Cross-source distortion

Distortion node accepts carrier and modulator.

Examples:
- kick envelope biases bass saturation
- snare transient opens drum bus wavefolder
- noise controls distortion asymmetry
- pad harmonics modulate bass drive

Concept:

```text
out = nonlinear(carrier, drive + modulator * amount)
```

Rules:
- keep modulation visible/explainable
- oversample nonlinear sections when needed
- avoid uncontrolled intermodulation mud unless explicitly desired

Difficulty: medium.
Demo value: high.

## 8. Rhythmic DSP

DSP state can be quantized to musical time.

Examples:
- filter changes only on step boundaries
- delay feedback changes per bar
- reverb tail ducked on pattern transition
- compressor release follows groove template
- distortion oversampling increases during fills
- spectral freeze latches on selected steps

This is cheap and powerful because the sequencer already knows time.

Difficulty: medium-low.
Demo value: high.

## 9. Resynthesis-lite

Useful subset:
- onset detection
- transient/body/noise split
- spectral freeze
- granular playback
- harmonic tracking for monophonic material
- envelope extraction

Groovebox-friendly use:
- split snare into transient/body/noise
- resequence transient while stretching noise
- turn sample bodies into tuned resonators
- extract ghost textures from old bars

Difficulty: high if broad.
Demo value: high when constrained.

## Parameter smoothing

Any parameter controlled by UI, automation, relation rules, or events must support smoothing.

Avoid zipper noise.

```rust
pub struct ParamRamp {
    pub current: f32,
    pub target: f32,
    pub step: f32,
    pub remaining_frames: u32,
}
```

Some parameters should update:
- sample-accurately
- per block
- per step
- per beat

The update rate is a musical and DSP decision.

## Oversampling policy

Use oversampling for nonlinear processors:
- saturation
- clipping
- wavefolding
- aggressive distortion

Initial policy:
- no oversampling by default
- optional 2x/4x for selected nodes
- render-farm/offline chunks may afford higher quality
- realtime fallback may use cheaper mode

## Latency/tail policy

Every effect reports:
- latency
- lookahead
- tail

This affects:
- scheduling
- invalidation
- cache chunk boundaries
- UI explanation
- pattern transition behavior

## First DSP milestone

Implement:

1. event-aware sends
2. envelope follower
3. simple sidechain ducking
4. 3-band or 4-band dynamic ducking
5. role table
6. explanation panel data model

This proves the project’s identity without requiring a full spectral research basement.
