#!/usr/bin/env bash
set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
binary="${MELDRITCH2_BINARY:-$root/target/debug/meldritch2}"
source_file="$root/crates/meldritch2/examples/81-self-reproducing-playground.lua"
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

cargo test -p meldritch2 --manifest-path "$root/Cargo.toml" --lib \
  apc::tests::authored_keylocked_piano_roll_edits_drums_notes_durations_and_key
cargo test -p meldritch2 --manifest-path "$root/Cargo.toml" --test web_server \
  playground_successor_reproduces_browser_and_apc_logical_notes

if [[ ! -x "$binary" ]]; then
  cargo build -p meldritch2 --bin meldritch2 --manifest-path "$root/Cargo.toml"
fi

run_generation() {
  MELDRITCH_PLAYGROUND_NATIVE_ONLY=1 \
  MELDRITCH_PLAYGROUND_ACCEPTANCE=1 \
  MELDRITCH_PLAYGROUND_OUTPUT_DIR="$work" \
  "$binary" "$1" >/dev/null
}

run_generation "$source_file"
run_generation "$work/81-self-reproducing-playground.g1.lua"
run_generation "$work/81-self-reproducing-playground.g2.lua"

for generation in 1 2 3; do
  file="$work/81-self-reproducing-playground.g${generation}.lua"
  [[ -s "$file" ]]
  grep -q 'SAVE SUCCESSOR LUA' "$file"
  grep -q 'instrument:control_surface' "$file"
  grep -q 'instrument:arpeggiator' "$file"
  grep -q 'instrument:note_source' "$file"
  grep -q 'local STATE = {' "$file"
done

grep -q 'bpm = 127' "$work/81-self-reproducing-playground.g1.lua"
grep -q 'variant = "expanded"' "$work/81-self-reproducing-playground.g1.lua"
grep -q 'step = 15' "$work/81-self-reproducing-playground.g1.lua"
for length in 16 20 24 32; do
  grep -q "length = $length" "$work/81-self-reproducing-playground.g1.lua"
done

normalize() {
  sed -E \
    -e 's/generation = [0-9]+/generation = N/' \
    -e 's#source = "[^"]+"#source = "SOURCE"#' \
    "$1"
}
cmp <(normalize "$work/81-self-reproducing-playground.g1.lua") \
  <(normalize "$work/81-self-reproducing-playground.g2.lua")
cmp <(normalize "$work/81-self-reproducing-playground.g2.lua") \
  <(normalize "$work/81-self-reproducing-playground.g3.lua")

printf 'three generations executed; unchanged state is source-equivalent after generation normalization\n'
