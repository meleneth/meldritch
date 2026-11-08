# Pressure System Live

`crates/meldritch2/examples/80-pressure-system-live.lua` is an eight-minute,
four-performer industrial electronic composition and an indefinitely playable
instrument. The same Lua graph drives live playback, browser control, and the
offline reference render.

## Required native plugins

The composition resolves plugins by stable VST3 class ID:

| Role | Plugin | Class ID |
|---|---|---|
| Bass, lead, drone | Serum 2 | `56534558667350736572756D20320000` |
| Drums | PULSE Drum Synth | `ABCDEF019182FAEB506C7365506C6473` |
| Master dynamics | LSP Compressor Stereo | `647370206B31732020202020756E7363` |
| Peak protection | LSP Limiter Stereo | `64737020623173202020202072667563` |

Serum and PULSE are expected below `$HOME/.vst3`. PULSE build instructions are
in [Native Linux VST3 hosting](vst3.md#pulse-drum-synth-010). On Debian, the
tested LSP build is the official distribution package `lsp-plugins-vst3`
1.2.21-5:

```sh
sudo apt install lsp-plugins-vst3 lsp-plugins-r3d-glx
```

LSP installs one multi-class bundle at `/usr/lib/vst3/lsp-plugins.vst3`.
Meldritch enumerates its `moduleinfo.json`, resolves the compressor and limiter
classes independently, validates their stereo main buses, and hosts both in the
ordinary compiled graph. No plugin binary or proprietary preset is stored in
this repository.

## Run the performance

```sh
PRESSURE_PORT=8080 cargo run -p meldritch2 -- perform \
  crates/meldritch2/examples/80-pressure-system-live.lua
```

Open `http://127.0.0.1:8080/`. The server binds only to loopback by default.
Set `PRESSURE_HOST=0.0.0.0` only on a trusted LAN, then use the host's LAN
address from phones and tablets. Plain HTTP has no TLS; do not expose it to the
public Internet.

Use one of the composition-issued credentials in the browser's Join field:

| Credential | Role | Writable station |
|---|---|---|
| `pressure-conductor` | Conductor | All stations, master, macros, transport actions |
| `pressure-bass` | Bass performer | Bass piano roll, patterns, tone, envelope, ducking |
| `pressure-lead` | Lead performer | Lead piano roll, patterns, tone, envelope |
| `pressure-drone` | Drone performer | Drone voicings, states, tone, long envelope |
| `pressure-drums` | Drummer | PULSE grid, velocity, lane mutes, grooves, synthesis |
| `pressure-observer` | Observer | Read-only projection of every station |

The first four controls in the Conductor's Master station are authoritative
`DRUMS`, `BASS`, `LEAD`, and `DRONE` mutes. They multiply the corresponding
stereo branch to silence immediately before the mixer, while the instrument
continues processing and retains notes and plugin state. This makes diagnostic
soloing practical without reinitializing a VST3 instance: mute three branches,
listen to the remaining part, then restore them. The state is synchronized to
every browser and Rust enforces the Conductor-only mutation permission.

The browser contains no song-specific code. Lua declares the stations and
controls, Rust filters them by role and publishes revisioned authoritative
state, Pinia reconciles it, and Vue selects the reusable piano-roll, drum-grid,
pattern, knob, fader, envelope, and trigger components. Reconnecting obtains a
full current snapshot. Hiding a component is never an authorization boundary;
Rust rejects commands outside the resolved role.

## Musical architecture

Four existing Rust sequencers run at 128 BPM. Separate sequencers let bass,
lead, drone, and drums quantize pattern changes independently while sharing the
same engine-owned time model. They are not browser clocks and do not schedule
audio from JavaScript.

- Bass has eight phrases: Foundation, Teeth, Acid Pressure, Broken Machine,
  Subduction, Overdrive, Empty Space, and Final Transmission.
- Lead has six phrases: Signal, Razor, Voltage, Fracture, Ascension, and
  Transmission.
- Drone has six 64-step states: Reactor, Subterranean, Ghost Signal, Unstable
  Core, Blackout, and Event Horizon. Long overlapping notes and slow normalized
  parameter automation make each state evolve.
- PULSE has six grooves: Iron March, Broken Conveyor, Hydraulic Press, Empty
  Factory, Overpressure, and Terminal Velocity. Its four actual parts are used
  as kick, snare, hat, and metallic percussion; no fictitious extra PULSE
  voices are advertised.

The 256-bar arrangement is exactly 480 seconds at 128 BPM. Its ten sections
follow the Ignition through Dissolution form in the composition source.
Pattern changes, intensity changes, kick dropouts, sparse sections, and slowly
moving plugin parameters create development rather than merely repeating a
four-bar loop. After bar 256, the final patterns keep looping for manual live
performance. An offline render supplies its own duration and may include a
tail.

## Signal flow and timing

```text
PULSE drums -----------------------------+
Serum bass -> kick-triggered ducking -----+--> stereo mixer
Serum lead -------------------------------+       |
Serum drone ------------------------------+       v
                                      LSP Compressor Stereo
                                               |
                                               v
                                        LSP Limiter Stereo
                                               |
                                               v
                                          master output
```

The bass ducker is the existing deterministic Meldritch gain-envelope module.
The PULSE kick gate is its detector; ordinary bass audio remains in the compiled
graph. Depth and release are performer controls. This avoids claiming external
VST3 sidechain routing, which the current single-main-bus effect adapter does
not implement.

The LSP effects receive preallocated stereo blocks and normalized parameter
queues in the same audio callback as instruments. Discovery, instantiation,
state work, and filesystem access happen before processing. Effects report
their latency to `Machine::vst3_latency_samples`; automatic compensation for
parallel effect paths is not yet implemented. Pressure System has one serial
master path, so there is no dry parallel path to align.

Arrangement automation supplies the baseline value. A browser or hardware
control publishes an override at the next processing boundary, so the performer
does not fight the next automation point. This release does not yet project an
explicit “return to automation” button; restarting the composition clears
published overrides. Pattern selection is quantized to bars. VST3 parameter
changes are block-boundary accurate, not sample-accurate.

## Authentic Serum patch banks

All visible mappings use plugin-reported stable IDs. Important IDs include
main volume `0`, portamento `3`, oscillator A/B position `1000039`/`1001039`,
warp amount `1000034`/`1001034`, filter cutoff/resonance/drive
`2000003`/`2000004`/`2000005`, amp envelope A/D/S/R
`3000000`/`3000002`/`3000003`/`3000004`, and macros beginning at `7000000`.
Values are normalized VST3 values; Serum supplies display conversions.

Pressure System loads three identity-checked local banks from
`~/.local/share/meldritch2/patches/serum2/pressure-system` (override the root
with `PRESSURE_SERUM_BANK`). Bass offers Master Reese Theatre, Gnarly Reese,
Sub Sustain - Low, and Harmonic Twist. Lead offers Classic Techno Lead, Deep
Acid, Hypa Saw, and Crunchwave. Drone offers Dark Shuffle Motion, Dark Space,
Distant Desolation, and Motion Tech. These are factory presets selected in
Serum's native browser and captured through component/controller state; no
factory asset is stored in this repository.

Each performer plane has an authoritative `*_patch` selector. Bass, lead, and
drone roles may change only their own selector; conductor can change all, and
observer is read-only. WebSocket state derives `active_name` from the compiled
selector value, so clients display what is actually audible. Banks preload
states during graph construction, but inactive residents do not run DSP. Patch
changes flush the outgoing notes once, reconstruct held notes in the incoming
processor, and switch output branches at a block boundary; no file I/O or state
restoration occurs on the audio callback. Wavetable-position controls remain
live, though their audible range depends on preset routing.

## Offline reference render

Render the complete arrangement plus a ten-second release tail:

```sh
cargo run -p meldritch2 -- render \
  crates/meldritch2/examples/80-pressure-system-live.lua \
  /tmp/pressure-system-live.wav 490 --workers=4
```

The output is 48 kHz, 24-bit stereo. The composition uses no musical
randomness, so Meldritch's notes, transitions, velocities, and automation are
deterministic. Exact samples can still differ with plugin versions, CPU
implementations, and plugin-internal behavior. The LSP limiter is final peak
protection, not a substitute for the mixer's conservative gain staging.

The regression suite renders the authentic captured states, verifies finite
stereo output, and compares fresh-instance state round-trips against the same
MIDI phrase. A full 490-second render is intentionally a local release check:
third-party plugin version and CPU behavior prevent publishing a universal
sample hash. On the documented Serum 2.1.5 setup, a ten-second opening render
produced 480,000 24-bit stereo frames at 48 kHz; FFmpeg measured -6.95 dBFS
overall peak and -21.74 dBFS RMS. Inspect a full local render before using it as
a live gain reference. With resident-patch DSP suspension enabled, a 30-second
release render measured 4.32x real time with four workers and 2.56x with one
worker on the target development host.

## Current boundaries

- Serum exposes no supported headless native-preset loader; bank creation needs
  X11 editor hosting, while runtime recall is headless.
- Four resident Serum processors per instrument increase startup time and RAM,
  but only one per instrument runs steady-state DSP. A switch can briefly cost
  one outgoing flush plus one incoming block.
- PULSE has four parts; “open” hat and auxiliary metallic roles are achieved by
  different patterns and synthesis settings, not imaginary extra plugin parts.
- Master and gain-reduction diagnostics exist in the engine, but a dedicated
  high-rate browser meter control is not yet in schema v3. Network updates stay
  bounded and never run on the audio callback.
- LSP external sidechains, auxiliary effect buses, and automatic latency
  compensation are future general host work.
- In-process native plugins can still crash, deadlock, or corrupt the host;
  these failures cannot be safely recovered inside the process.
