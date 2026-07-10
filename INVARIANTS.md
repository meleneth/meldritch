# Project Invariants

These are non-negotiable. If an implementation choice violates one of these, the choice loses.

## 1. The audio callback is sacred

The realtime audio callback must not:
- allocate on the heap
- block on locks
- wait on worker threads indefinitely
- parse DSL
- load files
- log in hot paths
- perform unbounded graph traversal
- perform UI work
- touch the filesystem
- depend on network state

Allowed:
- read precompiled state through lock-free or wait-free access
- drain bounded realtime-safe event queues
- render live overlays
- pull cached chunks
- mix buffers
- apply final safety limiting
- report minimal counters through atomics

## 2. Separate source truth from rendered artifacts

The system must distinguish:

```text
Source graph:
  patterns, samples, instruments, effects, roles, scenes, macros

Relation graph:
  audio edges, control edges, sidechains, modulation, mix rules

Artifact graph:
  rendered chunks, stems, previews, spectra, analysis, cache entries

Realtime graph:
  the tiny set of state required to emit audio now
```

Rendered artifacts are disposable. Source and relation graphs are truth.

## 3. All edits go through commands

UI code must not directly mutate audio engine state.

Every edit is a command:

```rust
Command::SetStep { pattern, track, step, event }
Command::SetParam { target, value, smoothing }
Command::ToggleMute { track }
Command::AssignRole { source, role }
Command::SetRelation { relation_id, relation_def }
```

Command processing:
1. validate
2. mutate source graph
3. bump revisions/fingerprints
4. compute dirty ranges
5. enqueue render jobs
6. publish compiled state safely

## 4. Audio and control dependencies both matter

A dependency is not only “audio flows from A to B.”

Control edges also invalidate downstream artifacts.

Example:

```text
kick audio -> drum bus
kick envelope -> bass sidechain compressor
```

If the kick changes, both the drum bus and ducked bass artifacts are dirty.

## 5. Do not enumerate the combination explosion

The power set of sources is not a render plan.

Represent combinations symbolically.
Materialize only wanted futures.

Wanted futures include:
- audio needed near the playhead
- queued scenes
- selected pattern previews
- likely performance gestures
- visible meters and analysis
- manually requested bounces/freezes

## 6. Exploit linearity, declare barriers

Every node must declare properties:

```rust
NodeProperties {
    linearity,
    latency_frames,
    tail_frames,
    can_cache,
    needs_group_signal,
}
```

Linear nodes can be factored and reused.

Nonlinear/group-dependent/time-variant/feedback nodes create cache boundaries.

Examples of barriers:
- bus compressor
- saturation
- limiter
- sidechain compressor
- feedback delay
- reverb
- spectral processors
- envelope followers used as control dependencies

## 7. Dirty invalidation must be precise and tail-aware

Edits invalidate ranges, not entire universes.

If a node has a tail, the dirty range expands:

```text
dirty_end += node.tail_frames
```

If a node has lookahead, the dirty range expands backward:

```text
dirty_start -= node.lookahead_frames
```

## 8. Determinism is a feature

For offline/precomputed artifacts:

```text
same source graph
+ same relation graph
+ same frame range
+ same sample rate
+ same seed/probability commit
= same artifact fingerprint
```

Random/probabilistic behavior must be seedable and commit-able.

## 9. The machine consumes the whole CPU only when work exists

The desired behavior:

```text
dirty work exists -> saturate available worker cores
dirty work complete -> sleep
```

No fake busy work.
No constant CPU burn for ego.
All-core hunger must be tied to useful jobs.

## 10. Realtime playback must degrade gracefully

If a hot artifact misses its deadline:
- use a previous compatible chunk with fade
- render a simplified fallback
- silence the missing bus
- report the miss

Do not block the callback waiting for perfection.

## 11. Performance behavior must be inspectable

Relationship DSP must not feel haunted.

Every active rule should be explainable:
- why a source was ducked
- which role priority won
- which control edge fired
- which chunk was invalidated
- which cache artifact was used
- whether a fallback occurred

## 12. Source relationships are first-class

A source is not just audio. It can expose:
- audio stream
- event stream
- envelope stream
- transient stream
- spectral features
- role metadata
- density/intensity metrics
- tags from the sequencer

These may drive downstream DSP.

## 13. Use typed boundaries

Avoid “string soup” after parsing.

The DSL can contain strings.
The validated model should use typed IDs and typed enums.

```rust
TrackId
PatternId
NodeId
RelationId
ParamId
FrameRange
SampleRate
Fingerprint
```

## 14. No arbitrary user code in the audio callback

Scripting can exist later, but only outside the realtime path.

Scripts may generate source graph changes or command streams.
Scripts must not execute inside `process_block`.

## 15. Test graph behavior before chasing sound toys

The first correctness target is not a lush reverb.
It is:
- commands produce predictable graph changes
- dependencies are correct
- invalidation is correct
- fingerprints are stable
- render jobs are deterministic
- realtime callback stays bounded
