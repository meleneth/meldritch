# Compose a patch from existing typed modules

Status: `accept` for automated headless behavior, executable with
`cargo test -p meldritch-control --test patch_authoring` and the render
`song_examples` suite. Physical controller/listening acceptance remains pending.
The target is the already supported note compiler topology from example 25.
The actual keyboard script and Lua widget construct a replacement draft under
the existing synth/track identity, not a new song or track. Apply
[shared rules](../../../AUTHORING.md#acceptance-execution-contract).

Create a synth with declared pitch/gate inputs and oscillator, ADSR, VCA,
low-pass and audio-output modules. Connect the table in acceptance.feature.
The TUI shows module identities, instantiated defaults and typed endpoints and allows
module/cable creation, inspection and cable removal/reconnection. The existing track/pattern assignment is retained, and compilation produces
gated filtered notes. Rust must use
the existing graph validation/compiler, not construct a second hidden instrument.

An incomplete draft may be visible, but never published as executable audio.
Committing invalid/incomplete/unsupported topology reports its endpoint or
compiler diagnostic and retains the previous valid published revision. A cable
batch is transactional. Fingerprint every module/default and cable; keep
source/relation/artifact/realtime ownership separate. No automatic hidden
adapters or arbitrary DAG execution required.

Persist readable module IDs/types/defaults, input types, explicit cables and
track assignment in Lua; fresh reload reconstructs equal source and audio.
Reject duplicate IDs, missing ports, pitch-to-audio mismatch, unbroken cycles
and validly typed graphs unsupported by the current compiler. The primary
example composes existing oscillator modules; a tested Plaits main/aux replacement
uses the independently verified example 32 module, with no hidden voice wrapper.
See [PATCH_EDITOR.md](../../../PATCH_EDITOR.md) for the executable subset,
version-2 Lua snapshot and routing correction.

Tests also prove authored automation/reference protection, fader pickup rearming,
no lossy structural-default translation, checked-in invalid Lua requests,
continued transport/callback audio, no publication while drafting,
stale-base rejection, held input prompt at 80×24, explicit cable removal policy,
changed topology followed by parameter edits, graph reload diagnostics, unique
saves and preserved base definitions. Source/graph/audio match on fresh reload.
The existing source snapshot retains external inputs, polyphony, output layout,
track/pattern references and unrelated definitions; unsupported changes reject.
