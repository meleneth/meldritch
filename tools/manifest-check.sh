#!/usr/bin/env bash
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
manifest="${1:-artifacts/control_relations.manifest.json}"
pattern_id="${MELDRITCH_PATTERN_ID:-8}"
sample_sources="${MELDRITCH_SAMPLE_SOURCES:-1}"
relations="${MELDRITCH_RELATIONS:-2}"
sample_to_pattern="${MELDRITCH_SAMPLE_TO_PATTERN:-1}"
pattern_controls="${MELDRITCH_PATTERN_CONTROLS:-1}"

cd "$root"

if [[ "$manifest" = /* ]]; then
  manifest_path="$manifest"
else
  manifest_path="$root/$manifest"
fi

if [[ ! -f "$manifest_path" ]]; then
  printf 'error: manifest not found: %s\n' "$manifest_path" >&2
  exit 1
fi

cargo run -p meldritch-cli -- manifest-check "$manifest_path" \
  --pattern-id "$pattern_id" \
  --sample-sources "$sample_sources" \
  --relations "$relations" \
  --relation-kind "SampleToPattern=$sample_to_pattern" \
  --relation-kind "PatternControlsPattern=$pattern_controls" \
  --finite \
  --nonzero
