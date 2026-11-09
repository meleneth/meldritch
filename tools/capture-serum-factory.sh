#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
preset_root="${SERUM_FACTORY_PRESETS:-$HOME/Documents/Xfer/Serum 2 Presets/Presets/Factory}"
bank="${SERUM_FACTORY_BANK:-$HOME/.local/share/meldritch2/patches/serum2/factory}"
plan_only=false
capture_limit=0

usage() {
  echo "usage: tools/capture-serum-factory.sh [--plan] [--limit COUNT] [BANK_DIRECTORY]" >&2
}

while (($#)); do
  case "$1" in
    --plan) plan_only=true ;;
    --limit)
      shift
      [[ "${1:-}" =~ ^[1-9][0-9]*$ ]] || { usage; exit 2; }
      capture_limit="$1"
      ;;
    -h|--help) usage; exit 0 ;;
    -*) usage; exit 2 ;;
    *)
      if [[ "$bank" != "${SERUM_FACTORY_BANK:-$HOME/.local/share/meldritch2/patches/serum2/factory}" ]]; then
        usage
        exit 2
      fi
      bank="$1"
      ;;
  esac
  shift
done

[[ -d "$preset_root" ]] || {
  echo "Serum factory preset directory does not exist: $preset_root" >&2
  exit 2
}

declare -a ids labels searches relative_paths
declare -A seen_ids
while IFS= read -r -d '' preset; do
  relative="${preset#"$preset_root"/}"
  label="${relative##*/}"
  label="${label%.SerumPreset}"
  search="$label"
  if [[ "$search" == *" - "* ]]; then
    search="${search#* - }"
  fi
  # xdotool's synthetic apostrophe is not accepted by Serum's XEmbed search
  # field on this layout. Use a unique apostrophe-free token sequence for the
  # five affected factory names.
  case "$label" in
    "GTR - 70's Funk Guitar") search="Funk Guitar" ;;
    "LOOP - Apollo's Aria") search="Apollo" ;;
    "PD - Brass 'n' Plucks") search="Plucks" ;;
    "PD - Cal's Chimes") search="Cal Chimes" ;;
    "FX - Didn't Hit Me") search="Hit Me" ;;
  esac
  id="$(printf '%s' "${relative%.SerumPreset}" \
    | LC_ALL=C tr '[:upper:]' '[:lower:]' \
    | sed -E 's/[^a-z0-9]+/_/g; s/^_+//; s/_+$//')"
  id="factory_${id}"
  [[ -n "$id" ]] || { echo "could not derive ID for $relative" >&2; exit 1; }
  if [[ -n "${seen_ids[$id]:-}" ]]; then
    echo "factory preset ID collision: $relative and ${seen_ids[$id]} both map to $id" >&2
    exit 1
  fi
  seen_ids[$id]="$relative"
  ids+=("$id")
  labels+=("$label")
  searches+=("$search")
  relative_paths+=("$relative")
done < <(find "$preset_root" -type f -name '*.SerumPreset' -print0 | LC_ALL=C sort -z)

