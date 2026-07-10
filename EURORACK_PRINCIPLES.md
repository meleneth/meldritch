# Eurorack Principles

Eurorack is a major design inspiration.

The project should not become a monolithic integrated workstation where every feature is fused into every other feature.

The preferred model is:

```text
small combinable modules
+ typed patch points
+ explicit signal/control/event flow
+ optional automation that builds useful integrations
```

In other words:

> Patchable pieces first. Automated integrations second.

## Core principle

A feature should usually begin life as a module with ports.

Then higher-level conveniences may generate patch graphs from those modules.

Example:

```text
User says:
  kick ducks bass

System expands to:
  kick audio -> envelope follower
  envelope follower -> ducking control signal
  bass audio -> gain/dynamic EQ node
  control signal -> gain/dynamic EQ amount
  ducked bass -> bass bus
```

The integration is automated, but the resulting graph is still visible and inspectable.

## Signal kinds

The system should support more than audio.

Typed signal families:

```text
Audio
  sample-rate signal buffers

Gate
  on/off state, often step/event derived

Trigger
  short impulses or event pulses

CV / Control
  continuous parameter streams, possibly audio-rate or block-rate

Event
  notes, steps, ratchets, accents, fills, scene transitions

Feature
  derived analysis streams such as envelope, transient, RMS, centroid

Metadata
  role, tags, source identity, probability, humanization, scene context
```

The important move is not to flatten all of these into “parameters.”
They are different kinds of musical electricity.

## Ports

Every module declares typed ports.

```rust
pub enum PortKind {
    AudioIn,
    AudioOut,
    GateIn,
    GateOut,
    TriggerIn,
    TriggerOut,
    ControlIn,
    ControlOut,
    EventIn,
    EventOut,
    FeatureIn,
    FeatureOut,
}
```

A connection must be type-valid or explicitly adapted.

Adapters are first-class modules:
- envelope follower: audio -> control/feature
- transient detector: audio -> trigger/feature
- gate extractor: event -> gate
- event tag filter: event -> event
- smoother: stepped control -> smoothed control
- quantizer: continuous control -> stepped/musical control
- sample-and-hold: trigger + control -> control

## Rates

Ports should declare rate expectations.

```text
audio-rate
block-rate
step-rate
beat-rate
event-rate
scene-rate
manual/UI-rate
offline-only
```

The compiler can insert smoothing, resampling, latching, or quantization modules when needed.

Example:

```text
step-rate automation -> smoother -> audio-rate filter cutoff
```

## Modules over features

Prefer modules such as:
- sampler
- oscillator
- envelope
- LFO
- envelope follower
- transient detector
- gate sequencer
- event filter
- role priority router
- sidechain gain cell
- dynamic EQ cell
- mixer
- crossfader
- delay
- reverb
- distortion
- spectral freeze
- chunk transform
- cache source
- artifact recorder

Avoid implementing “big features” as opaque blobs when they can be expressed as patchable module graphs.

## Patch recipes

Convenience integrations should compile into patch recipes.

Examples:
- `kick_ducks_bass`
- `accented_snare_delay`
- `ghost_notes_to_reverb`
- `lead_carves_pad`
- `scene_intensity_saturation`
- `fill_only_spectral_smear`

A recipe is not magic. It expands into modules and connections.

```rust
pub struct PatchRecipe {
    pub name: RecipeId,
    pub params: RecipeParams,
    pub expands_to: PatchGraph,
}
```

The TUI should let the user inspect the expansion.

## Normalization

Eurorack modules often have normalized connections: useful defaults that are overridden when patched.

This project can use the same idea.

Example:
- a track’s envelope may normally drive its own amp
- patching another control source into amp CV overrides or blends with the default
- a bus normally routes to master
- patching a bus to a transform recorder adds a parallel route

Normalization gives fast setup without hiding the patch model.

## Modulation matrix as patch view

A modulation matrix is just another view of the patch graph.

Do not build a separate hidden modulation system.

```text
source control port -> target parameter port
```

The TUI may present this as:
- patch cables
- relation table
- modulation matrix
- rule inspector
- dependency graph

Same underlying model.

## Graph compiler responsibilities

The compiler should:
1. validate port compatibility
2. insert explicit adapters where allowed
3. reject ambiguous connections
4. mark audio and control dependencies
5. compute latency/tail/lookahead
6. identify cache boundaries
7. lower patch graph into render plans
8. preserve explainability

## Automation of integrations

The project should absolutely automate integrations when useful.

But automation must produce inspectable structure.

Bad:

```text
turn on "smart mix" and hidden code changes everything
```

Good:

```text
apply "kick owns sub" recipe
  -> creates envelope follower
  -> creates multiband ducking cell
  -> connects kick feature output to bass low-band gain
  -> marks control dependency
  -> displays the rule in the TUI
```

## Module metadata

Every module should declare:

```rust
pub struct ModuleProperties {
    pub latency_frames: u32,
    pub lookahead_frames: u32,
    pub tail_frames: u32,
    pub linearity: Linearity,
    pub can_cache: bool,
    pub realtime_safe: bool,
    pub deterministic: bool,
}
```

This supports:
- scheduling
- cache invalidation
- worker rendering
- realtime safety checks
- graph explanation

## UX principle

The user should be able to work at two levels:

### Patch level

Build from primitives.

```text
kick -> envelope follower -> bass duck amount
snare accent events -> delay send
pad -> spectral freezer -> ghost bus
```

### Recipe level

Ask for a known integration.

```text
apply kick_ducks_bass amount=0.7 bands=sub,low
apply ghost_notes_to_reverb mix=0.35
apply fill_bar_spectral_smear bar=4
```

Both produce the same underlying graph.

## Anti-invariant

Do not make “integrated because convenient” the default architecture.

Convenience should be a compiler layer over composable parts.
