"""Local source integrity/cache/command checks. Does not invoke Modal or Cargo."""

import importlib.util
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
        for command in (["cargo", "test", "--locked"], ["cargo", "run", "--locked"],
                        ["cargo", "bench", "--locked"], ["cargo", "build", "--locked", "--tests"],
                        ["cargo", "build", "--locked", "--all-targets"], ["cargo", "check"]):
            with self.subTest(command=command), self.assertRaises(ValueError):
                worker.validate_command(command)


if __name__ == "__main__":
    unittest.main()
