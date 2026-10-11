# Nothing is stable yet

MELDRITCH is in massive development. **Nothing is stable yet:** public APIs,
project formats, CLI commands, controller mappings, UI layouts, sound and
persistence behavior may change. There is no production-readiness guarantee.

The goal is coherence under composition: existing supported projects remain
loadable, typed commands govern mutations, interfaces observe authoritative
state, and the realtime audio boundary remains protected. Compatibility tests
cover the supported historical corpus; they do not make every design fixture an
implemented feature or promise compatibility with arbitrary future versions.

## Read the evidence

- [Example capabilities](../examples/capabilities) identify each implementation stage.
- [Progress and remaining tasks](../TODO) are the active implementation queue.
- [Lua API and verification record](../CONTROL_PLANE) separate headless acceptance
  from deferred production playback, hardware and recording integration.
- [APC playground acceptance](../examples/apc-playground) records its bindings,
  executable checks and remaining physical smoke tests.
- [Testing strategy](../TESTING_STRATEGY) describes intended test layers. Some
  architecture documents describe design direction rather than completed code.

`accept` means automated tests prove the stated example scope. It does not mean
all controllers, all platforms or every realtime condition have been tested.
`play` means rendering or playback works within the documented supported path.

## Current gaps

Production Lua song-host integration, physical Stream Deck transport, live
MicroLab voice input, APC LED feedback and soft takeover, exact/quantized session
replay, exhaustive session capture and full all-parameters inspection remain
unfinished. Future multicore scheduling requires correctness tests and measured
bottlenecks; the control-plane work does not replace the renderer.
