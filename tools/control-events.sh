#!/usr/bin/env bash
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
project="${1:-fixtures/control_relations.toml}"
pattern_id="${MELDRITCH_PATTERN_ID:-8}"
frames="${MELDRITCH_FRAMES:-48000}"

cd "$root"

if [[ "$project" = /* ]]; then
  project_path="$project"
else
  project_path="$root/$project"
fi

if [[ ! -f "$project_path" ]]; then
  printf 'error: project not found: %s\n' "$project_path" >&2
  exit 1
fi

cargo run -p meldritch-cli -- control-events-json "$project_path" \
  --pattern-id "$pattern_id" \
  --frames "$frames"
