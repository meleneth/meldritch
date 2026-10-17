# Meldritch 2 agent guide

This crate is an isolated new implementation. Read `ARCHITECTURE.md` and its
examples before changes. Do not depend on old Meldritch crates or import old
formats. It has no backward-compatibility obligation to Meldritch 1's APIs,
projects, sound, or behavior. The root Meldritch 1 plans are archived research,
not this crate's active queue. Each `src/units/*.rs` file owns one module's
contract and DSP behavior;
`src/contract.rs` defines the shared shape and explicit registry. Lua discovery,
validation, inspection, and lowering derive from that registry. Add a small Lua example
and a behavior test for each new capability. Keep Lua, validation, and compilation
off the sample-processing path. Preserve accepted machines on failed revisions.
