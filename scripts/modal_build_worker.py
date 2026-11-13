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
import stat
import tomllib
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


APPLE_TARGET = "aarch64-apple-darwin"
APPLE_TEST_MODE = "apple-composition-lib-test-build/1"
APPLE_BUILD_MODE = "apple-cli-build/1"
TEST_PACKAGE = "brassclaw_reborn_composition"


def command_options(command, target=None):
    """Bounded remote Cargo grammar; no aliases, config overrides or test args."""
    if not isinstance(command, list) or not all(isinstance(arg, str) and arg and "\0" not in arg for arg in command):
        raise ValueError("Cargo command must be an argument list")
    if len(command) < 2 or command[0] != "cargo" or command[1] not in {"build", "check", "clippy", "test"}:
        raise ValueError("Only direct cargo build/check/clippy/test(--no-run) are permitted")
    switches = {"--locked", "--offline", "--frozen", "--release", "--lib", "--bins",
                "--all-features", "--no-default-features", "--verbose", "-v", "-vv"}
    values = {"-p": "--package", "--package": "--package", "--bin": "--bin",
              "--features": "--features", "-F": "--features", "--profile": "--profile", "--target": "--target"}
    if command[1] in {"check", "clippy"}:
        switches |= {"--tests", "--all-targets", "--benches"}
        values.update({"--test": "--test", "--bench": "--bench"})
    if command[1] == "test":
        switches |= {"--no-run"}
        values["--message-format"] = "--message-format"
    options = {}
    args = iter(command[2:])
    for arg in args:
        if arg == "--" and command[1] == "clippy":
            if list(args) != ["-D", "warnings"]:
                raise ValueError("Only -D warnings may be forwarded to Clippy")
            break
        flag, sep, inline = arg.partition("=")
        if flag in switches and not sep:
            options.setdefault(flag, []).append(True)
            continue
        if flag not in values:
            raise ValueError(f"Unsupported/overriding Cargo argument: {arg}")
        value = inline if sep else next(args, "")
        if not value or value.startswith("-"):
            raise ValueError("Missing Cargo option value")
        options.setdefault(values[flag], []).append(value)
    if "--locked" not in options:
        raise ValueError("Remote Cargo requires --locked")
    targets = options.get("--target", [])
    if len(targets) > 1 or (target is not None and targets and targets != [target]):
        raise ValueError("Command target differs from compiler image")
    if command[1] == "test":
        if "--no-run" not in options:
            raise ValueError("Remote cargo test requires --no-run")
        # --profile is valid Cargo syntax. Other profiles/targets/harnesses need
        # their own connected collection + native execution qualification.
        if (target not in {None, APPLE_TARGET} or targets != [APPLE_TARGET]
                or options.get("--package") != [TEST_PACKAGE] or options.get("--lib") != [True]
                or any(key in options for key in {"--release", "--bins", "--bin"})
                or options.get("--profile", ["test"]) != ["test"]):
            raise ValueError("Test preparation supports only Apple composition --lib with the test profile")
        formats = options.get("--message-format", [])
        if len(formats) > 1 or any(value not in {"json", "json,json-diagnostic-short",
                "json,json-diagnostic-rendered-ansi", "json-render-diagnostics"} for value in formats):
            raise ValueError("Test artifacts require a supported Cargo JSON message format")
    return options


def validate_command(command, target=None):
    command_options(command, target)
    if command[1] != "test":
        return list(command)
    # Normalize before admission; retain the original separately. The worker
    # revalidates that this is the command actually passed to Popen.
    normalized = []
    args = iter(command)
    for arg in args:
        if arg == "--message-format":
            next(args)
        elif not arg.startswith("--message-format="):
            normalized.append(arg)
    return [*normalized, "--message-format=json"]


def command_mode(command, target):
    options = command_options(command, target)
    if command[1] == "test":
        return APPLE_TEST_MODE
    if target == APPLE_TARGET:
        if (command[1] != "build" or options.get("--package") != ["brassclaw_reborn_cli"]
                or options.get("--bin") != ["brassclaw-reborn"]
                or any(key in options for key in {"--profile", "--lib", "--bins"})):
            raise ValueError("The qualified Apple path builds -p brassclaw_reborn_cli --bin brassclaw-reborn using dev or --release")
        return APPLE_BUILD_MODE
    return "linux-compile/1"


def worker_identity():
    # The executing module is loaded from immutable /opt/brassclaw, not sync.
    return {"modal_build_worker.py": digest_file(__file__),
            "modal_source_sync.py": digest_file(Path(__file__).with_name("modal_source_sync.py"))}


