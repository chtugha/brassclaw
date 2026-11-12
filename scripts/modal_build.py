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
import sys
import tomllib
import uuid

from modal_build_worker import encoded, validate_command
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
        result = {"environment": args.environment, "image": image_name(args.platform),
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
    validate_command(command, target)
    if args.platform == "apple":
        if (command[1] != "build" or "brassclaw_reborn_cli" not in command
                or "brassclaw-reborn" not in command
                or any(arg.split("=", 1)[0] == "--profile" for arg in command)):
            raise ValueError("The qualified Apple path builds -p brassclaw_reborn_cli --bin brassclaw-reborn using dev or --release")
    if not args.qualification:
        require_activation(args.platform)
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
        if not args.qualification and require_activation(args.platform)["image_id"] != image.object_id:
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
        directory = jobs / job_id
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
            evidence.mkdir(parents=True, exist_ok=True)
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
                state.update(state="submitting", app_id=app.app_id)
                write_record(directory / "state.json", state)
                call = compile_source.spawn(request)
                state.update(state="running", function_call_id=call.object_id)
                write_record(directory / "state.json", state)
                while True:
                    try:
                        result = call.get(timeout=5)
                        break
                    except TimeoutError:
                        continue
                # The returned original is also retained durably by the worker.
                retained = json.loads(b"".join(volume.read_file(f"/cache/jobs/{job_id}/completed.json")))
                if result != retained or result["job_id"] != job_id or result["command"] != command:
                    raise ValueError("Remote completion differs from original build")
                if result["manifest_sha256"] != request["manifest_sha256"] or result["image_id"] != image.object_id:
                    raise ValueError("Remote source/image differs")
                write_record(directory / "completed.json", result)
                (directory / "image-provenance.json").write_bytes(b"".join(volume.read_file(f"/cache/jobs/{job_id}/image-provenance.json")))
                for name, artifact in result.get("artifacts", {}).items():
                    if "/" in name:
                        continue
                    destination = directory / name
                    with destination.open("wb") as output:
                        for chunk in volume.read_file(f"/cache/jobs/{job_id}/artifacts/{name}"):
                            output.write(chunk)
                    if hashlib.file_digest(destination.open("rb"), "sha256").hexdigest() != artifact["sha256"]:
                        raise ValueError("Downloaded artifact checksum differs")
                with (directory / "cargo.log").open("wb") as output:
                    for chunk in volume.read_file(f"/cache/jobs/{job_id}/cargo.log"):
                        output.write(chunk)
                state.update(state="completed", returncode=result["returncode"])
                write_record(directory / "state.json", state)
                print(json.dumps(result, indent=2))
                return result["returncode"]
        except BaseException:
            if state["state"] != "not_started":
                state["state"] = "unresolved"
                write_record(directory / "state.json", state)
            raise


def status(args):
    import modal

    path = records_root() / "jobs" / str(uuid.UUID(args.job_id)) / "state.json"
    state = json.loads(path.read_bytes())
    if state.get("function_call_id") and state["state"] != "completed":
        try:
            result = modal.FunctionCall.from_id(state["function_call_id"]).get(timeout=0)
        except TimeoutError:
            print(json.dumps(state, indent=2))
            return
        except (modal.exception.FunctionTimeoutError, modal.exception.RemoteError) as error:
            # These are terminal FunctionCall results, not a disconnected client
            # or an empty poll. Never turn other transport failures into proof.
            write_record(path.with_name("terminal.json"), {
                "function_call_id": state["function_call_id"], "job_id": state["job_id"],
                "remote_result_exception": type(error).__name__, "message": str(error)})
            state.update(state="failed", reason="terminal remote result without a worker completion receipt")
            write_record(path, state)
            print(json.dumps(state, indent=2))
            return
        original = json.loads(path.with_name("request.json").read_bytes())
        if (result["job_id"] != original["job_id"] or result["command"] != original["command"]
                or result["image_id"] != original["image_id"]
                or result["manifest_sha256"] != original["manifest_sha256"]):
            raise ValueError("Recovered completion differs from original")
        volume = modal.Volume.from_name(original["volume"], environment_name=original["environment"])
        retained = json.loads(b"".join(volume.read_file(f'/cache/jobs/{original["job_id"]}/completed.json')))
        if result != retained:
            raise ValueError("Recovered completion is not the retained original")
        write_record(path.with_name("completed.json"), result)
        state.update(state="completed", returncode=result["returncode"])
        write_record(path, state)
    print(json.dumps(state, indent=2))


def require_activation(platform):
    path = records_root() / f"activation-{platform}.json"
    if not path.exists():
        raise ValueError("Modal is DEACTIVATED; only an explicitly authorized --qualification run is allowed")
    record = json.loads(path.read_bytes())
    image = json.loads((records_root() / f"image-{platform}.json").read_bytes())
    if not record.get("active") or record.get("image_id") != image["image_id"]:
        raise ValueError("Modal is DEACTIVATED or the qualified image has changed")
    return record


def accept_native(args):
    if sys.platform != "darwin":
        raise ValueError("Native Apple acceptance must run on macOS")
    directory = records_root() / "jobs" / str(uuid.UUID(args.job_id))
    receipt = json.loads((directory / "completed.json").read_bytes())
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
        if receipt["returncode"] != 0 or receipt["image_id"] != image["image_id"] or not receipt.get("artifacts"):
            raise ValueError("Activation requires a successful qualified build of this exact image")
        native = records_root() / "jobs" / args.job_id / "native-accepted.json"
        accepted = json.loads(native.read_bytes())
        if set(accepted.get("checks", {})) != {"file", "dependencies", "version", "help"}:
            raise ValueError("Native acceptance is incomplete")
        if accepted["artifact_sha256"] != receipt["artifacts"]["brassclaw-reborn"]["sha256"]:
            raise ValueError("Native acceptance differs from the cross-built artifact")
        write_record(path, {"active": True, "image_id": image["image_id"], "job_id": args.job_id})
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
