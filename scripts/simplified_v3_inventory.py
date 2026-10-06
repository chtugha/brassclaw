#!/usr/bin/env python3
"""Generate the reproducible Phase-0 source inventory; never mutate product data.

References are evidence, not authorization to delete a scope. Consumers must be
reviewed for every semantic use; a file may use one ID in several ways.
"""
from __future__ import annotations

import argparse
import json
from pathlib import Path
import re
import subprocess
import tomllib

ROOT = Path(__file__).resolve().parents[1]
SYMBOLS = re.compile(
    r"\b(?:ResourceScope|ExecutionContext|WebUiAuthenticatedCaller|TurnScope|"
    r"RuntimeProfile|DeploymentMode|TrustClass|Actor|Principal|"
    r"(?:Tenant|User|Project|Agent|Thread|Invocation)Id|"
    r"(?:tenant|user|project|agent|thread|invocation)_id|"
    r"CapabilityGrant|CapabilityLease|ApprovalLease|AcceptedMessageRef|"
    r"LimitedTracker|MontySession|prepare_monty_session|drive_to_yield|"
    r"BRASSCLAW_[A-Z_]*(?:PROFILE|DEPLOYMENT)[A-Z_]*)\b"
)
EXTENSIONS = {".rs", ".sql", ".toml", ".js", ".ts", ".tsx", ".py", ".yaml", ".yml"}


def inventory(root: Path) -> dict:
    paths = sorted(subprocess.check_output(
        ["rg", "--files", "--hidden", "-g", "!.git", "-g", "!target", "-g", "!node_modules"],
        cwd=root, text=True).splitlines())
    references, routes, placeholders, settings_sources, tests, manifests = [], [], [], [], [], []
    for relative in paths:
        path = Path(relative)
        if path.suffix not in EXTENSIONS or relative == "scripts/simplified_v3_inventory.py":
            continue
        source = (root / path).read_text(encoding="utf-8")
        if path.name == "Cargo.toml":
            manifest = tomllib.loads(source)
            manifests.append({"path": relative, "package": manifest.get("package", {}).get("name"),
                              "features": manifest.get("features", {}),
                              "default_features": manifest.get("features", {}).get("default", [])})
        if "/tests/" in relative or relative.startswith("tests/") or relative.startswith(".github/workflows/"):
            tests.append(relative)
        if any(term in relative for term in ("config", "settings", "runtime_policy", "crypto.rs")):
            settings_sources.append(relative)
        for number, line in enumerate(source.splitlines(), 1):
            symbols = sorted(set(SYMBOLS.findall(line)))
            if symbols:
                references.append({"path": relative, "line": number, "symbols": symbols,
                                   "consumer_review": "required", "source": line.strip()})
            if ("webui" in relative or "ingress" in relative) and re.search(r"\.(?:route|nest)\s*\(|/api/", line):
                routes.append({"path": relative, "line": number, "source": line.strip()})
            if ("webui" in relative or "ingress" in relative) and re.search(r"placeholder|not implemented|TODO.*(?:endpoint|route)|501", line, re.I):
                placeholders.append({"path": relative, "line": number, "source": line.strip()})
    return {"schema_version": 1,
            "classification": "References require semantic review for authorization, storage/FK, AAD, audit/replay and UI filtering; no heuristic classification is authoritative.",
            "counts": {"reference_lines": len(references), "routes": len(routes), "placeholder_candidates": len(placeholders),
                       "test_and_ci_files": len(tests), "manifests": len(manifests)},
            "references": references, "route_candidates": routes, "placeholder_candidates": placeholders,
            "settings_source_candidates": settings_sources, "test_and_ci_files": tests, "cargo_manifests": manifests}


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", required=True, type=Path)
    args = parser.parse_args()
    report = inventory(ROOT)
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(report, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")
    print(json.dumps(report["counts"], sort_keys=True))


if __name__ == "__main__":
    main()
