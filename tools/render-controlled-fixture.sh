#!/usr/bin/env bash
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
project="${1:-fixtures/control_relations.toml}"
output="${2:-artifacts/control_relations.controlled.wav}"
pattern_id="${MELDRITCH_PATTERN_ID:-8}"
frames="${MELDRITCH_FRAMES:-48000}"
channels="${MELDRITCH_CHANNELS:-2}"
active_scale="${MELDRITCH_ACTIVE_SCALE:-0.5}"
normalize="${MELDRITCH_NORMALIZE:-0}"
overwrite="${MELDRITCH_OVERWRITE:-0}"

cd "$root"

if [[ "$project" = /* ]]; then
  project_path="$project"
else
  project_path="$root/$project"
fi

if [[ "$output" = /* ]]; then
  output_path="$output"
else
  output_path="$root/$output"
fi

if [[ ! -f "$project_path" ]]; then
  printf 'error: project not found: %s\n' "$project_path" >&2
  exit 1
fi

if [[ -e "$output_path" ]]; then
  if [[ "$overwrite" != 1 ]]; then
    printf 'error: output exists: %s\n' "$output_path" >&2
    printf 'set MELDRITCH_OVERWRITE=1 to replace it\n' >&2
    exit 1
  fi
  rm -f "$output_path"
fi

mkdir -p "$(dirname "$output_path")"

args=(
  run -p meldritch-cli --
  render-controlled-samples "$project_path"
  --pattern-id "$pattern_id"
  --frames "$frames"
  --channels "$channels"
  --active-scale "$active_scale"
  --output "$output_path"
)

if [[ "$normalize" == 1 ]]; then
  args+=(--normalize)
fi

cargo "${args[@]}"
printf 'Rendered controlled %s\n' "$output_path"
