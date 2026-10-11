# Getting started

::: warning Development software
Nothing is stable yet. Keep backups of authored projects and sessions. Passing
tests establish specific example behavior, not production or hardware readiness.
:::

Build from the repository with Rust **1.97 or newer**. Linux requires ALSA
development headers (`libasound2-dev` on Debian/Ubuntu or `alsa-lib-devel` on
Fedora). No external Lua installation is needed for the vendored Lua adapter.

```sh
git clone https://github.com/meleneth/meldritch.git
cd meldritch
cargo install --path . --locked
meldritch --help
```

Validate a project without audio or controllers:

```sh
meldritch validate-song songs/examples/09-curated-performance-controls
```

Play that project's curated TUI:

```sh
meldritch tui-song songs/examples/09-curated-performance-controls
```

Playback starts automatically. Use `p` for play/stop, `r` to rewind and `q` to
quit. `Ctrl-Tab` switches cockpit modes; full all-parameters inspection is still
unfinished. Accepted actions are captured in the song's `performances/` directory;
exact and quantized replay coverage is incomplete.

For a connected APC Mini MK2, use the [dedicated playground](../examples/apc-playground).
The existing LaunchControl XL projects remain available under
`songs/examples/16-launch-control-xl-playground` and
`songs/examples/17-launch-control-xl-ensemble`.

The [capability matrix](../examples/capabilities) distinguishes design, parsing,
compilation, playback and accepted behavior. The [Lua adapter](../CONTROL_PLANE)
currently runs headlessly; `tui-song` uses the established TOML song host.
