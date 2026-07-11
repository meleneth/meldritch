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
run cargo run -p meldritch-cli -- validate fixtures/explicit_relations.toml
run cargo run -p meldritch-cli -- validate fixtures/control_relations.toml
run cargo run -p meldritch-cli -- inspect fixtures/basic_drums.toml
run cargo run -p meldritch-cli -- summary-json fixtures/basic_drums.toml
run cargo run -p meldritch-cli -- summary-json fixtures/explicit_relations.toml
run cargo run -p meldritch-cli -- summary-json fixtures/control_relations.toml
run cargo run -p meldritch-cli -- graph-json fixtures/basic_drums.toml
run cargo run -p meldritch-cli -- graph-json fixtures/explicit_relations.toml
run cargo run -p meldritch-cli -- graph-json fixtures/control_relations.toml
run cargo run -p meldritch-cli -- relations-json fixtures/basic_drums.toml
run cargo run -p meldritch-cli -- relations-json fixtures/explicit_relations.toml
run cargo run -p meldritch-cli -- relations-json fixtures/control_relations.toml
run cargo run -p meldritch-cli -- samples-json fixtures/basic_drums.toml
run cargo run -p meldritch-cli -- events-json fixtures/basic_drums.toml --pattern-id 1 --frames 48000
run cargo run -p meldritch-cli -- events-json fixtures/explicit_relations.toml --pattern-id 7 --frames 48000
run cargo run -p meldritch-cli -- control-events-json fixtures/control_relations.toml --pattern-id 8 --frames 48000
run cargo run -p meldritch-cli -- control-events-check fixtures/control_relations.toml --pattern-id 8 --frames 48000 --events 1 --controller-patterns 1 --active-events 1 --min-active-controllers 1
run cargo run -p meldritch-cli -- dirty-json fixtures/basic_drums.toml --source-id 1036 --start 0 --end 48000
run cargo run -p meldritch-cli -- dirty-note-json fixtures/basic_drums.toml --note 36 --start 0 --end 48000
run cargo run -p meldritch-cli -- dirty-note-json fixtures/explicit_relations.toml --note 36 --start 0 --end 48000
run cargo run -p meldritch-cli -- dirty-pattern-json fixtures/control_relations.toml --pattern-id 7 --start 0 --end 48000
run cargo run -p meldritch-cli -- dirty-step fixtures/basic_drums.toml --pattern-id 1 --step 4 --cycle 2
run bash tools/control-events.sh
run bash tools/control-events-check.sh
run bash tools/relations.sh
run bash tools/dirty-pattern.sh
rm -f artifacts/check_render.wav artifacts/check_render.manifest.json
run cargo run -p meldritch-cli -- render-samples fixtures/basic_drums.toml --pattern-id 1 --frames 96000 --channels 2 --output artifacts/check_render.wav --manifest artifacts/check_render.manifest.json --normalize --cache-probe
run cargo run -p meldritch-cli -- manifest-summary-json artifacts/check_render.manifest.json
run cargo run -p meldritch-cli -- manifest-check artifacts/check_render.manifest.json --pattern-id 1 --sample-sources 4 --relations 4 --relation-kind SampleToPattern=4 --finite --nonzero
rm -f artifacts/check_control_render.wav artifacts/check_control_render.manifest.json
run cargo run -p meldritch-cli -- render-samples fixtures/control_relations.toml --pattern-id 8 --frames 48000 --channels 2 --output artifacts/check_control_render.wav --manifest artifacts/check_control_render.manifest.json --normalize --cache-probe
run cargo run -p meldritch-cli -- manifest-summary-json artifacts/check_control_render.manifest.json
run cargo run -p meldritch-cli -- manifest-check artifacts/check_control_render.manifest.json --pattern-id 8 --sample-sources 1 --relations 2 --relation-kind SampleToPattern=1 --relation-kind PatternControlsPattern=1 --finite --nonzero
MELDRITCH_OVERWRITE=1 run bash tools/render-control-fixture.sh
run bash tools/manifest-summary.sh
run bash tools/manifest-check.sh

MELDRITCH_OVERWRITE=1 run bash tools/render-fixture.sh
