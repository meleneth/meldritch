# Plaits model selection and explicit triggers

Status: accept for automated headless behavior. Physical listening pending.

The same typed Plaits module exposes all 24 pinned upstream Voice model choices,
without importing its hidden LPG. All choices must be reachable with editor
Left/Right, typed commands and Lua; names are listed in PLAITS.md.
The graph explicitly cables input.gate to source.gate as well as the ADSR.
Gate edges trigger physical/drum/FM engines; an unconnected source gate uses
upstream Unpatched (continuous excitation), preserving example 32.

For each model, select it, inspect the accepted value, compile/render finite,
non-silent main and aux audio, publish through the normal song host, save
human-readable Lua, and independently reload the same graph, IDs, values and
source fingerprint. Random Plaits samples need not match after reload.
Nonrandom reference engines retain exact full/split and reload equivalence.
Reject unknown models and incompatible gate cables without changing source.
Processing after worker initialization allocates nothing; constructors may
allocate fixed engine buffers. Native 24-frame blocks sample gate state at the
next block: note starts latch RisingEdge until consumed, note ends set Low.
External ADSR timing stays sample accurate; engine triggers have 0..23 frames
of block latency. Overlapping polyphonic notes have independent engine state.

APC/terminal cell 0 cycles models through the authored Lua widget. Releases do
not change the selected model. The invalid/ files must diagnose unknown model
names and audio-to-gate incompatibility without changing source/publication.

Run cargo test -p meldritch-control --test plaits_catalog and
cargo test -p meldritch-render --test plaits_allocation.
No physical APC/MI listening or callback deadline benchmark is claimed.
