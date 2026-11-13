#!/usr/bin/env python3
"""Run direct local Cargo with its own lock and optional host-specific job limit."""

import argparse
import json
import os
import re
from pathlib import Path
import shutil
import subprocess

from build_settings import load_settings
from modal_source_sync import ROOT, cargo_lock


# Cargo's global grammar: https://doc.rust-lang.org/cargo/commands/cargo.html
# This launcher deliberately rejects aliases, external subcommands, config/-C/-Z
# and special top-level actions. Direct Cargo under the native lock remains
# available for forms outside this bounded launcher grammar.
COMPILE_COMMANDS = {"build", "check", "clippy", "test", "run", "bench", "doc",
                    "rustc", "rustdoc", "fix", "install", "package", "publish"}
INSPECT_COMMANDS = {"metadata", "tree", "pkgid", "locate-project", "fetch", "verify-project"}
GLOBAL_SWITCHES = {"--offline", "--locked", "--frozen", "--verbose", "--quiet", "-q"}


def _cargo_subcommand(command):
    if not command:
        return None
    if command[0] != "cargo" or any(not isinstance(arg, str) or not arg or "\0" in arg for arg in command):
        raise ValueError("Supply a direct cargo command")
    index = 1
    if len(command) > index and command[index].startswith("+"):
        if command[index] == "+":
            raise ValueError("Missing toolchain")
        index += 1
    while index < len(command) and command[index].startswith("-"):
        arg = command[index]
        if arg in GLOBAL_SWITCHES or re.fullmatch(r"-v{1,2}", arg):
            index += 1
        elif arg == "--color" or arg.startswith("--color="):
            if arg == "--color":
                index += 1
                value = command[index] if index < len(command) else ""
            else:
                value = arg.split("=", 1)[1]
            if value not in {"auto", "always", "never"}:
                raise ValueError("Invalid/missing Cargo --color value")
            index += 1
        else:
            raise ValueError(f"Unsupported Cargo global option: {arg}")
    if index >= len(command) or command[index] not in COMPILE_COMMANDS | INSPECT_COMMANDS | {"clean"}:
        raise ValueError("Unsupported Cargo subcommand/alias")
    # These options can redirect target custody even after a real subcommand.
    for arg in command[index + 1:]:
        if (arg.split("=", 1)[0] in {"--target-dir", "--artifact-dir", "--build-dir", "--out-dir", "--config"}
                or arg.startswith(("-Z", "-C"))):
            raise ValueError("Use the canonical CARGO_TARGET_DIR; Cargo config/directory overrides are unsupported")
    if command[index] == "clean":
        _validate_package_clean(command[index + 1:])
    return command[index]


def _validate_package_clean(args):
    # Parse *all* clean arguments. A bare -p or a -p occurring after -- is no
    # proof of package selection. Attached -pfoo and both long forms are valid.
    packages = []
    index = 0
    while index < len(args):
        arg = args[index]
        flag, sep, value = arg.partition("=")
        if arg in GLOBAL_SWITCHES or arg in {"-v", "-vv", "--release", "--doc", "--dry-run"}:
            index += 1
            continue
        if arg.startswith("-p") and not arg.startswith("--") and arg != "-p":
            flag, sep, value = "-p", "=", arg[2:].removeprefix("=")
        if flag not in {"-p", "--package", "--target", "--profile", "--color"}:
            raise ValueError(f"Unsupported package cleanup argument: {arg}")
        if not sep:
            index += 1
            value = args[index] if index < len(args) else ""
        if not value or value.startswith(("-", "+")):
            raise ValueError("Missing package cleanup option value")
        if flag in {"-p", "--package"}:
            packages.append(value)
        if flag == "--color" and value not in {"auto", "always", "never"}:
            raise ValueError("Invalid Cargo --color value")
        index += 1
    if not packages:
        raise ValueError("Whole-target cleanup would remove retained evidence; select a package")


def run_local(command, jobs, *, show_settings=False):
    if command and command[0] == "--":
        command = command[1:]
    if command and (len(command) < 2 or command[0] != "cargo"):
        raise ValueError("Supply a direct cargo command")
    subcommand = _cargo_subcommand(command)
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
        if subcommand in COMPILE_COMMANDS:
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
