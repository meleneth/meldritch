# Lua-programmable HTTP and WebSocket server

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

Each committed observed change increments one process-local `u64` revision and
broadcasts a complete `state.snapshot`. A new or reconnected client subscribes
before its initial snapshot is read, ignores older revisions, and receives a
fresh snapshot if it falls behind. Full snapshots deliberately avoid patch-gap
and initial-snapshot races in this milestone.

HTTP clients may read `/api/state` and post JSON `{ "value": ... }` to
`/api/control/:name`. WebSocket clients send:

```json
{"type":"control.set","name":"bass_filter","value":0.72}
```

The built-in browser fetches its WebSocket path, renders the published control
metadata, sends structured changes, and always replaces its display with the
latest authoritative snapshot. It has no song, synthesizer, or plugin-specific
model. Its controls have touch-sized targets and responsive layout.

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
does not provide TLS, durable sessions, incremental state patches, database
storage, or process isolation for Lua. A CPU-bound Lua handler stalls other Lua
control work until it returns, though audio continues independently. Choose a
handler timeout that reports this condition and keep handlers short.

Run the actual transport and retained-Lua integration tests with:

```sh
cargo test -p meldritch2 --test web_server
```
