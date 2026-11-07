# Native Linux VST3 hosting

Meldritch 2 can load native Linux VST3 instruments as ordinary stereo-producing
graph modules. Notes, velocity, tempo, and normalized parameter values enter the
same compiled graph used by built-in oscillators, Plaits, samples, effects,
buses, real-time playback, and offline rendering.

## Host implementation choice

The host uses `vst3-host` 0.11 with its CPAL and process-isolation features
disabled. Meldritch already owns the audio device and callback, so a second
audio backend would create the parallel engine this integration is intended to
avoid. The selected crate is an actual VST3 host: it loads factories and
classes, follows component/controller and processing lifecycle calls, negotiates
32-bit audio processing, supplies events and parameter queues, and implements
component/controller state streams. It uses the generated `vst3` 0.3 bindings
and is MIT licensed.

The alternatives considered were:

- `vst3` alone. It provides current generated COM bindings, but intentionally
  has no host abstraction. Using it directly would make Meldritch responsible
  for the full lifecycle, host interfaces, bus negotiation, event lists,
  parameter queues, and state streams.
- The official Steinberg VST3 SDK. It has the reference host-side API and Linux
  support and is now MIT licensed, but integrating it would add a substantial
  C++ shim and build system. It remains the fallback if the Rust host layer
  proves incompatible with important native plugins.
- `mkaudiolibrary` hosting. Its unified host API is smaller, but its VST3 host
  has less demonstrated lifecycle, event, state, multi-class, and Linux fixture
  coverage than `vst3-host` for this milestone.

No plugin-authoring-only library was treated as a hosting solution.

## Discovery and identity

With no configuration, discovery scans the native Linux locations used by the
host library:

- `/usr/lib/vst3`
- `/usr/local/lib/vst3`
- `$HOME/.vst3`

Set `MELDRITCH_VST3_PATH` to a colon-separated path list to replace those
locations. Lua may also pass `paths = { ... }` for a specific module. Linux
`.vst3` bundles are inspected through their plugin factory; a bundle may export
multiple audio classes.

`meldritch.vst3_plugins()` returns Lua tables containing `id`, `name`,
`vendor`, `category`, `version`, `instrument`, and `path`. The `id` is the
canonical VST3 class ID and is the stable identity persisted by compositions.
A display name may be used only when it resolves uniquely. Ambiguous names are
an error rather than an arbitrary selection.

Discovery is in-process in this milestone. Scanning an invalid bundle returns a
diagnostic, but a plugin that crashes inside its factory can terminate
Meldritch. Configure trusted directories rather than pointing discovery at an
unreviewed plugin collection.

## Lua use

The module fits the existing instrument DSL:

```lua
local synth = instrument:vst3 {
  plugin = "0123456789ABCDEFFEDCBA9876543210", -- stable class ID preferred
  -- paths = { "/opt/my-plugins" },
}

instrument:connect(notes.pitch, synth.pitch)
instrument:connect(notes.gate, synth.gate)
instrument:connect(notes.velocity, synth.velocity)
instrument:connect(sequence.tempo, synth.tempo)
instrument:connect(synth.left, instrument.output.left)
instrument:connect(synth.right, instrument.output.right)

for _, parameter in ipairs(synth:parameter_info()) do
  print(parameter.id, parameter.name, parameter.value, parameter.unit)
end
synth:set_parameter(42, 0.5)
```

Logical note tracks connect all of their bounded lanes to one plugin instance;
the plugin, not Meldritch's lane allocator, owns synthesis voices:

```lua
local lead = sequence:note_track("Lead", { voices = 4 })
synth:connect_track(lead, sequence)
local cutoff = synth:parameter(2000003) -- stable numeric ID
```

`parameter` accepts a numeric ID or an exact, unambiguous plugin-reported
name. Numeric IDs should be persisted in compositions. Each lane is sent on a
separate VST3 event channel, so overlapping pitches and deterministic lane
steals generate paired note-off/note-on events without stuck notes.

