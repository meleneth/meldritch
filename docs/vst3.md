# Native Linux VST3 hosting

The complete instrument-and-effects reference composition is documented in
[Pressure System Live](pressure-system-live.md).

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
  -- midi_channels = { 10, 10, 10, 10 }, -- one channel per note lane
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
name. Numeric IDs should be persisted in compositions. Lanes default to MIDI
channels 1 through 8. `midi_channels` can assign any channel 1–16 to each
connected lane, including sharing channel 10 for a General MIDI drum kit.
Overlapping pitches and deterministic lane steals generate paired
note-off/note-on events without stuck notes.

Each parameter table contains its numeric `id`, stable graph `key`, display
`name`, normalized current and default values, plugin-formatted `display`, unit,
step count, automation/read-only/bypass flags, and raw VST3 flags. The graph
parameter key is `parameter_<id>`; controls
and the existing automation system target that key, so no plugin-specific
cutoff/envelope vocabulary is invented.

`Machine::vst3_state` captures the component stream through
`IComponent::getState` and, for a separate controller that supports it, the
controller stream through `IEditController::getState`. The host keeps the two
streams distinct in an opaque, versioned snapshot. Restore stops processing,
deactivates the component, calls component `setState`, synchronizes that stream
to the controller with `setComponentState`, applies optional controller
`setState`, then reactivates and resumes. Unsupported controller state is not an
error; component-state failure is.

Meldritch state files add a `MELDRITCH_VST3` versioned binary envelope around
that byte-exact snapshot. JSON metadata records class ID, vendor, plugin
name/version, patch name, optional source, and separate component/controller
byte counts. The loader rejects identity and size mismatches before calling
plugin code. Legacy raw component snapshots remain accepted through
`state_file`, but do not have identity validation.

Stereo audio effects use the same prepared lifecycle and parameter model:

```lua
local compressor = instrument:vst3_effect {
  id = "master_compressor",
  plugin = "647370206B31732020202020756E7363",
}
instrument:connect(mix.left, compressor.input.left)
instrument:connect(mix.right, compressor.input.right)
instrument:connect(compressor.out.left, instrument.output.left)
instrument:connect(compressor.out.right, instrument.output.right)
```

`vst3_effect` accepts only classes whose metadata does not identify an
instrument, and currently requires exactly one stereo main input and stereo
main output. Effects participate in normalized parameter automation, opaque
state round-trips, offline mode, failure diagnostics, and latency reporting via
`Machine::vst3_latency_samples`. The reported latency is observable for render
tail and routing decisions; automatic compensation across parallel paths is not
yet implemented.

## Processing model

VST3 is block-oriented while Meldritch's native modules have scalar DSP
contracts. A graph containing a VST3 module therefore uses the existing
topological schedule with preallocated per-slot block storage. Native modules
still run unchanged; the VST3 operation receives one block after its upstream
dependencies and its left/right outputs then feed ordinary downstream effects.

The adapter currently:

- converts Meldritch `f64` planar graph signals to the VST3 host's `f32`
  processing buffers;
- negotiates either a no-input stereo instrument or a stereo-input/stereo-output
  effect at the engine sample rate;
- accepts blocks up to 1024 frames and rejects larger configurations clearly;
- schedules note-on, note-off, velocity, configured channels 1–16, and host-assigned note
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

Live output requests a 1024-frame device period and permits VST3 batches up to
that size. Serum 2's per-call overhead makes the former forced 256-frame host
chunks miss real-time deadlines even when its aggregate DSP load is
sustainable. At 48 kHz a full block trades roughly 21 ms of output latency for
reliable callback headroom; an audio backend may return a nearby size instead.
Startup reports the actual buffer size returned by the backend.

Live shutdown is an explicit two-phase operation. The host first asks the
device callback to stop entering the graph, return silence, and transfer its
`LiveRuntime` back to the control thread. Only after that acknowledgement does
it pause and destroy the CPAL stream. This keeps plugin teardown on the thread
that created the graph; in particular, JUCE-based plugins must not have their
message-manager-backed instances destroyed by CPAL's audio worker. Failure to
quiesce or return the runtime within two seconds is reported instead of waiting
without a bound.

## Reproducible native fixture and demo

The repository contains an MIT/Apache-2.0 Rust VST3 test instrument. It is
built from source and never committed as a binary:

