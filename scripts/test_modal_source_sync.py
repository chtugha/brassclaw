"""Local real-filesystem/Git checks; never contacts Modal or starts Cargo."""

import importlib.util
import os
from pathlib import Path
import subprocess
import tarfile
import tempfile
import unittest


spec = importlib.util.spec_from_file_location(
    "modal_source_sync", Path(__file__).with_name("modal_source_sync.py")
)
sync = importlib.util.module_from_spec(spec)
spec.loader.exec_module(sync)


class SourceFixture(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.base = Path(self.temporary.name)
        self.root = self.base / "repository"
        self.root.mkdir()
        self.git("init", "-q", "-b", "main")
        (self.root / ".gitignore").write_text("LOCAL_TEST_ENV.md\nprivate.txt\n")
        (self.root / "source.rs").write_text("original source\n")
        self.git("add", ".")
        self.git("-c", "user.name=Sync Test", "-c", "user.email=sync@example.invalid",
                 "-c", "commit.gpgsign=false", "commit", "-qm", "initial")

    def git(self, *arguments):
        subprocess.run(["git", "-C", str(self.root), *arguments], check=True,
                       stdout=subprocess.PIPE, stderr=subprocess.PIPE)



class SourceCaptureTests(SourceFixture):
    def test_snapshot_preserves_pending_changes_deletions_modes_and_links(self):
        (self.root / "source.rs").unlink()
        (self.root / "pending.rs").write_text("pending source\n")
        (self.root / "run.sh").write_text("#!/bin/sh\nexit 0\n")
        (self.root / "run.sh").chmod(0o755)
        os.symlink("pending.rs", self.root / "linked.rs")
        (self.root / "LOCAL_TEST_ENV.md").write_text("private operator configuration")
        (self.root / "private.txt").write_text("private credentials")
        archive = self.base / "source.tar.gz"
        manifest = sync.capture(self.root, archive)
        self.assertEqual(set(manifest["files"]), {".gitignore", "pending.rs", "run.sh", "linked.rs"})
        with tarfile.open(archive) as captured:
            self.assertEqual(captured.extractfile("pending.rs").read(), b"pending source\n")
            self.assertEqual(captured.getmember("run.sh").mode, 0o755)
            self.assertTrue(captured.getmember("linked.rs").issym())
            self.assertEqual(captured.getmember("linked.rs").linkname, "pending.rs")

    def test_external_symlink_is_rejected(self):
        outside = self.base / "outside"
        outside.write_text("private external file")
        os.symlink("../outside", self.root / "external")
        with self.assertRaisesRegex(ValueError, "symlink escapes"):
            sync.capture(self.root, self.base / "source.tar.gz")

    def test_absolute_internal_symlink_is_rejected_as_nonportable(self):
        os.symlink(self.root / "source.rs", self.root / "absolute")
        with self.assertRaisesRegex(ValueError, "not portable"):
            sync.capture(self.root, self.base / "source.tar.gz")

    def test_source_changes_after_capture_are_rejected(self):
        manifest = sync.capture(self.root, self.base / "source.tar.gz")
        (self.root / "source.rs").write_text("changed source\n")
        with self.assertRaisesRegex(ValueError, "Source changed"):
            sync.verify_source(self.root, manifest)
        (self.root / "added.rs").write_text("new member")
        with self.assertRaisesRegex(ValueError, "membership changed"):
            sync.verify_source(self.root, manifest)

    def test_identical_inputs_produce_identical_archives(self):
        first = sync.capture(self.root, self.base / "first.tar.gz")
        second = sync.capture(self.root, self.base / "second.tar.gz")
        self.assertEqual(first, second)
        self.assertEqual((self.base / "first.tar.gz").read_bytes(),
                         (self.base / "second.tar.gz").read_bytes())

    def test_delta_transfers_only_changes_and_retains_exact_deletions(self):
        baseline = sync.capture(self.root, self.base / "base.tar.gz")
        (self.root / "source.rs").unlink()
        (self.root / "added.rs").write_text("new source\n")
        delta = sync.capture(self.root, self.base / "delta.tar.gz", baseline)
        self.assertEqual(delta["changed_files"], ["added.rs"])
        self.assertEqual(delta["deleted_files"], ["source.rs"])
        self.assertEqual(set(delta["files"]), {".gitignore", "added.rs"})
        with tarfile.open(self.base / "delta.tar.gz") as archive:
            self.assertEqual(archive.getnames(), ["added.rs"])


if __name__ == "__main__":
    unittest.main()
