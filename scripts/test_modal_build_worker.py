"""Local source integrity/cache/command checks. Does not invoke Modal or Cargo."""

import importlib.util
import os
import struct
from types import SimpleNamespace
from unittest.mock import patch
import json
from pathlib import Path
import shutil
import sys
import tempfile
import unittest

sys.path.insert(0, str(Path(__file__).parent))
import modal_build_worker as worker
from test_modal_source_sync import SourceFixture


class WorkerTests(SourceFixture):
    def records(self):
        storage = self.base / "volume"
        source = storage / "source"
        source.mkdir(parents=True)
        baseline = __import__("modal_source_sync").capture(self.root, source / "base.tar.gz")
        baseline["source_archive"] = "/source/base.tar.gz"
        (source / "base-manifest.json").write_bytes(worker.encoded(baseline))
        (self.root / "source.rs").unlink()
        (self.root / "new.rs").write_text("changed source\n")
        current = __import__("modal_source_sync").capture(self.root, source / "repository.tar.gz", baseline)
        current.update(source_archive="/source/repository.tar.gz",
                       base_archive_sha256=baseline["archive_sha256"],
                       base_manifest_sha256=worker.digest_file(source / "base-manifest.json"))
        (source / "manifest.json").write_bytes(worker.encoded(current))
        return storage, current, worker.digest_file(source / "manifest.json")

    def test_materialization_removes_old_members_and_verifies_delta(self):
        storage, current, checksum = self.records()
        destination = self.base / "materialized"
        result = worker.materialize(storage, destination, checksum)
        self.assertEqual(result, current)
        self.assertEqual((destination / "new.rs").read_text(), "changed source\n")
        self.assertFalse((destination / "source.rs").exists())

    def test_tampered_archive_and_admission_hash_are_rejected(self):
        storage, _, checksum = self.records()
        with self.assertRaisesRegex(ValueError, "admitted build"):
            worker.materialize(storage, self.base / "wrong", "0" * 64)
        with (storage / "source/repository.tar.gz").open("ab") as stream:
            stream.write(b"tampered")
        with self.assertRaisesRegex(ValueError, "checksum differs"):
            worker.materialize(storage, self.base / "tampered", checksum)

    def test_cache_reuse_preserves_unchanged_mtimes_and_replaces_stale_source(self):
        self.assertIsNotNone(shutil.which("rsync"), "actual rsync required")
        storage, manifest, checksum = self.records()
        staged = self.base / "staged"
        worker.materialize(storage, staged, checksum)
        cache = self.base / "cached"
        worker.update_cache(staged, cache, manifest)
        unchanged = (cache / ".gitignore").stat().st_mtime_ns
        (cache / "stale.rs").write_text("stale")
        worker.update_cache(staged, cache, manifest)
        self.assertEqual((cache / ".gitignore").stat().st_mtime_ns, unchanged)
        self.assertFalse((cache / "stale.rs").exists())

    def test_invocation_stream_is_logged_and_only_exact_admitted_executable_is_snapshotted(self):
        self.exercise_invocation()

    def test_complete_package_runs_one_normal_worker_build_and_exports_original_deps_layout(self):
        self.exercise_invocation(packaged=True)

    def test_failed_companion_cannot_publish_a_runnable_partial_package(self):
        self.exercise_invocation(packaged=True, companion_code=1)

    def test_complete_package_keeps_offline_and_frozen_for_companion(self):
        self.exercise_invocation(packaged=True, main_flags=("--offline", "--frozen"))

    def exercise_invocation(self, packaged=False, companion_code=0, main_flags=()):
        # Synthetic Popen/stdout/header IO boundary, not a Rust build or native
        # executable qualification. Source archives/materialization remain real.
        package = self.root / "crates" / worker.TEST_PACKAGE
        (package / "src").mkdir(parents=True)
        (package / "Cargo.toml").write_text('[package]\nname="brassclaw_reborn_composition"\nversion="0.1.0"\n')
        (package / "src/lib.rs").write_text("// fixture only\n")
        if packaged:
            for name in ("brassclaw_monty_host", "brassclaw_embedded_postgres"):
                dependency = self.root / "crates" / name
                dependency.mkdir()
                (dependency / "Cargo.toml").write_text('[package]\nname="' + name + '"\nversion="0.1.0"\n')
        storage, manifest, checksum = self.records()
        storage = storage.resolve()
        original = [*test_command(), *main_flags, "--message-format", "json-render-diagnostics"]
        command = worker.validate_command(original)
        request = {"job_id": "736e8ba8-ad53-4d53-9011-c1c9b67083fe", "command": command,
                   "original_command": original, "mode": worker.APPLE_TEST_MODE,
                   "worker_identity": worker.worker_identity(), "image_id": "im-fixture",
                   "manifest_sha256": checksum, "target_triple": worker.APPLE_TARGET,
                   "jobs": 1, "timeout_secs": 300, "shutdown_grace_secs": 1,
                   "test_subject": worker.test_subject(self.root, command)["contract"]}
        if packaged:
            request["runtime_package"] = 1
        logged, invocations = [], []
        def popen(actual, **kwargs):
            invocations.append(actual)
            companion = len(invocations) == 2
            expected_companion = worker.worker_command(worker.APPLE_TARGET)
            expected_companion[3:3] = list(main_flags)
            self.assertEqual(actual, expected_companion if companion else command)
            cache = storage / "cache/images/im-fixture"
            self.assertEqual(kwargs["env"]["CARGO_TARGET_DIR"], str(cache / "target"))
            binary = cache / "target" / worker.APPLE_TARGET / ("debug/monty_worker" if companion else "debug/deps/composition-123")
            binary.parent.mkdir(parents=True, exist_ok=True)
            binary.write_bytes(struct.pack("<IIIIIIII", 0xfeedfacf, 0x0100000c, 0, 2, 0, 0, 0, 0))
            binary.chmod(0o755)
            subject = worker.test_subject(cache / "repository", command)
            artifact = {"reason": "compiler-artifact", "package_id": subject["package_ids"][0],
                        "manifest_path": subject["manifest_path"], "features": [], "fresh": True,
                        "target": {"name": worker.TEST_PACKAGE, "kind": ["lib"], "crate_types": ["lib"],
                                   "src_path": subject["src_path"], "test": True},
                        "profile": {"test": True}, "filenames": [str(binary)], "executable": str(binary)}
            extra = []
            if companion:
                manifest = cache / "repository/crates/brassclaw_monty_host/Cargo.toml"
                artifact.update(package_id=sorted(worker.package_ids(manifest))[0], manifest_path=str(manifest),
                                target={"name": "monty_worker", "kind": ["bin"]}, profile={"test": False})
            elif packaged:
                manifest = cache / "repository/crates/brassclaw_embedded_postgres/Cargo.toml"
                identity = sorted(worker.package_ids(manifest))[0]
                out = cache / "target" / worker.APPLE_TARGET / "debug/build/brassclaw_embedded_postgres-fixture/out"
                out.mkdir(parents=True)
                with worker.tarfile.open(out / "pgvector.tar.gz", "w:gz") as archive:
                    for name, data in (("lib/vector.dylib", struct.pack("<II", 0xfeedfacf, 0x0100000c)),
                                       ("share/extension/vector.control", b"fixture"),
                                       ("share/extension/vector--0.8.0.sql", b"fixture")):
                        member = worker.tarfile.TarInfo(name)
                        member.size = len(data)
                        archive.addfile(member, worker.io.BytesIO(data))
                extra = [{"reason": "compiler-artifact", "package_id": identity, "manifest_path": str(manifest), "profile": {"test": False}},
                         {"reason": "build-script-executed", "package_id": identity, "out_dir": str(out)},
                         {"reason": "build-script-executed", "package_id": identity,
                          "out_dir": str(cache / "target/debug/build/brassclaw_embedded_postgres-host/out")}]
            code = companion_code if companion else 0
            lines = ["non-JSON build output\n", json.dumps(artifact) + "\n", *[json.dumps(item) + "\n" for item in extra],
                     json.dumps({"reason": "build-finished", "success": code == 0}) + "\n"]
            logged.extend(lines)
            return SimpleNamespace(stdout=iter(lines), wait=lambda **kw: code, poll=lambda: code)
        commits = []
        with patch.object(worker.subprocess, "Popen", side_effect=popen) as execute, \
                patch.object(worker, "update_cache", side_effect=lambda src, dest, manifest: shutil.copytree(src, dest)), \
                patch.object(worker.os, "sync"), \
                patch.object(worker.subprocess, "check_output", return_value="fixture:\n\t/usr/lib/libSystem.B.dylib (compatibility version 1.0)\n"):
            result = worker.run_build(storage, request, lambda: commits.append(True))
        self.assertEqual(execute.call_count, 2 if packaged else 1)
        self.assertEqual(result["command"], command)
        self.assertEqual(result["original_command"], original)
        self.assertEqual(result["worker_identity"], request["worker_identity"])
        self.assertEqual(result["test_subject"], request["test_subject"])
        expected = {"deps/composition-123", "monty_worker", "pgvector.tar.gz"} if packaged else {"composition-123"}
        self.assertEqual(result["returncode"], companion_code)
        self.assertEqual(set(result["artifacts"]), set() if companion_code else expected)
        job = storage / "cache/jobs" / request["job_id"]
        text = (job / "cargo.log").read_text()
        self.assertTrue(all(line in text for line in logged))
        self.assertEqual(text.count("Command: "), len(invocations))
        # Re-reading the same completed ID returns its original; never Popen again.
        with patch.object(worker.subprocess, "Popen") as execute:
            self.assertEqual(worker.run_build(storage, request, lambda: None), result)
            execute.assert_not_called()

    def test_source_preparation_consumes_the_same_deadline_before_any_cargo_start(self):
        self.exercise_preparation_deadline()

    def test_function_preflight_before_worker_consumes_the_original_deadline(self):
        self.exercise_preparation_deadline(outer=True)

    def exercise_preparation_deadline(self, outer=False):
        storage, _, checksum = self.records()
        request = {"job_id": "736e8ba8-ad53-4d53-9011-c1c9b67083fe", "command": ["cargo", "build", "--locked",
                   "-p", "brassclaw_reborn_cli", "--bin", "brassclaw-reborn", "--target", worker.APPLE_TARGET],
                   "image_id": "im-fixture", "manifest_sha256": checksum, "target_triple": worker.APPLE_TARGET,
                   "jobs": 1, "timeout_secs": 300, "shutdown_grace_secs": 1}
        request["command"] = worker.validate_command(request["command"], worker.APPLE_TARGET)
        with patch.object(worker.time, "monotonic", side_effect=[181] if outer else [0, 181]), \
                patch.object(worker, "update_cache", side_effect=lambda src, dest, manifest: shutil.copytree(src, dest)), \
                patch.object(worker.os, "sync"), patch.object(worker.subprocess, "Popen") as cargo:
            with self.assertRaisesRegex(TimeoutError, "Source preparation exhausted"):
                worker.run_build(storage, request, lambda: None, **({"started_monotonic": 0} if outer else {}))
            cargo.assert_not_called()

    def test_target_and_host_override_arguments_are_rejected(self):
        worker.validate_command(["cargo", "build", "--locked", "--target=aarch64-apple-darwin"], "aarch64-apple-darwin")
        for flags in (["--target=x86_64-unknown-linux-gnu"], ["--config", "build.rustc=evil"],
                      ["--target-dir=/tmp/other"], ["--manifest-path=other/Cargo.toml"],
                      ["--tests=true"], ["--target=aarch64-apple-darwin", "--target=aarch64-apple-darwin"]):
            with self.subTest(flags=flags), self.assertRaises(ValueError):
                worker.validate_command(["cargo", "build", "--locked", *flags], "aarch64-apple-darwin")

    def test_only_compilation_and_static_analysis_commands_are_accepted(self):
        worker.validate_command(["cargo", "clippy", "--locked", "--tests", "--", "-D", "warnings"])
        worker.validate_command(["cargo", "build", "--locked", "-p", "brassclaw"])
        # cargo test --locked without --no-run must still be rejected.
        for command in (["cargo", "test", "--locked"], ["cargo", "run", "--locked"],
                        ["cargo", "bench", "--locked"], ["cargo", "build", "--locked", "--tests"],
                        ["cargo", "build", "--locked", "--all-targets"], ["cargo", "check"]):
            with self.subTest(command=command), self.assertRaises(ValueError):
                worker.validate_command(command)

    def test_only_explicit_lib_test_preparation_and_json_forms_are_accepted(self):
        command = test_command()
        expected = [*command, "--message-format=json"]
        self.assertEqual(worker.validate_command(command), expected)
        for flags in (["--message-format=json"], ["--message-format", "json-render-diagnostics"],
                      ["--message-format=json,json-diagnostic-short"], ["--profile=test"],
                      ["--profile", "test"], ["--features=skills-db"]):
            normalized = worker.validate_command([*command, *flags], worker.APPLE_TARGET)
            self.assertEqual(normalized.count("--message-format=json"), 1)
        inline = ["cargo", "test", "--locked", "--no-run", "--lib",
                  "--package=brassclaw_reborn_composition", "--target=aarch64-apple-darwin"]
        self.assertEqual(worker.command_mode(inline, worker.APPLE_TARGET), worker.APPLE_TEST_MODE)
        for flags in (["--doc"], ["--", "filter"], ["filter"], ["--test=fixture"], ["--bench", "fixture"],
                      ["--all-targets"], ["--bins"], ["--bin=other"], ["--release"], ["--profile=release"],
                      ["--profile", "custom"], ["--message-format=human"], ["--message-format=json-diagnostic-short"],
                      ["--message-format=json", "--message-format=json"], ["-p", "another-package"],
                      ["--target-dir=/tmp/other"]):
            with self.subTest(flags=flags), self.assertRaises(ValueError):
                worker.validate_command([*command, *flags], worker.APPLE_TARGET)
        for missing in ("--no-run", "--locked", "--lib"):
            with self.subTest(missing=missing), self.assertRaises(ValueError):
                worker.validate_command([arg for arg in command if arg != missing])


