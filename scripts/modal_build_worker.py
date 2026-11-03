"""Linux build worker and source verifier. Importing this module starts no work."""

from __future__ import annotations

import hashlib
import io
import json
import os
from pathlib import Path, PurePosixPath
import re
import shutil
import signal
import subprocess
import tarfile
import tempfile
import time
import threading
import struct
import uuid

from modal_source_sync import describe


def encoded(value):
    return json.dumps(value, sort_keys=True, separators=(",", ":")).encode()


def digest_file(path):
    with Path(path).open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def load_record(path):
    def unique(pairs):
        result = {}
        for key, value in pairs:
            if key in result:
                raise ValueError("Duplicate JSON key")
            result[key] = value
        return result

    return json.loads(Path(path).read_bytes(), object_pairs_hook=unique)


def validate_command(command, target=None):
    if not isinstance(command, list) or not all(isinstance(arg, str) and "\0" not in arg for arg in command):
        raise ValueError("Cargo command must be an argument list")
    if len(command) < 2 or command[0] != "cargo" or command[1] not in {"build", "check", "clippy"}:
        raise ValueError("Only direct cargo build/check/clippy are permitted; no test execution")
    if "--locked" not in command:
        raise ValueError("Remote Cargo requires --locked")
    switches = {"--locked", "--offline", "--frozen", "--release", "--lib", "--bins",
                "--all-features", "--no-default-features", "--verbose", "-v", "-vv"}
    values = {"-p", "--package", "--bin", "--features", "-F", "--profile", "--target"}
    if command[1] in {"check", "clippy"}:
        switches |= {"--tests", "--all-targets", "--benches"}
        values |= {"--test", "--bench"}
    args = iter(command[2:])
    targets = []
    for arg in args:
        if arg == "--" and command[1] == "clippy":
            if list(args) != ["-D", "warnings"]:
                raise ValueError("Only -D warnings may be forwarded to Clippy")
            break
        flag, sep, inline = arg.partition("=")
        if flag in switches and not sep:
            continue
        if flag not in values:
            raise ValueError(f"Unsupported/overriding Cargo argument: {arg}")
        value = inline if sep else next(args, "")
        if not value or value.startswith("-"):
            raise ValueError("Missing Cargo option value")
        if flag == "--target":
            targets.append(value)
    if len(targets) > 1 or (target is not None and targets and targets != [target]):
        raise ValueError("Command target differs from compiler image")


def validate_manifest(record):
    if record.get("format") != "brassclaw-modal-source/2" or not isinstance(record.get("files"), dict):
        raise ValueError("Unsupported source manifest")
    files = record["files"]
    for name, entry in files.items():
        path = PurePosixPath(name)
        if not name or str(path) != name or path.is_absolute() or ".." in path.parts or ".git" in path.parts:
            raise ValueError("Unsafe source path")
        for parent in path.parents:
            if str(parent) in files:
                raise ValueError("A source leaf cannot also be a parent directory")
        if entry.get("kind") == "file":
            if set(entry) != {"kind", "mode", "size", "sha256"}:
                raise ValueError("Unknown file metadata")
            if type(entry["size"]) is not int or entry["size"] < 0 or entry["mode"] not in {0o644, 0o755}:
                raise ValueError("Invalid file metadata")
            if not isinstance(entry["sha256"], str) or re.fullmatch("[0-9a-f]{64}", entry["sha256"]) is None:
                raise ValueError("Invalid source checksum")
        elif entry.get("kind") == "symlink":
            if set(entry) != {"kind", "target"} or not isinstance(entry["target"], str):
                raise ValueError("Invalid symlink metadata")
            if not entry["target"] or PurePosixPath(entry["target"]).is_absolute():
                raise ValueError("Absolute/empty symlink target")
        else:
            raise ValueError("Unsupported source entry")


def source_names(root):
    names = set()
    for directory, folders, files in os.walk(root, followlinks=False):
        for name in list(folders):
            path = Path(directory) / name
            if path.is_symlink():
                folders.remove(name)
                files.append(name)
        names.update(str((Path(directory) / name).relative_to(root)) for name in files)
    return names


def verify_tree(root, files):
    root = Path(root).resolve()
    if source_names(root) != set(files):
        raise ValueError("Materialized source has missing or extra files")
    for name, entry in files.items():
        if describe(root, name) != entry:
            raise ValueError(f"Materialized source differs: {name}")


