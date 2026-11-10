# Temporary Lua library compatibility shim

The Rust host currently embeds the Lua 1.0 `musical` and author-side `arp`
libraries so applications created before repository fracture continue to run.
Their authoritative source and future development moved to `meldritch-lua`.

This shim is frozen compatibility code. It must not gain new author features.
It is removed when manifest library resolution can load the pinned
`meldritch-lua` library artifact through configured search paths and the
integration lock has upgraded every shipped application. Contract validation
and library loading remain control-thread startup work and must never enter an
audio callback.
