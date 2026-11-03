#!/usr/bin/env python3
"""On the native Mac, verify the captured target pg_config report against its archive."""
import argparse
import hashlib
from pathlib import Path
import re
import subprocess
import sys
import tarfile
import tempfile

ROOT = Path(__file__).resolve().parents[1]
CRATE = ROOT / "crates/brassclaw_embedded_postgres"


def verify(archive):
    if sys.platform != "darwin":
        raise RuntimeError("The target pg_config must be executed natively on macOS")
    metadata = (CRATE / "pg_config-aarch64-apple-darwin.txt").read_text()
    expected = re.search(r'"aarch64-apple-darwin"\s*=>\s*\{\s*Some\("([0-9a-f]{64})"\)',
                         (CRATE / "build.rs").read_text()).group(1)
    with Path(archive).open("rb") as stream:
        actual = hashlib.file_digest(stream, "sha256").hexdigest()
    if actual != expected or f"# archive-sha256: {expected}\n" not in metadata:
        raise ValueError("Archive identity differs from the product pin or captured metadata")
    with tempfile.TemporaryDirectory(prefix="brassclaw-native-pg-config-") as temporary:
        with tarfile.open(archive) as source:
            source.extractall(temporary, filter="data")
        installations = list(Path(temporary).glob("*/bin/pg_config"))
        if len(installations) != 1:
            raise ValueError("Expected exactly one target pg_config")
        binary = installations[0].resolve()
        root = binary.parent.parent
        with binary.open("rb") as stream:
            binary_sha = hashlib.file_digest(stream, "sha256").hexdigest()
        if f"# pg_config-sha256: {binary_sha}\n" not in metadata:
            raise ValueError("Target binary identity differs")
        lines = [line for line in metadata.splitlines() if not line.startswith("#")]
        report = subprocess.check_output([str(binary)], text=True).replace(str(root), "@PGROOT@")
        if report != "\n".join(lines) + "\n":
            raise ValueError("Captured full report differs from the actual target binary")
        for line in lines:
            key, value = line.split(" = ", 1)
            result = subprocess.check_output([str(binary), "--" + key.lower()], text=True)
            if result != value.replace("@PGROOT@", str(root)) + "\n":
                raise ValueError(f"Option --{key.lower()} differs from captured metadata")
        return {"archive_sha256": actual, "pg_config_sha256": binary_sha,
                "verified_options": len(lines), "result": "native full-report and every option match"}


if __name__ == "__main__":
    import json
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("archive", type=Path)
    print(json.dumps(verify(parser.parse_args().archive), indent=2))
