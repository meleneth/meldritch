# Controllers

All controllers request typed application commands. Musical state belongs to the
application; a controller is an input and presentation surface.

| Interface | Current scope |
| --- | --- |
| TUI | Production song host, curated controls, lane/page state and session capture |
| LaunchControl XL | Existing script-declared MIDI controls, buttons, pages and modifiers |
| APC Mini MK2 | [Four-lane playground](../examples/apc-playground) and optional [Lua widgets with state-driven RGB feedback](../examples/lua-apc-performance); production wiring accepted headlessly, hardware/listening acceptance pending |
| MicroLab mk3 | Protocol investigation and generic note/pitch/modulation codecs; live voice integration deferred |
| Stream Deck XL | 32-button simulated logical surface, Lua pages and semantic presentation; physical HID/image transport deferred |

The [Lua example](../examples/lua-control-plane) proves shared authoritative
state on simulated surfaces, including reconnect restoration. This does not
establish hardware-tested reconnect behavior. The production MIDI host now
includes [paced input/output recovery](../examples/midi-reconnect), stale-input
rejection, pickup rearming and RGB restoration, accepted with simulated failures.
Physical unplug/replug and color/listening confirmation remain open.

The Lua performance example wires live APC RGB feedback and enables
[fader pickup](../examples/midi-soft-takeover). Existing TOML projects can opt
in with `--soft-takeover`; their default input behavior remains direct.
Pickup waits for the physical fader to reach the current value after a page,
layer or external value change. Its behavior is accepted headlessly; physical
interaction and listening remain unverified. Hardware-specific
protocol references and transport limitations are recorded in the
[control-plane note](../CONTROL_PLANE).

The separate [Lua color demo](../examples/apc-colors) sends a bounded palette
animation and leaves static playground hints on the real APC. This is physical
LED output. The Lua performance example separately derives live colors from
authoritative lane and transport state; physical color observation is pending.
