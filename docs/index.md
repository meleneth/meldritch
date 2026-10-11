---
layout: home
hero:
  name: MELDRITCH
  text: Patch a groove. Play the system.
  tagline: A Rust, TUI-first groovebox exploring modular synthesis, relational DSP and Lua-programmable control. In massive development — nothing is stable yet.
  actions:
    - theme: brand
      text: Try the APC playground
      link: /examples/apc-playground
    - theme: alt
      text: What actually works
      link: /examples/capabilities
features:
  - title: Human-readable projects
    details: TOML song directories describe typed synth graphs, patterns, sample banks and curated performance controls. Supported examples are executable contracts.
    link: /ML_FORMATS
  - title: One authoritative performance
    details: Controller input and the TUI use typed application commands. A headless Lua slice demonstrates hierarchical pages and coherent simulated surfaces.
    link: /CONTROL_PLANE
  - title: Protect the audio boundary
    details: Rust owns DSP and audio publication. Parsing, Lua execution, UI work and graph compilation stay outside the realtime callback.
    link: /INVARIANTS
---

::: warning Nothing is stable yet
MELDRITCH is experimental software in active development. There is no stable
release or stable public API. Formats, commands, bindings, interfaces and sound
may change. Check the [development status](./guide/status) and each example's
acceptance scope before relying on a capability.
:::
