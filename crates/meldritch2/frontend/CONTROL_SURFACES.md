# Phaser control surfaces

Meldritch's control-surface layer is a rendering and interaction layer over the
existing performance store. Pinia remains the browser authority and the
revisioned server snapshot remains the system authority. No musical state is
stored in Phaser objects.

## Architecture

`PhaserControlSurface.vue` creates one Phaser game per mounted host. It owns
dynamic loading, DPR-aware rendering, container resizing, prop rebinding, and
teardown. Each surface implements `SurfaceDefinition<T>` and returns a small
runtime with `resize`, `setBinding`, and `destroy`. This makes instances
independent: scenes, input listeners, graphics, and subscriptions are not
global or shared.

`SurfaceBinding<T>` is the only state API a scene receives. A binding reads,
subscribes, writes immediately, and marks gesture boundaries. The Vue adapter
optimistically patches Pinia before sending the existing WebSocket command, so
two mounted surfaces see one another's edits without waiting for a snapshot.
A matching state echo is suppressed. If an unrelated authoritative update
arrives during a gesture, only its latest value is queued and delivered with a
`reconcile` origin when the gesture ends. Thus gestures never delay local
writes; the server still wins conflicts. Optional transaction hooks are ready
for an undo manager, but Meldritch does not currently expose one.

The common `SurfaceGestureController` uses scoped Pointer Events and pointer
capture. A single owned pointer edits. A second pointer cancels/rolls back that
edit and switches to navigation; two-pointer center movement pans and distance
changes pinch-zoom around the pointer center. `pointercancel` rolls active
edits back. Only a surface canvas uses `touch-action: none`; page scrolling is
not disabled globally. Hit areas are larger than their visible handles.

`SurfaceViewport` keeps domain coordinates separate from CSS screen pixels.
It provides invertible transforms, independent axis zoom, screen-space pan,
anchor-preserving zoom, fit, reset, limits, and resize without changing domain
data. Phaser's resolution setting handles backing-store density; interaction
coordinates remain CSS pixels.

## Adding a surface

1. Define a serializable domain model. Never put Phaser objects or pixels in
   it.
2. Build a narrow Vue-side `SurfaceBinding<T>` around existing Pinia state and
   semantic commands.
3. Implement a `SurfaceDefinition<T>` whose runtime owns its graphics and a
   shared gesture controller.
4. Render it with `PhaserControlSurface`, and dispose the binding when the Vue
   wrapper unmounts.

Surface runtimes should update graphics incrementally in response to binding
notifications. They must not import a Pinia store.

## Delivered surfaces

- XY maps normalized controller coordinates independently to any number of
  destinations. Linear, positive logarithmic, bipolar, inverted, and quantized
  mappings are pure/tested functions. Absolute, relative, and return-to-center
  modes are available. The performance-plane playground derives a linked XY
  surface from its first two real range controls and mounts two instances over
  the same Pinia values.
- Piano roll stores time in the backend's native steps and pitch as MIDI. It
  supports polyphony, overlap, stable identities, creation, cumulative
  multi-selection, movement, touch-sized duration handles, deletion, pan, and
  zoom. Tonic, mode, and chord degree are separate inputs. Scale changes only
  constrain new/moved notes. Pure transformations cover transpose, shift,
  scale, inversion, rotation, quantization, and interpolation. Collisions are
  preserved as polyphony; values are clamped rather than discarded.
  Interpolation pairs common IDs first, then stable `(start, pitch, id)` order;
  unmatched notes remain and fade toward/from minimum velocity.
- ADSR keeps attack, decay, and release as durations and sustain as a level.
  Handles and numeric fields write continuously using each Lua-authored range
  and scale. The graph distinguishes gate-on and gate-off segments. The data
  model is separate from rendering so it can grow beyond four stages later.

## Backend compatibility and limitations

The implementation uses only `control.set`, `note.create/update/delete`, and
`adsr.set`; server-projected `read_only` remains enforced in both UI and server.
It introduces no persistence format and makes no Rust changes.

The current Lua piano-roll interface exposes one viewed 16-step pattern at a
time, so the interpolation function is reusable/tested but cannot offer a
two-pattern picker until both source patterns are present in the existing
frontend snapshot. Transformation batches are sent as existing per-note
semantic commands because no batch/undo protocol exists. ADSR curve shapes are
not editable because the present backend exposes only the four values. The XY
playground is frontend composition of real range controls; adding an authored
Lua XY layout would require a future schema capability, not a new persistence
protocol.