((${#ids[@]} > 0)) || { echo "no Serum factory presets found in $preset_root" >&2; exit 1; }

if $plan_only; then
  printf 'id\tname\tsearch\tsource\n'
  for index in "${!ids[@]}"; do
    printf '%s\t%s\t%s\t%s\n' \
      "${ids[$index]}" "${labels[$index]}" "${searches[$index]}" "${relative_paths[$index]}"
  done
  exit 0
fi

if [[ -z "${DISPLAY:-}" ]]; then
  echo "DISPLAY is required for Serum's native editor" >&2
  exit 2
fi
for command in cargo xdotool xwininfo; do
  command -v "$command" >/dev/null || { echo "required command is unavailable: $command" >&2; exit 2; }
done

runtime="$(mktemp -d)"
log="$runtime/capture.log"
capture_pid=""
cleanup() {
  status=$?
  if [[ -n "$capture_pid" ]]; then kill "$capture_pid" 2>/dev/null || true; fi
  if ((status != 0)) && [[ -s "$log" ]]; then tail -n 80 "$log" >&2; fi
  rm -rf "$runtime"
}
trap cleanup EXIT

mkdir -p "$bank"
cd "$repo_root"

captured=0
skipped=0
total="${#ids[@]}"
for index in "${!ids[@]}"; do
  id="${ids[$index]}"
  label="${labels[$index]}"
  search="${searches[$index]}"
  state_file="$bank/$id.meldritch-vst3-state"
  if [[ -f "$state_file" ]]; then
    ((skipped += 1))
    printf '[%d/%d] already captured: %s\n' "$((index + 1))" "$total" "$label"
    continue
  fi

  printf '[%d/%d] loading: %s (%s)\n' "$((index + 1))" "$total" "$label" "${relative_paths[$index]}"
  : >"$log"
  fifo="$runtime/commands-$index"
  mkfifo "$fifo"
  exec 3<>"$fifo"
  cargo run --offline -q -p meldritch2 --bin meldritch-vst3-state -- \
    capture "Serum 2" "$bank" "$HOME/.vst3" <&3 >"$log" 2>&1 &
  capture_pid=$!

  parent=""
  for _ in $(seq 1 400); do
    parent="$(xdotool search --name '^Serum 2 - VST3$' 2>/dev/null | tail -n 1 || true)"
    [[ -n "$parent" ]] && break
    kill -0 "$capture_pid" 2>/dev/null || { cat "$log" >&2; exit 1; }
    sleep 0.05
  done
  [[ -n "$parent" ]] || { echo "Serum editor did not open" >&2; exit 1; }
  child_hex="$(xwininfo -tree -id "$parent" | awk '/^[[:space:]]+0x[0-9a-f]+ \(has no name\):/ {print $1; exit}')"
  [[ -n "$child_hex" ]] || { echo "Serum embedded view is unavailable" >&2; exit 1; }
  child="$((child_hex))"

  xdotool windowactivate --sync "$parent"
  sleep 1
  [[ "$(xdotool getactivewindow)" == "$parent" ]] || { echo "refusing input: Serum is not active" >&2; exit 1; }

  xdotool mousemove --window "$child" 1010 17 click 1
  sleep 1
  xdotool mousemove --window "$child" 422 84 click 1
  sleep 1
  xdotool mousemove --window "$child" 350 84 click 1
  sleep 1
  focused="$(xdotool getwindowfocus)"
  [[ "$focused" == "$parent" || "$focused" == "$child" ]] || {
    echo "refusing keyboard input: Serum parent=$parent child=$child focus=$focused" >&2
    exit 1
  }
  [[ "$(xdotool getactivewindow)" == "$parent" ]] || { echo "Serum lost activation before search" >&2; exit 1; }
  xdotool key --clearmodifiers ctrl+a BackSpace
  sleep 1
  xdotool type --delay 2 "$search"
  sleep 1
  [[ "$(xdotool getactivewindow)" == "$parent" ]] || { echo "Serum lost activation before submission" >&2; exit 1; }
  xdotool key Return
  sleep 1
  xdotool mousemove --window "$child" 350 140 click --repeat 2 --delay 60 1
  sleep 1
  printf 'capture %s %s\n' "$id" "$label" >&3

  captured_line="captured $id:"
  refused_line="refused $id:"
  for _ in $(seq 1 200); do
    grep -Fq "$captured_line" "$log" && break
    if grep -Fq "$refused_line" "$log"; then
      grep -F "$refused_line" "$log" >&2
      exit 1
    fi
    kill -0 "$capture_pid" 2>/dev/null || { cat "$log" >&2; exit 1; }
    sleep 0.1
  done
  grep -Fq "$captured_line" "$log" || {
    echo "Serum did not produce verified state for $label" >&2
    exit 1
  }
  ((captured += 1))
  printf 'quit\n' >&3
  exec 3>&-
  wait "$capture_pid"
  capture_pid=""
  rm -f "$fifo"
  sleep 3
  if ((capture_limit > 0 && captured >= capture_limit)); then
    break
  fi
done

manifest_count="$(grep -c '"id": "factory_' "$bank/manifest.json")"
if ((capture_limit == 0)) && [[ "$manifest_count" != "$total" ]]; then
  echo "factory bank is incomplete: manifest has $manifest_count patches, expected $total" >&2
  exit 1
fi
echo "captured $captured and retained $skipped verified Serum factory patches in $bank ($manifest_count of $total total)"
