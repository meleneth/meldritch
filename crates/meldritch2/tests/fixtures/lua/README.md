# Lua engine conformance fixtures

These files are frozen implementation test inputs, not the Meldritch
application distribution. The independently maintained application corpus and
author libraries live in `meldritch-lua` and are tested there against the
published Lua host contract.

`engine/` exercises low-level graph compilation and negative boundary cases.
`authoring/` contains focused compatibility inputs for sequencer, persistence,
device, and DSP integration tests. Engine tests may depend on the public
behavior represented here, but production code must never discover or load
this directory.

Cross-repository tests run the current `meldritch-lua` revision from an
arbitrary configured path. New application behavior belongs there, not here.
