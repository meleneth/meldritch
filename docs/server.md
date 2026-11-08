# Lua-programmable HTTP and WebSocket server

For the full three-Serum, PULSE, and LSP multi-performer composition, see
[Pressure System Live](pressure-system-live.md). It uses this same projected
control-plane protocol; the browser has no composition-specific components.

Meldritch 2 embeds an asynchronous application server for live performance.
Rust owns HTTP parsing, WebSocket framing, sockets, limits, connection liveness,
JSON transport, and shutdown. The composition's retained Lua state owns route
handlers and musical behavior. Published performance controls are projections
of existing compiled `ControlHandle`s, so network, MIDI, hardware surfaces,
the TUI, automation, native DSP, and VST3 modules continue to meet at the same
control boundary.

## Thread and ownership model

A composition loaded for `perform` already retains one Lua state in
`EventOwner`. The TUI thread is its owner and was already responsible for
serial Lua event callbacks. The server extends that owner loop instead of
creating another Lua VM:

```text
Axum/Tokio HTTP and WebSocket tasks
             |
      bounded FIFO (default 128)
             |
UI-owner EventOwner + the retained Lua state
             |
       existing ControlHandle
             |
audio callback's existing parameter boundary
```

At most 64 queued network events are serviced on one owner-loop pass. Lua is
never called concurrently. The CPAL callback does not parse HTTP, serialize
JSON, execute Lua, wait for the network, or acquire a server lock. Conversely,
Lua callbacks are not invoked while holding an audio graph lock.

Accepted queue entries have FIFO mutation order. An HTTP caller that
disconnects or times out before dispatch begins is skipped and cannot mutate
state. Once dispatch begins, the Lua operation completes atomically even if
the caller's timeout expires; its mutation and resulting revision remain
authoritative. This makes cancellation behavior deterministic rather than
interrupting Lua at an arbitrary instruction.

## Enabling the server

Declare one server in the composition:

```lua
local server = meldritch.server {
  host = "127.0.0.1",       -- the default
  port = 8080,              -- use 0 in tests for an ephemeral port
  queue_capacity = 128,
  body_limit = 64 * 1024,
  message_limit = 64 * 1024,
  max_connections = 64,
  timeout_ms = 2000,
  anonymous_role = "conductor", -- suitable only for loopback demos
}
```

The server starts with `meldritch2 perform`, after the graph is compiled and
its real control handles are available. The listening URL is printed before
the TUI enters its loop. Quitting the performance closes WebSockets, stops the
Tokio runtime, joins the server thread, and releases the port. A bind conflict
is reported with its address and operating-system error. Offline rendering and
the one-shot graph inspection command do not start networking.

Run the included demo and open the printed URL in two browsers:

```sh
cargo run -p meldritch2 -- perform \
  crates/meldritch2/examples/70-network-performance.lua
```

Both browsers discover the composition controls. Pressing **DROP** toggles the
gain of an actual square-wave layer, publishes a new revision, and updates both
browsers. It needs no plugin, MIDI device, or proprietary asset.

## Lua HTTP routes

`get`, `post`, `put`, `patch`, and `delete` register Lua functions. Colon path
parameters are accepted in authored routes and normalized to Axum's matcher:

```lua
server:get("/api/status/:section", function(request)
  return {
    status = "running",
    section = request.params.section,
    filter = request.query.filter,
    request_id = request.id,
  }
end)

server:post("/api/set", function(request)
  performance:set("bass_filter", request.json.value)
  return { value = performance:value("bass_filter") }, 201,
    { ["x-meldritch"] = "lua" }
end)
```

The request table contains `id`, `method`, `route`, `path`, decoded `params`,
decoded `query`, selected non-sensitive `headers`, parsed `json`, UTF-8-lossy
`raw`, `remote`, and the server-resolved `role`. Authorization headers and raw
socket objects are never exposed to Lua.

A returned Lua table or scalar becomes JSON with status 200. A string becomes
plain text and `nil` becomes an empty response. The optional second and third
return values set the numeric status and string header table. Invalid JSON is
rejected before Lua with 400; oversized bodies with 413; full queues with 503;
and dispatch timeouts with 504. Missing routes and unsupported methods use
normal HTTP 404 and 405 responses. Lua exceptions return a generic 500 to the
client while a sanitized request-specific error is written locally.

## WebSockets

Lua registers application callbacks but never sees frames or sockets:

```lua
server:websocket("/performance", {
  connected = function(client)
    return { type = "welcome", client = client.id } -- targeted response
  end,
  message = function(client, message)
    if message.type == "ping" then return { type = "pong" } end
  end,
  disconnected = function(client)
    -- release composition-owned performer state here
  end,
})
```

