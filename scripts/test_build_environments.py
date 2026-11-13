"""Local settings/lock/controller regression tests; no Cargo or Modal compute."""

import argparse
from contextlib import nullcontext
import json
import hashlib
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
    def _ample_disk(self):
        """Return a disk_usage-like namespace with plenty of free space."""
        total = 500 * 1024**3
        used = 100 * 1024**3
        return SimpleNamespace(total=total, used=used, free=total - used)

    def test_direct_cargo_keeps_target_and_uses_only_local_jobs(self):
        with patch.dict(os.environ, {"CARGO_TARGET_DIR": "/tmp/canonical", "CARGO_BUILD_JOBS": "2"}), \
                patch.object(local_build, "cargo_lock", return_value=nullcontext()), \
                patch.object(local_build.shutil, "disk_usage", return_value=self._ample_disk()), \
                patch.object(local_build.subprocess, "run", return_value=SimpleNamespace(returncode=0)) as run:
            self.assertEqual(local_build.run_local(["cargo", "metadata", "--no-deps"], 4), 0)
            self.assertEqual(run.call_args.args[0], ["cargo", "metadata", "--no-deps"])
            self.assertEqual(run.call_args.kwargs["env"]["CARGO_TARGET_DIR"], "/tmp/canonical")
            self.assertEqual(run.call_args.kwargs["env"]["CARGO_BUILD_JOBS"], "4")
            self.assertEqual(os.environ["CARGO_BUILD_JOBS"], "2")

    def test_disk_gate_checks_both_volumes(self):
        tight = SimpleNamespace(total=100 * 1024**3, used=96 * 1024**3, free=4 * 1024**3)
        ample = self._ample_disk()
        # Cargo-volume tight → rejected.
        with patch.object(local_build, "cargo_lock", return_value=nullcontext()), \
                patch.object(local_build.shutil, "disk_usage", side_effect=[tight, ample]):
            with self.assertRaisesRegex(RuntimeError, "local build space"):
                local_build.run_local(["cargo", "build", "--locked"], "inherit")
        # Artifact-volume tight → rejected.
        with patch.object(local_build, "cargo_lock", return_value=nullcontext()), \
                patch.object(local_build.shutil, "disk_usage", side_effect=[ample, tight]):
            with self.assertRaisesRegex(RuntimeError, "artifact volume"):
                local_build.run_local(["cargo", "build", "--locked"], "inherit")

    def test_toolchain_option_does_not_skip_disk_gate(self):
        # cargo +1.100.0 check must still trigger the disk gate (the defect in
        # the original implementation was that command[1] == "+1.100.0", which is
        # not in the set of subcommand names, so the gate was silently skipped).
        tight = SimpleNamespace(total=100 * 1024**3, used=96 * 1024**3, free=4 * 1024**3)
        ample = self._ample_disk()
        with patch.object(local_build, "cargo_lock", return_value=nullcontext()), \
                patch.object(local_build.shutil, "disk_usage", side_effect=[tight, ample]):
            with self.assertRaisesRegex(RuntimeError, "local build space"):
                local_build.run_local(["cargo", "+1.100.0", "check", "--locked"], "inherit")

    def test_global_options_keep_compile_disk_guards_before_subprocess(self):
        tight = SimpleNamespace(total=100 * 1024**3, used=96 * 1024**3, free=4 * 1024**3)
        prefixes = [[], ["+1.100.0"], ["--offline"], ["-v"],
                    ["+1.100.0", "-v", "--locked"], ["--color", "never", "--frozen"],
                    ["--color=auto", "-vv"]]
        for prefix in prefixes:
            with self.subTest(prefix=prefix), \
                    patch.object(local_build, "cargo_lock", return_value=nullcontext()), \
                    patch.object(local_build.shutil, "disk_usage", return_value=tight), \
                    patch.object(local_build.subprocess, "run") as execute:
                with self.assertRaisesRegex(RuntimeError, "local build space"):
                    local_build.run_local(["cargo", *prefix, "check", "--locked"], "inherit")
                execute.assert_not_called()

    def test_whole_clean_and_uninterpretable_forms_never_execute(self):
        commands = [
            ["cargo", "-v", "clean"], ["cargo", "+1.100.0", "-v", "clean"],
            ["cargo", "--offline", "clean"], ["cargo", "clean", "-p"],
            ["cargo", "clean", "--doc", "-p", "foo"],
            ["cargo", "clean", "--package=foo", "--doc"],
            ["cargo", "clean", "--package="], ["cargo", "clean", "--", "-p", "foo"],
            ["cargo", "clean", "-p", "--release"], ["cargo", "clean", "-p", "foo", "--package"],
            ["cargo", "--color", "clean"], ["cargo", "--", "clean", "-p", "foo"],
            ["cargo", "--unknown", "check"], ["cargo", "b"], ["cargo", "custom-alias"],
            ["cargo", "--config", "build.target-dir='/tmp/other'", "check"],
            ["cargo", "check", "--config=build.target-dir='/tmp/other'"],
            ["cargo", "-C", "/tmp", "check"], ["cargo", "-Zunstable-options", "check"],
            ["cargo", "check", "--target-dir", "/tmp/other"],
            ["cargo", "check", "--target-dir=/tmp/other"],
            ["cargo", "check", "--build-dir=/tmp/other"],
            ["CARGO_TARGET_DIR=/tmp/other", "cargo", "check"],
        ]
        for command in commands:
            with self.subTest(command=command), patch.object(local_build.subprocess, "run") as execute:
                with self.assertRaises(ValueError):
                    local_build.run_local(command, "inherit")
                execute.assert_not_called()

    def test_package_clean_value_forms_are_direct_and_locked(self):
        for option in (["-p", "foo"], ["-pfoo"], ["-p=foo"], ["--package", "foo"],
                       ["--package=foo"], ["-p", "foo", "-pbar", "--profile=test"]):
            command = ["cargo", "+1.100.0", "--offline", "-v", "clean", *option]
            with self.subTest(option=option), \
                    patch.object(local_build, "cargo_lock", return_value=nullcontext()) as lock, \
                    patch.object(local_build.subprocess, "run", return_value=SimpleNamespace(returncode=0)) as execute:
                self.assertEqual(local_build.run_local(command, "inherit"), 0)
                lock.assert_called_once_with()
                self.assertEqual(execute.call_args.args[0], command)
                self.assertNotIn("shell", execute.call_args.kwargs)

    def test_existing_standalone_manifest_gates_keep_local_subcommand_options(self):
        for subcommand, manifest in (("clippy", "tests/monty_control/Cargo.toml"),
                                    ("test", "tests/monty_v1_upgrade/Cargo.toml"),
                                    ("test", "tests/monty_legacy_baseline/Cargo.toml")):
            command = ["cargo", "+1.100.0", "--offline", subcommand,
                       "--manifest-path", manifest, "--locked", "--",
                       *(["-D", "warnings"] if subcommand == "clippy" else ["--list"])]
            with self.subTest(manifest=manifest), \
                    patch.object(local_build, "cargo_lock", return_value=nullcontext()), \
                    patch.object(local_build.shutil, "disk_usage", return_value=self._ample_disk()) as disk, \
                    patch.object(local_build.subprocess, "run", return_value=SimpleNamespace(returncode=0)) as execute:
                self.assertEqual(local_build.run_local(command, "inherit"), 0)
                self.assertEqual(execute.call_args.args[0], command)
                self.assertEqual(disk.call_count, 2)

    def test_top_level_inspection_and_show_settings_do_not_require_disk_or_cargo(self):
        for command in (["cargo", "--help"], ["cargo", "--version"],
                        ["cargo", "+1.100.0", "-v", "--version"], ["cargo", "--list"]):
            with self.subTest(command=command), \
                    patch.object(local_build, "cargo_lock", return_value=nullcontext()), \
                    patch.object(local_build.shutil, "disk_usage") as disk, \
                    patch.object(local_build.subprocess, "run", return_value=SimpleNamespace(returncode=0)) as execute:
                self.assertEqual(local_build.run_local(command, "inherit"), 0)
                execute.assert_called_once()
                disk.assert_not_called()
        with patch.object(local_build.subprocess, "run") as execute:
            self.assertEqual(local_build.run_local([], "inherit", show_settings=True), 0)
            execute.assert_not_called()

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
        self._run_transport()

    def test_run_download_failure_is_pending_and_status_retries_original_collection_only(self):
        self._run_transport(fail_download=True)

    def test_test_preparation_pins_normalized_command_worker_and_subject_in_transport(self):
        self._run_transport(test_mode=True)

    def _run_transport(self, fail_download=False, test_mode=False):
        # Mock SDK transport only. This does not execute/qualify a remote worker.
        (self.root / "scripts").mkdir()
        (self.root / "scripts/modal_build_worker.py").write_text("# fixture identity\n")
        if test_mode:
            for name in ("modal_build_worker.py", "modal_source_sync.py"):
                (self.root / "scripts" / name).write_bytes(Path(__file__).with_name(name).read_bytes())
            package = self.root / "crates/brassclaw_reborn_composition"
            (package / "src").mkdir(parents=True)
            (package / "Cargo.toml").write_text('[package]\nname="brassclaw_reborn_composition"\nversion="0.1.0"\n')
            (package / "src/lib.rs").write_text("// transport fixture; no compilation\n")
        manifest = sync.capture(self.root, self.base / "source.tar.gz")
        manifest["source_archive"] = sync.BASE_SOURCE_PATH
        manifest_bytes = encoded(manifest)
        records = self.base / "records"
        records.mkdir()
        (records / "image-apple.json").write_text(json.dumps({"rust_version": modal_build.compiler_version("apple"),
                                                               "environment": "brassclaw", "image_id": "im-test",
                                                               "worker_identity": modal_build.worker_identity(),
                                                               "supported_modes": [modal_build.APPLE_TEST_MODE]}))
        resources, submitted = {}, {}
        spawns = []
        remote_files = {sync.MANIFEST_PATH: manifest_bytes, sync.BASE_MANIFEST_PATH: manifest_bytes}

        class App:
            app_id = "ap-test"

            def function(self, **kwargs):
                resources.update(kwargs)
                def decorate(function):
                    def spawn(original):
                        spawns.append(original["job_id"])
                        submitted.update(original)
                        result = {**{key: original[key] for key in
                                      ("job_id", "command", "manifest_sha256", "image_id")},
                                  "returncode": 0, "artifacts": {}}
                        prefix = f'/cache/jobs/{original["job_id"]}'
                        if test_mode:
                            result.update({key: original[key] for key in
                                           ("mode", "original_command", "worker_identity", "test_subject")})
                            body = b"transport fixture only; no native executable"
                            result["artifacts"] = {"composition-fixture": {
                                "sha256": hashlib.sha256(body).hexdigest(), "size": len(body),
                                "mode": original["mode"], "image_id": original["image_id"],
                                "manifest_sha256": original["manifest_sha256"], "target_triple": original["target_triple"],
                                "features": original["test_subject"]["features"], "profile": {"test": True},
                                "target": {"name": "brassclaw_reborn_composition", "kind": ["lib"]}}}
                            remote_files[prefix + "/artifacts/composition-fixture"] = body
                        remote_files[prefix + "/completed.json"] = encoded(result)
                        remote_files[prefix + "/image-provenance.json"] = b"{}"
                        remote_files[prefix + "/cargo.log"] = b"SDK fixture; no Cargo executed\n"
                        return SimpleNamespace(object_id="fc-test", get=lambda **kwargs: result)
                    return SimpleNamespace(spawn=spawn)
                return decorate

            def run(self, **kwargs):
                return nullcontext()

        failing = {"enabled": fail_download}
        def read_file(path):
            if failing["enabled"] and path.endswith("/cargo.log"):
                raise OSError("collection transport interrupted")
            return [remote_files[path]]
        volume = SimpleNamespace(read_file=read_file)
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
            if not fail_download:
                options = dict(cpu_limit=16, jobs=16)
                if test_mode:
                    from test_modal_build_worker import test_command
                    options["cargo"] = [*test_command(), "--message-format=json-render-diagnostics"]
                self.assertEqual(modal_build.run(arguments(**options)), 0)
            else:
                with self.assertRaisesRegex(OSError, "collection transport"):
                    modal_build.run(arguments(cpu_limit=16, jobs=16))
                state_path = records / "jobs" / submitted["job_id"] / "state.json"
                state = json.loads(state_path.read_bytes())
                self.assertEqual(state["state"], "collection_pending")
                self.assertEqual(state["remote_state"], "completed")
                self.assertNotIn("collection", state)
                self.assertTrue(state_path.with_name("completed.json").exists())
                failing["enabled"] = False
                modal_build.status(SimpleNamespace(job_id=submitted["job_id"], volume="changed"))
                state = json.loads(state_path.read_bytes())
                self.assertEqual((state["state"], state["collection"]), ("completed", "verified"))
                self.assertEqual(spawns, [submitted["job_id"]])
        if test_mode:
            self.assertEqual(submitted["command"][-1], "--message-format=json")
            self.assertEqual(submitted["original_command"][-1], "--message-format=json-render-diagnostics")
            self.assertEqual(submitted["worker_identity"], modal_build.worker_identity())
            self.assertEqual(submitted["test_subject"]["target_name"], "brassclaw_reborn_composition")
        upload.assert_called_once_with("brassclaw", "brassclaw-build")
        self.assertEqual(resources["cpu"], (8, 16))
        self.assertEqual(resources["memory"], (32768, 32768))
        self.assertEqual((resources["max_containers"], resources["min_containers"]), (1, 0))
        self.assertEqual(submitted["jobs"], 16)
        self.assertEqual(submitted["lock_scope"], "modal")
        self.assertEqual(submitted["manifest_sha256"], __import__("hashlib").sha256(manifest_bytes).hexdigest())