def test_subject(repository, command):
    repository = Path(repository).resolve(strict=True)
    manifest_path = repository / "crates" / TEST_PACKAGE / "Cargo.toml"
    source_path = manifest_path.parent / "src/lib.rs"
    if (not manifest_path.is_file() or manifest_path != manifest_path.resolve(strict=True)
            or not source_path.is_file() or source_path != source_path.resolve(strict=True)):
        raise ValueError("Test manifest/source must be regular canonical files")
    manifest = tomllib.loads(manifest_path.read_text(encoding="utf-8"))
    package, library = manifest["package"], manifest.get("lib", {})
    if (package["name"] != TEST_PACKAGE or not isinstance(package["version"], str)
            or library.get("harness", True) is not True or library.get("test", True) is not True
            or library.get("proc-macro", False) or library.get("crate-type", ["lib"]) != ["lib"]):
        raise ValueError("Unqualified composition test harness")
    name = library.get("name", TEST_PACKAGE)
    if name != TEST_PACKAGE or library.get("path", "src/lib.rs") != "src/lib.rs":
        raise ValueError("Unqualified composition library target")
    options = command_options(command, APPLE_TARGET)
    features = manifest.get("features", {})
    selected = set(features) if "--all-features" in options else set()
    if "--no-default-features" not in options and "default" in features:
        selected.add("default")
    for group in options.get("--features", []):
        for feature in re.split(r"[,\s]+", group):
            if feature not in features:
                raise ValueError("Only named composition features are supported for test preparation")
            selected.add(feature)
    pending = list(selected)
    while pending:
        for dependency in features[pending.pop()]:
            if dependency in features and dependency not in selected:
                selected.add(dependency)
                pending.append(dependency)
            elif "/" not in dependency and not dependency.startswith("dep:") and dependency not in features:
                raise ValueError("Unqualified implicit feature expansion")
    return {"manifest_path": str(manifest_path), "src_path": str(manifest_path.parent / "src/lib.rs"),
            "name": name, "version": package["version"], "features": sorted(selected),
            "contract": {"package": package["name"], "version": package["version"], "target_name": name,
                         "target_kind": ["lib"], "profile": "test", "features": sorted(selected),
                         "manifest_path": manifest_path.relative_to(repository).as_posix(),
                         "manifest_sha256": digest_file(manifest_path)},
            "package_ids": [manifest_path.parent.as_uri().replace("file:", "path+file:", 1) + "#" + suffix
                            for suffix in [package["version"], package["name"] + "@" + package["version"]]]}


def snapshot_executable(binary, target_root, artifact_dir):
    """Copy a regular, non-symlink Mach-O arm64 executable from exact Cargo output."""
    binary, target_root = Path(binary), Path(target_root)
    if not binary.is_absolute() or binary != binary.resolve(strict=True):
        raise ValueError("Artifact path must be canonical without symlinks")
    binary.relative_to(target_root.resolve(strict=True))
    selected = binary.lstat()
    if not stat.S_ISREG(selected.st_mode) or not selected.st_mode & 0o111 or selected.st_size < 32:
        raise ValueError("Artifact is not a nonempty regular executable")
    descriptor = os.open(binary, os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK)
    with os.fdopen(descriptor, "rb") as source:
        before = os.fstat(source.fileno())
        if ((before.st_dev, before.st_ino) != (selected.st_dev, selected.st_ino)
                or not stat.S_ISREG(before.st_mode) or not before.st_mode & 0o111 or before.st_size < 32):
            raise ValueError("Artifact is not a nonempty regular executable")
        header = source.read(32)
        if struct.unpack("<IIII", header[:16]) != (0xfeedfacf, 0x0100000c, 0, 2):
            # arm64 CPU subtype may use capability bits; accept ARM64_ALL (0)
            # only for this initial architecture restriction.
            raise ValueError("Artifact is not a Mach-O arm64 executable")
        source.seek(0)
        destination = artifact_dir / binary.name
        with destination.open("xb") as output:
            shutil.copyfileobj(source, output)
            output.flush()
            os.fsync(output.fileno())
        after = os.fstat(source.fileno())
    if (before.st_dev, before.st_ino, before.st_size, before.st_mtime_ns, before.st_ctime_ns) != (
            after.st_dev, after.st_ino, after.st_size, after.st_mtime_ns, after.st_ctime_ns):
        raise ValueError("Artifact changed during snapshot")
    if binary.stat(follow_symlinks=False) != after or binary != binary.resolve(strict=True):
        raise ValueError("Artifact path changed during snapshot")
    destination.chmod(0o500)
    return {"sha256": digest_file(destination), "size": destination.stat().st_size,
            "file": "Mach-O 64-bit arm64 executable"}