Each parameter table contains its numeric `id`, stable graph `key`, display
`name`, normalized current and default values, unit, step count, automation
flag, and read-only flag. The graph parameter key is `parameter_<id>`; controls
and the existing automation system target that key, so no plugin-specific
cutoff/envelope vocabulary is invented.

Opaque state previously returned by Meldritch's Rust `Machine::vst3_state`
may be embedded as a Lua byte array with `state = { ... }` or applied before
compilation with `synth:load_state(bytes)`. State is restored before processing
starts. Treat it as meaningful only with the same class ID and plugin version.

## Processing model

VST3 is block-oriented while Meldritch's native modules have scalar DSP
contracts. A graph containing a VST3 module therefore uses the existing
topological schedule with preallocated per-slot block storage. Native modules
still run unchanged; the VST3 operation receives one block after its upstream
dependencies and its left/right outputs then feed ordinary downstream effects.

The adapter currently:

- converts Meldritch `f64` planar graph signals to the VST3 host's `f32`
  processing buffers;
- negotiates a no-input, stereo-output instrument at the engine sample rate;
- accepts blocks up to 1024 frames and rejects larger configurations clearly;
- schedules note-on, note-off, velocity, channels 1–4, and host-assigned note
  identity at exact offsets in the upcoming block;
- sends a note-off before a changed-pitch note-on and therefore handles notes
  beginning and ending within the same block;
- publishes the connected tempo and advances the VST3 processing context with
  the processed block;
- applies normalized parameter changes at processing boundaries through VST3
  parameter queues; and
- requests VST3 offline mode for file rendering, falling back to real-time mode
  when the plugin rejects offline processing.

Real-time processing does not allocate, touch the filesystem, or take a
Meldritch graph-wide lock. Plugin loading, discovery, parameter enumeration,
state work, and buffer allocation happen during graph preparation. The selected
safe host wrapper updates its own audio meter behind an internal mutex after a
process call; upstream does not yet claim a complete zero-blocking RT audit.
This is isolated inside the VST3 adapter and is the main real-time limitation of
the current dependency.

Processing errors silence that module rather than exposing partially written
buffers. Control/offline diagnostics can inspect `Machine::failed_vst3_modules`;
plugins that rejected offline mode are listed by
`Machine::realtime_fallback_vst3_modules`. Existing graph signal diagnostics
continue to measure the module's inputs, outputs, RMS, peak, and non-finite
samples.

Offline and real-time playback use the same graph and module implementation.
Exact sample-for-sample reproducibility still depends on the hosted plugin;
plugins may use random seeds, wall time, internal threading, or different
offline algorithms.

## Reproducible native fixture and demo

The repository contains an MIT/Apache-2.0 Rust VST3 test instrument. It is
built from source and never committed as a binary:

```sh
tools/build-vst3-fixture.sh
cargo run -p meldritch2 -- render \
  crates/meldritch2/examples/69-vst3-test-instrument.lua \
  /tmp/meldritch-vst3-demo.wav 4 --workers=2
cargo test -p meldritch2 --test vst3_host
```

The integration test uses the actual VST3 ABI and verifies discovery, metadata,
class identity, creation/configuration/activation, sample-offset note delivery,
audible output, note-off, parameter enumeration and changes, state round-trip,
repeated destruction, invalid identity errors, unsupported block diagnostics,
Lua construction, offline rendering, and routing through Meldritch's existing
drive effect.

## Serum 2

Install the native Linux bundle at `$HOME/.vst3/Serum2.vst3`. Its stable class
ID is `56534558667350736572756D20320000`; the conditional host test runs when
that bundle is present and otherwise leaves portable CI on the source-built
fixture. The complete performance example is:

```sh
cargo run -p meldritch2 -- perform crates/meldritch2/examples/73-serum-performance.lua
cargo run -p meldritch2 -- render \
  crates/meldritch2/examples/73-serum-performance.lua \
  /tmp/meldritch-serum-performance.wav 8
```

