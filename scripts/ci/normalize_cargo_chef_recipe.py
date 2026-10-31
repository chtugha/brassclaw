"""Keep cargo-chef's generated targets on Cargo's package-edition contract.

cargo-chef completes inferred targets before serializing its skeleton. This can
emit deprecated per-target editions even when the source uses package editions.
Remove only redundant editions and verify the complete parsed manifest after
each edit. Different target editions remain a build error requiring migration.
This edits generated recipe data only, never the original Cargo manifests.
"""

import copy
import json
from pathlib import Path
import re
import sys
import tomllib


def without_target_editions(manifest):
    result = copy.deepcopy(manifest)
    editions = []
    for kind in ("lib", "bin", "test", "bench", "example"):
        targets = result.get(kind, [] if kind != "lib" else {})
        if isinstance(targets, dict):
            targets = [targets]
        for target in targets:
            if "edition" in target:
                editions.append(target.pop("edition"))
    return result, editions


def normalize_manifest(contents, workspace_edition):
    manifest = tomllib.loads(contents)
    expected, editions = without_target_editions(manifest)
    if not editions:
        return contents, 0
    package_edition = manifest.get("package", {}).get("edition", "2015")
    if package_edition == {"workspace": True}:
        package_edition = workspace_edition
    if not isinstance(package_edition, str) or any(e != package_edition for e in editions):
        raise ValueError("nonredundant target edition requires a source package migration")

    # A regex identifies candidates, never establishes their TOML meaning.
    # Parsed equality protects multiline strings, package/workspace settings,
    # target order and all unrelated fields without adding a TOML serializer.
    lines = contents.splitlines(keepends=True)
    remaining = len(editions)
    for index, line in enumerate(lines):
        if not re.match(r"\s*(?:edition|\"edition\"|'edition')\s*=", line):
            continue
        candidate = "".join(lines[:index] + [""] + lines[index + 1 :])
        try:
            parsed = tomllib.loads(candidate)
        except tomllib.TOMLDecodeError:
            continue
        stripped, retained = without_target_editions(parsed)
        if stripped == expected and len(retained) == remaining - 1:
            lines[index] = ""
            remaining -= 1
    normalized = "".join(lines)
    if remaining or tomllib.loads(normalized) != expected:
        raise ValueError("unsupported generated target edition layout")
    return normalized, len(editions)


def normalize_recipe(recipe):
    result = copy.deepcopy(recipe)
    manifests = result["skeleton"]["manifests"]
    root = next(m for m in manifests if m["relative_path"] == "Cargo.toml")
    workspace = tomllib.loads(root["contents"]).get("workspace", {})
    workspace_edition = workspace.get("package", {}).get("edition")
    total = 0
    for manifest in manifests:
        try:
            manifest["contents"], count = normalize_manifest(manifest["contents"], workspace_edition)
        except (ValueError, TypeError) as error:
            raise ValueError(f"{manifest['relative_path']}: {error}") from error
        total += count
    return result, total


def main():
    path = Path(sys.argv[1])
    recipe = json.loads(path.read_text(encoding="utf-8"))
    normalized, count = normalize_recipe(recipe)
    # No write occurs until every manifest has passed the equivalence check.
    path.write_text(json.dumps(normalized, ensure_ascii=False) + "\n", encoding="utf-8")
    print(f"Migrated {count} redundant generated target editions to package editions")


if __name__ == "__main__":
    main()
