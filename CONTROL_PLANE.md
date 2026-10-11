# Lua control plane, API version 1

This milestone implements the headless vertical slice in
`songs/examples/18-lua-control-plane`. Run it with:

```sh
cargo run -p meldritch-control --example headless
cargo run -p meldritch-control --example headless -- \
  songs/examples/18-lua-control-plane apc:0 deck:31 deck:disconnect apc:0 deck:connect
cargo test -p meldritch-control
```

The runner validates and renders the example oscillator, executes actual app
commands, prints an inspectable trace and semantic surface frames, and writes a
unique UTC timestamped session under `/tmp/meldritch-lua-headless/performances`.
It does not open an audio device. It constructs the existing app's backing
pattern/coordinator for headless mode-control commands; it is not a general
song-playback host. Production `tui-song` remains the existing TOML/MIDI path.

## Reuse and boundaries

The separate [APC Lua color diagnostic](https://github.com/meleneth/meldritch/blob/main/songs/examples/20-apc-lua-colors/ACCEPTANCE.md)
uses the same constrained VM to author bounded LED frames. Run
`cargo run -- apc-lua-colors songs/examples/20-apc-lua-colors/colors.lua`, or add
`--dry-run` for a hardware-free wire preview. Rust validates all pad/button
addresses, palette values, brightness, frame counts and holds before opening
the matching MIDI output. Lua never receives a raw MIDI or filesystem capability.
This presentation-only diagnostic does not mutate app state or enter audio;
it leaves static hints, not authoritative live feedback. Its API is documented
in the example's acceptance file.

`meldritch-dsl::load_song_directory` remains the semantic validator and reference
resolver. `LuaProject::load(root, source)` is an explicit composition adapter:
Lua chooses a song directory within the supplied root, then the current typed
loader resolves all synths, DSP, patterns and samples unchanged. There is no
parallel music model, schema validator or graph compiler.

`AppController::dispatch` remains the only command processor. Lua returns
versioned request descriptions which translate to existing `AppCommand`
variants. Validation failures never produce accepted journal facts. The
existing app owns transport, mode, controls, pages, lanes and command history;
each callback reads a detached, read-only projection of that state. The Lua HSM
owns presentation-page state only. It cannot own a second transport or musical
pattern state. An existing frontend can dispatch commands as before, then call
`ControlPlane::refresh` to update connected surfaces from the app view.

The new crate depends on app/DSL, and has no dependency edge from audio back to
Lua. The VM is single-owner and does not enable mlua's `send` feature. Hosts
must run all its operations on a non-realtime control thread. Audio callbacks,
immutable `PublishedAudio`, atomic snapshot publication, renderer and current
LaunchControl mapping are unchanged.

There are two integration gaps in existing boundaries. Song rerender requests
and publication live in private CLI `SongRerenderWorker` code; app lane/curated
control results alone do not schedule that work. The new trace states this
explicitly. The CLI `PerformanceSessionCapture` also privately stores debug
command strings and `u32` frames. This slice adds a typed `u64` Lua command
outbox and replay journal without pretending to migrate old capture files.
Extracting reusable CLI host/capture adapters is subsequent work. Only mode
commands are exercised across surfaces in this example; they need no rerender.

## Minimal Lua API

`main.lua` returns a table with `version = 1`, a stable `id`, confined `song`
directory, `initial` state, `surfaces`, `widgets` and `states`.

Surfaces have an `id` and a `kind`: `apc_mini_mk2` (64 grid cells),
`stream_deck_xl` (32 LCD keys), or `tui` (64 logical text cells). Device ids,
widget ids and state ids must be nonempty ASCII alphanumeric/underscore/hyphen
names of at most 64 bytes. They do not depend on enumeration order.

A widget declares `id`, `dependencies`, `interact(state)` and
`present(state, surface_kind, page)`. Allowed dependencies are `mode`, `page`,
`controls` and `lanes`. This version refreshes at explicit command/page/host
boundaries rather than maintaining a subscription graph. `interact` returns
one command request. `present` returns `{label, color, style}`. Semantically
unchanged frames are suppressed; outputs replace a complete surface frame so
removed cells are cleared by the transport. The mode widget is reusable on all
three surfaces, with LCD, LED and text presentations of authoritative app mode.
There is no pattern-launch widget in this milestone.

The authorized state projection exposes `mode`, `frame`, `page`,
`controls[id]` and `lanes[id].muted/variation`. These tables have read-only
proxies; modifying a local script variable never edits app state. Lua receives
no app pointer or audio publication handle.

Supported command request tables:

| name | Fields | Existing command |
| --- | --- | --- |
| `set_mode` | `mode = "performance" / "all_parameters"` | `SetCockpitMode` |
| `play`, `stop`, `rewind` | none | existing transport commands |
| `select_page` | `id` | `SelectPerformancePage` |
| `select_lane_variation` | `lane`, `variation` | `SelectLaneVariation` |
| `toggle_lane_mute` | `lane` | `ToggleLaneMute` |
| `set_control` | `id`, finite `value` in 0..1 | `SetCuratedControlNormalized` |

Unknown request names/fields are errors. IDs and parameter values are checked
again by the existing app command processor. Returning a request is not proof
that the app accepted it or that queued audio transport work has executed.

## Hierarchical execution and dispatch

States declare `id`, optional `parent`, allowed `transitions`, optional
`handle(state, event)`, `entry(state)`, `exit(state)` and `bindings`. A binding
has a surface id, zero-based cell and exactly one widget id or named event.
All destinations/parents/bindings are validated before accepting a project.
Parent cycles, duplicate ids/cells, missing widgets and out-of-range cells fail.

Handlers return nil to bubble to the parent; otherwise they return a decision:
`{command = request}`, `{transition = id}`, `{reject = reason}`, or a command
plus transition. Guards are ordinary Lua conditions reading app projections.
An undeclared transition or rejected command prevents the transition. At most
one command is allowed per decision, avoiding ambiguous partially committed
command batches. Lifecycle callbacks return optional diagnostic text and cannot
dispatch commands. Callback errors are traced; an explicit transition still
commits and reconstructs its projections. There is no rollback of accepted
musical commands because a presentation/lifecycle callback failed.

The HSM interpreter is Rust-owned, with up to 64 declared states. Handling is
child-first; exit walks leaf upward to the least common ancestor, then entry
walks downward. Shared ancestors do not re-enter on sibling transitions. Self
transitions retain the current path. `home`/`detail` share `online`; detail entry
is guarded and its back event is handled by the parent. This is a hierarchical
machine rather than a renamed toggle. State-local arbitrary data, orthogonal
regions, timers and history states are deliberately not part of API v1.

External inputs enter a 64-event FIFO. Each event retains source and `u64`
control-frame time. A pump processes at most 64 events, runs handlers to
completion, applies a typed command, records its accepted result, then queues a
`command_accepted` domain follow-up. Follow-ups run before the next external
input, preserving causal frame order. They are never dispatched recursively.
Chains exceeding 16 causal levels are explicitly rejected and traced.

Queue overflow returns an error to the producer, which must retain/retry the
input or report its rejection. A disconnected device's input is rejected;
disconnect never changes authoritative musical state. Reconnect reconstructs
the current complete surface frame, including clearing stale cells.

Lua event frame values use exact decimal strings, because Lua's signed integers
cannot represent all Rust `u64` frames. Reserved domain facts cannot be injected
as external named inputs or button bindings.

The accepted-command outbox has capacity 256. The dispatcher reserves outbox
and domain-queue capacity before applying commands; a full outbox rejects the
command with an explicit persist/drain/retry diagnostic. `take_journal` moves
ownership to the host, which must persist it before dropping it. The ring trace
holds 256 records with 512-byte details and reports its dropped-record count. Trace and semantic
surface frames are refreshable telemetry; accepted commands are lossless facts.
There are no handler-issued recursive subscriptions or redraw subscriptions to
leak. Dropping a project/plane releases all callback handles with its VM.

## Sandbox, persistence and compatibility

Vendored Lua 5.4 uses mlua 0.12.2 (`lua54`, `vendored`, `serialize`). No installed
Lua is required. Only base/table/string/UTF8 facilities are loaded; OS, IO,
package, debug and coroutine libraries are unavailable. File/process/module
loaders, bytecode loading, catchable-budget retry helpers and `rawset` are
removed. VM memory is limited to 4 MiB, source to 256 KiB, definitions/bindings
to 64 per collection, and callbacks/load to approximately 101,000 Lua
instructions. This is a constrained capability environment, not an OS process
isolation boundary; the instruction hook cannot interrupt a long native C
library operation. Lua memory/CPU faults remain on the control thread.

`save` stores exact authored Lua text, API version, stable project identity,
song semantic fingerprint and a source-sensitive control fingerprint in
deterministic human-readable TOML. `restore`
creates a fresh VM and revalidates the underlying song; changed identity or
fingerprint is rejected. The control fingerprint uses stable FNV-1a over API
version, exact Lua source and typed song fingerprint; it is not an authentication
hash. Functions are recompiled from source, never serialized
as bytecode. This saves project definitions, not active HSM runtime state.

Accepted-command journals use typed requests, sequence, source, `u64` frame and
result. They encode/decode deterministically. `write_session` uses create-new
UTC timestamped `.mlperformance` files under `performances/`, writes a
`kind = "session"` header and nested `lua_control` journal, and syncs the file.
It never overwrites an earlier session. It is a final-journal writer, not a
crash-tolerant checkpoint service; a failed write leaves an incomplete reserved
file and the caller retains its journal for retry.

Replay checks song/project identity and control-source fingerprint, then applies the typed requests through the
real app. The caller must provide equivalent initial app state and advance the
audio engine to the returned frames for timed playback. Headless tests prove
command/state equivalence, not sample-identical performance replay, wall-clock
reconstruction or quantized scheduling. Presentation-page transitions and
device lifecycle traces are not musical replay commands in this API.

TOML remains canonical for song graphs. Introducing Lua does not change default
entrypoint precedence or rewrite historical fixtures. The pinned compatibility
suite covers ten supported song directories through their specific supported
compilers, finite/non-silent playback and split-range equality. Seven older
single-file projects independently parse/validate, compile their relation
graphs and render both event clicks and their real WAV fixtures. Unsupported
design fixtures are not advertised as supported projects. Existing renderer
tests retain more detailed musical assertions. Future migrations must add
immutable persistent fixtures and explicit version adapters to this corpus.

## HSM/library investigation

[statig](https://github.com/mdeloof/statig) provides nested superstates, child-to-
parent bubbling (`Super`), explicit transitions, state-local storage, and entry/
exit actions with shared-ancestor ordering. Guards can be expressed in state
handlers. Its synchronous API is directly testable and a suitable candidate for
future fixed Rust engine/device lifecycle machines. It does not automatically
provide our lossless outbox, input ordering, causal limits or rejected-command
diagnostics. Its state types/macros are compiled in Rust and cannot directly
instantiate a changing project-defined Lua state graph.

The existing app launcher already owns quantized musical transitions; replacing
it would discard functioning semantics. Flat match/enum handling remains enough
for this slice's two-state connection observation. The bounded runtime page
interpreter implements only the validated example-required Lua hierarchy.
Statig is not added as an unused dependency, and no other FSM framework is
introduced. A future reconnect/discovery/backoff lifecycle should evaluate
statig with executable transport failure cases rather than turn every scalar
and toggle into a machine.

## Hardware evidence and deliberately missing transports

- [Akai APC Mini MK2 protocol v1.0](https://cdn.inmusicbrands.com/akai/attachments/APC%20mini%20mk2%20-%20Communication%20Protocol%20-%20v1.0.pdf): USB port 0 Session grid notes 0..63, button notes,
  CC 0x30..0x38 faders, channel/port distinctions, and 8-byte-per-pad RGB SysEx
  payloads. Codecs implement these and normalized faders. Soft takeover is a
  resettable crossing/near-target foundation; the host must reset it when a
  bank/context or another surface changes the target. It is not wired to an
  attached fader or Lua binding in this example.
- [Arturia MicroLab mk3 manual 1.0.0](https://dl.arturia.net/products/microlab-mk3/manual/microlab-mk3_Manual_1_0_0_EN.pdf): velocity-sensitive keyboard, pitch/modulation strips and configurable
  sustain/expression pedal controls. The decoder preserves note velocity,
  release (including note-on-zero), channel, 14-bit pitch and caller-configured
  CC mappings. It does not guess the user's configuration or implement live
  keyboard voice allocation.
- [Stream Deck XL implementation evidence](https://github.com/abcminiuser/python-elgato-streamdeck/blob/master/src/StreamDeck/Devices/StreamDeckXL.py): 32 keys in an 8x4 grid, 96x96 JPEG key images and chunked HID image reports.
  This slice models 32 keys, complete semantic presentation, pages, simulated
  disconnect/reconnect and restoration. Physical discovery, HID reports,
  JPEG rendering, image updates and reconnect polling are deferred. Neither USB
  nor image encoding is in domain widget logic.

No new physical hardware has been tested. Existing LaunchControl MIDI input,
mapping and song playback tests remain unchanged.

## Multicore/render audit, without a scheduler rewrite

The existing `RenderWorkerPool` has a configurable worker count, priority heap,
deduplicated wanted artifact keys, and sleeping condition-variable workers.
Queue/cache metadata use mutexes outside realtime. The coordinator submits
chunk/future jobs and atomically publishes ready chunks; its worker diagnostics
count queued/active/completed jobs, not measured utilization or deadline slack.
It does not currently execute a general dependency-aware DSP DAG scheduler.

The CLI `SongRerenderWorker` is one worker with latest-request replacement and
an unbounded completion channel. A rerender rebuilds the full song frame range
from zero; obsolete in-flight work cannot be cancelled. In `song_render`, the
mixed track sum is serial, and oscillator/envelope/filter/delay paths preroll
from frame zero to each range's end. No persistent stateful-DSP checkpoint cache
avoids this repeated history. Sample playback already clips overlapping events
to the requested range. Polyphonic notes have independent voice state but are
rendered serially. These are observable structural bottlenecks, not measured
claims about this machine's core utilization or throughput.

Parallel independent tracks/voices and DSP checkpoints are promising follow-up
work. They must preserve frame-boundary state, ordered `f64` summation,
probability commitments, semantic fingerprints and existing split-range tests.
Worker completion order must not become floating-point reduction order. Larger
worker counts may increase lock contention, preroll duplication, allocations
and cache/memory traffic. Offline throughput and realtime deadline compliance
need separate reproducible benchmarks. This milestone changes neither worker
count nor scheduler and makes no new performance/deadline guarantee.

## Verification record

Verified on Rust 1.99.0, Linux. Tests also assert that the embedded VM reports
`Lua 5.4`. Cargo required permission to populate the configured registry cache;
subsequent checks ran offline.

| Command | Actual result |
| --- | --- |
| `cargo test --workspace --offline` | exit 0; 312 test executions, 0 failures (CLI tests run for both CLI targets) |
| `cargo test -p meldritch-control --offline` | exit 0; 11 acceptance tests and 2 compatibility tests |
| `cargo clippy -p meldritch-control --all-targets --no-deps --offline -- -D warnings` | exit 0 |
| `cargo fmt --all -- --check` | exit 0 |
| `git diff --check` | exit 0 |
| `cargo run -q --offline -- validate-song songs/examples/09-curated-performance-controls` | exit 0; fingerprint `41472b38b65b60ed` |
| `cargo run -q -p meldritch-control --example headless --offline -- songs/examples/18-lua-control-plane apc:0 deck:31 deck:disconnect apc:0 deck:connect` | exit 0; typed commands, hierarchical page change, disconnect/reconnect restoration, trace and unique session file |

The workspace pass was followed by focused control tests after the final trace
typing/frame refinements. Clippy without `--no-deps` also inspects existing
dependencies and fails on five pre-existing DSL `collapsible_if` /
`if_same_then_else` findings. Those unrelated files were not changed; the new
crate is strict-Clippy clean.

No historical fixtures, LaunchControl code, DSP code or callback/publication
code were changed. No functional compatibility regressions were observed in
these tests. No device/realtime soak or throughput measurements were performed.
Production Lua song playback, physical transports, complete session fidelity
and crash-tolerant checkpointing remain partial/deferred as detailed above.