Client records contain a monotonically assigned connection `id`, remote
address, optional performer name, and server-resolved role. Callback return
values are targeted to that connection. Authoritative state publications are
broadcast to every connection. Incoming/outgoing queues and the broadcast
ring are bounded. A lagging receiver is sent the current full snapshot instead
of replaying an unbounded history. Ping frames are sent every 20 seconds;
malformed JSON receives an application error, and protocol message limits are
enforced by the WebSocket implementation.

## Authoritative performance controls

Performance controls name an existing Meldritch control rather than creating a
remote-only parameter:

```lua
local performance = server:performance { name = "Pressure System" }

performance:control("bass_filter", {
  type = "continuous", label = "Bass Filter", group = "Bass",
  target = "bass_cutoff", min = 0, max = 1,
  changed = function(value, previous)
    -- optional composition behavior, executed before publication
  end,
})
performance:control("drum_mute", {
  type = "toggle", target = "drum_mute", min = 0, max = 1,
})
performance:control("section", {
  type = "choice", target = "section", min = 0, max = 3,
  choices = { "intro", "build", "drop", "outro" },
})
performance:control("DROP", {
  type = "trigger", target = "drop_layer", min = 0, max = 1,
})
```

Continuous, toggle, trigger, and choice controls publish through the target's
existing range-checked handle. Trigger controls toggle their target endpoints,
which makes the effect persistent and visible to every client. Lua can call
`performance:set(name, value)` and `performance:value(name)` from any retained
handler. Changes published by MIDI, the TUI, or another control surface are
observed from the same handle on the next owner-loop pass.

Each committed observed change increments one process-local `u64` revision.
Every server run also has an `epoch`; clients discard stale revision state when
the epoch changes. Initial/recovery messages contain the typed plane schema,
while ordinary `state.updated` messages contain values only. Schema mutations
increment `schema_revision` and send a fresh projected schema. A client that
detects a revision gap reconnects for a complete snapshot instead of guessing
at missing state. A lagging socket is likewise sent the current full snapshot.

HTTP clients may read `/api/state` and post JSON `{ "value": ... }` to
`/api/control/:name`. WebSocket clients send:

```json
{"type":"control.set","name":"bass_filter","value":0.72}
```

The built-in Vue 3 browser fetches its WebSocket path and Pinia maintains the
authoritative projection, connection state, revision/epoch tracking, pending
interactions, and reconnect recovery. Components contain no song, synthesizer,
or plugin-specific behavior.

## Lua control planes

A plane is a role-projected view over existing musical targets; it is not an
instrument, sequencer, or browser-owned control system. Planes may combine
targets from multiple subsystems and two planes may point at the same target.

```lua
local rhythm = performance:plane {
  id = "rhythm", label = "Rhythm",
  roles = { "drummer", "leader" },
  read_only_roles = { "observer" },
}
rhythm:step_sequencer {
  id = "kick_pattern", label = "Kick", steps = 16,
  track = kick_track, pattern = pattern,
}
rhythm:trigger {
  id = "fill", label = "FILL",
  triggered = function() pattern:steps("kick", fill_steps) end,
}

local synthesis = performance:plane {
  id = "synthesis", label = "Synthesizer",
  roles = { "synth", "leader" },
  read_only_roles = { "observer" },
}
synthesis:fader {
  id = "filter_cutoff", label = "Cutoff", target = "filter_cutoff",
  min = 20, max = 20000, scale = "log", unit = "Hz",
}
synthesis:knob {
  id = "resonance", label = "Resonance", target = "resonance",
  min = 0, max = 1,
}
```

IDs are stable wire identities and must be nonempty and unique in their
scope. Faders and knobs bind existing compiled control IDs, so changes from
MIDI, automation, Lua, another browser, or the TUI converge on the same value.
The schema-v2 library supports linear/logarithmic faders and knobs, a
single-row 16-step editor, event-semantic triggers, a piano roll, ADSR editor,
and pattern selector. Trigger publications carry an increasing serial; they
are not transient booleans.

The step editor mutates the sequencer's authored pattern on the Lua/control
thread. The existing audio sequencer adopts its pending pattern revision at a
scheduling boundary without rebuilding the graph. The browser never schedules
notes or owns a musical clock. Current step observations are sampled on the
owner loop and published only when the step changes.

Plane visibility is projected by Rust before data reaches a socket. `roles`
may see and mutate a plane; `read_only_roles` may see it but all commands are
still rejected server-side. `performance:assign_plane(id, spec)` can replace
those assignments at runtime; the next owner-loop pass publishes a revisioned
schema. Hiding or disabling a Vue component is only presentation, never the
authorization boundary.

