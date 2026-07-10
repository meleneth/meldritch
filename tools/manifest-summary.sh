#!/usr/bin/env bash
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
manifest="${1:-artifacts/control_relations.manifest.json}"

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

cargo run -p meldritch-cli -- manifest-summary-json "$manifest_path"
