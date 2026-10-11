# Compose a patch from existing typed modules

Status: `design`; topology authoring is unimplemented. The target is the
already supported filtered note topology from example 02. Apply
[shared rules](../../../AUTHORING.md#acceptance-execution-contract).

Create a synth with declared pitch/gate inputs and oscillator, ADSR, VCA,
low-pass and audio-output modules. Connect the table in acceptance.feature.
The TUI must show module identities, defaults and typed endpoints and allow
module/cable creation, inspection and cable removal/reconnection. Assign the
patch to an existing note pattern and hear gated filtered notes. Rust must use
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
and validly typed graphs unsupported by the current compiler. Existing engines
only; no Plaits integration or opaque convenience voice in this example.
