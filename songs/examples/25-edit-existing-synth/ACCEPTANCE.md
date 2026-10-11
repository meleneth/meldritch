# Edit an existing synth

Status: `accept` for automated headless behavior. Physical controller interaction
and listening acceptance remain untested. The executable bindings for
[acceptance.feature](acceptance.feature) are Rust integration tests, run with
`cargo test -p meldritch-control --test instrument_authoring`; there is no
separate Gherkin runner. The fixture uses an existing oscillator → VCA → low-pass
bass with ADSR → VCA wiring. Gaps between notes make release edits measurable.

The TUI shows module identity/type, current source values, imported defaults,
parameter units/ranges, ownership and explicit cables. Actual Ctrl-E/arrow
navigation is verified at 80×24. Keyboard and actual Lua dispatch from APC,
simulated deck and terminal share the source model. Worker publication and
production callback consumption are checked separately. Rapid edits publish only
the latest source. All seven supported parameters individually change finite
rendered samples and round-trip with identical graph, fingerprint and audio.
Resetting imported implicit defaults also restores their original representation
and fingerprint; the exported definition independently reloads unchanged.
Full and split renders match. Historical example 02 verifies automation-owned
cutoff rejects changed edits. Multi-synth selection preserves transport and
other instruments. Invalid ID/type/range requests leave source, accepted
journal, worker generation and publication unchanged.

Persist every supported parameter of the selected instrument as absolute Lua
values over the immutable base fingerprint, preserving IDs, cables and references.
Other instruments remain at base values. Unique atomic saves preserve TOML,
prior exports and journals; failures retain edits. Unknown versions/fields,
duplicates/missing values, path escapes and changed bases reject reload.
Authored exports live in `authored/*.lua`; journals remain under `performances/`.
The checked-in `edited-instrument.lua` independently reloads cutoff 900 Hz and
release 0.25 seconds without session replay or hardware:

```sh
cargo run -- validate-song songs/examples/25-edit-existing-synth --instrument edited-instrument.lua
cargo run -- render-instrument songs/examples/25-edit-existing-synth edited-instrument.lua --output /tmp/edited-bass.wav
```

See [the guide](../../../INSTRUMENT_EDITOR.md) for syntax and limits.
