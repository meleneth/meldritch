# Lua APC color demo

Status: `compile`. Headless plan/protocol/negative tests pass. The physical
Control output accepted nine frames (720 messages) on Linux; visual confirmation
of the animation and palette is still pending.

This is a bounded, non-realtime LED diagnostic, not a musical project or live
state-feedback integration. Lua authors a color animation and leaves static
color hints for the [APC playground](https://github.com/meleneth/meldritch/tree/main/songs/examples/19-apc-mini-mk2-playground).
Grid pads support colors; the peripheral track buttons are red and scene buttons
are green hardware LEDs, so their programmable behavior is on/off/blink.

```sh
cargo run -- apc-lua-colors songs/examples/20-apc-lua-colors/colors.lua --dry-run
cargo run -- apc-lua-colors songs/examples/20-apc-lua-colors/colors.lua
```

The second command opens only the matching MIDI **output** Control port. It
does not start audio. Use Session mode. `--clear` clears LEDs after the animation;
without it, the final static playground hints remain. Run example 19 separately
to play music. These hints do not track current variation, mute or solo state.

## Lua API version 1

Return `{version = 1, frames = {...}}`. Each frame has `hold_ms`, a `pads` array
and a `buttons` array (both may be empty).

- A pad is `{cell = 0..63, color = 0..127, brightness = 0..6}`. Color 0 turns it
  off. Brightness defaults to 2 (50%); 6 is full brightness. Palette values are
  the documented Akai fixed palette, not arbitrary RGB tuples.
- A peripheral button is `{note = 100..107 or 112..119, mode = "off"|"on"|"blink"}`.
- At most 32 frames, 64 pads and 16 buttons per frame; duplicate identities
  within a frame are rejected. Holds are 0..1000 ms and total hold ≤10 seconds.
- Rust validates the entire plan before opening hardware. Source is limited to
  256 KiB, VM memory to 4 MiB and instructions to the existing Lua budget.
  No OS, filesystem, module loading, processes, raw MIDI or unbounded execution.
- The dry run prints the validated MIDI plan without opening a device.

## Executable acceptance

- The authored animation loads in the shared constrained VM and generates nine
  frames, each with 64 pad updates and 16 peripheral button updates.
- Wire messages have the documented channels, notes and palette velocities.
  Ordering is authored frame order, pads first, then buttons; hold follows sends.
- The final frame lights the playground's bound controls and clears unused pads.
- Invalid pad/color/brightness/button addresses, duplicates, unknown fields,
  excessive duration, malformed frames and unavailable Lua capabilities fail.
- All validation and dry-run behavior works without MIDI hardware.

Hardware acceptance requires observing animation/colors on the connected APC.
Sending messages successfully proves transport acceptance, not human-observed
color accuracy. Live authoritative LED feedback and reconnect restoration remain
deferred. Nothing is stable yet.

Protocol: [Akai APC Mini MK2 communication protocol v1.0](https://cdn.inmusicbrands.com/akai/attachments/APC%20mini%20mk2%20-%20Communication%20Protocol%20-%20v1.0.pdf), pages 2–6.
