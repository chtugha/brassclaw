"""Local settings/lock/controller regression tests; no Cargo or Modal compute."""

import argparse
from contextlib import nullcontext
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
from types import SimpleNamespace
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).parent))
import build_settings
import local_build
import modal_build
import modal_source_sync as sync
from modal_build_worker import encoded
from test_modal_source_sync import SourceFixture


def arguments(**overrides):
    values = dict(config=None, platform="apple", environment="brassclaw", volume="brassclaw-build",
                  qualification=True, confirmed_modal=True, estimated_local_secs=600,
                  cargo=["cargo", "build", "--locked", "-p", "brassclaw_reborn_cli",
                         "--bin", "brassclaw-reborn", "--target", "aarch64-apple-darwin"])
    values.update(overrides)
    return argparse.Namespace(**values)


class SettingsTests(unittest.TestCase):
    def test_qualified_defaults_and_explicit_burst_limit(self):
        defaults = build_settings.modal_resources(arguments())
        burst = build_settings.modal_resources(arguments(cpu_limit=16, jobs=16))
        self.assertEqual((defaults["cpu"], defaults["cpu_limit"], defaults["jobs"]), (8, 8, 8))
        self.assertEqual((burst["cpu"], burst["cpu_limit"], burst["jobs"]), (8, 16, 16))

    def test_explicit_old_shorthand_overrides_both_request_and_limit(self):
        value = build_settings.modal_resources(arguments(cpu=16, memory_mib=65536,
                                                        nonpreemptible=False))
        self.assertEqual((value["cpu"], value["cpu_limit"]), (16, 16))
        self.assertEqual((value["memory_mib"], value["memory_limit_mib"]), (65536, 65536))
        self.assertFalse(value["nonpreemptible"])

    def test_host_and_platform_profiles_are_independent(self):
        with tempfile.TemporaryDirectory() as tmp:
            config = Path(tmp) / "build.toml"
            config.write_text(build_settings.DEFAULT_CONFIG.read_text().replace('jobs = "inherit"', 'jobs = 3')
                              .replace('[modal.apple]', '[modal.apple]\ncpu_limit = 16.0\njobs = 16'))
            self.assertEqual(build_settings.load_settings("local", config=config), {"jobs": 3})
            apple = build_settings.modal_resources(arguments(config=config))
            linux = build_settings.modal_resources(arguments(config=config, platform="linux"))
            self.assertEqual((apple["jobs"], apple["cpu_limit"]), (16, 16))
            self.assertEqual((linux["jobs"], linux["cpu_limit"]), (8, 8))

    def test_invalid_resources_fail_before_any_cloud_access(self):
        for value in (dict(cpu=float("nan")), dict(cpu=float("inf")), dict(cpu=-1),
                      dict(cpu=16, cpu_limit=8), dict(memory_mib=65536, memory_limit_mib=32768),
                      dict(jobs=0), dict(jobs=True), dict(timeout_secs=179),
                      dict(shutdown_grace_secs=120)):
            with self.subTest(value=value), self.assertRaises(ValueError):
                build_settings.modal_resources(arguments(**value))

    def test_unknown_and_invalid_local_settings_are_rejected(self):
        with tempfile.TemporaryDirectory() as tmp:
            config = Path(tmp) / "build.toml"
            for text in ('version = 1\n[local]\njobs = 0',
                         'version = 1\n[local]\ncpu = 8',
                         'version = 1\n[unknown]', 'version = true'):
                config.write_text(text)
                with self.subTest(text=text), self.assertRaises(ValueError):
                    build_settings.load_settings("local", config=config)

    def test_nonfinite_or_short_estimates_reject_before_sdk_import(self):
        for estimate in (float("nan"), float("inf"), 59):
            with self.subTest(estimate=estimate), patch.dict(sys.modules, {"modal": None}):
                with self.assertRaisesRegex(ValueError, "explicit Modal choice"):
                    modal_build.run(arguments(estimated_local_secs=estimate))


