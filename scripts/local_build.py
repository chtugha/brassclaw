#!/usr/bin/env python3
"""Run direct local Cargo with its own lock and optional host-specific job limit."""

import argparse
import json
import os
from pathlib import Path
import shutil
import subprocess

from build_settings import load_settings
from modal_source_sync import ROOT, cargo_lock


def _cargo_subcommand(command):
    """Return the Cargo subcommand from a command list, skipping toolchain options.

    A leading ``+toolchain`` element (e.g. ``+1.100.0``) shifts the subcommand
    to position 2.  Without a toolchain option the subcommand is at position 1.
    Returns ``None`` when the list is too short to contain a subcommand.
    """
    if not command or command[0] != "cargo":
        return None
    # command[1] is either a "+toolchain" specifier or the subcommand itself.
    if len(command) >= 2 and command[1].startswith("+"):
        return command[2] if len(command) >= 3 else None
    return command[1] if len(command) >= 2 else None


def run_local(command, jobs, *, show_settings=False):
    if command and command[0] == "--":
        command = command[1:]
    if command and (len(command) < 2 or command[0] != "cargo"):
        raise ValueError("Supply a direct cargo command")
    if any(arg.split("=", 1)[0] == "--target-dir" for arg in command):
        raise ValueError("Use the canonical CARGO_TARGET_DIR from host configuration")
    subcommand = _cargo_subcommand(command)
    if subcommand == "clean" and not any(
            arg.split("=", 1)[0] in {"-p", "--package"} for arg in command):
        raise ValueError("Whole-target cleanup would remove retained evidence; select a package")
    environment = os.environ.copy()
    if jobs != "inherit":
        if type(jobs) is not int or jobs <= 0:
            raise ValueError("Local jobs must be positive")
        environment["CARGO_BUILD_JOBS"] = str(jobs)
    target = Path(environment.get("CARGO_TARGET_DIR", ROOT / "target"))
    if not target.is_absolute():
        target = ROOT / target
    settings = {"host": "local", "command": command,
                "target_dir": str(target.resolve()),
                "jobs": environment.get("CARGO_BUILD_JOBS", "Cargo configuration/default")}
    if show_settings:
        print(json.dumps(settings, indent=2))
        return 0
    if not command:
        raise ValueError("Supply a direct cargo command or --show-settings")
    with cargo_lock():
        if subcommand in {"build", "check", "clippy", "test", "run", "bench"}:
            # Disk gate: Cargo-volume (canonical target directory).
            destination = Path(settings["target_dir"])
            while not destination.exists():
                destination = destination.parent
            usage = shutil.disk_usage(destination)
            if usage.free < 15 * 1024**3 or usage.used > usage.total * 0.90:
                raise RuntimeError("Insufficient local build space; preserve evidence and apply the development disk policy first")
            # Disk gate: artifact-volume (source/validation root on external drive).
            artifact_root = ROOT
            while not artifact_root.exists():
                artifact_root = artifact_root.parent
            artifact_usage = shutil.disk_usage(artifact_root)
            if artifact_usage.free < 15 * 1024**3 or artifact_usage.used > artifact_usage.total * 0.90:
                raise RuntimeError("Insufficient artifact volume space; free space on the source/validation volume first")
        # No shell, target override, Modal import, cloud probe or source sync.
        return subprocess.run(command, cwd=ROOT, env=environment).returncode


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--config", type=Path)
    parser.add_argument("--jobs", type=int)
    parser.add_argument("--show-settings", action="store_true")
    parser.add_argument("cargo", nargs=argparse.REMAINDER)
    args = parser.parse_args()
    settings = load_settings("local", config=args.config)
    jobs = args.jobs if args.jobs is not None else settings["jobs"]
    raise SystemExit(run_local(args.cargo, jobs, show_settings=args.show_settings))


if __name__ == "__main__":
    main()
