#!/usr/bin/env python3
"""Prepare a reusable compiler image; run explicitly approved ephemeral builds.

setup creates/publishes the toolchain image, without compiling the project.
run requires an explicit Local/Modal decision and resource/time settings.
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
import sys
import tomllib
import uuid

from modal_build_worker import (encoded, validate_command, command_mode, worker_identity,
                                test_subject, APPLE_TEST_MODE, APPLE_BUILD_MODE)
from build_settings import modal_resources
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


def image_name(platform="linux"):
    return "brassclaw-rust:" + compiler_version(platform) + ("-apple-arm64" if platform == "apple" else "")


def compiler_version(platform):
    if platform == "apple":
        from modal_apple_toolchain import RUST
        return RUST
    return rust_version()


def setup(args):
    import modal

    version = compiler_version(args.platform)
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
        image.publish(image_name(args.platform), environment_name=args.environment)
        if worker_identity() != identity:
            raise ValueError("Worker inputs changed during image preparation")
        result = {"worker_identity": identity,
                  "supported_modes": [APPLE_BUILD_MODE, APPLE_TEST_MODE] if args.platform == "apple" else ["linux-compile/1"],
                  "environment": args.environment, "image": image_name(args.platform),
                  "image_id": image.object_id, "rust_version": version,
                  "app_id": app.app_id, "toolchain": config, "platform": args.platform, "scope": "compiler image prepared; no project build/tests"}
        write_record(records_root() / f"image-{args.platform}.json", result)
        print(json.dumps(result, indent=2))


def run(args):
    for key, value in modal_resources(args).items():
        setattr(args, key, value)
    command = args.cargo
    if command and command[0] == "--":
        command = command[1:]
    target = "aarch64-apple-darwin" if args.platform == "apple" else "x86_64-unknown-linux-gnu"
    original_command = list(command)
    command = validate_command(original_command, target)
    mode = command_mode(command, target)
    test_contract = test_subject(ROOT, command)["contract"] if mode == APPLE_TEST_MODE else None
    # This worker is imported from /opt/brassclaw in the immutable image. Syncing
    # repository source cannot upgrade it. Old product image receipts retain
    # their product scope; they cannot authorize this new test protocol.
    if mode == APPLE_TEST_MODE:
        prepared = json.loads((records_root() / f"image-{args.platform}.json").read_bytes())
        if (prepared.get("worker_identity") != worker_identity()
                or mode not in prepared.get("supported_modes", [])
                or prepared.get("rust_version") != compiler_version(args.platform)
                or prepared.get("environment") != args.environment):
            raise ValueError("Test mode requires preparation of this exact worker/image; old product images are inapplicable")
    if not args.qualification:
        require_activation(args.platform, mode)
    if (not args.confirmed_modal or not math.isfinite(args.estimated_local_secs)
            or args.estimated_local_secs < 60):
        raise ValueError("An explicit Modal choice and expected local runtime >=60s are required")
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
        if prepared["rust_version"] != compiler_version(args.platform):
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
                return run_build("/build", original, volume.commit)
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
                return result["returncode"]
        except BaseException:
            if state["state"] not in {"not_started", "collection_pending", "completed"}:
                state["state"] = "unresolved"
                write_record(directory / "state.json", state)
            raise


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
    descriptor, temporary = tempfile.mkstemp(prefix=".download-", dir=destination.parent)
    temporary = Path(temporary)
    try:
        digest, size = hashlib.sha256(), 0
        with os.fdopen(descriptor, "wb") as output:
            for chunk in volume.read_file(remote_path):
                output.write(chunk)
                digest.update(chunk)
                size += len(chunk)
            output.flush()
            os.fsync(output.fileno())
        if artifact is not None:
            if digest.hexdigest() != artifact["sha256"] or size != artifact["size"]:
                raise ValueError("Downloaded artifact checksum/size differs")
            temporary.chmod(0o500)
        temporary.replace(destination)
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
                    or "/" in name or "\\" in name):
                raise ValueError("Test artifact does not belong to original job")
    write_record(directory / "completed.json", result)  # remote completion, not local readiness
    for name, artifact in result.get("artifacts", {}).items():
        # Legacy product pgvector entries retain their original relative-cache
        # metadata semantics. Product executable and test snapshots live here.
        if "/" in name:
            continue
        if re.fullmatch(r"[A-Za-z0-9_-]+", name) is None:
            raise ValueError("Unsafe artifact name")
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
    if mode == APPLE_TEST_MODE:
        # No native test execution/fixture qualification or activation writer is
        # delivered by this preparation package. --qualification may compile
        # only after exact new-image preparation; normal test offload stays shut.
        raise ValueError("Apple test mode is DEACTIVATED: native test qualification/activation remains open")
    path = records_root() / f"activation-{platform}.json"
    if not path.exists():
        raise ValueError("Modal is DEACTIVATED; only an explicitly authorized --qualification run is allowed")
    record = json.loads(path.read_bytes())
    image = json.loads((records_root() / f"image-{platform}.json").read_bytes())
    if (not record.get("active") or record.get("image_id") != image["image_id"]
            or (platform == "apple" and record.get("mode", APPLE_BUILD_MODE) != APPLE_BUILD_MODE)):
        raise ValueError("Modal is DEACTIVATED or the qualified image has changed")
    return record


def accept_native(args):
    if sys.platform != "darwin":
        raise ValueError("Native Apple acceptance must run on macOS")
    directory = records_root() / "jobs" / str(uuid.UUID(args.job_id))
    receipt = json.loads((directory / "completed.json").read_bytes())
    if (receipt.get("mode") == APPLE_TEST_MODE
            or command_mode(receipt["command"], "aarch64-apple-darwin") != APPLE_BUILD_MODE):
        raise ValueError("Test builds require their own native test execution acceptance; CLI acceptance cannot qualify them")
    binary = directory / "brassclaw-reborn"
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
    path = records_root() / f"activation-{args.platform}.json"
    if args.action == "deactivate":
        write_record(path, {"active": False})
    elif args.action == "activate":
        receipt = json.loads((records_root() / "jobs" / str(uuid.UUID(args.job_id)) / "completed.json").read_bytes())
        image = json.loads((records_root() / f"image-{args.platform}.json").read_bytes())
        if (receipt.get("mode") == APPLE_TEST_MODE or (args.platform == "apple"
                and command_mode(receipt["command"], "aarch64-apple-darwin") != APPLE_BUILD_MODE)):
            raise ValueError("Test mode cannot use product-build activation")
        if receipt["returncode"] != 0 or receipt["image_id"] != image["image_id"] or not receipt.get("artifacts"):
            raise ValueError("Activation requires a successful qualified build of this exact image")
        native = records_root() / "jobs" / args.job_id / "native-accepted.json"
        accepted = json.loads(native.read_bytes())
        if set(accepted.get("checks", {})) != {"file", "dependencies", "version", "help"}:
            raise ValueError("Native acceptance is incomplete")
        if accepted["artifact_sha256"] != receipt["artifacts"]["brassclaw-reborn"]["sha256"]:
            raise ValueError("Native acceptance differs from the cross-built artifact")
        write_record(path, {"active": True, "image_id": image["image_id"], "job_id": args.job_id,
                            "mode": APPLE_BUILD_MODE if args.platform == "apple" else "linux-compile/1"})
    try:
        print(json.dumps({"status": "ACTIVE", **require_activation(args.platform)}, indent=2))
    except (ValueError, FileNotFoundError) as error:
        print(json.dumps({"status": "DEACTIVATED", "reason": str(error)}, indent=2))


def main():
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
    execute.add_argument("--estimated-local-secs", type=float, required=True)
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
        if action in {"activate", "accept-native"}:
            control.add_argument("job_id")
    args = parser.parse_args()
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
        print(json.dumps(modal_resources(args), indent=2))
    elif args.action == "accept-native":
        accept_native(args)
    elif args.action == "status":
        status(args)
    else:
        activation(args)


if __name__ == "__main__":
    main()