class LockTests(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        root = Path(self.tmp.name)
        self.env = {"BRASSCLAW_CARGO_LOCK": str(root / "local.lock"),
                    "BRASSCLAW_MODAL_LOCK": str(root / "modal.lock"),
                    "CARGO_TARGET_DIR": str(root / "target")}
        self.guard = patch.dict(os.environ, self.env)
        self.guard.start()
        self.addCleanup(self.guard.stop)

    def child_lock(self, name):
        return subprocess.run([sys.executable, "-c",
                               f"from modal_source_sync import {name};\nwith {name}(): pass"],
                              cwd=Path(__file__).parent, capture_output=True, text=True)

    def test_local_and_remote_locks_allow_overlap_in_separate_processes(self):
        with sync.cargo_lock():
            remote = self.child_lock("modal_lock")
            local = self.child_lock("cargo_lock")
        self.assertEqual(remote.returncode, 0, remote.stderr)
        self.assertNotEqual(local.returncode, 0)

    def test_remote_lock_serializes_sync_and_builds(self):
        with sync.modal_lock():
            remote = self.child_lock("modal_lock")
            local = self.child_lock("cargo_lock")
        self.assertNotEqual(remote.returncode, 0)
        self.assertEqual(local.returncode, 0, local.stderr)

    def test_lock_paths_cannot_alias(self):
        with patch.dict(os.environ, {"BRASSCLAW_MODAL_LOCK": self.env["BRASSCLAW_CARGO_LOCK"]}):
            with self.assertRaisesRegex(ValueError, "different paths"):
                sync.modal_lock()

    def test_disconnected_remote_blocks_source_sync_but_not_local_lock(self):
        state = Path(self.env["CARGO_TARGET_DIR"]) / "modal-build/jobs/original/state.json"
        state.parent.mkdir(parents=True)
        state.write_text(json.dumps({"state": "unresolved"}))
        with sync.cargo_lock():
            with self.assertRaisesRegex(RuntimeError, "Unsettled remote"):
                sync.assert_remote_settled()
        state.write_text(json.dumps({"state": "completed"}))
        sync.assert_remote_settled()

    def test_unsettled_remote_rejects_new_controller_before_sync(self):
        state = Path(self.env["CARGO_TARGET_DIR"]) / "modal-build/jobs/original/state.json"
        state.parent.mkdir(parents=True)
        state.write_text(json.dumps({"state": "running"}))
        with patch.dict(sys.modules, {"modal": SimpleNamespace()}), patch.object(modal_build, "sync_source") as upload:
            with self.assertRaisesRegex(RuntimeError, "Unsettled remote"):
                modal_build.run(arguments())
            upload.assert_not_called()

    def test_standalone_sync_rejects_unsettled_job_before_any_cloud_access(self):
        state = Path(self.env["CARGO_TARGET_DIR"]) / "modal-build/jobs/original/state.json"
        state.parent.mkdir(parents=True)
        state.write_text(json.dumps({"state": "unresolved"}))
        with patch.object(sys, "argv", ["modal_source_sync.py"]), patch.object(sync, "sync_source") as upload:
            with self.assertRaisesRegex(RuntimeError, "Unsettled remote"):
                sync.main()
            upload.assert_not_called()


class LocalRunnerTests(unittest.TestCase):
    def test_direct_cargo_keeps_target_and_uses_only_local_jobs(self):
        with patch.dict(os.environ, {"CARGO_TARGET_DIR": "/tmp/canonical", "CARGO_BUILD_JOBS": "2"}), \
                patch.object(local_build, "cargo_lock", return_value=nullcontext()), \
                patch.object(local_build.subprocess, "run", return_value=SimpleNamespace(returncode=0)) as run:
            self.assertEqual(local_build.run_local(["cargo", "metadata", "--no-deps"], 4), 0)
            self.assertEqual(run.call_args.args[0], ["cargo", "metadata", "--no-deps"])
            self.assertEqual(run.call_args.kwargs["env"]["CARGO_TARGET_DIR"], "/tmp/canonical")
            self.assertEqual(run.call_args.kwargs["env"]["CARGO_BUILD_JOBS"], "4")
            self.assertEqual(os.environ["CARGO_BUILD_JOBS"], "2")

    def test_target_override_whole_cleanup_and_shell_are_rejected(self):
        for command in (["cargo", "build", "--target-dir=/tmp/other"], ["cargo", "clean"],
                        ["sh", "-c", "cargo build"]):
            with self.subTest(command=command), self.assertRaises(ValueError):
                local_build.run_local(command, "inherit")


class ControllerTests(SourceFixture):
    def test_status_uses_original_volume_despite_changed_current_options(self):
        directory = self.base / "records/jobs/736e8ba8-ad53-4d53-9011-c1c9b67083fe"
        directory.mkdir(parents=True)
        job = directory.name
        original = {"job_id": job, "command": ["cargo", "build", "--locked"],
                    "image_id": "im-original", "manifest_sha256": "a" * 64,
                    "volume": "original-volume", "environment": "brassclaw"}
        result = {**{key: original[key] for key in ("job_id", "command", "image_id", "manifest_sha256")},
                  "returncode": 0}
        (directory / "state.json").write_bytes(encoded({"state": "running", "job_id": job,
                                                       "function_call_id": "fc-original", "environment": "brassclaw"}))
        (directory / "request.json").write_bytes(encoded(original))
        volume = SimpleNamespace(read_file=lambda path: [encoded(result)])
        selected = []

        def from_name(name, **kwargs):
            selected.append((name, kwargs))
            return volume

        sdk = SimpleNamespace(FunctionCall=SimpleNamespace(from_id=lambda value:
                              SimpleNamespace(get=lambda **kwargs: result)),
                              Volume=SimpleNamespace(from_name=from_name))
        with patch.dict(sys.modules, {"modal": sdk}), \
                patch.object(modal_build, "records_root", return_value=directory.parent.parent):
            modal_build.status(SimpleNamespace(job_id=job, volume="changed-volume"))
        self.assertEqual(selected, [("original-volume", {"environment_name": "brassclaw"})])
        self.assertEqual(json.loads((directory / "state.json").read_text())["state"], "completed")

    def test_resources_and_snapshot_are_submitted_under_remote_not_local_lock(self):
        # Mock SDK transport only. This does not execute/qualify a remote worker.
        (self.root / "scripts").mkdir()
        (self.root / "scripts/modal_build_worker.py").write_text("# fixture identity\n")
        manifest = sync.capture(self.root, self.base / "source.tar.gz")
        manifest["source_archive"] = sync.BASE_SOURCE_PATH
        manifest_bytes = encoded(manifest)
        records = self.base / "records"
        records.mkdir()
        (records / "image-apple.json").write_text(json.dumps({"rust_version": "1.96.0",
                                                               "environment": "brassclaw", "image_id": "im-test"}))
        resources, submitted = {}, {}
        remote_files = {sync.MANIFEST_PATH: manifest_bytes, sync.BASE_MANIFEST_PATH: manifest_bytes}

        class App:
            app_id = "ap-test"

            def function(self, **kwargs):
                resources.update(kwargs)
                def decorate(function):
                    def spawn(original):
                        submitted.update(original)
                        result = {**{key: original[key] for key in
                                      ("job_id", "command", "manifest_sha256", "image_id")},
                                  "returncode": 0, "artifacts": {}}
                        prefix = f'/cache/jobs/{original["job_id"]}'
                        remote_files[prefix + "/completed.json"] = encoded(result)
                        remote_files[prefix + "/image-provenance.json"] = b"{}"
                        remote_files[prefix + "/cargo.log"] = b"SDK fixture; no Cargo executed\n"
                        return SimpleNamespace(object_id="fc-test", get=lambda **kwargs: result)
                    return SimpleNamespace(spawn=spawn)
                return decorate

            def run(self, **kwargs):
                return nullcontext()

        volume = SimpleNamespace(read_file=lambda path: [remote_files[path]])
        sdk = SimpleNamespace(App=lambda *a, **kw: App(), enable_output=nullcontext,
                              Volume=SimpleNamespace(from_name=lambda *a, **kw: volume),
                              Image=SimpleNamespace(from_id=lambda value: SimpleNamespace(object_id=value)))
        env = {"BRASSCLAW_CARGO_LOCK": str(self.base / "local.lock"),
               "BRASSCLAW_MODAL_LOCK": str(self.base / "modal.lock"),
               "CARGO_TARGET_DIR": str(self.base / "target")}

        def check_sync_lock(*args):
            result = subprocess.run([sys.executable, "-c",
                                     "from modal_source_sync import modal_lock;\nwith modal_lock(): pass"],
                                    cwd=Path(__file__).parent, capture_output=True, text=True)
            self.assertNotEqual(result.returncode, 0, "sync must hold the same remote lock as submission")

        with patch.dict(os.environ, env), patch.dict(sys.modules, {"modal": sdk}), \
                patch.object(modal_build, "ROOT", self.root), \
                patch.object(modal_build, "records_root", return_value=records), \
                patch.object(modal_build, "rust_version", return_value="1.96.0"), \
                patch.object(modal_build, "sync_source", side_effect=check_sync_lock) as upload, \
                sync.cargo_lock():
            self.assertEqual(modal_build.run(arguments(cpu_limit=16, jobs=16)), 0)
        upload.assert_called_once_with("brassclaw", "brassclaw-build")
        self.assertEqual(resources["cpu"], (8, 16))
        self.assertEqual(resources["memory"], (32768, 32768))
        self.assertEqual((resources["max_containers"], resources["min_containers"]), (1, 0))
        self.assertEqual(submitted["jobs"], 16)
        self.assertEqual(submitted["lock_scope"], "modal")
        self.assertEqual(submitted["manifest_sha256"], __import__("hashlib").sha256(manifest_bytes).hexdigest())


if __name__ == "__main__":
    unittest.main()