def collect_test_artifacts(messages, finished, subject, cache, job, request):
    # Fresh=true is valid: Cargo's invocation still selected this exact artifact.
    if finished != [{"reason": "build-finished", "success": True}]:
        raise ValueError("Missing/ambiguous successful Cargo build-finished message")
    selected = []
    for message in messages:
        profile = message.get("profile", {})
        if profile.get("test") is not True:
            continue
        if not message.get("executable"):
            raise ValueError("Cargo test artifact has no executable")
        target = message.get("target", {})
        if (message.get("package_id") not in subject["package_ids"]
                or message.get("manifest_path") != subject["manifest_path"]
                or target.get("name") != subject["name"] or target.get("kind") != ["lib"]
                or target.get("crate_types") != ["lib"] or target.get("src_path") != subject["src_path"]
                or target.get("test") is not True or message.get("features") != subject["features"]
                or type(message.get("fresh")) is not bool):
            raise ValueError("Unexpected Cargo test artifact identity/target/features")
        selected.append(message)
    if len(selected) != 1:
        raise ValueError("Expected exactly one composition library test artifact")
    message = selected[0]
    binary = Path(message["executable"])
    expected_parent = cache / "target" / APPLE_TARGET / "debug/deps"
    if binary.parent != expected_parent or str(binary) not in message.get("filenames", []):
        raise ValueError("Test executable differs from requested target/profile")
    artifact_dir = job / "artifacts"
    artifact_dir.mkdir(mode=0o700)
    artifact = snapshot_executable(binary, cache / "target", artifact_dir)
    artifact.update({key: message[key] for key in ("package_id", "manifest_path", "target", "profile", "features", "fresh", "executable")})
    artifact.update(image_id=request["image_id"], manifest_sha256=request["manifest_sha256"],
                    target_triple=APPLE_TARGET, mode=APPLE_TEST_MODE)
    return {binary.name: artifact}


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
    command = request["command"]
    target = request.get("target_triple")
    normalized = validate_command(request.get("original_command", command), target)
    if command != normalized:
        raise ValueError("Admitted command differs from normalized original")
    mode = command_mode(command, target)
    if mode == APPLE_TEST_MODE and (request.get("mode") != mode or request.get("worker_identity") != worker_identity()):
        raise ValueError("Test mode requires the exact prepared immutable worker")
    started = time.time()
    job_id = str(uuid.UUID(request["job_id"]))
    if not re.fullmatch("[a-zA-Z0-9_-]+", request["image_id"]):
        raise ValueError("Invalid image cache identity")
    storage = Path(storage)
    job = storage / "cache/jobs" / job_id
    job.mkdir(mode=0o700, parents=True, exist_ok=True)
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
        subject = test_subject(cache / "repository", command) if mode == APPLE_TEST_MODE else None
        if subject is not None and subject["contract"] != request.get("test_subject"):
            raise ValueError("Materialized test subject differs from admission")
        messages, finished = [], []
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
                if mode == APPLE_TEST_MODE and line.startswith("{"):
                    try:
                        message = json.loads(line)
                    except json.JSONDecodeError:
                        continue  # Non-Cargo build-script/proc-macro output remains in log.
                    if isinstance(message, dict) and message.get("reason") == "compiler-artifact":
                        messages.append(message)
                    elif isinstance(message, dict) and message.get("reason") == "build-finished":
                        finished.append(message)
            code = child.wait()
        result = {"job_id": job_id, "manifest_sha256": request["manifest_sha256"],
                  "image_id": request["image_id"], "command": request["command"],
                  "returncode": code, "target": str(cache / "target"),
                  "started_at": started, "duration_secs": time.time() - started,
                  "toolchain": load_record("/opt/brassclaw/toolchain.json") if Path("/opt/brassclaw/toolchain.json").exists() else None}
        if mode == APPLE_TEST_MODE:
            result.update(original_command=request["original_command"], mode=mode,
                          worker_identity=worker_identity(), test_subject=subject["contract"])
        artifacts = {}
        command = request["command"]
        apple_target = request.get("target_triple") == "aarch64-apple-darwin"
        if code == 0 and apple_target and command[1] == "build":
            # Product binary collector: only for explicit 'build' on Apple target.
            profile = "release" if "--release" in command else "debug"
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
        elif code == 0 and mode == APPLE_TEST_MODE:
            artifacts = collect_test_artifacts(messages, finished, subject, cache, job, request)
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
