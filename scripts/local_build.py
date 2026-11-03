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


def run_local(command, jobs, *, show_settings=False):
    if command and command[0] == "--":
        command = command[1:]
    if command and (len(command) < 2 or command[0] != "cargo"):
        raise ValueError("Supply a direct cargo command")
    if any(arg.split("=", 1)[0] == "--target-dir" for arg in command):
        raise ValueError("Use the canonical CARGO_TARGET_DIR from host configuration")
    if command and command[1] == "clean" and not any(
            arg.split("=", 1)[0] in {"-p", "--package"} for arg in command[2:]):
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
        if command[1] in {"build", "check", "clippy", "test", "run", "bench"}:
            destination = Path(settings["target_dir"])
            while not destination.exists():
                destination = destination.parent
            usage = shutil.disk_usage(destination)
            if usage.free < 15 * 1024**3 or usage.used > usage.total * 0.90:
                raise RuntimeError("Insufficient local build space; preserve evidence and apply the development disk policy first")
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