WebSocket plane commands are deliberately small and typed:

```json
{"type":"step.toggle","plane":"rhythm","control":"kick_pattern","step":7}
{"type":"trigger.fire","plane":"rhythm","control":"fill"}
```

Range controls continue to use `control.set` with their stable control ID.

### Notes, envelopes, and patterns

One logical note track is a bounded group of one to four existing sequencer
tracks. This supplies chords without adding another scheduler:

```lua
local bass = sequence:note_track("Bass", { voices = 3 })
local verse = sequence:pattern("Verse")
verse:notes(bass, {
  { id=1, step=1, pitch="C2", velocity=110, duration=3 },
  { id=2, step=5, pitch=43,   velocity=96,  duration=2 },
}, 16)

plane:piano_roll {
  id="bass_notes", label="Bass", track=bass, steps=16,
  pitch_min=36, pitch_max=60,
}
plane:pattern_selector {
  id="patterns", label="Patterns", sequencer=sequence, boundary="bar",
}
```

Lua steps are one-based; wire and Rust steps are zero-based. Pitch accepts MIDI
0–127 or note names such as `C4`, `F#3`, and `Bb2` (`C4` is MIDI 60). Velocity
is 1–127 and duration is a positive count of sequencer steps wholly contained
in the pattern. IDs are positive and unique within a logical track/pattern.
Rests are simply steps without notes. Multiple events may share a step.

The compiler assigns notes deterministically to the lowest free voice lane. If
a later note exceeds configured polyphony it steals the oldest sounding lane
(lowest lane breaks ties) and closes that note's gate at the replacement step.
A same-step chord larger than the configured voice count is rejected instead
of silently dropping notes. At most four voices and 256 note events are
accepted per logical track. The existing sequencer publishes note-on/off edges;
stop, restart, and pattern changes clear gates and scheduled work. A pattern
change inserts a single low sample before a logical note lane's first event so
ADSR modules retrigger deterministically.

ADSR controls bind four ordinary compiled control targets, so hardware, Lua,
automation, and browsers share the same parameter identities:

```lua
plane:adsr {
  id="amp_envelope", label="Amplitude ADSR",
  attack="amp_attack", decay="amp_decay",
  sustain="amp_sustain", release="amp_release",
}
```

For plugin-owned envelopes whose VST3 parameters are normalized, set
`normalized=true`. The same ADSR component then projects four linear 0–1
parameters and does not imply that Meldritch owns or adds an envelope:

```lua
plane:adsr {id="kick_envelope",label="Kick Envelope",normalized=true,
  attack="kick_a",decay="kick_d",sustain="kick_s",release="kick_r"}
```

Attack, decay, and release are seconds (logarithmic UI ranges); sustain is
linear 0–1. Note-on begins attack, then decay reaches sustain. Note-off latches
a release slope from the envelope's current level, including during attack or
decay, so the configured release time is meaningful and click-resistant.
Changes to attack/decay/sustain affect subsequent samples; an in-progress
release retains its latched slope until the next gate edge.

Pattern selection accepts `immediate`, `beat`, `bar`, or `cycle`. “Immediate”
means the next sequencer step boundary, not an arbitrary audio sample. The
server publishes `pending` separately and changes `active` only after the audio
sequencer commits the switch. Piano-roll edits are prepared on the serialized
Lua owner, then the complete immutable pattern revision is adopted at a step
boundary. Concurrent accepted edits therefore use FIFO server-dispatch order;
there is no partially visible pattern and no browser clock.

The corresponding commands are:

```json
{"type":"note.create","plane":"rhythm","control":"bass_notes","pattern":0,"step":4,"pitch":48,"velocity":100,"duration":2}
{"type":"note.update","plane":"rhythm","control":"bass_notes","pattern":0,"id":7,"step":4,"pitch":50,"velocity":92,"duration":3}
{"type":"note.delete","plane":"rhythm","control":"bass_notes","pattern":0,"id":7}
{"type":"pattern.select","plane":"rhythm","control":"patterns","pattern":1,"boundary":"bar"}
{"type":"adsr.set","plane":"synthesis","control":"amp_envelope","parameter":"attack","value":0.02}
```

Plugins may expose a finite, Lua-configured performance bank through the
schema-v3 preset selector:

```lua
plane:preset_selector {
  id="serum_preset", label="Performance Patch",
  target="serum_preset", presets={"Clean Pulse","Glass Motion","Driven Bloom"},
}
```

