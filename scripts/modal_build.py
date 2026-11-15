#!/usr/bin/env python3
"""Prepare a reusable compiler image; run explicitly approved ephemeral builds.

setup creates/publishes the toolchain image, without compiling the project.
run requires an explicit Modal choice; activation and host resource policy apply.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import math
import os
from pathlib import Path
import subprocess
import stat
import tempfile
import re
import shutil
import signal
import sys
import tomllib
import uuid

from modal_build_worker import (encoded, validate_command, command_mode, worker_identity,
                                test_subject, worker_command, APPLE_TEST_MODE, APPLE_BUILD_MODE)
from build_settings import modal_resources, load_settings
from modal_source_sync import ROOT, MANIFEST_PATH, modal_lock, assert_remote_settled, sync_source, verify_source


def records_root():
    return Path(os.environ.get("CARGO_TARGET_DIR", str(ROOT / "target"))) / "modal-build"


def write_record(path, value):
    path.parent.mkdir(parents=True, exist_ok=True)
    temporary = path.with_suffix(".tmp")
    with temporary.open("wb") as stream:
        stream.write(encoded(value))
        stream.flush()
        os.fsync(stream.fileno())
    temporary.replace(path)


def rust_version():
    version = tomllib.loads((ROOT / "Cargo.toml").read_text())["package"]["rust-version"]
    return version + ".0" if version.count(".") == 1 else version


def image_name(platform="linux", config=None):
    return "brassclaw-rust:" + compiler_version(platform, config) + ("-apple-arm64" if platform == "apple" else "")


def compiler_version(platform, config=None):
    if platform == "apple":
        from modal_apple_toolchain import RUST
        if load_settings("modal", platform, config).get("compiler", RUST) != RUST:
            raise ValueError("Apple compiler selection must match the pinned OSXCross image compiler")
        return RUST
    version = load_settings("modal", platform, config).get("compiler")
    if not isinstance(version, str) or re.fullmatch(r"[0-9]+\.[0-9]+\.[0-9]+", version) is None:
        raise ValueError("Select an exact Linux compiler in build-environments.toml; the MSRV is not a compiler selection")
    return version


def setup(args):
    import modal

    version = compiler_version(args.platform, args.config)
    identity = worker_identity()
    image = (
        modal.Image.from_registry(f"rust:{version}-slim-bookworm", add_python="3.13")
        .apt_install("build-essential", "pkg-config", "libssl-dev", "cmake", "clang",
                     "libclang-dev", "libdbus-1-dev", "libsecret-1-dev", "git", "curl",
                     "ca-certificates", "rsync")
        .run_commands(f"rustup component add clippy rustfmt --toolchain {version}",
                      "rustup component list --installed", "rustc --version")
        .add_local_file(ROOT / "scripts/modal_source_sync.py", "/opt/brassclaw/modal_source_sync.py", copy=True)
        .add_local_file(ROOT / "scripts/modal_build_worker.py", "/opt/brassclaw/modal_build_worker.py", copy=True)
    )
    config = None
    if args.platform == "apple":
        from modal_apple_toolchain import configuration, image as apple_image
        config = configuration(args.sdk)
        image = apple_image(args.sdk, config)
    app = modal.App("brassclaw-build-setup", include_source=False)
    with modal.enable_output(), app.run(environment_name=args.environment):
        image.build(app)
        image.publish(image_name(args.platform, args.config), environment_name=args.environment)
        if worker_identity() != identity:
            raise ValueError("Worker inputs changed during image preparation")
        result = {"worker_identity": identity,
                  "supported_modes": [APPLE_BUILD_MODE, APPLE_TEST_MODE] if args.platform == "apple" else ["linux-compile/1"],
                  "environment": args.environment, "image": image_name(args.platform, args.config),
                  "image_id": image.object_id, "rust_version": version,
                  "app_id": app.app_id, "toolchain": config, "platform": args.platform, "scope": "compiler image prepared; no project build/tests"}
        write_record(records_root() / f"image-{args.platform}.json", result)
        print(json.dumps(result, indent=2))


def run(args):
    for key, value in modal_resources(args).items():
        setattr(args, key, value)
    if (not args.confirmed_modal or (args.estimated_local_secs is not None
            and (not math.isfinite(args.estimated_local_secs) or args.estimated_local_secs < 0))):
        raise ValueError("An explicit Modal choice and a finite nonnegative runtime estimate, when supplied, are required")
    if getattr(args, "tests", None):
        if args.platform != "apple" or sys.platform != "darwin":
            raise ValueError("Automatic native test execution requires the Apple target and a macOS controller")
        validate_native_selection(args)
        native_environment(args)  # Reject bad fixture inputs before any paid work.
    command = args.cargo
    if command and command[0] == "--":
        command = command[1:]
    target = "aarch64-apple-darwin" if args.platform == "apple" else "x86_64-unknown-linux-gnu"
    original_command = list(command)
    command = validate_command(original_command, target)
    mode = command_mode(command, target)
    if getattr(args, "tests", None) and mode != APPLE_TEST_MODE:
        raise ValueError("--native-test requires an Apple composition test build")
    test_contract = test_subject(ROOT, command)["contract"] if mode == APPLE_TEST_MODE else None
    # This worker is imported from /opt/brassclaw in the immutable image. Syncing
    # repository source cannot upgrade it. Old product image receipts retain
    # their product scope; they cannot authorize this new test protocol.
    assert_remote_settled()
    if args.platform == "apple":
        prepared = json.loads((records_root() / f"image-{args.platform}.json").read_bytes())
        if (prepared.get("worker_identity") != worker_identity()
                or mode not in prepared.get("supported_modes", [])
                or prepared.get("rust_version") != compiler_version(args.platform)
                or prepared.get("environment") != args.environment):
            raise ValueError("Runtime packages require preparation of this exact worker/image; old product images are inapplicable")
    if not args.qualification:
        require_activation(args.platform, mode)
    import modal

    # One remote critical section includes sync, submission and terminal receipt.
    # Local Cargo has a separate lock and never writes this host's remote cache.
    with modal_lock():
        assert_remote_settled()
        sync_source(args.environment, args.volume)
        jobs = records_root() / "jobs"
        volume = modal.Volume.from_name(args.volume, environment_name=args.environment)
        manifest_bytes = b"".join(volume.read_file(MANIFEST_PATH))
        manifest = json.loads(manifest_bytes)
        verify_source(ROOT, manifest)
        prepared = json.loads((records_root() / f"image-{args.platform}.json").read_bytes())
        if prepared["rust_version"] != compiler_version(args.platform, args.config):
            raise ValueError("Prepared image Rust version differs from the selected compiler")
        if prepared["environment"] != args.environment:
            raise ValueError("Prepared image environment differs")
        # Named images are lazy and cannot be hydrated on demand. Use the
        # immutable ID returned by the actual successful image build.
        image = modal.Image.from_id(prepared["image_id"])
        if image.object_id != prepared["image_id"]:
            raise ValueError("Resolved image differs from prepared immutable image")
        if not args.qualification and require_activation(args.platform, mode)["image_id"] != image.object_id:
            raise ValueError("Named image differs from the explicitly activated image")
        job_id = str(uuid.uuid4())
        request = {"job_id": job_id, "command": command, "image_id": image.object_id,
                   "manifest_sha256": hashlib.sha256(manifest_bytes).hexdigest(),
                   "cpu": args.cpu, "cpu_limit": args.cpu_limit,
                   "memory_mib": args.memory_mib, "memory_limit_mib": args.memory_limit_mib,
                   "timeout_secs": args.timeout_secs,
                   "shutdown_grace_secs": args.shutdown_grace_secs, "jobs": args.jobs,
                   "nonpreemptible": args.nonpreemptible,
                   "allow_parallel_hosts": True, "lock_scope": "modal",
                   "target_triple": target, "volume": args.volume, "environment": args.environment,
                   "estimated_local_secs": args.estimated_local_secs}
        if args.platform == "apple":
            if prepared.get("worker_identity") != worker_identity() or mode not in prepared.get("supported_modes", []):
                raise ValueError("Runtime package worker changed before admission")
            request.update(runtime_package=1, worker_identity=worker_identity(), mode=mode,
                           original_command=original_command)
        if mode == APPLE_TEST_MODE:
            if (prepared.get("worker_identity") != worker_identity()
                    or mode not in prepared.get("supported_modes", [])):
                raise ValueError("Prepared worker changed before admission")
            if test_subject(ROOT, command)["contract"] != test_contract:
                raise ValueError("Test subject changed before admission")
            request.update(original_command=original_command, mode=mode,
                           worker_identity=worker_identity(), test_subject=test_contract)
        directory = (jobs / job_id).resolve()
        directory.mkdir(mode=0o700, parents=True)
        write_record(directory / "request.json", request)
        write_record(directory / "source.json", manifest)
        (directory / "original-runner.py").write_bytes((ROOT / "scripts/modal_build_worker.py").read_bytes())
        baseline = json.loads(b"".join(volume.read_file("/source/base-manifest.json")))
        write_record(directory / "applied.json", {
            name: {"before": baseline["files"].get(name), "after": manifest["files"].get(name)}
            for name in sorted(set(baseline["files"]) | set(manifest["files"]))
            if baseline["files"].get(name) != manifest["files"].get(name)})
        state = {"state": "not_started", "job_id": job_id, "environment": args.environment}
        write_record(directory / "state.json", state)
        app = modal.App("brassclaw-build", include_source=False)

        @app.function(image=image, serialized=True, include_source=False, volumes={"/build": volume},
                      cpu=(args.cpu, args.cpu_limit), memory=(args.memory_mib, args.memory_limit_mib),
                      timeout=args.timeout_secs, max_containers=1, min_containers=0,
                      buffer_containers=0, retries=0, single_use_containers=True,
                      nonpreemptible=args.nonpreemptible, restrict_modal_access=True)
        def compile_source(original):
            import time
            function_started = time.monotonic()
            import sys

            sys.path.insert(0, "/opt/brassclaw")
            from modal_build_worker import run_build
            import shutil
            import json
            from pathlib import Path
            from uuid import UUID
            evidence = Path("/build/cache/jobs") / str(UUID(original["job_id"]))
            evidence.mkdir(mode=0o700, parents=True, exist_ok=True)
            evidence.chmod(0o700)
            (evidence / "image-provenance.json").write_text(json.dumps({p.name: p.read_text() for p in Path("/opt/brassclaw").glob("*.txt")}))
            for filesystem in ["/build", "/tmp"]:
                usage = shutil.disk_usage(filesystem)
                print(f"Disk preflight {filesystem}: {usage}", flush=True)
                if usage.free < 15 * 1024**3 or usage.used > usage.total * 0.90:
                    raise RuntimeError(f"Insufficient build space on {filesystem}")

            try:
                return run_build("/build", original, volume.commit, started_monotonic=function_started)
            finally:
                volume.commit()

        try:
            with modal.enable_output(), app.run(environment_name=args.environment):
                verify_source(ROOT, manifest)
                state.update(state="submitting", app_id=app.app_id)
                write_record(directory / "state.json", state)
                call = compile_source.spawn(request)
                state.update(state="running", function_call_id=call.object_id)
                write_record(directory / "state.json", state)
                while True:
                    try:
                        result = call.get(timeout=5)
                        break
                    except (modal.exception.FunctionTimeoutError, modal.exception.RemoteError):
                        # A terminal Function result is never an empty poll.
                        # Leave run; status reconciles any durable worker receipt.
                        raise
                    except TimeoutError:
                        continue
                # Remote termination and verified local collection are distinct.
                state.update(state="collection_pending", remote_state="completed")
                write_record(directory / "state.json", state)
                collect_result(volume, directory, request, result)
                state.update(state="completed", returncode=result["returncode"], collection="verified")
                write_record(directory / "state.json", state)
                print(json.dumps(result, indent=2))
        except BaseException:
            if state["state"] not in {"not_started", "collection_pending", "completed"}:
                state["state"] = "unresolved"
                write_record(directory / "state.json", state)
            raise
    # Release the remote cache lock/app before using independent native custody.
    if result["returncode"] == 0 and getattr(args, "tests", None):
        accept_native_tests(args, directory, result)
    return result["returncode"]


def local_artifact_matches(path, artifact):
    """Reuse only the canonical regular original bytes, verified without following links."""
    path = Path(path)
    try:
        selected = path.lstat()
        if (not stat.S_ISREG(selected.st_mode) or selected.st_size != artifact["size"]
                or path != path.resolve(strict=True)):
            return False
        descriptor = os.open(path, os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK)
        with os.fdopen(descriptor, "rb") as source:
            before = os.fstat(source.fileno())
            if not stat.S_ISREG(before.st_mode) or (before.st_dev, before.st_ino) != (selected.st_dev, selected.st_ino):
                return False
            digest = hashlib.file_digest(source, "sha256").hexdigest()
            after = os.fstat(source.fileno())
        def identity(value):
            return (value.st_dev, value.st_ino, value.st_size, value.st_mtime_ns, value.st_ctime_ns)
        return (identity(selected) == identity(before) == identity(after) == identity(path.lstat())
                and path == path.resolve(strict=True) and digest == artifact["sha256"])
    except OSError:
        return False  # Missing/unreadable/nonregular/corrupt files must be recollected.


def atomic_download(volume, remote_path, destination, artifact=None):
    """Private staging + verify + rename; a failed fetch never publishes bytes."""
    destination = Path(destination)
    if destination.parent.is_symlink() or destination.parent != destination.parent.resolve():
        raise ValueError("Collection directory must be canonical without symlinks")
    if artifact is not None and local_artifact_matches(destination, artifact):
        return
    if artifact is not None and (destination.exists() or destination.is_symlink()):
        raise FileExistsError("Artifact conflict: preserve the existing published job bytes; inspect before repair")
    if artifact is not None:
        if type(artifact.get("size")) is not int or artifact["size"] <= 0:
            raise ValueError("Artifact requires a positive exact byte length")
        usage = shutil.disk_usage(destination.parent)
        if usage.free - artifact["size"] < 15 * 1024**3 or usage.used + artifact["size"] > usage.total * 0.90:
            raise RuntimeError("Insufficient artifact volume space; preserve evidence before collecting")
    descriptor, temporary = tempfile.mkstemp(prefix=".download-", dir=destination.parent)
    temporary = Path(temporary)
    try:
        digest, size = hashlib.sha256(), 0
        with os.fdopen(descriptor, "wb") as output:
            for chunk in volume.read_file(remote_path):
                if artifact is not None and size + len(chunk) > artifact["size"]:
                    raise ValueError("Downloaded artifact checksum/size differs")
                output.write(chunk)
                digest.update(chunk)
                size += len(chunk)
            output.flush()
            os.fsync(output.fileno())
        if artifact is not None:
            if digest.hexdigest() != artifact["sha256"] or size != artifact["size"]:
                raise ValueError("Downloaded artifact checksum/size differs")
            temporary.chmod(0o500)
        if artifact is None:
            temporary.replace(destination)
        else:
            # Exclusive publication, including a competing publisher after preflight.
            os.link(temporary, destination, follow_symlinks=False)
    finally:
        temporary.unlink(missing_ok=True)


def collect_result(volume, directory, original, result):
    """One collector shared by initial run and status; never submits work."""
    prefix = f'/cache/jobs/{original["job_id"]}'
    retained = json.loads(b"".join(volume.read_file(prefix + "/completed.json")))
    if result != retained or any(result[key] != original[key] for key in (
            "job_id", "command", "manifest_sha256", "image_id")):
        raise ValueError("Remote completion differs from retained original build")
    if original.get("mode") == APPLE_TEST_MODE:
        if any(result.get(key) != original[key] for key in ("mode", "original_command", "worker_identity", "test_subject")):
            raise ValueError("Remote test mode/worker/subject differs from original")
        artifacts = result.get("artifacts", {})
        tests = {name: value for name, value in artifacts.items() if name.startswith("deps/")}
        if original.get("runtime_package") == 1:
            artifacts = tests
        if result["returncode"] == 0 and len(artifacts) != 1:
            raise ValueError("Successful test build requires one exact artifact")
        for name, artifact in artifacts.items():
            if (artifact.get("mode") != APPLE_TEST_MODE or artifact.get("image_id") != original["image_id"]
                    or artifact.get("manifest_sha256") != original["manifest_sha256"]
                    or artifact.get("features") != original["test_subject"]["features"]
                    or artifact.get("target", {}).get("name") != original["test_subject"]["target_name"]
                    or artifact.get("target", {}).get("kind") != ["lib"]
                    or artifact.get("profile", {}).get("test") is not True
                    or artifact.get("target_triple") != "aarch64-apple-darwin"
                    or "/" in name.removeprefix("deps/") or "\\" in name):
                raise ValueError("Test artifact does not belong to original job")
    packaged = original.get("runtime_package") == 1
    if packaged and result["returncode"] == 0:
        package = result.get("runtime_package", {})
        roles = [artifact.get("role") for artifact in result["artifacts"].values()]
        if (package.get("version") != 1 or roles.count("monty_worker") != 1
                or roles.count("embedded-pgvector") != 1
                or roles.count("test" if original["mode"] == APPLE_TEST_MODE else "product") != 1
                or len(roles) != 3 or result.get("companion_command") != worker_command(original["target_triple"])):
            raise ValueError("Incomplete/mismatched runtime package")
        if ({name for name, artifact in result["artifacts"].items() if artifact["role"] == "monty_worker"} != {"monty_worker"}
                or {name for name, artifact in result["artifacts"].items() if artifact["role"] == "embedded-pgvector"} != {"pgvector.tar.gz"}
                or (original["mode"] == APPLE_BUILD_MODE and "brassclaw-reborn" not in result["artifacts"])):
            raise ValueError("Runtime package layout differs from the installed worker contract")
        if (result.get("worker_identity") != original.get("worker_identity")
                or result.get("mode") != original.get("mode")):
            raise ValueError("Runtime package worker/mode differs from original")
    write_record(directory / "completed.json", result)  # remote completion, not local readiness
    for name, artifact in result.get("artifacts", {}).items():
        # Legacy product pgvector entries retain their original relative-cache
        # metadata semantics. Product executable and test snapshots live here.
        if "/" in name and not packaged:
            continue
        if (re.fullmatch(r"(?:deps/)?[A-Za-z0-9_.-]+", name) is None
                or name.split("/")[-1] in {".", ".."}):
            raise ValueError("Unsafe artifact name")
        (directory / name).parent.mkdir(mode=0o700, exist_ok=True)
        atomic_download(volume, prefix + "/artifacts/" + name, directory / name, artifact)
    for name in ("image-provenance.json", "cargo.log"):
        atomic_download(volume, prefix + "/" + name, directory / name)


def status(args):
    import modal

    with modal_lock():
        path = (records_root() / "jobs" / str(uuid.UUID(args.job_id)) / "state.json").resolve()
        state = json.loads(path.read_bytes())
        if state["state"] == "failed":
            print(json.dumps(state, indent=2))
            return
        original = json.loads(path.with_name("request.json").read_bytes())
        if original["job_id"] != str(uuid.UUID(args.job_id)) or state["job_id"] != original["job_id"]:
            raise ValueError("Local state/request differs from original job UUID")
        volume = modal.Volume.from_name(original["volume"], environment_name=original["environment"])
        # Volume transport errors are never terminal FunctionCall results.
        # A previously returned completion needs no still-live SDK result handle.
        if (not state.get("function_call_id") or state.get("remote_state") == "completed" or state["state"] == "completed"
                or path.with_name("completed.json").exists()):
            result = json.loads(b"".join(volume.read_file(
                f'/cache/jobs/{original["job_id"]}/completed.json')))
        else:
            receipt_error = None
            result_expired = False
            try:
                result = modal.FunctionCall.from_id(state["function_call_id"]).get(timeout=0)
            except modal.exception.OutputExpiredError as error:
                receipt_error = error
                result_expired = True
            except (modal.exception.FunctionTimeoutError, modal.exception.RemoteError) as error:
                receipt_error = error
            except TimeoutError:
                print(json.dumps(state, indent=2))
                return
            if receipt_error is not None:
                # Result expiry does not reveal the worker outcome. A terminal
                # function result can also follow durable worker completion.
                # Volume transport errors stay outside the FunctionCall catch.
                if result_expired:
                    state.update(state="unresolved", reason="SDK result expired; reconcile original worker completion")
                    write_record(path, state)
                else:
                    write_record(path.with_name("terminal.json"), {
                        "function_call_id": state["function_call_id"], "job_id": state["job_id"],
                        "remote_result_exception": type(receipt_error).__name__, "message": str(receipt_error)})
                try:
                    result = json.loads(b"".join(volume.read_file(
                        f'/cache/jobs/{original["job_id"]}/completed.json')))
                except FileNotFoundError:
                    if not result_expired:
                        state.update(state="failed", reason="terminal remote result without a worker completion receipt")
                    write_record(path, state)
                    print(json.dumps(state, indent=2))
                    return
        state.update(state="collection_pending", remote_state="completed")
        write_record(path, state)
        collect_result(volume, path.parent, original, result)
        state.pop("reason", None)
        state.update(state="completed", returncode=result["returncode"], collection="verified")
        write_record(path, state)
        print(json.dumps(state, indent=2))


def require_activation(platform, mode=None):
    suffix = "-test" if mode == APPLE_TEST_MODE else ""
    path = records_root() / f"activation-{platform}{suffix}.json"
    if not path.exists():
        raise ValueError("Modal is DEACTIVATED; only an explicitly authorized --qualification run is allowed")
    record = json.loads(path.read_bytes())
    image = json.loads((records_root() / f"image-{platform}.json").read_bytes())
    if platform == "apple" and (image.get("worker_identity") != worker_identity()
            or (mode or APPLE_BUILD_MODE) not in image.get("supported_modes", [])):
        raise ValueError("Modal is DEACTIVATED for this worker protocol; preserve the historical image/activation receipts")
    if (not record.get("active") or record.get("image_id") != image["image_id"]
            or (platform == "apple" and record.get("mode", APPLE_BUILD_MODE) != (mode or APPLE_BUILD_MODE))
            or (mode == APPLE_TEST_MODE and (record.get("worker_identity") != worker_identity()
                                           or image.get("worker_identity") != worker_identity()))):
        raise ValueError("Modal is DEACTIVATED or the qualified image has changed")
    return record


def validate_native_selection(args):
    tests = getattr(args, "tests", None)
    timeout = getattr(args, "test_timeout_secs", None)
    if (not tests or len(set(tests)) != len(tests) or any(not isinstance(name, str) or not name for name in tests)
            or timeout is None or not math.isfinite(timeout) or timeout <= 0):
        raise ValueError("Native test acceptance needs exact test names and an explicit finite positive timeout")
    return tests, timeout


def native_environment(args):
    environment = os.environ.copy()
    if getattr(args, "test_env", None) is not None:
        supplied = json.loads(args.test_env.read_bytes())
        if not isinstance(supplied, dict) or any(not isinstance(key, str) or not key or "=" in key or "\0" in key
                or not isinstance(value, str) or "\0" in value for key, value in supplied.items()):
            raise ValueError("Fixture environment must be a string mapping")
        environment.update(supplied)
    environment["BRASSCLAW_TEST_SOURCE_ROOT"] = str(ROOT.resolve(strict=True))
    return environment


def accept_native_tests(args, directory, receipt):
    """Run the collected harness directly under existing native custody; no Cargo."""
    from modal_source_sync import cargo_lock
    tests, timeout = validate_native_selection(args)
    if receipt.get("returncode") != 0 or receipt.get("runtime_package", {}).get("version") != 1:
        raise ValueError("Native tests require a complete successful runtime package")
    artifacts = receipt["artifacts"]
    names = [name for name, artifact in artifacts.items() if artifact.get("role") == "test"]
    if len(names) != 1 or not names[0].startswith("deps/") or "monty_worker" not in artifacts:
        raise ValueError("Native test package has no unique harness/normal worker pair")
    for name, artifact in artifacts.items():
        if not local_artifact_matches(directory / name, artifact):
            raise ValueError("Native package artifact changed before execution")
    source = json.loads((directory / "source.json").read_bytes())
    verify_source(ROOT, source)
    environment = native_environment(args)
    binary = directory / names[0]
    checks = []
    with cargo_lock():
        listing = subprocess.run([str(binary), "--list", "--format", "terse"], cwd=ROOT,
                                 env=environment, capture_output=True, text=True, timeout=timeout, check=True)
        available = {line.removesuffix(": test") for line in listing.stdout.splitlines() if line.endswith(": test")}
        if not set(tests) <= available:
            raise ValueError("Requested exact tests do not exist in the collected harness; no tests executed")
        for index, name in enumerate(tests):
            command = [str(binary), "--exact", name, "--nocapture", "--test-threads=1"]
            log_path = directory / f"native-test-{index}.log"
            # Do not replay effects by overwriting an earlier execution's evidence.
            with log_path.open("x") as log:
                process = subprocess.Popen(command, cwd=ROOT, env=environment, stdout=log,
                                           stderr=subprocess.STDOUT, text=True, start_new_session=True)
                try:
                    code = process.wait(timeout=timeout)
                finally:
                    if process.poll() is None:
                        try:
                            os.killpg(process.pid, signal.SIGTERM)
                        except ProcessLookupError:
                            pass  # It exited between poll and the signal.
                        grace = getattr(args, "shutdown_grace_secs", None) or load_settings(
                            "modal", "apple", getattr(args, "config", None))["shutdown_grace_secs"]
                        try:
                            process.wait(timeout=grace)
                        except subprocess.TimeoutExpired:
                            try:
                                os.killpg(process.pid, signal.SIGKILL)
                            except ProcessLookupError:
                                pass
                            process.wait()
            summary = None
            with log_path.open() as log:
                for line in log:
                    match = re.search(r"test result: ok\. (\d+) passed; (\d+) failed; (\d+) ignored;", line)
                    if match:
                        summary = match.groups()
            if code != 0 or summary != ("1", "0", "0"):
                raise ValueError(f"Native test failed/was ignored: {name}; inspect {log_path}")
            checks.append({"name": name, "returncode": code, "log": log_path.name})
    verify_source(ROOT, source)
    for name, artifact in artifacts.items():
        if not local_artifact_matches(directory / name, artifact):
            raise ValueError("Native artifact changed during execution")
    accepted = {"image_id": receipt["image_id"], "worker_identity": receipt["worker_identity"],
                "artifacts": artifacts, "tests": checks,
                "scope": "Only the exact listed native cases and their retained runtime package"}
    write_record(directory / "native-tests-accepted.json", accepted)
    print(json.dumps(accepted, indent=2))


def accept_native(args):
    if sys.platform != "darwin":
        raise ValueError("Native Apple acceptance must run on macOS")
    directory = records_root() / "jobs" / str(uuid.UUID(args.job_id))
    receipt = json.loads((directory / "completed.json").read_bytes())
    if receipt.get("mode") == APPLE_TEST_MODE:
        return accept_native_tests(args, directory, receipt)
    if command_mode(receipt["command"], "aarch64-apple-darwin") != APPLE_BUILD_MODE:
        raise ValueError("Native Apple acceptance requires an Apple product or test package")
    binary = directory / "brassclaw-reborn"
    if receipt.get("runtime_package"):
        for name, artifact in receipt["artifacts"].items():
            if not local_artifact_matches(directory / name, artifact):
                raise ValueError("Native runtime package is incomplete or changed")
    expected = receipt["artifacts"][binary.name]["sha256"]
    with binary.open("rb") as stream:
        actual = hashlib.file_digest(stream, "sha256").hexdigest()
    if actual != expected or receipt["returncode"] != 0:
        raise ValueError("Native artifact does not match successful cross-build")
    binary.chmod(0o755)
    checks = {}
    for name, command in {"file": ["file", str(binary)], "dependencies": ["otool", "-L", str(binary)],
                          "version": [str(binary), "--version"], "help": [str(binary), "--help"]}.items():
        result = subprocess.run(command, capture_output=True, text=True, timeout=30)
        (directory / f"native-{name}.log").write_text(result.stdout + result.stderr)
        if result.returncode != 0:
            raise ValueError(f"Native {name} failed: {result.returncode}")
        checks[name] = {"command": command, "returncode": result.returncode}
    write_record(directory / "native-accepted.json", {"artifact_sha256": actual, "checks": checks,
                 "scope": "native Mach-O inspection and actual CLI --version/--help; no complete product acceptance"})
    print(json.dumps({"native_acceptance": "passed", "artifact_sha256": actual}, indent=2))


def activation(args):
    selected_mode = APPLE_TEST_MODE if getattr(args, "test_mode", False) else None
    path = records_root() / f"activation-{args.platform}{'-test' if selected_mode else ''}.json"
    if args.action == "deactivate":
        write_record(path, {"active": False})
    elif args.action == "activate":
        receipt = json.loads((records_root() / "jobs" / str(uuid.UUID(args.job_id)) / "completed.json").read_bytes())
        image = json.loads((records_root() / f"image-{args.platform}.json").read_bytes())
        if selected_mode:
            if args.platform != "apple":
                raise ValueError("Only the connected Apple composition test mode can be activated")
            accepted = json.loads((records_root() / "jobs" / args.job_id / "native-tests-accepted.json").read_bytes())
            if (receipt.get("mode") != selected_mode or receipt.get("returncode") != 0
                    or receipt.get("image_id") != image["image_id"]
                    or image.get("worker_identity") != worker_identity()
                    or accepted.get("worker_identity") != worker_identity()
                    or accepted.get("image_id") != image["image_id"]
                    or accepted.get("artifacts") != receipt.get("artifacts")
                    or not accepted.get("tests") or any(test.get("returncode") != 0 for test in accepted["tests"])):
                raise ValueError("Test activation requires actual successful exact-package native tests for this worker/image")
            for name, artifact in receipt["artifacts"].items():
                if not local_artifact_matches(records_root() / "jobs" / args.job_id / name, artifact):
                    raise ValueError("Qualified native test package is missing or changed")
            write_record(path, {"active": True, "image_id": image["image_id"], "job_id": args.job_id,
                                "mode": selected_mode, "worker_identity": worker_identity()})
        elif (receipt.get("mode") == APPLE_TEST_MODE or (args.platform == "apple"
                and command_mode(receipt["command"], "aarch64-apple-darwin") != APPLE_BUILD_MODE)):
            raise ValueError("Test mode cannot use product-build activation")
        elif receipt["returncode"] != 0 or receipt["image_id"] != image["image_id"] or not receipt.get("artifacts"):
            raise ValueError("Activation requires a successful qualified build of this exact image")
        if not selected_mode:
            if args.platform == "apple" and (image.get("worker_identity") != worker_identity()
                    or receipt.get("worker_identity") != worker_identity()
                    or receipt.get("runtime_package", {}).get("version") != 1):
                raise ValueError("Product activation requires this exact worker's complete runtime package")
            native = records_root() / "jobs" / args.job_id / "native-accepted.json"
            accepted = json.loads(native.read_bytes())
            if set(accepted.get("checks", {})) != {"file", "dependencies", "version", "help"}:
                raise ValueError("Native acceptance is incomplete")
            if accepted["artifact_sha256"] != receipt["artifacts"]["brassclaw-reborn"]["sha256"]:
                raise ValueError("Native acceptance differs from the cross-built artifact")
            write_record(path, {"active": True, "image_id": image["image_id"], "job_id": args.job_id,
                                "mode": APPLE_BUILD_MODE if args.platform == "apple" else "linux-compile/1"})
    try:
        print(json.dumps({"status": "ACTIVE", **require_activation(args.platform, selected_mode)}, indent=2))
    except (ValueError, FileNotFoundError) as error:
        print(json.dumps({"status": "DEACTIVATED", "reason": str(error)}, indent=2))


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--environment", default="brassclaw")
    parser.add_argument("--volume", default="brassclaw-build")
    parser.add_argument("--platform", choices=["linux", "apple"], default="linux")
    parser.add_argument("--config", type=Path, help="Build-host TOML settings (no cloud access to read)")
    commands = parser.add_subparsers(dest="action", required=True)
    prepare = commands.add_parser("setup")
    prepare.add_argument("--confirmed-paid-setup", action="store_true")
    prepare.add_argument("--sdk", type=Path)
    execute = commands.add_parser("run")
    execute.add_argument("--allow-parallel-hosts", action="store_true", help="Compatibility option; isolated local/Modal overlap is now supported by separate locks")
    execute.add_argument("--qualification", action="store_true", help="Explicitly authorized qualification; does not activate Modal")
    execute.add_argument("--confirmed-modal", action="store_true")
    execute.add_argument("--estimated-local-secs", type=float,
                         help="Optional observed/expected local seconds; no minimum runtime gate")
    execute.add_argument("--native-test", dest="tests", action="append", help="Run this exact collected Apple libtest case locally after build; repeat as needed")
    execute.add_argument("--test-timeout-secs", type=float)
    execute.add_argument("--test-env", type=Path)
    execute.add_argument("--cpu", type=float, help="Reserved cores; also sets limit unless --cpu-limit is given")
    execute.add_argument("--cpu-limit", type=float, help="Maximum cores, at least --cpu")
    execute.add_argument("--memory-mib", type=int, help="Reserved RAM; also sets limit unless --memory-limit-mib is given")
    execute.add_argument("--memory-limit-mib", type=int, help="Hard RAM ceiling in MiB")
    execute.add_argument("--jobs", type=int)
    execute.add_argument("--timeout-secs", type=int)
    execute.add_argument("--shutdown-grace-secs", type=int)
    execute.add_argument("--nonpreemptible", action=argparse.BooleanOptionalAction, default=None)
    execute.add_argument("cargo", nargs=argparse.REMAINDER)
    inspect = commands.add_parser("status")
    inspect.add_argument("job_id")
    commands.add_parser("settings", help="Show resolved target settings without cloud access or compilation")
    for action in ["activation-status", "activate", "deactivate", "accept-native"]:
        control = commands.add_parser(action)
        control.add_argument("--test-mode", action="store_true", help="Use the independent Apple test activation")
        if action in {"activate", "accept-native"}:
            control.add_argument("job_id")
        if action == "accept-native":
            control.add_argument("--test", dest="tests", action="append", help="Exact libtest name; repeat for required cases")
            control.add_argument("--test-timeout-secs", type=float)
            control.add_argument("--test-env", type=Path, help="Local fixture environment JSON; values are never copied into acceptance logs")
    args = parser.parse_args(argv)
    if args.environment != "brassclaw":
        parser.error("This builder is restricted to the brassclaw environment")
    if args.action == "setup":
        if not args.confirmed_paid_setup:
            parser.error("Paid image setup requires an explicit approved build/setup cost decision")
        with modal_lock():
            assert_remote_settled()
            setup(args)
    elif args.action == "run":
        raise SystemExit(run(args))
    elif args.action == "settings":
        print(json.dumps({**modal_resources(args), "compiler": compiler_version(args.platform, args.config)}, indent=2))
    elif args.action == "accept-native":
        accept_native(args)
    elif args.action == "status":
        status(args)
    else:
        activation(args)


if __name__ == "__main__":
    main()
