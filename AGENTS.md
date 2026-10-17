# Repository agent guide

**Meldritch 2 is the only active development target.** Read
`crates/meldritch2/AGENTS.md`, `crates/meldritch2/ARCHITECTURE.md`, and the
relevant files under `crates/meldritch2/examples/` before changing it. Its
small Lua examples and behavior tests define accepted scope. Keep contracts,
validation, compilation, and Lua off the sample-processing path; preserve an
accepted machine when a candidate revision fails.

Meldritch 1 is completed problem-space research, documented in
`MELDRITCH1_ARCHIVE.md`. The root historical design files (`TODO.md`,
`ROADMAP.md`, `FEATURES.md`, `INVARIANTS.md`, and peers), `songs/examples/`,
`fixtures/`, the `meldritch-*` crates, and the root `meldritch` package belong
to that archive. `MELDRITCH1_AGENTS.md` preserves its former working guide as
historical evidence. Do not use those documents as Meldritch 2 requirements or
make their tests an active development gate. Meldritch 2 has no compatibility
obligation to Meldritch 1 and must not depend on its crates or formats. Preserve
the archive and its independent build path; do not migrate components without
an explicit task.

Ordinary root `cargo build`, `cargo test`, and `cargo run` select Meldritch 2
through Cargo `default-members`. Use `tools/check.sh` for the active check.
Use explicit `-p meldritch` or `tools/meldritch1-check.sh` only for intentional
archive inspection. `cargo test --workspace` includes the archive and is not a
Meldritch 2 acceptance requirement. Keep commits focused and commit completed,
verified slices as work proceeds.

## Optional C/C++ toolchains

The host provides GCC 16.2.0 at `/opt/devastation/toolchains/gcc-16.2` and
Clang 22.1.6 at `/opt/devastation/toolchains/clang-22.1.6`. Prefer a
repository-requested compiler. When a task calls for either version, use
`CC=/opt/devastation/toolchains/gcc-16.2/bin/gcc`
`CXX=/opt/devastation/toolchains/gcc-16.2/bin/g++`, or
`CC=/opt/devastation/toolchains/clang-22.1.6/bin/clang`
`CXX=/opt/devastation/toolchains/clang-22.1.6/bin/clang++`. Do not change
system defaults or `update-alternatives`.