```sh
tools/build-vst3-fixture.sh
cargo run -p meldritch2 -- render \
  crates/meldritch2/examples/69-vst3-test-instrument.lua \
  /tmp/meldritch-vst3-demo.wav 4 --workers=2
cargo test -p meldritch2 --features vst3-integration-tests --test vst3_host
```

The integration test uses the actual VST3 ABI and verifies discovery, metadata,
class identity, creation/configuration/activation, sample-offset note delivery,
audible output, note-off, parameter enumeration and changes, state round-trip,
repeated destruction, invalid identity errors, unsupported block diagnostics,
Lua construction, offline rendering, and routing through Meldritch's existing
drive effect.

When Debian's `lsp-plugins-vst3` package is installed, the same suite also
discovers every class in LSP's multi-class bundle and processes real stereo
audio through LSP Compressor Stereo and LSP Limiter Stereo. This conditional
test covers effect buses, graph lowering, automation metadata, state, latency,
offline processing, and teardown without making LSP a CI prerequisite.

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

Serum 2.1.5 exposes `IUnitInfo`, but its root list is only 128 generic names
(`Prog 1` through `Prog 128`). Selecting programs 0, 1, and 127 leaves state
byte-identical and does not select factory content. `IProgramListData` provides
no usable factory data, and passing an installed `.SerumPreset` directly to
component `setState` is rejected. VST3 program selection, Xfer's native preset
browser, and captured VST3 state restoration are distinct mechanisms.

Authentic factory loading uses Serum's supported native editor. The vendored
host exposes one shared Linux `IRunLoop` from the factory host context (where
Serum queries it before class creation) and from `IPlugFrame`, embeds
`IPlugView` in X11, and services timers/file descriptors on the control thread.
The automated Pressure capture requires `xdotool` and `xwininfo` (Debian
packages `xdotool` and `x11-utils`). It verifies the active Serum parent and
embedded child before every keyboard sequence and waits one second between UI
steps. Create the Pressure bank with:

```sh
tools/capture-pressure-serum.sh /tmp/pressure-serum
cargo run -p meldritch2 --bin meldritch-vst3-state -- subset \
  /tmp/pressure-serum ~/.local/share/meldritch2/patches/serum2/pressure-system/bass \
  bass_reese bass_distorted bass_sub bass_rhythmic
```

Repeat `subset` for lead and drone IDs listed in
`examples/pressure-system-serum-capture.json`. The automation targets only the
active Serum XEmbed window, opens a fresh process per capture, and refuses
Init/duplicate processor state. State files remain in user data and out of Git.
The supplied coordinates target Serum 2.1.5 at its default editor scale; use
the interactive `meldritch-vst3-state capture` command if a future editor
layout or custom scaling moves those controls.

To capture the complete installed factory library into a resumable user-local
bank, run:

```sh
tools/capture-serum-factory.sh
```

The default destination is
`~/.local/share/meldritch2/patches/serum2/factory`. `--plan` prints the stable
ID, display name, browser search text, and source-relative path without opening
Serum. Existing state files are skipped, so an interrupted run can be resumed.
Use `--limit COUNT` to capture a bounded number of new presets in one session;
the manifest remains valid and the next run resumes after those states.
Each preset uses a fresh editor process because Serum 2.1.5 can retain the
previous search result when the embedded browser is reused programmatically.
The extractor enumerates filenames only; it loads every preset through Serum's
native editor, waits one second between UI operations, and captures opaque VST3
state. For performance use, create small role-specific banks with the `subset`
command: loading the entire factory bank as one Lua patch bank would construct
one resident Serum processor per preset and is intentionally not recommended.

Lua loads a validated bank directly:

```lua
local bass = instrument:vst3 {
  plugin = "56534558667350736572756D20320000",
  patch_bank = os.getenv("HOME") .. "/.local/share/meldritch2/patches/serum2/pressure-system/bass",
  initial_patch = "bass_reese",
}
```

`available_patches()` exposes IDs/names and an unavailable initial ID is an
error. Calling `load_patch(id_or_name)` during Lua graph authoring changes the
initial selection; during performance the authoritative preset selector is the
switching API. Banks preload one processor per patch, but only the selected
processor runs steady-state DSP. At a block-boundary switch, the outgoing
processor receives one final all-notes-off block and the incoming processor
reconstructs currently held notes from the graph inputs. Rapid requests reduce
to the latest selector value. This avoids state restoration on the audio thread
but does not promise click-free timbral transitions. Identity/load failures
abort startup rather than falling back to Init.

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
limited to 65 MiB including its envelope, and is never accepted from browser
commands. Opaque state can still be saved through `Machine::vst3_state`.

