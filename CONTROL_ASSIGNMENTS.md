# Editable faders and track mixer

> **Meldritch 1 archive:** This document records completed research and its
> historical plans. Meldritch 2 is the active project; this is not
> its current architecture, acceptance queue, or compatibility contract.

Run the Lua-authored APC Mini MK2 example:

```sh
cargo run -- tui-song songs/examples/28-author-fader-mappings --lua main.lua
```

Press **Ctrl-M** to inspect assignments. Up/down selects a declared fader,
left/right selects a target, **Enter** assigns it, **Delete** removes its
assignment, and PgUp/PgDown changes the authored control page. **Ctrl-S** saves
the current mappings and supported source values as Lua; Esc closes the panel.
Playback continues. The panel shows target identity, range/step, authoritative
value, last raw observation, pickup state and audio publication status.

The example's Lua policy also maps APC pads 0..8 to cycling the corresponding
fader's target, pads 16..24 to removing its assignment, button 106 to page
switching and button 115 to saving. These are MIDI note addresses. Channels in
Lua are 1-based; raw MIDI decoding is 0-based. Physical behavior, orientation,
feedback and listening still require testing on the connected APC.

## Targets and ownership

An `assignments` table in the version-1 control composition declares pages, a
bounded catalog of existing numeric targets and physical fader identities:

```lua
assignments = {
  pages = { "main" },
  targets = {
    { id="tone", label="Cutoff",
      target={kind="synth",id="bass",module="filter",parameter="cutoff_hz"},
      minimum=80, maximum=6000, step=10 },
    { id="pan", label="Track pan",
      target={kind="track",id="bass",parameter="pan"},
      minimum=-1, maximum=1, step=.01 },
  },
  faders = {
    {id="fader-a",device="apc-mini",channel=1,cc=48,page="main",target="tone"},
  },
}
```

Supported identities are source-owned synth low-pass `cutoff_hz` (>0 Hz) and
`resonance` (0..1); track `volume` (0..2 linear gain) and `pan` (-1..1); and the
existing tempo-delay `mix` (0..1) and `feedback` (0..0.99). DSP edits require
the executable single-track delay path. Automation-owned parameters, nonnumeric
parameters, missing modules/tracks and unsupported render graphs reject.
The catalog can choose a narrower physical range and step within those existing
parameter bounds. Mapping uses the same affine, nearest-step, clamped policy
as existing curated controls; absolute `edit_parameter` requests use the
underlying parameter's bounds rather than the physical fader's step.

Lua declares hardware addresses, available targets and controller policy. Rust
validates and owns mutable assignments and the single validated authoring song
source. Lua reads `view.assignments` and `view.controls`; these projections are
recursively read-only. The synth inspector and observing faders read that same
source. Supported commands are `assign_fader` (`target` omitted removes),
`assignment_page`, `edit_parameter` (`edit={target=...,value=...}`) and
`save_assignments`. The catalog and device declarations are composition source;
this workflow edits assignments among those declarations.

Device IDs must resolve to song-declared MIDI devices. Duplicate device/channel/
CC assignments on the same page, duplicate catalog identities and conflicting
TOML MIDI declarations reject. Different faders may observe the same parameter.
Synthetic control IDs beginning `assignment:` are reserved. Remapping, page
changes, source edits on another surface and disconnection rearm existing soft
takeover. Until near-target/crossing acquisition, observations change telemetry
only. A target outside its declared physical range cannot acquire pickup;
choose a compatible catalog range. Accepted edits create typed source commands,
track dirty ranges, captured facts and existing background rerender requests.
All parameter edits conservatively invalidate the affected track's full finite
render horizon, including the rendered delay tail. The realtime callback only
consumes the completed atomic audio snapshot.

## Mixer and stereo law

Track volume multiplies audio after track DSP and before existing song-mix
normalization. Unity is 1, zero silences that track. Existing lane-level live
overrides remain an additional performance multiplier.

Pan is linear stereo balance: `left = 1-max(pan,0)` and
`right = 1+min(pan,0)`. Center has unity gains (0 dB); hard left/right mutes
the opposite channel. A mono track with authored pan duplicates into stereo
before balancing. Native stereo preserves its separate left/right content;
balance does not sum channels or transfer content to the other side. Mono
tracks coexist with stereo by duplication in the mixed output. Untouched tracks
default to volume 1 and absent pan, retaining historical mono samples, channel
counts and semantic fingerprints. Explicit `pan=0` opts into stereo. The TUI
opens a stereo device stream when its Lua catalog exposes pan on a mono song.
No new buses or routing graph are introduced.

## Save and reload

Saving creates a unique `authored/controls-*.lua`, separate from recorded
`performances/*.mlperformance`. It never overwrites TOML, another export or a
journal. Version-1 `kind="controls"` contains the TOML base fingerprint, full
ordered assignment declarations and absolute catalog parameter values that
differ from that base. Explicit center pan is retained when it promotes mono.
Declaration order, stable IDs, quoted strings and round-trippable f64 numbers
make repeated exports deterministic. Runtime page, selection, physical pickup,
transport and journal facts are transient. Uncatalogued source edits reject
export rather than being discarded. Save failure retains unsaved edits.

Reload the saved mappings and values with the original controller composition:

```sh
cargo run -- tui-song songs/examples/28-author-fader-mappings --lua main.lua --controls edited-controls.lua
cargo run -- validate-song songs/examples/28-author-fader-mappings --controls edited-controls.lua
cargo run -- render-controls songs/examples/28-author-fader-mappings --controls edited-controls.lua --output /tmp/controls.wav
```

Reload is confined to the song root, verifies the unchanged TOML base, applies
typed validation and compiles through the existing song compiler. Rendering
the saved file needs no controller script, hardware or session replay. The
TUI uses the saved assignment definition rather than replacing it with the
composition's initial assignments. This is a supported control/mixer snapshot
over an existing song. [Example 29](MUSICAL_AUTHORING.md) now composes the
selected synth graph/pattern, declared mixer values and bindings with F6.

Headless acceptance: `cargo test -p meldritch-control --test assignments` and
`cargo run -p meldritch-control --example assignments`. See
[example 28](https://github.com/meleneth/meldritch/blob/main/songs/examples/28-author-fader-mappings/ACCEPTANCE.md) for exact
observable behavior and outstanding physical acceptance.
