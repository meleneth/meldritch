#!/usr/bin/env bash
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$root"

cargo fmt -p meldritch2 -- --check
cargo clippy -p meldritch2 --all-targets
cargo test -p meldritch2
cargo run -q -p meldritch2 -- crates/meldritch2/examples/01-basic.lua
