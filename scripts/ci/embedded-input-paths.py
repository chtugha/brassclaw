#!/usr/bin/env python3
"""List actual immutable release/seed inputs for CI scope, without compiling.

The release inventory includes archival/development-only sources too. Its
model-view disposition does not say whether Rust embeds the file. Read the
owning include_str!/include_bytes! declarations instead, and check inventory
coverage so an owner change cannot silently drop inputs from CI.
"""

import json
from pathlib import Path
import re

ROOT = Path(__file__).resolve().parents[2]
OWNERS = ("prefix_release_documents.rs", "prefix_reference_seeds.rs",
          "prefix_installation_baseline.rs", "builtin_bootstrap.rs")


def embedded_paths(root=ROOT):
    paths = set()
    for name in OWNERS:
        owner = root / "crates/brassclaw_reborn_composition/src" / name
        for relative in re.findall(r'include_(?:str|bytes)!\(\s*"([^"\n]+)"\s*\)', owner.read_text(encoding="utf-8")):
            path = (owner.parent / relative).resolve().relative_to(root.resolve()).as_posix()
            paths.add(path)
    inventory = json.loads((root / "scripts/prefix/release-reference-inventory.json").read_text(encoding="utf-8"))
    if inventory["format"] != "brassclaw-prefix-source-inventory/1":
        raise ValueError("Unsupported release inventory")
    missing = {entry["path"] for entry in inventory["sources"]} - paths
    if missing:
        raise ValueError(f"Release inventory has inputs without literal embedding coverage: {sorted(missing)}")
    return paths


if __name__ == "__main__":
    print("\n".join(sorted(embedded_paths())))
