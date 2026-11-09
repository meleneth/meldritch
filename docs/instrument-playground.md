# Instrument Playground

`82-instrument-playground.lua` is a standalone Lua-authored performance for exploring two PULSE drum machines and four Serum 2 instruments. It does not import or modify the quine groovebox or Pressure System composition.

## Launch

Install Serum 2 at `~/.vst3/Serum2.vst3`, PULSE Drum Synth at `~/.vst3/PULSE Drum Synth.vst3`, and place the converted Serum catalog at `~/.local/share/meldritch2/patches/serum2/factory`. Then run:

```sh
bin/instrument-playground
```

Open `http://127.0.0.1:8080`, authenticate with `instrument-playground`, and start the transport. Override the credential with `MELDRITCH_PLAYGROUND_TOKEN`, the catalog with `MELDRITCH_SERUM_CATALOG`, and device names with `MICROLAB_MIDI` or `APC_MINI_MIDI`.

The six planes contain independent mute, solo, level, pattern, and preset controls. PULSE planes use 32-step four-lane drum grids with velocity editing. Serum planes use piano rolls; the lead's fourth voice accepts live MicroLab notes through Meldritch's existing live arpeggiator path. The Stream Deck page exposes the six instrument mutes, while APC and keyboard behavior continues through the shared device infrastructure and clean shutdown path.

## Preset browser and synchronization

The reusable `preset_selector` schema accepts either a small authored `presets` list or a VST3 `catalog`. Catalog entries have stable IDs, display names, banks, and categories. Display names need not be unique. The Vue browser searches names, filters bank/category, supports arrow/Enter navigation and previous/next actions, and renders at most 40 rows at once.

Selection is not optimistic. A browser command resolves a stable ID on the retained Rust/Lua application thread and publishes the numeric selector. A bounded worker reads and validates the selected state, constructs the replacement processor, and sends it to the audio runtime. The audio callback only polls bounded channels and swaps at a block boundary; it performs no filesystem access or state deserialization. Old processors are retired on the worker. Held gate inputs reconstruct notes on the replacement and the discarded processor cannot retain hanging notes. Rapid requests are coalesced and stale completed loads are retired instead of becoming active. Rust publishes `requested`, `pending`, authoritative `active`, and failure state back to every browser.

The selector is an ordinary authoritative graph control and the loaded processor participates in existing VST3 state capture. A composition reload restores the authored `initial_preset`; runtime browser selections are retained by host state/control capture where that workflow supports runtime control persistence. There is not yet a general standalone session-file command that writes live edits back into this example Lua file.

## Serum inventory

Meldritch reads `manifest.json`, validates every state document against the installed plugin class/vendor/name/version, and exposes every successfully converted entry. It never builds a hardcoded Serum list. Bank/category metadata is used when present; the current converter manifest lacks it, so `Factory` is the bank and category is conservatively derived from the stable `factory_<category>_...` ID.

At implementation time the local Serum 2.1.5 manifest contains 364 successfully converted factory states. The upstream native inventory is larger: the current scraper is blocked by another preset filename containing a single quote. Those unconverted files are intentionally not advertised. Fixing/resuming the scraper and updating the manifest automatically expands all four browsers.

The starting sounds are real converted states: `BA - Gnarly Reese`, `PD - Analog Silk Sweep`, `LD - Classic Techno Lead`, and `ARP - Aardvark`.

## PULSE banks

Installed PULSE 0.1.0 reports one root program with an empty name, no usable `IProgramListData`, and no program-change parameter. Therefore Meldritch does not claim native factory presets. Upstream and installed metadata agree on version 0.1.0 and on APVTS state serialization, including sample paths.

The playground supplies two Meldritch-managed parameter banks. Each preset is a complete synthesis-only configuration of kick/snare/hat/percussion frequency, source level, noise/tone/filter, volume, and envelope decay. Both sample-slot levels are forced to zero, so these starter kits have no missing-sample portability dependency. The 808-inspired bank contains Deep 808, Tight 808, Distorted 808, Soft 808, and Long Decay 808. The 909-inspired bank contains Classic 909-ish, Punchy 909, Industrial 909, Tight 909, and Aggressive 909. These names describe intent, not hardware equivalence.

To author another kit, add a `pulse_preset(name, values)` entry. Use normalized values from `parameter_info()` and retain the stable ID assertions documented in `docs/vst3.md`. For sample-based kits, capture opaque VST3 state instead and disclose that PULSE serializes paths rather than embedding sample audio; a missing file must be treated as a load failure.

PULSE parameter banks use the reusable managed-parameter-bank abstraction. Each logical bank owns exactly one persistent processor; selection applies the complete parameter set at the next audio block while explicit live parameter overrides remain authoritative. Thus the finished graph has exactly two simultaneous PULSE processors plus four Serum processors. The two selectors, MIDI lanes, audio outputs, parameters, and state snapshots are disjoint.

## Extending another VST3 instrument

Create a versioned `PatchBank` manifest whose stable IDs point to identity-checked Meldritch VST3 state documents, pass its directory as `preset_catalog`, bind the returned `preset` parameter as an instrument control, and pass `preset_catalog` to a plane's `preset_selector`. No frontend special case is required. Catalog loading currently requires compatible parameter metadata across states of the same plugin class.

## Validation and limitations

The real-plugin integration test saves state for both PULSE banks and all four Serum processors, changes each PULSE selector independently, coalesces rapid Serum requests, renders every role in isolation, checks finite bounded output, and checks host failure state. The frontend suite covers catalog searching, stable IDs, bounded row rendering, authoritative state, and read-only projection.

The recorded validation render was 20.000 seconds / 960,000 stereo frames at 48 kHz and used the four-worker, 1,024-frame offline executor. It rendered at 1.46x realtime with an overall peak of -8.97 dBFS, RMS -23.61 dBFS, no clipped samples, and no failed VST3 module. Its SHA-256 was `320b25a8e3f6be85daa0cb03b369c4716517b393bf6772c92b7df55db2eaa6d6`. Isolated two-second role energies (left plus right squared-sample sum) were PULSE 808 566.63, PULSE 909 62.69, bass 7.35, drone 39.77, lead 42.89, and arp 13.62; every role was finite, non-silent, and below the test's absolute sample bound of 2.0.

Live playback uses the same preallocated dependency-level block scheduler. This is necessary for this graph: the serialized live-equivalent path measured only 0.53x realtime, while the ten-worker schedule measured 1.56x realtime on the documented Ryzen 9 7940HS host. The `play` command's startup line reports the selected worker count. By default Meldritch uses up to ten available logical CPUs; set `MELDRITCH_LIVE_WORKERS` to a positive integer to tune a different machine. Worker threads and buffers are created before the stream starts, and graph reloads retain the configured count. The web header displays the authoritative cumulative playback `XRUNS` count; it is published from the audio backend alongside total stream errors, timed-note errors, the first error classification, buffer size, sample rate, channel count, and DSP worker count.

Known limitations:

- The converted Serum inventory is currently 364 entries because of the single-quote scraper failure described above.
- PULSE has no usable native factory program list. Sample-based user states are path-dependent.
- Activating two solo controls simultaneously is not a grouped-solo operation; each solo independently gates the other five channels. Single-instrument solo is the intended performance gesture.
- Serum state preparation can take perceptible time on this machine. The old sound continues until the replacement is ready and the browser remains pending.
- Phaser is bundled into the single embedded `app.js` artifact because the Rust server exposes fixed asset names. The uncompressed local bundle is about 1.61 MB; this avoids runtime requests for unserved content-hashed chunks.