The target is an existing compiled control; clients can select only one of the
configured names and cannot submit paths. Rust revalidates the plane role and
index/name before publishing the control. The snapshot reports `active`,
`active_name`, `pending`, and `failed`. Authentic Serum banks preload every
captured patch as an isolated processor while running DSP only for the selected
one, so selection is a bounded graph parameter change plus a one-block note
handoff rather than filesystem access or plugin state mutation on the audio
callback.

Rust revalidates every field and role independently of the Vue interface.
Unauthorized clients neither receive hidden plane state nor mutate it.

### Drum grid

`sequence:drum_kit` groups up to eight ordinary sequencer tracks and binds
their explicit MIDI pitches to one VST3 instance. It does not introduce a
second clock or scheduler:

```lua
local kit = sequence:drum_kit {
  name="TimeKit",
  voices={
    {id="kick",label="Kick",note=36,channel=10},
    {id="snare",label="Snare",note=38,channel=10},
    {id="closed_hat",label="Closed Hat",note=42,channel=10},
  },
}
pattern:drums(kit, {
  kick={{step=1,velocity=127},{step=5,velocity=110}},
  snare={{step=5,velocity=118},{step=13,velocity=124}},
},16)
local plugin=instrument:vst3 {plugin=class_id,midi_channels={10,10,10}}
plugin:connect_drum_kit(kit)
plane:drum_grid {id="drums",label="Drums",kit=kit,steps=16}
```

The Vue drum grid projects authoritative hits, MIDI velocity, lane mute,
active pattern, and the coarse engine playhead. Clicking an empty cell adds a
hit; selecting one exposes velocity and a touch-friendly Remove button;
double-click also removes it. Mute suppresses
future note-ons for only that lane and lowers its gate at the next engine
frame. Rust authorizes and serializes every edit through the existing pattern
mutation boundary. The browser never schedules notes.

Run `examples/74-geonkick-performance.lua` with `meldritch2 perform`, open
`http://127.0.0.1:8080`, and use `drummer-demo-token`, `serum-demo-token`,
`leader-demo-token`, or `observer-demo-token`. `GEONKICK_VST3` and
`GEONKICK_KIT` override the user-local installation paths. Drummer and Serum
roles receive independent planes; leader receives both plus arrangement;
observer receives read-only projections.

Example `75-pulse-performance.lua` applies the same generic surface to four
PULSE Drum Synth parts. It exposes the drum grid, per-hit velocity and mute,
bar-quantized patterns, 16 curated synthesis knobs, and four plugin-owned ADSR
panels. The `drummer` and `synth` roles remain isolated; `leader` receives both
and `observer` receives read-only projections. Serum 2 is used when available,
with an explicit native Plaits fallback when it is absent:

```sh
cargo run -p meldritch2 -- perform crates/meldritch2/examples/75-pulse-performance.lua
```

Open `http://127.0.0.1:8080` and use `drummer-demo-token`,
`synth-demo-token`, `leader-demo-token`, or `observer-demo-token`. PULSE
parameter commands are ordinary authorized `control.set` or `adsr.set`
mutations and arrive at the plugin at the next audio block boundary. Pattern
edits still adopt atomically at sequencer step boundaries; the browser owns no
clock.

Set `PULSE_DISABLE_SERUM=1` to force the native Plaits synthesis fallback.
This is useful for a PULSE-only session and also avoids Serum's factory-context
`IRunLoop` diagnostic; the diagnostic does not indicate a failed headless audio
instance.

## Vue and Pinia workflow

The frontend source is isolated in `crates/meldritch2/frontend`. For local UI
development, start the composition on port 8080 and run:

```sh
cd crates/meldritch2/frontend
npm install
npm run dev
```

Vite proxies `/api` and `/performance`. Production does not require Node: run
`npm run build` to generate `crates/meldritch2/web/dist`, which Rust embeds with
`include_str!`. Run `npm test` for the lightweight native-Vue Vitest suite and
`npm run typecheck` for Vue/TypeScript validation. The control registry selects
components solely from the versioned server schema, so adding
another composition plane requires Lua changes only.

The store distinguishes optimistic pending values from confirmed values.
Fader and knob drags are rate-limited to roughly 30 updates/second and always
send the final value. Pointer, touch, and keyboard input use native accessible
controls; responsive CSS reduces the step grid to four columns on narrow
screens.

## Two-performer demonstration

The complete native-DSP demonstration needs no plugins or MIDI hardware:

```sh
cargo run -p meldritch2 -- perform \
  crates/meldritch2/examples/71-control-planes.lua
```

Open the printed address in separate tabs/devices and join with
`drummer-demo-token`, `synth-demo-token`, `leader-demo-token`, or
`observer-demo-token`. The drummer edits the real kick pattern and fires a
fill; the synth performer changes an audible SVF cutoff and resonance; the
leader receives both planes; the observer receives read-only projections.
Reconnection restores the current schema, pattern, controls, trigger serials,
and revision.

