#!/usr/bin/env bash
set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
cd "$root"
cargo build -p meldritch-vst3-fixture

case "$(rustc -vV | sed -n 's/^host: \([^-]*\)-.*/\1/p')" in
  x86_64) architecture=x86_64-linux ;;
  aarch64) architecture=aarch64-linux ;;
  *) echo "unsupported Linux architecture" >&2; exit 1 ;;
esac

destination="target/vst3-fixtures/TestSynth.vst3/Contents/$architecture/TestSynth.so"
mkdir -p "$(dirname "$destination")"
cp target/debug/libmeldritch_vst3_fixture.so "$destination"
echo "built $destination"
