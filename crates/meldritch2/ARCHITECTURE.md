# Meldritch 2

Meldritch 2 composes musical machines from independently defined Rust modules.
Its first slices are an offline mono `f64` graph executor, isolated from the
prototype.
Meldritch 1 is archived completed research. This crate has no dependency on it
and no backward-compatibility obligation to its formats, APIs, or behavior.

Each `src/units/*.rs` file owns one module type's ports, parameters, and
processing function. `contract.rs` defines their shared shape and explicit
registry. A module contract declares its Output role; compilation selects that
role rather than recognizing a module type string.
Input ports declare fan-in and a required or neutral disconnected value;
output ports declare only a name and signal domain. Their distinct Rust types
keep input-only policy out of source-port contracts.
Graph instances have unique string IDs. A port is an instance ID plus a named,
directional endpoint. Graphs retain explicit connections and authored
normalizations separately. A normalization is a fallback edge selected at
compile time only when its destination has no explicit cable. Every authored
normalization is validated, including overridden ones; multiple normalizations
for one input are ambiguous. Selected edges then use the existing input fan-in,
required-input, and cycle checks. `Machine` exposes authored normalizations and
resolved effective connections separately; sample processing sees only compiled
input slots. Parameters are identified
by `(instance ID, parameter name)` from the same contract used by Lua discovery,
validation, and lowering. Authored values live in `Graph`; live overrides are
published through per-revision `ParameterHandle`s. `Machine::process` snapshots
all overrides once at buffer entry, so each buffer uses one effective value per
parameter. Clearing an override restores the authored value.
Lua can declare named controls bound to these same parameter IDs with a
presentation range. Rust validates the binding against the module contract;
`ControlHandle` publishes through the existing parameter cell. A control's
range limits that control's gestures, not DSP parameter validity. Handles are
scoped to one compiled revision.
`Panel` turns declared controls into selectable rows and bounded gestures.
`perform` renders those rows in a small terminal view while CPAL runs; it has
no fixed instrument knobs. This first view tracks the opened graph revision
and does not refresh itself after a structural graph replacement.
Lua may declare root-graph screens with `graph:screen(id, title)`. A screen
collects `screen:label(text)`, `screen:control(label, control_id)`, and
`screen:link(label, screen_id)` widgets. Compilation validates text, unique
screen IDs, control references, and link targets before accepting the graph;
subpatches cannot declare screens. `perform` displays one screen at a time.
Up/Down selects control or link widgets, Left/Right and Home/End publish through
the existing control handles, Enter opens a selected link, and Tab/Shift+Tab
cycles screens. Each screen retains its selection. Graphs without screens keep
the original control list. Lua constructs the screen model before compilation;
terminal gestures use only Rust state and compiled control handles.
Lua may also bind channel 1..16 MIDI CC numbers to a named control. Graph
validation checks those declarations; `MidiRouter` precomputes a bounded raw
CC lookup and publishes through `ControlHandle`. DSP modules never see MIDI
bytes. The router belongs to one compiled graph revision. MIDI device opening
is explicit through `midi-ports` and `perform --midi=INDEX`. The input callback
only decodes and publishes CC values; the terminal view reads those values from
the same parameter cells. Physical CC gestures remain unverified.
Lua may also map a MIDI channel's note messages to named pitch and gate
controls. The MIDI adapter keeps a bounded last-pressed held-key stack per
mapped channel, converts the active note to Hertz, and publishes 0/1 gate
through those same control cells. Note-on with zero velocity releases a key;
releasing the top key restores the prior held pitch without dropping gate.
The controls must cover all MIDI note frequencies and 0..1 gate. This is a
buffer-boundary mono route. Each note channel owns a shared publication epoch;
the audio callback makes one bounded pair read at buffer entry. If a note
write overlaps that read, it reuses the prior complete pitch/gate pair for
that buffer. Note writers serialize on their bounded held-key state lock; the
audio path reads only atomics and its own cached pair. Other control gestures
on those parameters remain independent publications. This immediate router
still serves direct/headless callers; `perform` now uses the timed queue for
declared notes. An optional fourth `midi_notes` control resolves unit-interval
velocity to an independent graph parameter. The immediate router snapshots
pitch, Gate, and velocity under one note epoch. The timed router carries
velocity through fixed queue slots and restores each held key's velocity on
fallback. DSP uses velocity only where the author explicitly cables that
control's Constant output; three-argument bindings retain their prior sound.

