"""Read independent host settings without importing Modal or invoking Cargo."""

from __future__ import annotations

import math
import os
from pathlib import Path
import tomllib


DEFAULT_CONFIG = Path(__file__).with_name("build-environments.toml")
RESOURCE_KEYS = {"cpu", "cpu_limit", "memory_mib", "memory_limit_mib", "jobs",
                 "timeout_secs", "shutdown_grace_secs", "nonpreemptible"}


def load_settings(host, platform=None, config=None):
    path = Path(config or os.environ.get("BRASSCLAW_BUILD_CONFIG", DEFAULT_CONFIG))
    record = tomllib.loads(path.read_text())
    if type(record.get("version")) is not int or record["version"] != 1:
        raise ValueError("Unsupported build settings version")
    if set(record) - {"version", "local", "modal"}:
        raise ValueError("Unknown build settings section")
    if host == "local":
        settings = record.get("local", {})
        if not isinstance(settings, dict) or set(settings) - {"jobs"}:
            raise ValueError("Local settings support only jobs")
        jobs = settings.get("jobs", "inherit")
        if jobs != "inherit" and (type(jobs) is not int or jobs <= 0):
            raise ValueError("Local jobs must be a positive integer or 'inherit'")
        return {"jobs": jobs}
    if host != "modal" or platform not in {"apple", "linux"}:
        raise ValueError("Unknown build host/target")
    common = record.get("modal", {})
    if not isinstance(common, dict) or set(common) - RESOURCE_KEYS - {"apple", "linux"}:
        raise ValueError("Unknown Modal build setting")
    for target in ("apple", "linux"):
        specific = common.get(target, {})
        if not isinstance(specific, dict) or set(specific) - RESOURCE_KEYS:
            raise ValueError(f"Unknown Modal {target} build setting")
    return {**{key: value for key, value in common.items() if key in RESOURCE_KEYS},
            **common.get(platform, {})}


def modal_resources(args):
    settings = load_settings("modal", args.platform, args.config)
    for key in RESOURCE_KEYS:
        value = getattr(args, key, None)
        if value is not None:
            settings[key] = value
    # Preserve existing --cpu / --memory-mib shorthand: an explicit request
    # overrides both reservation and limit unless a limit is also explicit.
    for key, limit in (("cpu", "cpu_limit"), ("memory_mib", "memory_limit_mib")):
        if getattr(args, key, None) is not None and getattr(args, limit, None) is None:
            settings[limit] = settings[key]
        settings.setdefault(limit, settings.get(key))
    if set(settings) != RESOURCE_KEYS:
        raise ValueError("Incomplete Modal resource settings")
    for key, value in settings.items():
        if key == "nonpreemptible":
            if type(value) is not bool:
                raise ValueError("nonpreemptible must be a boolean")
        elif key in {"cpu", "cpu_limit"}:
            if type(value) not in {int, float} or not math.isfinite(value) or value <= 0:
                raise ValueError(f"{key} must be finite and positive")
        elif type(value) is not int or value <= 0:
            raise ValueError(f"{key} must be a positive integer")
    if settings["cpu_limit"] < settings["cpu"] or settings["memory_limit_mib"] < settings["memory_mib"]:
        raise ValueError("Resource limits cannot be below reservations")
    if settings["timeout_secs"] < 180 or settings["shutdown_grace_secs"] >= 120:
        raise ValueError("Reserve at least 120 seconds for shutdown and durable receipts")
    return settings
