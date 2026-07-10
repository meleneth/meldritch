#!/usr/bin/env bash
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$root"

run() {
  printf '\n==> %s\n' "$*"
  "$@"
}

run cargo fmt --check
run cargo clippy --workspace --all-targets -- -D warnings
run cargo test --workspace
run cargo run -p meldritch-cli -- validate fixtures/basic_drums.toml
run cargo run -p meldritch-cli -- inspect fixtures/basic_drums.toml
run cargo run -p meldritch-cli -- summary-json fixtures/basic_drums.toml
run cargo run -p meldritch-cli -- graph-json fixtures/basic_drums.toml
run cargo run -p meldritch-cli -- samples-json fixtures/basic_drums.toml
run cargo run -p meldritch-cli -- events-json fixtures/basic_drums.toml --pattern-id 1 --frames 48000
run cargo run -p meldritch-cli -- dirty-json fixtures/basic_drums.toml --source-id 1036 --start 0 --end 48000
run cargo run -p meldritch-cli -- dirty-note-json fixtures/basic_drums.toml --note 36 --start 0 --end 48000
run cargo run -p meldritch-cli -- dirty-step fixtures/basic_drums.toml --pattern-id 1 --step 4 --cycle 2
rm -f artifacts/check_render.wav artifacts/check_render.manifest.json
run cargo run -p meldritch-cli -- render-samples fixtures/basic_drums.toml --pattern-id 1 --frames 96000 --channels 2 --output artifacts/check_render.wav --manifest artifacts/check_render.manifest.json --normalize --cache-probe

MELDRITCH_OVERWRITE=1 run bash tools/render-fixture.sh
