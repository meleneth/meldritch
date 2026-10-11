# Editable faders and basic track mixer

Status: `accept` for the automated headless contract below. Physical acceptance
is pending and is a separate checklist. See [the guide](../../../CONTROL_ASSIGNMENTS.md)
and [shared rules](../../../AUTHORING.md#acceptance-execution-contract).

The fixture is an existing modular eight-voice synth and mono tempo delay, with
track volume 1 and explicitly authored center pan. `main.lua` declares an APC
Mini MK2, two pages of nine physical faders and a catalog of cutoff, resonance,
track volume, track pan, delay mix and feedback. No controller policy lives in
the musical model. The target/device catalog is source-authored; assignments
within it can be changed or removed while playback continues.

## Executable headless contract

Run `cargo test -p meldritch-control --test assignments` and
`cargo run -p meldritch-control --example assignments`. The latter executes
`scripts/compose.faders` through real MIDI bytes/queue ownership, existing pickup,
typed commands, session capture, background compilation and atomic publication,
and prints the deterministic `edited-controls.lua` source. Tests use temporary
copies; nothing records into this checked-in example.

- Waiting observations create no command, dirty range, render generation or
  musical fact. Near-target/crossing acquires pickup; accepted movements create
  exactly one typed parameter fact, including no-op movements.
- The script finishes at cutoff 6000 Hz, resonance .5, volume .5, hard-right
  pan and delay mix 1. It records eleven accepted parameter observations and
  requests seven changed audio generations. The first main-page assignment is
  removed. Source edits dirty the affected track's complete finite render range.
- Published samples equal independently compiled final source samples, with
  silent left and finite audible right. The existing callback consumes the
  published generation while transport stays playing. No alternate renderer.
- Both observing surface projections agree; native Ctrl-M UI shows current
  target, range/step, authoritative value, raw observation, pickup and known
  pending/published status. Keyboard arrows/Enter/Delete/page/save/close execute
  typed assignment commands. Script pads cycle/remove targets and switch pages.
- Remapping, page away-and-back, another fader/source editor's change and
  disconnect rearm pickup. Removed assignments consume observations without
  musical mutation. Two faders can observe one source parameter unambiguously.
- Every catalog parameter changes audio independently. Minimum, midpoint and
  maximum values preserve source fingerprint, mappings, f64 value and exact
  rendered audio through deterministic Lua save/reload. Partial renders concatenate
  identically to full stereo renders. Mono volume 1 leaves historical samples
  and channels intact; zero silences; mixed tracks retain other audible lanes.
- Mixer law is unity-center linear balance: L gain `1-max(pan,0)`, R gain
  `1+min(pan,0)`, multiplied by source volume after DSP and before existing mix
  normalization. Authored pan duplicates mono; native stereo retains its separate
  channels. Unit tests verify center, partial balance and hard pan directly.
- Saving creates unique readable version-1 `kind="controls"` Lua under
  `authored/`, carrying stable identities, declarations and changed absolute
  catalog values over an unchanged fingerprinted TOML base. Reload restores
  assignments instead of reseeding them from the controller composition.
  Repeated exports are byte-identical. TOML, prior exports and existing journals
  remain unchanged. Save failure retains unsaved source and mappings.
- `invalid/*.lua` requests reject missing fader/target/parameter, incompatible
  waveform, invalid pan range and wrong value type. Additional tests reject
  duplicate same-context CC, missing device/page, descending/nonfinite mapping
  ranges, wrong export version, nonconfined paths and duplicate declarations.
  Rejection preserves source, mappings and command history. Read-only Lua nested
  assignment mutations reject without creating another state owner.

Independent acceptance commands:

```sh
cargo run -- validate-song songs/examples/28-author-fader-mappings --controls edited-controls.lua
cargo run -- render-controls songs/examples/28-author-fader-mappings --controls edited-controls.lua --output /tmp/controls.wav
cargo run -- tui-song songs/examples/28-author-fader-mappings --lua main.lua --controls edited-controls.lua
```

## Physical acceptance — pending

On the connected APC, verify fader channel/CC addresses, no jump before pickup,
assignment cycle/remove controls, page switching and pickup rearming, coherent
LED feedback, audible cutoff/resonance/delay and volume changes, hard-left/right
and centered stereo output, uninterrupted transport and saved-project reload.
No physical fader, LED or listening result is claimed by headless tests.