class CollectionBoundaryTests(unittest.TestCase):
    """Real temp-file custody with synthetic transport; no compiler/target execution."""

    def setUp(self):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.directory = Path(temporary.name).resolve()
        self.original = {"job_id": "736e8ba8-ad53-4d53-9011-c1c9b67083fe",
                         "command": ["cargo", "build", "--locked"], "manifest_sha256": "a" * 64,
                         "image_id": "im-fixture"}
        self.body = b"fixture bytes are not a genuine native artifact"
        self.result = {**self.original, "returncode": 0, "artifacts": {"brassclaw-reborn": {
            "sha256": hashlib.sha256(self.body).hexdigest(), "size": len(self.body)}}}
        self.files = {"completed.json": encoded(self.result), "image-provenance.json": b"{}",
                      "cargo.log": b"fixture stdout log", "brassclaw-reborn": self.body}
        self.volume = SimpleNamespace(read_file=lambda path: [self.files[path.rsplit("/", 1)[-1]]])

    def status_fixture(self, state_name="unresolved"):
        directory = self.directory / "jobs" / self.original["job_id"]
        directory.mkdir(parents=True)
        original = {**self.original, "volume": "original-volume", "environment": "brassclaw"}
        state = {"state": state_name, "job_id": original["job_id"]}  # Spawn answer/handle lost.
        (directory / "state.json").write_bytes(encoded(state))
        (directory / "request.json").write_bytes(encoded(original))
        reads, selected = [], []
        def read_file(path):
            self.assertTrue(path.startswith(f'/cache/jobs/{original["job_id"]}/'))
            reads.append(path)
            name = path.rsplit("/", 1)[-1]
            if name not in self.files:
                raise FileNotFoundError(path)
            if isinstance(self.files[name], Exception):
                raise self.files[name]
            return [self.files[name]]
        def from_name(name, **kwargs):
            selected.append((name, kwargs))
            return SimpleNamespace(read_file=read_file)
        # Intentionally no FunctionCall/App/spawn surface: recovery must work
        # solely from the original durable Volume receipt and byte snapshots.
        sdk = SimpleNamespace(Volume=SimpleNamespace(from_name=from_name))
        def invoke():
            with patch.dict(sys.modules, {"modal": sdk}), \
                    patch.object(modal_build, "records_root", return_value=self.directory), \
                    patch.object(modal_build, "modal_lock", return_value=nullcontext()):
                modal_build.status(SimpleNamespace(job_id=original["job_id"], volume="wrong-current-volume"))
        return directory, state, reads, selected, invoke

    def test_lost_spawn_handle_recovers_from_original_durable_job_receipt(self):
        directory, state, reads, selected, invoke = self.status_fixture("submitting")
        invoke()
        recovered = json.loads((directory / "state.json").read_bytes())
        self.assertEqual((recovered["state"], recovered["collection"]), ("completed", "verified"))
        self.assertEqual((directory / "brassclaw-reborn").read_bytes(), self.body)
        self.assertEqual(selected, [("original-volume", {"environment_name": "brassclaw"})])
        self.assertFalse((directory / "terminal.json").exists())

    def test_missing_or_transport_failed_receipt_without_handle_stays_unconfirmed(self):
        directory, state, reads, selected, invoke = self.status_fixture()
        for error in (FileNotFoundError("receipt absent"), OSError("Volume transport unavailable")):
            self.files["completed.json"] = error
            with self.subTest(error=error), self.assertRaises(type(error)):
                invoke()
            self.assertEqual(json.loads((directory / "state.json").read_bytes()), state)
            self.assertFalse((directory / "completed.json").exists())
            self.assertFalse((directory / "terminal.json").exists())
            self.assertFalse((directory / "brassclaw-reborn").exists())

    def test_repeated_status_reuses_verified_local_bytes_and_repairs_corrupt_or_missing_copy(self):
        directory, state, reads, selected, invoke = self.status_fixture()
        invoke()
        binary = directory / "brassclaw-reborn"
        original_stat = binary.stat()
        artifact_path = f'/cache/jobs/{self.original["job_id"]}/artifacts/brassclaw-reborn'
        self.assertEqual(reads.count(artifact_path), 1)
        invoke()
        self.assertEqual(reads.count(artifact_path), 1, "verified bytes must not be downloaded again")
        self.assertEqual((binary.stat().st_ino, binary.stat().st_mtime_ns),
                         (original_stat.st_ino, original_stat.st_mtime_ns))
        # Same-size corruption must trigger a hash check, not just size reuse.
        binary.chmod(0o600)
        binary.write_bytes(b"x" * len(self.body))
        invoke()
        self.assertEqual(reads.count(artifact_path), 2)
        self.assertEqual(binary.read_bytes(), self.body)
        binary.unlink()
        invoke()
        self.assertEqual(reads.count(artifact_path), 3)
        self.assertEqual(binary.read_bytes(), self.body)

    def test_symlink_local_artifact_is_replaced_without_following_its_target(self):
        directory, state, reads, selected, invoke = self.status_fixture()
        outside = self.directory / "outside-original-bytes"
        outside.write_bytes(self.body)
        binary = directory / "brassclaw-reborn"
        binary.symlink_to(outside)
        invoke()
        self.assertFalse(binary.is_symlink())
        self.assertEqual(binary.read_bytes(), self.body)
        self.assertEqual(outside.read_bytes(), self.body)
        self.assertEqual(sum(path.endswith("/artifacts/brassclaw-reborn") for path in reads), 1)

    def test_verified_atomic_copy_preserves_old_bytes_on_transfer_or_hash_failure(self):
        destination = self.directory / "brassclaw-reborn"
        destination.write_bytes(b"old verified copy")
        self.files["brassclaw-reborn"] = b"corrupt transfer"
        with self.assertRaisesRegex(ValueError, "checksum/size"):
            modal_build.collect_result(self.volume, self.directory, self.original, self.result)
        self.assertEqual(destination.read_bytes(), b"old verified copy")
        self.assertEqual(list(self.directory.glob(".download-*")), [])
        self.files["brassclaw-reborn"] = self.body
        modal_build.collect_result(self.volume, self.directory, self.original, self.result)
        self.assertEqual(destination.read_bytes(), self.body)
        self.assertEqual(destination.stat().st_mode & 0o777, 0o500)
        self.assertEqual((self.directory / "cargo.log").read_bytes(), b"fixture stdout log")

    def test_retained_job_or_original_identity_mismatch_rejects_before_publish(self):
        wrong = {**self.result, "image_id": "im-other"}
        with self.assertRaisesRegex(ValueError, "retained original"):
            modal_build.collect_result(self.volume, self.directory, self.original, wrong)
        self.assertFalse((self.directory / "brassclaw-reborn").exists())

    def test_original_product_activation_never_qualifies_new_test_mode(self):
        with self.assertRaisesRegex(ValueError, "DEACTIVATED"):
            modal_build.require_activation("apple", modal_build.APPLE_TEST_MODE)

    def test_volume_remote_error_does_not_reclassify_completed_job_as_terminal_failure(self):
        class TransportRemoteError(Exception):
            pass
        directory = self.directory / "jobs" / self.original["job_id"]
        directory.mkdir(parents=True)
        original = {**self.original, "volume": "original-volume", "environment": "brassclaw"}
        state = {"state": "collection_pending", "remote_state": "completed", "job_id": original["job_id"],
                 "function_call_id": "fc-original"}
        (directory / "state.json").write_bytes(encoded(state))
        (directory / "request.json").write_bytes(encoded(original))
        def read_file(path):
            raise TransportRemoteError("Volume unavailable; not a FunctionCall result")
        sdk = SimpleNamespace(Volume=SimpleNamespace(from_name=lambda *a, **kw: SimpleNamespace(read_file=read_file)),
                              exception=SimpleNamespace(RemoteError=TransportRemoteError, FunctionTimeoutError=TimeoutError))
        with patch.dict(sys.modules, {"modal": sdk}), \
                patch.object(modal_build, "records_root", return_value=self.directory), \
                patch.object(modal_build, "modal_lock", return_value=nullcontext()):
            with self.assertRaises(TransportRemoteError):
                modal_build.status(SimpleNamespace(job_id=original["job_id"]))
        self.assertEqual(json.loads((directory / "state.json").read_bytes()), state)
        self.assertFalse((directory / "terminal.json").exists())

    def test_legacy_product_activation_keeps_only_original_product_scope(self):
        image = {"image_id": "im-original-product"}
        record = {"active": True, "image_id": image["image_id"], "job_id": self.original["job_id"]}
        (self.directory / "image-apple.json").write_bytes(encoded(image))
        (self.directory / "activation-apple.json").write_bytes(encoded(record))
        with patch.object(modal_build, "records_root", return_value=self.directory):
            self.assertEqual(modal_build.require_activation("apple", modal_build.APPLE_BUILD_MODE), record)
            with self.assertRaisesRegex(ValueError, "test mode is DEACTIVATED"):
                modal_build.require_activation("apple", modal_build.APPLE_TEST_MODE)

    def test_old_image_and_unsupported_modes_reject_before_sdk_import_or_sync(self):
        from test_modal_build_worker import test_command
        image = self.directory / "image-apple.json"
        image.write_text(json.dumps({"rust_version": modal_build.compiler_version("apple"),
                                      "environment": "brassclaw", "image_id": "im-old-product"}))
        with patch.object(modal_build, "records_root", return_value=self.directory), \
                patch.dict(sys.modules, {"modal": None}), patch.object(modal_build, "sync_source") as upload:
            with self.assertRaisesRegex(ValueError, "exact worker/image"):
                modal_build.run(arguments(cargo=test_command()))
            for flags in (["--doc"], ["--profile=release"], ["--", "filter"], ["--test=other"]):
                with self.subTest(flags=flags), self.assertRaises(ValueError):
                    modal_build.run(arguments(cargo=[*test_command(), *flags]))
            upload.assert_not_called()



if __name__ == "__main__":
    unittest.main()