def materialize(storage, destination, expected_manifest):
    """Validate archives as data and materialize only the final selected files."""
    storage, destination = Path(storage), Path(destination)
    manifest_path = storage / "source/manifest.json"
    if digest_file(manifest_path) != expected_manifest:
        raise ValueError("Current source manifest differs from admitted build")
    manifest = load_record(manifest_path)
    baseline = load_record(storage / "source/base-manifest.json")
    validate_manifest(manifest)
    validate_manifest(baseline)
    if manifest["source_archive"] == "/source/base.tar.gz":
        if manifest != baseline:
            raise ValueError("Initial source differs from baseline")
        archives = [(storage / "source/base.tar.gz", baseline, list(baseline["files"]))]
    elif manifest["source_archive"] == "/source/repository.tar.gz":
        if manifest["base_manifest_sha256"] != digest_file(storage / "source/base-manifest.json"):
            raise ValueError("Baseline manifest differs")
        if manifest["base_archive_sha256"] != baseline["archive_sha256"]:
            raise ValueError("Baseline archive reference differs")
        changed = sorted(name for name, entry in manifest["files"].items()
                         if baseline["files"].get(name) != entry)
        deleted = sorted(set(baseline["files"]) - set(manifest["files"]))
        if manifest["changed_files"] != changed or manifest["deleted_files"] != deleted:
            raise ValueError("Delta declaration differs from exact source")
        archives = [(storage / "source/base.tar.gz", baseline, list(baseline["files"])),
                    (storage / "source/repository.tar.gz", manifest, changed)]
    else:
        raise ValueError("Unsupported source archive path")
    destination.mkdir(parents=True, exist_ok=False)
    for path, record, names in archives:
        allowed_names = set(names)
        if digest_file(path) != record["archive_sha256"]:
            raise ValueError("Source archive checksum differs")
        with tarfile.open(path, "r:gz") as archive:
            seen, seen_names = [], set()
            for member in archive:
                if member.name not in allowed_names or member.name in seen_names:
                    raise ValueError("Unexpected/duplicate archive member")
                seen.append(member.name)
                seen_names.add(member.name)
                entry = record["files"][member.name]
                selected = manifest["files"].get(member.name) == entry
                if entry["kind"] == "symlink":
                    if not member.issym() or member.linkname != entry["target"]:
                        raise ValueError("Archive symlink differs")
                    continue  # Create selected links after all regular files.
                if not member.isfile() or member.size != entry["size"] or member.mode != entry["mode"]:
                    raise ValueError("Archive file shape differs")
                target = destination / member.name
                if selected:
                    target.parent.mkdir(parents=True, exist_ok=True)
                output = target.open("wb") if selected else None
                digest = hashlib.sha256()
                try:
                    with archive.extractfile(member) as stream:
                        while chunk := stream.read(io.DEFAULT_BUFFER_SIZE):
                            digest.update(chunk)
                            if output is not None:
                                output.write(chunk)
                finally:
                    if output is not None:
                        output.close()
                if digest.hexdigest() != entry["sha256"]:
                    raise ValueError("Archived source bytes differ")
                if selected:
                    target.chmod(entry["mode"])
            if seen != names:
                raise ValueError("Archive coverage/order differs")
    for name, entry in manifest["files"].items():
        if entry["kind"] == "symlink":
            target = destination / name
            target.parent.mkdir(parents=True, exist_ok=True)
            target.symlink_to(entry["target"])
    verify_tree(destination, manifest["files"])
    return manifest


def update_cache(staging, destination, manifest):
    destination = Path(destination)
    if destination.is_symlink():
        raise ValueError("Cached source root cannot be a symlink")
    destination.mkdir(parents=True, exist_ok=True)
    # No -t: preserve mtimes of unchanged files, while changed content gets a
    # fresh timestamp. Setting all files to archive epoch could reuse stale Cargo
    # artifacts; resetting all mtimes would needlessly rebuild the whole tree.
    subprocess.run(["rsync", "-rlpc", "--delete", str(staging) + "/", str(destination) + "/"], check=True)
    verify_tree(destination, manifest["files"])


