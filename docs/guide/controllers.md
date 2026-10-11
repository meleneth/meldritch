# Controllers

All controllers request typed application commands. Musical state belongs to the
application; a controller is an input and presentation surface.

| Interface | Current scope |
| --- | --- |
| TUI | Production song host, curated controls, lane/page state and session capture |
| LaunchControl XL | Existing script-declared MIDI controls, buttons, pages and modifiers |
| APC Mini MK2 | [Runnable four-lane playground](../examples/apc-playground) on the existing MIDI song host; Linux discovery and real fader messages observed |
| MicroLab mk3 | Protocol investigation and generic note/pitch/modulation codecs; live voice integration deferred |
| Stream Deck XL | 32-button simulated logical surface, Lua pages and semantic presentation; physical HID/image transport deferred |

The [Lua example](../examples/lua-control-plane) proves shared authoritative
state on simulated surfaces, including reconnect restoration. This does not
establish physical reconnect behavior in the production MIDI host.

APC LED feedback and page-aware fader soft takeover are not wired into the
playground. Absolute faders can jump on first movement. Hardware-specific
protocol references and transport limitations are recorded in the
[control-plane note](../CONTROL_PLANE).

The separate [Lua color demo](../examples/apc-colors) sends a bounded palette
animation and leaves static playground hints on the real APC. This is physical
LED output, while live state-driven LED feedback remains unfinished.