def test_command():
    return ["cargo", "test", "--locked", "--no-run", "--lib", "-p", worker.TEST_PACKAGE,
            "--target", worker.APPLE_TARGET]


class ArtifactBoundaryTests(unittest.TestCase):
    """Synthetic JSON/header fixtures test custody guards, never native qualification."""

    def setUp(self):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.base = Path(temporary.name).resolve()
        self.cache = self.base / "cache"
        self.repository = self.cache / "repository"
        package = self.repository / "crates" / worker.TEST_PACKAGE
        package.mkdir(parents=True)
        (package / "src").mkdir()
        (package / "src/lib.rs").write_text("// fixture only\n")
        self.manifest = package / "Cargo.toml"
        self.manifest.write_text('[package]\nname="brassclaw_reborn_composition"\nversion="0.1.0"\n'
                                 '[features]\ndefault=["postgres"]\npostgres=[]\nskills-db=["postgres"]\n', encoding="utf-8")
        self.command = worker.validate_command(test_command())
        self.subject = worker.test_subject(self.repository, self.command)
        self.binary = self.cache / "target" / worker.APPLE_TARGET / "debug/deps/composition-abc123"
        self.binary.parent.mkdir(parents=True)
        self.binary.write_bytes(struct.pack("<IIIIIIII", 0xfeedfacf, 0x0100000c, 0, 2, 0, 0, 0, 0))
        self.binary.chmod(0o755)
        self.message = {"reason": "compiler-artifact", "package_id": self.subject["package_ids"][0],
                        "manifest_path": str(self.manifest),
                        "target": {"kind": ["lib"], "crate_types": ["lib"], "name": worker.TEST_PACKAGE,
                                   "src_path": self.subject["src_path"], "test": True},
                        "profile": {"test": True, "opt_level": "1"}, "features": self.subject["features"],
                        "filenames": [str(self.binary)], "executable": str(self.binary), "fresh": True}
        self.request = {"image_id": "im-fixture", "manifest_sha256": "a" * 64}
        self.finished = [{"reason": "build-finished", "success": True}]
        self.index = 0

    def collect(self, messages=None, finished=None):
        self.index += 1
        job = self.base / f"job-{self.index}"
        job.mkdir()
        result = worker.collect_test_artifacts([self.message] if messages is None else messages,
                        self.finished if finished is None else finished, self.subject, self.cache, job, self.request)
        return job, result

    def test_fresh_current_invocation_selects_one_snapshot_ignoring_stale_neighbors(self):
        stale = self.binary.with_name("unrelated-newer-executable")
        stale.write_bytes(self.binary.read_bytes())
        stale.chmod(0o755)
        os.utime(stale, (2000000000, 2000000000))
        job, artifacts = self.collect()
        self.assertEqual(set(artifacts), {self.binary.name})
        artifact = artifacts[self.binary.name]
        self.assertTrue(artifact["fresh"])
        self.assertEqual(artifact["sha256"], worker.digest_file(job / "artifacts" / self.binary.name))
        self.assertEqual(artifact["image_id"], self.request["image_id"])
        self.assertEqual(artifact["manifest_sha256"], self.request["manifest_sha256"])
        self.binary.write_bytes(b"later cache overwrite")
        self.assertEqual(worker.digest_file(job / "artifacts" / self.binary.name), artifact["sha256"])

    def test_missing_duplicate_or_wrong_identity_profile_features_are_rejected(self):
        for messages, finished in (([], self.finished), ([self.message, self.message], self.finished),
                                   ([self.message], []), ([self.message], self.finished * 2),
                                   ([self.message], [{"reason": "build-finished", "success": False}])):
            with self.subTest(messages=messages, finished=finished), self.assertRaises(ValueError):
                self.collect(messages, finished)
        alterations = [dict(package_id="registry+wrong#0.1.0"), dict(manifest_path="/wrong/Cargo.toml"),
                       dict(features=["unexpected"]), dict(fresh="true"), dict(profile={"test": False}),
                       dict(target={**self.message["target"], "kind": ["test"]}),
                       dict(executable=str(self.binary).replace("/debug/", "/release/")), dict(filenames=[])]
        for alteration in alterations:
            with self.subTest(alteration=alteration), self.assertRaises(ValueError):
                self.collect([{**self.message, **alteration}])

    def test_symlinks_nonregular_empty_and_wrong_architecture_never_export(self):
        original = self.binary.read_bytes()
        other = self.base / "other"
        other.write_bytes(original)
        other.chmod(0o755)
        self.binary.unlink()
        self.binary.symlink_to(other)
        with self.assertRaises(ValueError):
            self.collect()
        self.binary.unlink()
        self.binary.mkdir()
        with self.assertRaises((ValueError, IsADirectoryError)):
            self.collect()
        self.binary.rmdir()
        os.mkfifo(self.binary)
        with self.assertRaisesRegex(ValueError, "regular executable"):
            self.collect()
        self.binary.unlink()
        for data in (b"", b"ELF" * 40, struct.pack("<IIIIIIII", 0xfeedfacf, 0x01000007, 3, 2, 0, 0, 0, 0),
                     struct.pack("<IIIIIIII", 0xfeedfacf, 0x0100000c, 0, 6, 0, 0, 0, 0)):
            self.binary.write_bytes(data)
            self.binary.chmod(0o755)
            with self.subTest(data=data), self.assertRaises(ValueError):
                self.collect()

    def test_custom_harness_and_unresolved_feature_are_rejected(self):
        self.manifest.write_text(self.manifest.read_text() + '[lib]\nharness=false\n')
        with self.assertRaisesRegex(ValueError, "harness"):
            worker.test_subject(self.repository, self.command)
        self.manifest.write_text(self.manifest.read_text().replace('harness=false', 'harness=true'))
        with self.assertRaisesRegex(ValueError, "named composition features"):
            worker.test_subject(self.repository, [*self.command, "--features=unknown"])

    def test_requested_pg_dependency_feature_is_retained_separately_from_package_features(self):
        dependency = self.repository / "crates/brassclaw_pg"
        dependency.mkdir()
        (dependency / "Cargo.toml").write_text('[package]\nname="brassclaw_pg"\nversion="0.1.0"\n[features]\ntest-support=[]\n')
        self.manifest.write_text(self.manifest.read_text() + '[dependencies]\nbrassclaw_pg={path="../brassclaw_pg"}\n')
        selected = worker.test_subject(self.repository, [*self.command, "--features=brassclaw_reborn_composition/skills-db,brassclaw_pg/test-support"])
        self.assertEqual(selected["contract"]["dependency_features"], ["brassclaw_pg/test-support"])
        self.assertEqual(selected["features"], ["default", "postgres", "skills-db"])
        with self.assertRaises(ValueError):
            worker.test_subject(self.repository, [*self.command, "--features=brassclaw_pg/unknown"])

    def test_product_test_profile_is_not_a_harness_and_worker_is_one_normal_dev_binary(self):
        product = ["cargo", "build", "--locked", "-p", "brassclaw_reborn_cli", "--bin", "brassclaw-reborn",
                   "--target", worker.APPLE_TARGET, "--profile", "test"]
        self.assertEqual(worker.command_mode(product, worker.APPLE_TARGET), worker.APPLE_BUILD_MODE)
        self.assertEqual(worker.worker_command(worker.APPLE_TARGET), ["cargo", "build", "--locked", "-p",
                         "brassclaw_monty_host", "--bin", "monty_worker", "--target", worker.APPLE_TARGET,
                         "--message-format=json"])
        baseline = worker.worker_command(worker.APPLE_TARGET)
        for flags in (("--offline",), ("--frozen",), ("--offline", "--frozen")):
            with self.subTest(flags=flags):
                expected = [*baseline[:3], *flags, *baseline[3:]]
                self.assertEqual(worker.worker_command(worker.APPLE_TARGET, [*product, *flags]), expected)
        package = self.repository / "crates/brassclaw_reborn_cli"
        package.mkdir()
        manifest = package / "Cargo.toml"
        manifest.write_text('[package]\nname="brassclaw_reborn_cli"\nversion="0.1.0"\n')
        binary = self.binary.parent.parent / "brassclaw-reborn"
        shutil.copy2(self.binary, binary)
        message = {**self.message, "executable": str(binary), "filenames": [str(binary)],
                   "manifest_path": str(manifest), "package_id": sorted(worker.package_ids(manifest))[0],
                   "profile": {"test": False}, "target": {"name": "brassclaw-reborn", "kind": ["bin"]}}
        job = self.base / "product-job"
        job.mkdir()
        artifacts = worker.collect_product_artifact([message], self.finished, self.cache, job, product)
        self.assertEqual(artifacts["brassclaw-reborn"]["role"], "product")
        for change in ({"profile": {"test": True}}, {"package_id": "registry+other#0.1.0"},
                       {"executable": str(binary.parent / "stale")}, {"fresh": None}):
            with self.subTest(change=change), self.assertRaises(ValueError):
                worker.collect_product_artifact([{**message, **change}], self.finished, self.cache, job, product)




if __name__ == "__main__":
    unittest.main()
