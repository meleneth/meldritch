# Serum 2 solo playground

This playground hosts exactly one Serum 2 instance so presets and note behavior can be evaluated without the six-instrument arrangement. Serum's left and right outputs connect directly to Meldritch's host outputs. The graph contains no Meldritch mixer, gain stage, compressor, limiter, effect, or other audio processor.

Install Serum at `~/.vst3/Serum2.vst3` and keep the converted factory catalog at `~/.local/share/meldritch2/patches/serum2/factory`, then run:

```sh
bin/serum-playground
```

Open the musician access link:

```text
http://127.0.0.1:8080/#credential=serum-playground
```

The bare `http://127.0.0.1:8080/` landing screen also lists this musician link.

The fragment is read by the browser and is not sent in the HTTP request. Override the credential with `MELDRITCH_SERUM_TOKEN`, the catalog with `MELDRITCH_SERUM_CATALOG`, and the initial stable preset ID with `MELDRITCH_SERUM_PRESET`. Set `MICROLAB_MIDI` to an exact device name to enable a live keyboard; it is optional so an unplugged controller cannot prevent this diagnostic playground from launching.

The starting pattern plays four separated single notes to make unwanted noise easier to isolate. A second sustained-note pattern is available for checking envelopes and tails. The piano roll initially fits all 16 steps across its available width. The preset browser uses every successfully converted catalog entry and receives authoritative load state from the host.

`Open Serum UI on host` asks the retained host instance to open Serum's native editor on the machine running Meldritch. The editor-enabled instance uses the VST3 host's process-isolated path so Serum's GUI lifecycle stays off the realtime callback. Preset and parameter changes made inside Serum affect that sounding instance directly. If the web preset browser replaces the processor, press the button again to open the replacement's editor. Normal playground shutdown closes the helper-owned editor.

On the current host, opening Serum's 1190×740 native X11 editor caused one reported backend xrun during window creation; subsequent editor-open telemetry remained at that count. This is visible in the web header. The audio graph itself remains one Serum processor with no external DSP, so it still provides a useful comparison between catalog-state loading and Serum's own browser.

To resume the native-editor capture without attempting the entire remaining library in one run, use `--limit`:

```sh
tools/capture-serum-factory.sh --limit 5
```

Existing verified states are skipped. The factory capture uses unique apostrophe-free search tokens for affected names; for `PD - Brass 'n' Plucks`, the unique token is `Plucks`.
