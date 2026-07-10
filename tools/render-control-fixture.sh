#!/usr/bin/env bash
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

project="${1:-fixtures/control_relations.toml}"
output="${2:-artifacts/control_relations.wav}"
manifest="${3:-artifacts/control_relations.manifest.json}"

MELDRITCH_PATTERN_ID="${MELDRITCH_PATTERN_ID:-8}" \
MELDRITCH_FRAMES="${MELDRITCH_FRAMES:-48000}" \
MELDRITCH_OVERWRITE="${MELDRITCH_OVERWRITE:-0}" \
bash "$root/tools/render-fixture.sh" "$project" "$output" "$manifest"