Geonkick 3.9 exposes no automatable VST3 parameters, so its synthesis controls
cannot be honestly mapped to Meldritch knobs yet. Kit design remains in
Geonkick's editor; Meldritch provides note velocity, lane mute, pattern, and
kit-state control. The TimeKit mapping used by example 74 is explicit: kick
24, snare 26, clap 27, alternate snare 28, closed hat 30, hat 32, open hat 34,
and shaker 58. These are preset mappings, not assumed General MIDI values.

## PULSE Drum Synth 0.1.0

Meldritch has been validated against the native Linux build from the official
[PULSE source](https://github.com/audiodestrukt/pulse-vst), upstream commit
`220db4657abe391edecece9df79c275b9491567d`. Build JUCE and PULSE into the
user-local VST3 directory:

```sh
git clone https://github.com/audiodestrukt/pulse-vst.git
git -C pulse-vst checkout 220db4657abe391edecece9df79c275b9491567d
git clone --depth 1 https://github.com/juce-framework/JUCE.git pulse-vst/JUCE
cmake -S pulse-vst -B pulse-build -DCMAKE_BUILD_TYPE=Release
cmake --build pulse-build --parallel 2
```

The upstream post-build step installs `PULSE Drum Synth.vst3` under
`$HOME/.vst3`. The discovered class is `ABCDEF019182FAEB506C7365506C6473`,
vendor `Pulse`, version `0.1.0`, category `Instrument|Synth`, with one stereo
output and one event input. It exposes 2,245 controller entries: 164 writable
part parameters plus bypass are automatable; 2,080 JUCE MIDI-controller mapping
entries are non-automatable. Meldritch only polls the writable automatable
subset in the audio callback.

PULSE accepts either channels 1–4 (one part per channel) or General MIDI notes
36/38/42/46 on channel 10 for kick, snare, hat, and percussion. Example 75 uses
the latter explicitly:

```lua
local pulse=instrument:vst3 {
  plugin="ABCDEF019182FAEB506C7365506C6473",
  midi_channels={10,10,10,10},
}
local kit=sequence:drum_kit {voices={
  {id="kick",note=36,channel=10}, {id="snare",note=38,channel=10},
  {id="hat",note=42,channel=10}, {id="percussion",note=46,channel=10},
}}
pulse:connect_drum_kit(kit)
```

PULSE source IDs such as `p1_syn_lvl` are not VST3 parameter numbers. Example
75 asserts the plugin-reported numeric IDs and binds those ordinary VST3
parameters to Meldritch controls. Values remain normalized 0–1; the
plugin-formatted `display` string provides its engineering representation.
Envelope editors target PULSE's own Env2/Env3 attack, decay, sustain, and
release parameters—Meldritch does not add another synthesis envelope.

Opaque state round-trips through the existing VST3 state interface. A trusted
startup state may be selected with `PULSE_STATE=/path/to/state` and is read on
the control thread under the 65 MiB state-file limit. Example 75 defaults to a
self-contained synthesis-only configuration, so it has no sample paths to
lose. Browser clients never submit filesystem paths.

Run the performance and conditional real-plugin test with:

```sh
cargo run -p meldritch2 -- perform crates/meldritch2/examples/75-pulse-performance.lua
MELDRITCH_TEST_PULSE=1 cargo test -p meldritch2 --features vst3-integration-tests --test vst3_host installed_pulse
```

## Current boundaries

- Native Linux `.vst3` bundles only. Wine and yabridge are not configured or
  detected.
- Instruments and single-main-bus stereo effects are supported. Mono effects,
  auxiliary inputs, external sidechains, multi-output effect routing, and
  automatic latency compensation are not yet supported.
- Main stereo output is supported. A plugin that cannot negotiate the requested
  configuration fails graph preparation; incompatible buffers are never
  silently processed.
- X11 editor embedding is available to the dedicated state-capture command;
  performance and render modes remain headless and do not expose general DAW
  editor management.
- Opaque VST3 state round-trips are supported, but vendor-native preset files
  require a plugin-supported host interface. Serum's native files are selected
  in its editor and recalled from identity-checked, user-local captured state.
- Hosting is in-process. Rust ownership and lifecycle errors are contained by
  the adapter, but a native plugin can still abort, corrupt memory, deadlock, or
  run unbounded work; none of those failures can be recovered safely in the
  current process. The Lua and graph contract does not expose native pointers,
  leaving room for a future process-isolated implementation without changing
  compositions.
