#!/usr/bin/env python3
"""Publish an exact source archive to Modal storage, without starting compute.

Usage: python scripts/modal_source_sync.py [--create] [--volume NAME]
Run before compilation when Modal storage is available. A remote compiler must
verify the archive digest, extract into a fresh directory and use its separate
Linux target cache. This utility never starts Cargo or executes tests.
"""

from __future__ import annotations

import argparse
from contextlib import contextmanager
import gzip
import hashlib
import io
import json
import os
from pathlib import Path, PurePosixPath
import stat
import subprocess
import sys
import tarfile
import tempfile


ROOT = Path(__file__).resolve().parents[1]
DEFAULT_VOLUME = "brassclaw-build"
SOURCE_PATH = "/source/repository.tar.gz"
MANIFEST_PATH = "/source/manifest.json"
BASE_SOURCE_PATH = "/source/base.tar.gz"
BASE_MANIFEST_PATH = "/source/base-manifest.json"
PRIVATE_NAMES = {"LOCAL_TEST_ENV.md", ".env", ".modal.toml"}


def git(root: Path, *args: str) -> bytes:
    return subprocess.check_output(["git", "-C", str(root), *args])


def source_paths(root: Path) -> list[str]:
    root = root.resolve()
    names = git(root, "ls-files", "-z", "--cached", "--others", "--exclude-standard")
    paths = sorted(set(os.fsdecode(name) for name in names.split(b"\0") if name))
    result = []
    for name in paths:
        relative = PurePosixPath(name)
        if relative.is_absolute() or ".." in relative.parts or ".git" in relative.parts:
            raise ValueError(f"Unsafe source path: {name}")
        if relative.name in PRIVATE_NAMES:
            raise ValueError(f"Private configuration must be Git-ignored before sync: {name}")
        path = root / name
        if not path.parent.resolve().is_relative_to(root):
            raise ValueError(f"Source parent escapes repository: {name}")
        if path.exists() or path.is_symlink():
            result.append(name)
    return result


def describe(root: Path, name: str) -> dict:
    path = root / name
    info = path.lstat()
    if stat.S_ISLNK(info.st_mode):
        target = os.readlink(path)
        if Path(target).is_absolute():
            raise ValueError(f"Absolute source symlink is not portable: {name}")
        if not (path.parent / target).resolve().is_relative_to(root):
            raise ValueError(f"Source symlink escapes repository: {name}")
        return {"kind": "symlink", "target": target}
    if not stat.S_ISREG(info.st_mode):
        raise ValueError(f"Unsupported source entry (including unexported submodules): {name}")
    with path.open("rb") as stream:
        digest = hashlib.file_digest(stream, "sha256").hexdigest()
    return {
        "kind": "file",
        "mode": 0o755 if info.st_mode & 0o111 else 0o644,
        "size": info.st_size,
        "sha256": digest,
    }


def verify_source(root: Path, manifest: dict) -> None:
    root = root.resolve()
    if git(root, "rev-parse", "HEAD").decode().strip() != manifest["head"]:
        raise ValueError("Repository HEAD changed during synchronization")
    if source_paths(root) != list(manifest["files"]):
        raise ValueError("Source membership changed during synchronization")
    for name, expected in manifest["files"].items():
        if describe(root, name) != expected:
            raise ValueError(f"Source changed during synchronization: {name}")


def capture(root: Path, archive_path: Path, baseline: dict | None = None) -> dict:
    root = root.resolve()
    manifest = {
        "format": "brassclaw-modal-source/2",
        "head": git(root, "rev-parse", "HEAD").decode().strip(),
        "files": {name: describe(root, name) for name in source_paths(root)},
    }
    previous = baseline["files"] if baseline else {}
    selected = {name: entry for name, entry in manifest["files"].items()
                if previous.get(name) != entry}
    manifest["changed_files"] = list(selected)
    manifest["deleted_files"] = sorted(set(previous) - set(manifest["files"]))
    # Deterministic archive; symlinks remain links and executable modes survive.
    with archive_path.open("wb") as raw:
        with gzip.GzipFile(filename="", fileobj=raw, mode="wb", mtime=0) as zipped:
            with tarfile.open(fileobj=zipped, mode="w|", format=tarfile.PAX_FORMAT) as archive:
                for name, entry in selected.items():
                    member = tarfile.TarInfo(name)
                    if entry["kind"] == "symlink":
                        member.type = tarfile.SYMTYPE
                        member.linkname = entry["target"]
                        member.mode = 0o777
                        archive.addfile(member)
                    else:
                        member.mode = entry["mode"]
                        member.size = entry["size"]
                        with (root / name).open("rb") as stream:
                            archive.addfile(member, stream)
    # Verify the archived bytes themselves as well as the final working files.
    with tarfile.open(archive_path, "r:gz") as archive:
        members = archive.getmembers()
        if [member.name for member in members] != list(selected):
            raise ValueError("Archive membership differs from captured source")
        for member in members:
            expected = manifest["files"][member.name]
            if expected["kind"] == "symlink":
                if not member.issym() or member.linkname != expected["target"]:
                    raise ValueError(f"Archived symlink differs: {member.name}")
            else:
                stream = archive.extractfile(member)
                if stream is None or hashlib.file_digest(stream, "sha256").hexdigest() != expected["sha256"]:
                    raise ValueError(f"Archived bytes differ: {member.name}")
    verify_source(root, manifest)
    with archive_path.open("rb") as stream:
        manifest["archive_sha256"] = hashlib.file_digest(stream, "sha256").hexdigest()
    return manifest


