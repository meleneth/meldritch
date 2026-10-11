# Plaits as an authored typed module

Status: `accept` for headless tests, run with
`cargo test -p meldritch-control --test plaits_authoring` and
`cargo test -p meldritch-render --test plaits_allocation`.
Keyboard navigation uses the existing editor; model choice uses the same typed
adjust command. Physical listening remains pending.

The pinned sourcebox crate supplies FM and harmonic sound sources at 48 kHz.
The explicit pitch → source → VCA → filter → output graph retains its external
gate → ADSR → VCA control cable. There is no hidden Plaits Voice/LPG.

Inspect and edit `osc.model` (`fm`, `harmonic`), and `harmonics`, `timbre`, `morph`
(finite 0..1, defaults 0.5) using Ctrl-E, arrows, and Lua `edit_instrument`.
Each accepted changed value must alter finite rendered audio, invalidate the
song horizon, and reach the existing publication/callback path. Invalid types,
models, ranges, rates, ports and targets retain the prior source/publication.

Save via Ctrl-S to a unique authored instrument Lua file. Independent reload
must recover IDs, graph, model, values, fingerprint and exact audio without
session replay. Existing TOML songs and instrument exports remain compatible.
Full/split renders must agree. Direct upstream engine blocks are the adapter
reference; this is port equivalence, not hardware or C++ firmware certification.
Bounded block processing must allocate nothing after engine construction.

Plaits' f32 arithmetic is an explicit external DSP exception; source values,
filter/envelope/mixing remain f64. Plaits RNG is authorized as an exception to
determinism; the initial two engines do not use it. This initial example uses no random engine. Example 33 extends the catalog
and tests explicit gate connections without changing this historical fixture.