def run_build(storage, request, commit):
    validate_command(request["command"], request.get("target_triple"))
    started = time.time()
    job_id = str(uuid.UUID(request["job_id"]))
    if not re.fullmatch("[a-zA-Z0-9_-]+", request["image_id"]):
        raise ValueError("Invalid image cache identity")
    storage = Path(storage)
    job = storage / "cache/jobs" / job_id
    job.mkdir(parents=True, exist_ok=True)
    original = encoded(request)
    original_path = job / "request.json"
    if original_path.exists():
        if original_path.read_bytes() != original:
            raise ValueError("Build ID was reused for different inputs")
        if (job / "completed.json").exists():
            return load_record(job / "completed.json")
        raise RuntimeError("Original build was interrupted; inspect it before admitting a new build")
    with original_path.open("xb") as stream:
        stream.write(original)
    os.sync()
    commit()
    cache = storage / "cache/images" / request["image_id"]
    child = None
    watchdog = None
    try:
        with tempfile.TemporaryDirectory(prefix="brassclaw-build-") as temporary:
            staged = Path(temporary) / "source"
            manifest = materialize(storage, staged, request["manifest_sha256"])
            update_cache(staged, cache / "repository", manifest)
            (job / "source.json").write_bytes(encoded(manifest))
        environment = os.environ.copy()
        environment["CARGO_TARGET_DIR"] = str(cache / "target")
        environment["CARGO_HOME"] = str(storage / "cache/cargo")
        environment["CARGO_INCREMENTAL"] = "1"
        environment["CARGO_BUILD_JOBS"] = str(request["jobs"])
        with (job / "cargo.log").open("w") as log:
            child = subprocess.Popen(request["command"], cwd=cache / "repository",
                                     env=environment, stdout=subprocess.PIPE,
                                     stderr=subprocess.STDOUT, text=True, start_new_session=True)
            def stop_cargo():
                if child.poll() is None:
                    try:
                        os.killpg(child.pid, signal.SIGTERM)
                        child.wait(timeout=request["shutdown_grace_secs"])
                    except subprocess.TimeoutExpired:
                        os.killpg(child.pid, signal.SIGKILL)
                    except ProcessLookupError:
                        pass
            watchdog = threading.Timer(max(1, request["timeout_secs"] - 120), stop_cargo)
            watchdog.start()
            for line in child.stdout:
                log.write(line)
                log.flush()
                print(line, end="", flush=True)
            code = child.wait()
        result = {"job_id": job_id, "manifest_sha256": request["manifest_sha256"],
                  "image_id": request["image_id"], "command": request["command"],
                  "returncode": code, "target": str(cache / "target"),
                  "started_at": started, "duration_secs": time.time() - started,
                  "toolchain": load_record("/opt/brassclaw/toolchain.json") if Path("/opt/brassclaw/toolchain.json").exists() else None}
        artifacts = {}
        if code == 0 and request.get("target_triple") == "aarch64-apple-darwin":
            profile = "release" if "--release" in request["command"] else "debug"
            binary = cache / "target" / request["target_triple"] / profile / "brassclaw-reborn"
            if binary.exists():
                shape = subprocess.check_output(["file", str(binary)], text=True)
                if "Mach-O 64-bit arm64 executable" not in shape:
                    raise ValueError("Result is not an Apple Silicon executable")
                artifact_dir = job / "artifacts"
                artifact_dir.mkdir()
                shutil.copy2(binary, artifact_dir / binary.name)
                artifacts[binary.name] = {"sha256": digest_file(binary), "size": binary.stat().st_size, "file": shape.strip()}
                for label, arguments in {"headers": ["--macho", "--private-headers"], "libraries": ["--macho", "--dylibs-used"]}.items():
                    (job / f"{label}.txt").write_text(subprocess.check_output(["llvm-objdump", *arguments, str(binary)], text=True))
            pg_archives = list((cache / "target" / request["target_triple"] / profile / "build").glob("brassclaw_embedded_postgres-*/out/pgvector.tar.gz"))
            if not pg_archives or any(p.stat().st_size < 1024 for p in pg_archives):
                raise ValueError("Required pgvector artifact absent or empty")
            for pg in pg_archives:
                with tarfile.open(pg) as archive:
                    names = archive.getnames()
                    libraries = [m for m in archive if m.name in {"lib/vector.so", "lib/vector.dylib"}]
                    if len(libraries) != 1 or "share/extension/vector.control" not in names or "share/extension/vector--0.8.0.sql" not in names:
                        raise ValueError("Incomplete pgvector archive")
                    header = archive.extractfile(libraries[0]).read(8)
                    if header != struct.pack("<II", 0xfeedfacf, 0x0100000c):
                        raise ValueError("pgvector is not Mach-O arm64")
                artifacts[str(pg.relative_to(cache / "target"))] = {"sha256": digest_file(pg), "size": pg.stat().st_size}
        result["artifacts"] = artifacts
        temporary_result = job / "completed.json.tmp"
        temporary_result.write_bytes(encoded(result))
        temporary_result.replace(job / "completed.json")
        os.sync()
        commit()
        return result
    finally:
        if watchdog is not None:
            watchdog.cancel()
        if child is not None and child.poll() is None:
            try:
                os.killpg(child.pid, signal.SIGTERM)
            except ProcessLookupError:
                pass  # The process group has already exited; still reap Cargo.
            try:
                child.wait(timeout=request["shutdown_grace_secs"])
            except subprocess.TimeoutExpired:
                try:
                    os.killpg(child.pid, signal.SIGKILL)
                except ProcessLookupError:
                    pass
                child.wait()
        os.sync()
