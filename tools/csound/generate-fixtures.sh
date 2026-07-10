#!/usr/bin/env bash
set -euo pipefail

out_dir="${1:-fixtures/audio}"
root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
output="$root/$out_dir"
orc="$root/tools/csound/meldritch-fixtures.orc"

mkdir -p "$output"

require_csound() {
  if ! command -v csound >/dev/null 2>&1; then
    printf 'error: csound not found on PATH\n' >&2
    exit 1
  fi
}

render_sample() {
  local name="$1"
  local instrument="$2"
  local duration="$3"
  local wav="$output/$name.wav"
  local score
  local log

  score="$(mktemp)"
  log="$(mktemp)"
  trap 'rm -f "$score" "$log"' RETURN

  {
    printf 'i "%s" 0 %s\n' "$instrument" "$duration"
    printf 'e\n'
  } >"$score"

  if ! csound -d -m0 -W -o "$wav" "$orc" "$score" >"$log" 2>&1; then
    cat "$log" >&2
    exit 1
  fi

  if [[ ! -s "$wav" ]]; then
    cat "$log" >&2
    printf 'error: expected non-empty output %s\n' "$wav" >&2
    exit 1
  fi
}

require_csound
render_sample kick Kick 0.45
render_sample snare Snare 0.38
render_sample hat Hat 0.12
render_sample sub Sub 0.70

printf 'Wrote Csound fixture samples to %s\n' "$output"