`TimedNotePlan` is a separate prepared timeline for absolute `u64` frame
positions. The control side resolves at most 64 typed channel/pitch/gate
states against one machine revision. Each bounded render call snapshots
ordinary parameters, reapplies the held timed note states, then renders up to
1024 frames in segments split at event frames. Events at the same frame apply
in declaration order before that frame's sample. Rendering makes no source
lookups, allocations, locks, or retries. This prepared path is also checked
headlessly; the live queue and device callback path are described below. A plan owns
one sequential render; ordinary `Machine::process`
starts a new parameter snapshot outside that plan.
`TimedNotePlan::prepare_with_velocity` accepts explicit unit-interval velocity
alongside each note state. It resolves the optional velocity target once and
reapplies held pitch, Gate, and velocity at every render boundary. The original
`prepare` API supplies unity velocity. Both prepared paths validate before
rendering and share the same bounded sample path.
`graph:midi_poly(channel, voices)` declares two to four ordered voice bindings, each
with named pitch, Gate, and velocity controls. `PolyNotePlan` prepares raw
note-on/off events on that channel into resolved per-voice states. It assigns
the lowest free voice, updates a repeated held key on its existing voice,
and releases only the matching key. Note-on with zero velocity is a release.
A key beyond the declared voice count rejects the candidate plan before
rendering by default. An explicit `"steal_oldest"` third argument replaces
the voice whose held key had the earliest last note-on. A repeated note-on
refreshes that key's age; a later note-off for a stolen key is ignored.
The plan reapplies held voice
states after each buffer snapshot and uses the existing bounded sample path.
The timed MIDI router also assigns incoming notes to those voices and
publishes their resolved pitch, Gate, and velocity through the bounded live
queue. Repeated keys update their existing voice. With the default policy,
a full bank reports an error without changing held notes or the frame clock.
Channel reset, session
teardown, and queue-overflow recovery close all mapped Gates and discard older
queued states. `perform` opens this path for a polyphonic graph. Four-voice
physical listening remains unverified.
The pinned `midir` callback supplies elapsed microseconds from an arbitrary
origin fixed for one input connection. `MidiFrameClock` maps that timestamp to
absolute audio frames using a caller-provided first-unrendered frame and a
bounded lead. It anchors the first event, uses `u128` integer arithmetic and
floor rounding for later events, clamps late events to the first unrendered
frame, and re-anchors if MIDI time moves backward. A regressing audio cursor
or frame overflow rejects without changing the clock. `live_note_queue` adds
a fixed 64-state single-writer/single-reader handoff. Its producer validates
pitches against resolved MIDI note targets and publishes absolute frames through
atomic slots, clamping stale or reordered frames within each channel. Its
consumer holds at most 64 pending states in fixed storage and applies them by
frame, with arrival order breaking same-frame ties. The 64-state limit includes
deferred future states. Per-event reset generations invalidate old states even
when another channel advances past them. The consumer keeps per-channel held
state across buffers and publishes its first unrendered frame. Events that arrive
after a block has passed their frame apply at the next block start. A producer
passes that cursor to `MidiFrameClock` in the MIDI callback. The callback
keeps a fixed held-key priority stack per mapped channel and commits its
state only after a note event enters the queue. CC messages still publish
named controls. `DeviceOutput` renders queued notes through its bounded 256
frame scratch chunks with one parameter snapshot per host callback. A new
graph revision disables the old queue and continues ordinary rendering until
the output is reopened; the audio callback retains the old queue allocation.
The MIDI session shows clock/queue failures in the terminal and reports them
after exit.
When `perform` has a note-mapped graph and no external MIDI input, the TUI
owns that same timed-note producer. Its one-octave press-to-latch row selects
the lowest mapped channel, uses note off/on for pitch changes, and Space closes
the Gate. The TUI displays the active note, and teardown publishes the existing
all-notes-off reset. The control side timestamps gestures; the audio callback
keeps the same bounded note consumer.
The device MIDI callback uses a recovery route for queue overflow: a dropped
note change releases that channel through the reset marker, clears its held
stack, and records the error. The direct router API retains transactional
Full/retry behavior for callers that can retry. Other channels keep their
pending events and active Gates.
Timed MIDI CC 120/123 with value zero releases the addressed channel; MIDI
system reset releases every mapped channel. The producer publishes a separate
per-channel reset marker even when the 64-event ring is full. At the next
chunk the consumer closes its active Gate and skips queued states older than
that marker. The router clears its held-key stack, and later notes remain
eligible for scheduling. Closing a MIDI input session first stops its callback,
then retrieves the router and publishes the same release markers. Physical
latency, unplug detection, and listening remain open.

