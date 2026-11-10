#!/usr/bin/env python3
"""Keep native VST3 integration tests out of routine verification."""

from __future__ import annotations

import pathlib
import sys
import tomllib


ROOT = pathlib.Path(__file__).resolve().parents[1]
MANIFEST = ROOT / "crates" / "meldritch2" / "Cargo.toml"
FEATURE = "vst3-integration-tests"


def fail(message: str) -> None:
    print(f"test policy violation: {message}", file=sys.stderr)
    raise SystemExit(1)


manifest = tomllib.loads(MANIFEST.read_text(encoding="utf-8"))
features = manifest.get("features", {})
if FEATURE not in features:
    fail(f"{FEATURE!r} feature is missing")
if FEATURE in features.get("default", []):
    fail(f"{FEATURE!r} must never be a default feature")

targets = {
    target.get("name"): target for target in manifest.get("test", [])
}
vst3_target = targets.get("vst3_host")
if vst3_target is None:
    fail("the vst3_host integration target is missing")
if FEATURE not in vst3_target.get("required-features", []):
    fail(f"vst3_host must require the explicit {FEATURE!r} feature")

routine_files = [ROOT / "tools" / "check.sh", ROOT / ".github" / "workflows" / "engine.yml"]
for path in routine_files:
    contents = path.read_text(encoding="utf-8")
    if "--all-features" in contents:
        fail(f"{path.relative_to(ROOT)} may not enable all features")
    if FEATURE in contents:
        # The workflow documents the policy in comments, so only reject an
        # actual Cargo feature argument rather than every textual mention.
        for line in contents.splitlines():
            command = line.strip()
            if command.startswith(("cargo ", "- run: cargo ")) and FEATURE in command:
                fail(f"{path.relative_to(ROOT)} explicitly enables {FEATURE!r}")

print("test policy valid: VST3 integration remains explicit and opt-in")
