# Testing Strategy

The engine should be tested like a compiler and like an audio system.

## Test layers

### 1. Pure model tests

Test:
- IDs
- commands
- validation
- graph construction
- dependency edges
- dirty ranges
- node properties

These tests should not touch audio devices.

### 2. DSL fixture tests

Fixtures:
- minimal project
- missing sample
- invalid role
- invalid relation
- sidechain project
- event-aware send project
- multiband ducking project

Test:
- parse success/failure
- diagnostics
- typed model conversion
- stable fingerprints

### 3. Timeline tests

Test:
- BPM to frame conversion
- step to frame conversion
- gate length
- pattern loop boundaries
- probability seed determinism
- ratchets
- polymeter later

### 4. Graph invalidation tests

Must test both audio and control dependencies.

Examples:
- changing hat invalidates drum bus but not bass sidechain
- changing kick invalidates drum bus and bass sidechain target
- reverb tail expands dirty range
- lookahead expands dirty range backward
- nonlinear node creates cache boundary

### 5. Render plan tests

Test:
- source graph compiles to expected render ops
- render op fingerprints are stable
- equivalent linear mixes canonicalize where intended
- nonlinear barriers prevent invalid reuse

### 6. Offline audio tests

Test:
- render deterministic buffer
- render same artifact twice equals same fingerprint
- cache hit produces same output
- changing one event affects expected frame range
- final output contains finite samples only

Avoid brittle “exact floating point waveform” tests except for tiny deterministic kernels.
Prefer:
- fingerprints for known fixtures
- RMS/peak ranges
- no NaN/infinity
- expected silence/non-silence
- monotonic envelope behavior
- known impulse responses for simple filters

### 7. DSP kernel tests

Test:
- envelope follower attack/release behavior
- ducking gain never negative unless explicitly allowed
- param smoothing reaches target
- filters remain finite
- denormal prevention for long tails
- limiter catches overs

### 8. Realtime safety tests

Some realtime rules are hard to prove automatically, but enforce what we can.

Test/scan:
- no filesystem calls in callback modules
- no logging in hot callback
- no thread spawning in callback
- no unbounded allocation in callback path where practical
- callback consumes preallocated buffers

Use code review and module boundaries as part of the test.

### 9. Worker/cache tests

Test:
- dirty jobs enqueue
- jobs complete into cache
- priority ordering
- hot jobs outrank cold jobs
- clean state has empty queue
- artifact cache evicts safely
- missed artifact fallback path is explicit

### 10. TUI state tests

Test:
- key command maps to typed command
- UI never mutates engine state directly
- inspector data can render active relations
- diagnostics can display dirty ranges and cache hits

## Golden fixtures

Keep small fixture projects under version control.

Suggested:
- `fixtures/minimal_kick.toml`
- `fixtures/basic_drums.toml`
- `fixtures/event_sends.toml`
- `fixtures/kick_ducks_bass.toml`
- `fixtures/reverb_tail_dirty_range.toml`
- `fixtures/nonlinear_barrier.toml`

## Property tests

Useful properties:
- dirty range always includes edited event frame
- tail expansion never shrinks dirty range
- command application either succeeds atomically or fails without mutation
- fingerprints change when semantically relevant inputs change
- fingerprints do not change when irrelevant display-only fields change
- all rendered samples are finite

## Performance tests

Add later:
- offline render throughput
- worker saturation
- cache hit rate
- memory bandwidth hotspots
- callback time budget
- missed artifact count

## Philosophy

This project is graph/compiler-heavy. Test graph correctness early or every later feature becomes fog.
