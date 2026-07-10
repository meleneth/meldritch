#!/usr/bin/env bash
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
project="${1:-fixtures/basic_drums.toml}"
output="${2:-artifacts/basic_drums.wav}"
frames="${MELDRITCH_FRAMES:-96000}"
channels="${MELDRITCH_CHANNELS:-2}"
pattern_id="${MELDRITCH_PATTERN_ID:-1}"
normalize="${MELDRITCH_NORMALIZE:-1}"
cache_probe="${MELDRITCH_CACHE_PROBE:-1}"
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
  render-samples "$project_path"
  --pattern-id "$pattern_id"
  --frames "$frames"
  --channels "$channels"
  --output "$output_path"
)

if [[ "$normalize" == 1 ]]; then
  args+=(--normalize)
fi

if [[ "$cache_probe" == 1 ]]; then
  args+=(--cache-probe)
fi

cargo "${args[@]}"
printf 'Rendered %s\n' "$output_path"