@contextmanager
def cargo_lock():
    default = "/private/tmp/brassclaw-target-cargo.lock" if sys.platform == "darwin" else str(
        Path(tempfile.gettempdir()) / "brassclaw-target-cargo.lock"
    )
    with open(os.environ.get("BRASSCLAW_CARGO_LOCK", default), "a+b") as lock:
        if os.name == "nt":
            import msvcrt

            lock.seek(0, os.SEEK_END)
            if lock.tell() == 0:
                lock.write(b"\0")
                lock.flush()
            lock.seek(0)
            msvcrt.locking(lock.fileno(), msvcrt.LK_NBLCK, 1)
        else:
            import fcntl

            fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
        try:
            yield
        finally:
            if os.name == "nt":
                lock.seek(0)
                msvcrt.locking(lock.fileno(), msvcrt.LK_UNLCK, 1)
            else:
                fcntl.flock(lock, fcntl.LOCK_UN)


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--volume", default=os.environ.get("BRASSCLAW_MODAL_VOLUME", DEFAULT_VOLUME))
    parser.add_argument("--environment", default="brassclaw", help="Explicit Modal environment")
    parser.add_argument("--create", action="store_true", help="Create the named Volume if absent; no compute")
    args = parser.parse_args()
    import modal

    with cargo_lock(), tempfile.TemporaryDirectory(prefix="brassclaw-modal-source-") as scratch:
        if args.create:
            modal.Volume.objects.create(
                args.volume, version=2, allow_existing=True, environment_name=args.environment
            )
        volume = modal.Volume.from_name(args.volume, environment_name=args.environment)
        volume.hydrate()  # Actual availability/auth check; never starts a worker.
        try:
            baseline_bytes = b"".join(volume.read_file(BASE_MANIFEST_PATH))
        except FileNotFoundError:
            baseline_bytes = None
        baseline = json.loads(baseline_bytes) if baseline_bytes else None
        if baseline and (baseline.get("format") != "brassclaw-modal-source/2"
                         or baseline.get("source_archive") != BASE_SOURCE_PATH):
            raise ValueError("Unsupported or conflicting remote source baseline")
        archive = Path(scratch) / "repository.tar.gz"
        manifest = capture(ROOT, archive, baseline)
        destination = SOURCE_PATH if baseline else BASE_SOURCE_PATH
        manifest["source_archive"] = destination
        if baseline:
            manifest["base_archive_sha256"] = baseline["archive_sha256"]
            manifest["base_manifest_sha256"] = hashlib.sha256(baseline_bytes).hexdigest()
        encoded = json.dumps(manifest, sort_keys=True, separators=(",", ":")).encode()
        # Archive first, manifest last. No remote compile may run during sync;
        # its preflight must reject any incomplete/mismatched pair.
        with volume.batch_upload(force=True) as upload:
            upload.put_file(archive, destination)
            upload.put_file(io.BytesIO(b""), "/cache/.keep")
            if baseline is None:
                # A base is an immutable historical capture, not the active
                # source pointer. Keep its metadata even if later local edits
                # prevent publishing the current manifest; retry can send a delta.
                upload.put_file(io.BytesIO(encoded), BASE_MANIFEST_PATH)
        verify_source(ROOT, manifest)
        with volume.batch_upload(force=True) as upload:
            upload.put_file(io.BytesIO(encoded), MANIFEST_PATH)
        if b"".join(volume.read_file(MANIFEST_PATH)) != encoded:
            raise ValueError("Remote source manifest readback differs")
        digest = hashlib.sha256()
        for chunk in volume.read_file(destination):
            digest.update(chunk)
        if digest.hexdigest() != manifest["archive_sha256"]:
            raise ValueError("Remote source archive readback differs")
        verify_source(ROOT, manifest)
        print(json.dumps({
            "volume": args.volume,
            "environment": args.environment,
            "source": destination,
            "manifest": MANIFEST_PATH,
            "archive_sha256": manifest["archive_sha256"],
            "files": len(manifest["files"]),
            "changed_files": len(manifest["changed_files"]),
            "deleted_files": len(manifest["deleted_files"]),
            "compressed_bytes": archive.stat().st_size,
            "status": "source upload/readback verified; no compilation or tests executed",
        }, indent=2))


if __name__ == "__main__":
    main()
