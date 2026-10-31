"""Prepare exact public tokenizer assets for the independent native test oracle.

No weights, provider configuration, remote code or production selection. Existing
bytes must match the committed fixture; preparation never replaces an old source.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import tempfile
import urllib.request

NAMES = {"config.json", "tokenizer.json", "tokenizer_config.json", "chat_template.jinja", "vocab.json", "merges.txt"}


def verify(path, expected):
    if path.is_symlink() or not path.is_file():
        raise ValueError(f"expected a regular retained file: {path.name}")
    length = 0
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        while block := stream.read(min(1024 * 1024, expected["bytes"] - length + 1)):
            length += len(block)
            if length > expected["bytes"]:
                raise ValueError(f"retained fixture size differs: {path.name}")
            digest.update(block)
    if length != expected["bytes"] or digest.hexdigest() != expected["sha256"]:
        raise ValueError(f"retained fixture bytes differ: {path.name}")


def prepare(destination, manifest, offline, timeout):
    if not re.fullmatch(r"[A-Za-z0-9][A-Za-z0-9._-]*/[A-Za-z0-9][A-Za-z0-9._-]*", manifest["model"]) or not re.fullmatch(r"[a-f0-9]{40}", manifest["revision"]):
        raise ValueError("fixture needs an exact public model revision")
    if set(manifest["files"]) != NAMES:
        raise ValueError("fixture must contain exactly the six declared tokenizer/config files")
    destination.mkdir(parents=True, exist_ok=True)
    for name, expected in sorted(manifest["files"].items()):
        if type(expected["bytes"]) is not int or expected["bytes"] <= 0 or not re.fullmatch(r"[a-f0-9]{64}", expected["sha256"]):
            raise ValueError(f"invalid fixture integrity: {name}")
        target = destination / name
        if target.exists() or target.is_symlink():
            verify(target, expected)
            continue
        if offline:
            raise ValueError(f"missing offline retained file: {name}")
        url = f"https://huggingface.co/{manifest['model']}/resolve/{manifest['revision']}/{name}"
        temporary = None
        try:
            with tempfile.NamedTemporaryFile(dir=destination, prefix=".capture-", delete=False) as output:
                temporary = Path(output.name)
                length = 0
                digest = hashlib.sha256()
                with urllib.request.urlopen(url, timeout=timeout) as response:
                    while block := response.read(min(1024 * 1024, expected["bytes"] - length + 1)):
                        length += len(block)
                        if length > expected["bytes"]:
                            raise ValueError(f"download exceeds exact fixture size: {name}")
                        digest.update(block)
                        output.write(block)
                if length != expected["bytes"] or digest.hexdigest() != expected["sha256"]:
                    raise ValueError(f"download differs from exact fixture: {name}")
            try:
                os.link(temporary, target)
            except FileExistsError:
                verify(target, expected)
        finally:
            if temporary is not None:
                temporary.unlink()
        verify(target, expected)
    return {"model": manifest["model"], "revision": manifest["revision"], "files": manifest["files"], "offline": offline}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("destination", type=Path)
    parser.add_argument("--offline", action="store_true")
    parser.add_argument("--timeout-secs", type=float, default=60)
    args = parser.parse_args()
    if not 0 < args.timeout_secs < float("inf"):
        parser.error("timeout must be finite and positive")
    manifest = json.loads(Path(__file__).with_name("ornith-originals.json").read_text(encoding="utf-8"))
    print(json.dumps(prepare(args.destination, manifest, args.offline, args.timeout_secs), sort_keys=True))


if __name__ == "__main__":
    main()
