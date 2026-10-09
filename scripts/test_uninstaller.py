#!/usr/bin/env python3
"""Real temporary-filesystem tests for the self-contained uninstaller payload."""

import io
import os
import socket
from pathlib import Path
import tempfile
import types
import unittest

SCRIPT = Path(__file__).resolve().parents[1] / "uninstall.sh"
payload = SCRIPT.read_text().split("<<'PY'\n", 1)[1].rsplit("\nPY", 1)[0]
module = types.ModuleType("brassclaw_uninstaller_tests")
exec(compile(payload, str(SCRIPT), "exec"), module.__dict__)


class UninstallerTests(unittest.TestCase):
    def setUp(self):
        self.directory = tempfile.TemporaryDirectory(prefix="uninstaller-fixture-")
        self.addCleanup(self.directory.cleanup)
        self.root = Path(self.directory.name)
        self.home = self.root / "operator"
        self.home.mkdir()
        self.user = types.SimpleNamespace(pw_dir=str(self.home))
        self.plan = module.Plan([self.user])

    def populate(self):
        state = self.home / ".brassclaw"
        for name in ("reborn/postgres/data/PG_VERSION", "reborn/postgres/bin/postgres",
                     "reborn/db/secrets", "reborn/system/extensions/downloaded.wasm",
                     "reborn/workspace/result", "reborn/logs/run.log"):
            file = state / name
            file.parent.mkdir(parents=True, exist_ok=True)
            file.write_text("BrassClaw fixture")
        binary = self.home / ".local/bin"
        binary.mkdir(parents=True)
        for name in ("brassclaw-reborn", "brassclaw", "monty_worker", "monty_worker.bak", "brassclaw-reborn.bak"):
            (binary / name).write_text("owned executable")
        (binary / "unrelated-command").write_text("preserve")
        self.plan.add_root(state, "installed state")
        self.plan.binaries(binary)
        return state, binary

    def test_enter_wipes_but_eof_does_not_authorize(self):
        self.assertTrue(module.confirm(io.StringIO("\n")))
        self.assertFalse(module.confirm(io.StringIO("keep\n")))
        with self.assertRaisesRegex(RuntimeError, "No confirmation"):
            module.confirm(io.StringIO(""))
        with self.assertRaisesRegex(RuntimeError, "Cancelled"):
            module.confirm(io.StringIO("cancel\n"))

    def test_full_wipe_removes_nested_downloads_backups_and_worker(self):
        state, binary = self.populate()
        self.plan.delete(True)
        self.assertFalse(state.exists())
        self.assertEqual([p.name for p in binary.iterdir()], ["unrelated-command"])

    def test_keep_data_preserves_state(self):
        state, binary = self.populate()
        self.plan.delete(False)
        self.assertTrue((state / "reborn/postgres/data/PG_VERSION").exists())
        self.assertTrue((state / "reborn/db/secrets").exists())
        self.assertFalse((binary / "monty_worker").exists())

    def test_recursive_wipe_does_not_follow_nested_symlink(self):
        state, _ = self.populate()
        outside = self.root / "unrelated"
        outside.mkdir()
        sentinel = outside / "keep"
        sentinel.write_text("unrelated data")
        (state / "external-project").symlink_to(outside, target_is_directory=True)
        self.plan.delete(True)
        self.assertEqual(sentinel.read_text(), "unrelated data")

    def test_home_and_system_root_are_rejected_before_deletion(self):
        state, _ = self.populate()
        self.plan.add_root(self.home, "invalid configured home")
        self.plan.add_root("/", "invalid root")
        with self.assertRaisesRegex(RuntimeError, "unresolved"):
            self.plan.delete(True)
        self.assertTrue(state.exists())

    def test_parent_symlink_swap_invalidates_inventory(self):
        parent = self.root / "parent"
        parent.mkdir()
        (parent / "brassclaw-cache").mkdir()
        self.plan.add(parent / "brassclaw-cache", "runtime", "cache")
        parent.rename(self.root / "original")
        unrelated = self.root / "unrelated"
        unrelated.mkdir()
        (unrelated / "brassclaw-cache").mkdir()
        parent.symlink_to(unrelated, target_is_directory=True)
        with self.assertRaisesRegex(RuntimeError, "changed"):
            self.plan.delete(True)
        self.assertTrue((unrelated / "brassclaw-cache").exists())

    def test_temp_inventory_qualifies_legacy_downloads_and_preserves_other_files(self):
        temporary = self.root / "tmp"
        temporary.mkdir()
        owned = temporary / "tmp.legacy"
        owned.mkdir()
        (owned / "brassclaw-linux-amd64").write_text("download")
        mixed = temporary / "tmp.shared"
        mixed.mkdir()
        (mixed / "brassclaw-linux-amd64").write_text("download")
        (mixed / "unrelated.txt").write_text("preserve")
        (temporary / "brassclaw-download.test").mkdir()
        (temporary / "brassclaw-native-test").mkdir()
        unrelated_socket = temporary / ".s.PGSQL.5434"
        unrelated_socket.write_text("belongs to another database")
        self.plan.scan_temporary(temporary)
        self.plan.delete(True)
        self.assertFalse(owned.exists())
        self.assertFalse((temporary / "brassclaw-download.test").exists())
        self.assertFalse((temporary / "brassclaw-native-test").exists())
        self.assertTrue(mixed.exists())
        self.assertTrue(unrelated_socket.exists())

    def test_environment_files_are_parsed_without_executing_shell(self):
        config = self.root / "secrets.env"
        sentinel = self.root / "must-not-be-created"
        config.write_text('BRASSCLAW_REBORN_HOME="/srv/brassclaw with spaces"\n'
                          + 'VALUE="$(touch ' + str(sentinel) + ')"\n')
        values = module.environment_file(config)
        self.assertEqual(values["BRASSCLAW_REBORN_HOME"], "/srv/brassclaw with spaces")
        self.assertIn("$(touch", values["VALUE"])
        self.assertFalse(sentinel.exists())

    def test_external_database_prevents_false_complete_wipe(self):
        state, _ = self.populate()
        self.plan.consume_env({"BRASSCLAW_PG_URL": "postgresql://localhost/shared"})
        with self.assertRaisesRegex(RuntimeError, "unresolved"):
            self.plan.delete(True)
        self.assertTrue(state.exists())

    def test_invalid_configured_state_file_is_not_deleted(self):
        unrelated = self.root / "unrelated.conf"
        unrelated.write_text("preserve")
        self.plan.consume_env({"BRASSCLAW_REBORN_HOME": str(unrelated)})
        with self.assertRaisesRegex(RuntimeError, "unresolved"):
            self.plan.delete(True)
        self.assertTrue(unrelated.exists())

    def test_only_cluster_qualified_postgres_socket_is_removed(self):
        state, _ = self.populate()
        data = state / 'reborn/postgres/data'
        socket_path = self.root / '.s.PGSQL.5434'
        lock = Path(str(socket_path) + '.lock')
        fields = ['999999', str(data), '123456', '5434', str(self.root)]
        (data / 'postmaster.pid').write_text('\n'.join(fields) + '\n')
        with socket.socket(socket.AF_UNIX) as listener:
            listener.bind(str(socket_path))
            lock.write_text('111111\n/another/cluster\n123456\n')
            self.plan.discover_postgres_sockets()
            self.assertNotIn(socket_path, self.plan.paths)
            lock.write_text('\n'.join(fields) + '\n')
            self.plan.discover_postgres_sockets()
            self.plan.delete(True)
            self.assertFalse(socket_path.exists())
            self.assertFalse(lock.exists())


if __name__ == "__main__":
    unittest.main()
