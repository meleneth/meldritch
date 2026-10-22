# Delay Lab: Overdrive

Overdrive is an eight-track `meldritch.musical` project: five independently
editable 909 tracks, the two polymetric Delay Lab voices, and a separate lead.
Its full arrangement is exactly seven minutes at 112 BPM.

```sh
cargo run -- perform examples/overdrive/overdrive.lua
cargo run -- render examples/overdrive/overdrive.lua overdrive.wav 420
```

`F9` selects the performance surface and `F10` opens the shared track editor.
Pattern paging, edits, mute/solo, save, reload, keyboard operation, and APC
pages use the same semantic commands as the original groovebox. Save with `S`,
then launch the edit layer without changing the authored generator:

```sh
cargo run -- perform examples/overdrive/project.lua
```

The deliberately dense showcase uses the real Overdrive section and routing:

```sh
cargo run -- render examples/overdrive/showcase.lua overdrive-showcase.wav 18
```

Routing is explicit: the 909 bus stays dry; audible kick audio keys separate
left/right duckers on the dry Delay Lab voices and the 100%-wet delay return;
the lead has its own dry route and short stereo ambience network. The ambience
is built from the reusable tempo-synchronized stereo delay rather than claiming
to be a decorrelating algorithmic reverb. It is intentionally independent of
the Delay Lab feedback network.
