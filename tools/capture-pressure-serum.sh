#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
bank="${1:?usage: tools/capture-pressure-serum.sh NEW_BANK_DIRECTORY}"
if [[ -e "$bank" ]]; then
  echo "destination already exists: $bank" >&2
  exit 2
fi
if [[ -z "${DISPLAY:-}" ]]; then
  echo "DISPLAY is required for Serum's native editor" >&2
  exit 2
fi

runtime="$(mktemp -d)"
log="$runtime/capture.log"
capture_pid=""
cleanup() {
  status=$?
  if [[ -n "$capture_pid" ]]; then kill "$capture_pid" 2>/dev/null || true; fi
  if (( status != 0 )) && [[ -s "$log" ]]; then cat "$log" >&2; fi
  rm -rf "$runtime"
}
trap cleanup EXIT

cd "$repo_root"
ids=(bass_reese bass_distorted bass_sub bass_rhythmic lead_cutting lead_acid lead_wide lead_modulated drone_evolving drone_atmosphere drone_industrial drone_harmonic)
names=("Master Reese Theatre" "Gnarly Reese" "Sub Sustain - Low" "Harmonic Twist" "Classic Techno Lead" "Deep Acid" "Hypa Saw" "Crunchwave" "Dark Shuffle Motion" "Dark Space" "Distant Desolation" "Motion Tech")
labels=("BA - Master Reese Theatre" "BA - Gnarly Reese" "BA - Sub Sustain - Low" "BA - Harmonic Twist" "LD - Classic Techno Lead" "LD - Deep Acid" "LD - Hypa Saw" "LD - Crunchwave" "PD - Dark Shuffle Motion" "PD - Dark Space" "PD - Distant Desolation" "PD - Motion Tech")

for index in "${!ids[@]}"; do
  : >"$log"
  fifo="$runtime/commands-$index"
  mkfifo "$fifo"
  exec 3<>"$fifo"
  cargo run --offline -q -p meldritch2 --bin meldritch-vst3-state -- \
    capture "Serum 2" "$bank" "$HOME/.vst3" <&3 >"$log" 2>&1 &
  capture_pid=$!
  parent=""
  for _ in $(seq 1 200); do
    parent="$(xdotool search --name '^Serum 2 - VST3$' 2>/dev/null | tail -n 1 || true)"
    [[ -n "$parent" ]] && break
    kill -0 "$capture_pid" 2>/dev/null || { cat "$log" >&2; exit 1; }
    sleep 0.05
  done
  [[ -n "$parent" ]] || { echo "Serum editor did not open" >&2; cat "$log" >&2; exit 1; }
  child_hex="$(xwininfo -tree -id "$parent" | awk '/^[[:space:]]+0x[0-9a-f]+ \(has no name\):/ {print $1; exit}')"
  [[ -n "$child_hex" ]] || { echo "Serum embedded view is unavailable" >&2; exit 1; }
  child="$((child_hex))"

  xdotool windowactivate --sync "$parent"
  sleep 1
  [[ "$(xdotool getactivewindow)" == "$parent" ]] || { echo "refusing input: Serum is not the active window" >&2; exit 1; }

  xdotool mousemove --window "$child" 1010 17 click 1
  sleep 1
  xdotool mousemove --window "$child" 422 84 click 1
  sleep 1
  xdotool mousemove --window "$child" 350 84 click 1
  sleep 1
  focused="$(xdotool getwindowfocus)"
  [[ "$focused" == "$parent" || "$focused" == "$child" ]] || {
    echo "refusing keyboard input: Serum parent=$parent child=$child focus=$focused" >&2
    xwininfo -tree -id "$parent" >&2
    exit 1
  }
  [[ "$(xdotool getactivewindow)" == "$parent" ]] || { echo "Serum lost activation before clearing search" >&2; exit 1; }
  xdotool key --clearmodifiers ctrl+a BackSpace
  sleep 1
  [[ "$(xdotool getactivewindow)" == "$parent" ]] || { echo "Serum lost activation before preset entry" >&2; exit 1; }
  xdotool type --delay 2 "${names[$index]}"
  sleep 1
  [[ "$(xdotool getactivewindow)" == "$parent" ]] || { echo "Serum lost activation before search submission" >&2; exit 1; }
  xdotool key Return
  sleep 1
  xdotool mousemove --window "$child" 350 140 click --repeat 2 --delay 60 1
  sleep 1
  printf 'capture %s %s\n' "${ids[$index]}" "${labels[$index]}" >&3

  for _ in $(seq 1 100); do
    grep -q "captured ${ids[$index]}:" "$log" && break
    kill -0 "$capture_pid" 2>/dev/null || { cat "$log" >&2; exit 1; }
    sleep 0.1
  done
  grep -q "captured ${ids[$index]}:" "$log" || {
    echo "Serum did not produce the verified state for ${names[$index]}" >&2
    cat "$log" >&2
    exit 1
  }
  printf 'quit\n' >&3
  exec 3>&-
  wait "$capture_pid"
  capture_pid=""
  sleep 3
done

echo "captured ${#ids[@]} verified Serum patches into $bank"
