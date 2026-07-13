#!/usr/bin/env bash
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
source_url="${MELDRITCH_RAVEN_SOURCE_URL:-https://archive.org/download/raven/raven_poe.mp3}"
start="${MELDRITCH_RAVEN_START:-00:00:24.000}"
duration="${MELDRITCH_RAVEN_DURATION:-8.000}"
output="${1:-assets/samples/voice/raven_chris_goringe_librivox_pd.wav}"

cd "$root"

if ! command -v ffmpeg >/dev/null 2>&1; then
  printf 'error: ffmpeg not found on PATH\n' >&2
  exit 1
fi

if [[ "$output" = /* ]]; then
  output_path="$output"
else
  output_path="$root/$output"
fi

mkdir -p "$(dirname "$output_path")"

ffmpeg \
  -hide_banner \
  -loglevel error \
  -y \
  -ss "$start" \
  -t "$duration" \
  -i "$source_url" \
  -ac 1 \
  -ar 48000 \
  -sample_fmt s16 \
  "$output_path"

printf 'Wrote %s from %s at %s for %ss\n' "$output_path" "$source_url" "$start" "$duration"