The pitched-note demonstration adds two named patterns, chords, editable bass
and lead piano rolls, amplitude/filter ADSRs, and quantized selection:

```sh
cargo run -p meldritch2 -- perform \
  crates/meldritch2/examples/72-note-performance.lua
```

Use the same four demo credentials. `drummer-demo-token` receives the kick,
bass, fill and pattern surface; `synth-demo-token` receives the chord roll,
both envelopes and filter controls; `leader-demo-token` receives both planes;
`observer-demo-token` receives both read-only. An offline reference render is:

```sh
cargo run -p meldritch2 -- render \
  crates/meldritch2/examples/72-note-performance.lua \
  /tmp/meldritch-72-note-performance.wav 8
```

With native Serum 2 installed, the multi-user plugin demonstration is:

```sh
cargo run -p meldritch2 -- perform crates/meldritch2/examples/73-serum-performance.lua
```

Join with `drummer-demo-token`, `serum-demo-token`,
`leader-demo-token`, or `observer-demo-token`. The drummer receives only the
kick/fill plane; the Serum performer receives the piano roll, quantized pattern
selector, curated parameter surface, and safe preloaded patch selector; the
leader receives every plane; the observer receives read-only projections.
All note scheduling remains in the Rust sequencer and all plugin changes flow
through compiled control handles. See `docs/vst3.md` for Serum installation,
factory-preset capture workflow, state-bank limitations, and offline rendering.

## Performer roles and credentials

Roles are enforced by the Lua owner before an existing control handle is
published:

```lua
performance:role("bass", { controls = { "bass_filter", "bass_resonance" } })
performance:role("conductor", { controls = "*" })
performance:role("audience", { controls = {} })

local token = assert(os.getenv("MELDRITCH_CONDUCTOR_TOKEN"), "token required")
server:credential(token, "conductor")
```

Lua may change an existing role during a performance with
`performance:assign("bass", { "bass_filter" })` or assign all controls with
`performance:assign("conductor", "*")`. The shared assignment is consulted on
every mutation, so connected clients gain or lose authority immediately.

Credentials must be at least 12 characters. HTTP uses an `Authorization:
Bearer ...` header. The browser's performer button sends the credential in a
WebSocket `authenticate` message; it is never placed in a URL or log. A claimed
role in arbitrary JSON has no effect. Credentials only map to roles declared
by the composition.

Loopback compositions may choose an `anonymous_role`. Non-loopback/LAN binds
are rejected unless credentials exist, anonymous control is disabled, and at
least one `allowed_origins` entry is configured. This is a small capability
system, not identity proof or a comprehensive account service.

## Security and deployment assumptions

- Binding defaults to `127.0.0.1`; LAN exposure is always explicit.
- Request bodies, WebSocket messages, active connections, dispatch queues, and
  broadcast history are bounded.
- Axum/Hyper and Tungstenite perform HTTP and WebSocket protocol parsing.
- Network input can call only registered handlers and structured control
  commands. There is no remote Lua evaluation or filesystem route.
- Browser origins are restricted to the configured local origin by default;
  LAN operation requires an explicit allowlist.
- Handler errors do not return Lua traces or local paths to clients.
- Plain HTTP has no TLS. Use only a trusted LAN or place Meldritch behind a
  correctly configured TLS reverse proxy. The server does not implement custom
  cryptography.
- Browser credentials live in session storage for the tab lifetime. This is
  adequate for the initial trusted-LAN workflow, not hostile shared devices.

## Diagnostics and limits

`EventOwner::server_diagnostics` exposes active HTTP requests and WebSockets,
queue depth, dispatch count, handler errors, broadcasts, rejected work,
dropped/lagged messages, current revision, and the latest dispatch/handler
duration. These are atomics read outside the audio path; no separate telemetry
stack is introduced.

The server currently runs only with the interactive `perform` owner loop. It
does not provide TLS, durable sessions, arbitrary piano-roll step counts,
cross-cycle notes, per-note automation, a browser arrangement editor, database
storage, or process isolation for Lua. Logical note metadata is composition
state and is not yet included in the older canonical sequencer project-save
format. A CPU-bound Lua handler stalls other Lua control work until
it returns, though audio continues independently. Choose a handler timeout
that reports this condition and keep handlers short.

Run the actual transport and retained-Lua integration tests with:

```sh
cargo test -p meldritch2 --test web_server
cd crates/meldritch2/frontend && npm test && npm run typecheck && npm run build
```