The example uses programmatically enumerated Serum IDs for main level, filter
cutoff/resonance/drive, oscillator A/B wavetable position, and macros 1/2.
Values are normalized VST3 automation values and are delivered at the next
processing block; there is no browser automation clock.

Serum 2.1.5's native `.SerumPreset` files are Xfer-specific containers. Serum
rejects those bytes through component `setState` and does not expose
`IProgramListData`, so Meldritch does not pretend they are loadable through a
standard VST3 preset API. Example 73 instead preloads three independent Serum
instances from explicit normalized parameter snapshots and switches their
already-running outputs at a processing boundary. This is safe for live use,
retains independent state, avoids filesystem work on the audio thread, and is
deterministically reconstructed on composition reload. Exact factory-preset
loading remains unavailable until Xfer exposes a supported non-GUI interface.

Serum logs an `IRunLoop` warning because the current host does not provide GUI
event-loop facilities. Native DSP, notes, parameters, state, real-time playback,
and offline rendering have been validated despite that warning. Editor
embedding is intentionally out of scope.

## Geonkick 3.9 drum kits

Meldritch has been validated against native Geonkick 3.9.0 (class ID
`E2CC953290E863F06405E13924E34B24`, category `Instrument|Synth`). The upstream
VST3 is a 16-instrument kit and advertises 17 stereo output buses (16 kit
channels plus audition). Geonkick requires all buses to remain present. Set
`activate_all_outputs = true`; Meldritch processes every bus without allocating
in the callback and sums them into the module's existing stereo contract. The
individual buses are not yet separately routable in the graph.

Build the [official Geonkick source](https://github.com/quamplex/geonkick) with
its documented dependencies and the Steinberg VST3 SDK, then install to a user
prefix:

```sh
cmake -S geonkick -B build -DCMAKE_BUILD_TYPE=Release \
  -DCMAKE_INSTALL_PREFIX="$HOME/.local" \
  -DCMAKE_INSTALL_FULL_DATADIR="$HOME/.local/share" \
  -DVST3_SDK_PATH=/path/to/vst3sdk \
  -DGKICK_STANDALONE=OFF -DGKICK_PLUGIN_LV2=OFF
cmake --build build --parallel
cmake --install build
```

The normal locations are `$HOME/.local/lib/vst3/Geonkick.vst3` and
`$HOME/.local/share/geonkick/presets`. Pass the first directory in `paths` or
add it to `MELDRITCH_VST3_PATH`.

Geonkick `.gkit` presets are opaque component state. Restore one before
activation with `state_file`, alongside `activate_all_outputs = true`.
`state_file` is read on the Lua/control thread during composition loading, is
limited to 16 MiB, and is never accepted from browser commands. Opaque state
can still be saved through `Machine::vst3_state`.

Geonkick 3.9 exposes no automatable VST3 parameters, so its synthesis controls
cannot be honestly mapped to Meldritch knobs yet. Kit design remains in
Geonkick's editor; Meldritch provides note velocity, lane mute, pattern, and
kit-state control. The TimeKit mapping used by example 74 is explicit: kick
24, snare 26, clap 27, alternate snare 28, closed hat 30, hat 32, open hat 34,
and shaker 58. These are preset mappings, not assumed General MIDI values.

## Current boundaries

- Native Linux `.vst3` bundles only. Wine and yabridge are not configured or
  detected.
- Instruments only: no audio input bus is exposed yet. Effects are the next
  natural extension of the same adapter.
- Main stereo output is supported. A plugin that cannot negotiate the requested
  configuration fails graph preparation; incompatible buffers are never
  silently processed.
- No plugin editor or GUI embedding.
- Opaque VST3 state round-trips are supported, but vendor-native preset files
  require a plugin-supported host interface. The Serum example exposes only
  its explicitly configured preloaded performance patches.
- Hosting is in-process. Rust ownership and lifecycle errors are contained by
  the adapter, but a native plugin can still abort, corrupt memory, deadlock, or
  run unbounded work; none of those failures can be recovered safely in the
  current process. The Lua and graph contract does not expose native pointers,
  leaving room for a future process-isolated implementation without changing
  compositions.
