#!/usr/bin/env bash
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$root"

npm --prefix crates/meldritch2/frontend test
npm --prefix crates/meldritch2/frontend run format:check
npm --prefix crates/meldritch2/frontend run build
cargo fmt -p meldritch2 -- --check
cargo clippy -p meldritch2 --all-targets
cargo test -p meldritch2
cargo run -q -p meldritch2 -- crates/meldritch2/examples/01-basic.lua