Lua's `meldritch.graph()` API constructs a candidate graph. `graph:module(id,
type, values)` returns handles with contract-derived ports; `graph:connect(src,
dst)` records explicit edges and `graph:normalize(src, dst)` records fallback
edges. Rust receives only graph data, validates names, values,
fan-in, and cycles, then derives a topological schedule and compiles slots and
module operations. `Host::submit` replaces the accepted machine only after full
compilation. A rejected candidate leaves it usable.
`meldritch.subpatch(name)` defines a reusable graph with explicitly exported
ports via `subpatch:input(name, destination)` and
`subpatch:output(name, source)`. `graph:instance(id, definition)` returns port
handles usable by the same `connect` and `normalize` calls. Compilation first
expands nested instances into namespaced primitive modules, ordinary cables,
and per-instance normalizations. The existing compiler then validates and
schedules that graph. Each expanded primitive owns its own DSP state; the
callback performs no expansion. Subpatches contain modules, cables, and nested
instances; parent graphs own controls and MIDI bindings.
An exported input may name multiple destinations in one declaration:
`subpatch:input(name, first, second, ...)`. Each parent cable or normalization
to that input expands to every destination. Duplicate destinations reject;
the ordinary destination contracts resolve fan-in and normalizations.
`subpatch:parameter(name, module_or_instance, parameter)` exports a parameter
through the subpatch interface. A parent binds it with
`graph:control(id, instance, exported_name, range)`; nested subpatches may
forward an exported parameter. Expansion resolves each binding to its
namespaced primitive parameter before ordinary control and MIDI validation.
Invalid exports reject a candidate without replacing the accepted machine.
`graph:instance(id, definition, values)` may supply authored values keyed by
exported parameter name. Expansion writes each value to that instance's
namespaced primitive module; an outer instance value takes precedence over a
nested definition's value. The ordinary module contract validates the final
authored value. Live overrides still clear back to that authored value.
Each declared output port has its own compiled sample slot. A unit's process
function writes its declared outputs in contract order; cables read the named
source port's slot. The terminal Output role has a private sample slot but no
connectable output port. `Mirror` demonstrates two distinct outputs from one
module instance.

`LiveControl::submit` compiles a candidate before acquiring a short handoff
lock. `LiveRuntime::process` uses a nonblocking `try_lock` at buffer entry,
swaps in at most one complete candidate, and places the old machine in a
retirement slot. The control side reclaims and drops that old machine. Only one
candidate may be pending; while the handoff is busy, processing continues with
the current graph. This is a bounded in-process publication boundary, not a
physical audio host.

`Machine::process` fills a caller-owned buffer with no allocation, parsing,
validation, compilation, Lua, UI, or file access. Compiled operations read
inputs in contract port order from preallocated scratch space. Each operation
owns a fixed, contract-declared state vector allocated at compile time. `Sine`
uses one state value for phase and the machine's explicit sample rate; it keeps
phase across buffers and uses a published frequency at the next buffer entry.
Its optional `fm_hz` port adds an audio-rate Hertz offset to each sample's phase
advance; the current output sample is read before advancing phase. Self-feedback
still rejects as a cycle.
Device callbacks may exceed the fixed 256-frame scratch chunk. The callback
activates at most one pending revision and snapshots parameters once before
rendering all its chunks, so publication during a callback waits for the next
callback boundary.
`Mix` sums two named inputs; sources may fan out. Current ports are mono audio
at one sample per frame. `Multiply` multiplies two independent audio signals
per sample, supporting explicit audio-rate modulation through ordinary cables.
`gain.input` currently declares summed fan-in; its connected sources execute
in stable module-ID order. `gain.mod` permits one optional cable, using 1 when
disconnected; its audio-rate signal multiplies the authored/live Gain value
without changing parameter identity. Other inputs permit one cable. Cycles,
disconnected required inputs, and multiple Output instances are rejected. Feedback,
other domains, and block DSP remain future work. `Affine` demonstrates two
independently identified parameters on one module; the executor passes values
in contract order from preallocated scratch space. Parameter publication uses one
atomic `u64` per parameter; validation and handle lookup happen off the process
path. Handles from replaced revisions refer only to their old cells. The live
handoff uses a nonblocking lock attempt on the processing side; a busy handoff
delays activation to a later buffer.
`Lag` is a stateful signal processor with a contract-owned response range of
0..1. Authored values, live overrides, and declared control ranges are checked
against that one bound before publication; its state continues across buffers.
`Switch` chooses between two signals per sample from an independent selector
signal and a live threshold parameter; it uses the same contract and schedule
mechanism as the other units.
`SampleHold` detects upward threshold crossings on an audio-rate trigger input,
captures its separate signal input, and retains the value in compiled instance
state across buffers. This is a signal processor, not a general event domain.
`Envelope` follows a thresholded audio-rate Gate with separate attack and
release responses in 0..1; its compiled state continues across buffers.
Contracts expose domain and fan-in explicitly so these restrictions are visible.

`compile_at_rate` validates the sample rate; `compile` uses 48 kHz. Live graph
revisions inherit the active runtime's sample rate and start with fresh DSP
state. `DeviceOutput` opens CPAL's default device on explicit `play`, compiles
at its sample rate, and copies bounded mono `f64` chunks into interleaved device
channels. `devices` lists output endpoints and `--device=INDEX` selects one for
`play` or `perform`; this is host policy, outside the graph. The stream error
handler atomically records a bounded error category and count without formatting
or allocation; the first category is described outside the callback. Run
`cargo run -p meldritch2 -- crates/meldritch2/examples/08-sine.lua`,
`cargo run -p meldritch2 -- play crates/meldritch2/examples/08-sine.lua 3`,
`cargo run -p meldritch2 -- perform crates/meldritch2/examples/09-controls.lua`,
`cargo run -p meldritch2 -- midi-ports`,
`cargo run -p meldritch2 -- perform crates/meldritch2/examples/10-midi-control.lua --midi=INDEX`,
and `cargo test -p meldritch2`. Host smoke tests found underruns on the default
and PulseAudio routes, a busy Audio Expansion Card endpoint, and no stream
errors on the discard endpoint. The APC mini mk2 Control input opened in a
terminal session with the discard output. Physical CC gestures, listening, and
deadline reliability remain unverified. See `examples/ACCEPTANCE.md` for the corpus.
