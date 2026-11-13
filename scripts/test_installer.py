#!/usr/bin/env python3
"""Exercise installer file operations with real local tools, never a service install."""

import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest


INSTALLER = Path(__file__).resolve().parents[1] / "install.sh"


class InstallerTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix="brassclaw-installer-test-")
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.destination = self.root / "bin"
        self.download = self.root / "download"
        self.destination.mkdir()
        self.download.mkdir()
        # Real executables as payloads; no mocked install/mv/checksum commands.
        shutil.copyfile("/bin/sh", self.download / "artifact")
        shutil.copyfile("/usr/bin/env", self.download / "artifact-monty-worker")

    def run_shell(self, code, *, expected=0, path=None):
        environment = os.environ.copy()
        environment.update(
            TEST_INSTALLER=str(INSTALLER),
            TEST_DESTINATION=str(self.destination),
            TEST_DOWNLOAD=str(self.download),
            HOME=str(self.root / "home"),
            TMPDIR=str(self.root),
        )
        if path is not None:
            environment["PATH"] = str(path)
        result = subprocess.run(
            ["/bin/bash", "-c", 'source "$TEST_INSTALLER"\n'
             'INSTALL_DIR="$TEST_DESTINATION"\n'
             'DOWNLOAD_DIR="$TEST_DOWNLOAD"\n' + code],
            env=environment, capture_output=True, text=True, timeout=15,
        )
        self.assertEqual(result.returncode, expected, result.stdout + result.stderr)
        return result

    def old_pair(self, *, worker=True):
        old = self.destination / "brassclaw-reborn"
        old.write_bytes(b"previous application")
        old.chmod(0o751)
        if worker:
            (self.destination / "monty_worker").write_bytes(b"previous worker")

    def local_pair(self):
        for name in ("artifact", "artifact-monty-worker"):
            payload = self.download / name
            (self.download / (name + ".sha256")).write_text(
                hashlib.sha256(payload.read_bytes()).hexdigest() + "\n")

    def without_curl(self):
        tools = self.root / "offline-tools"
        tools.mkdir()
        # Expose only real installed executables; no network command or stubs.
        for name in ("python3", "cat", "tr", "sed", "tail", "mktemp", "install",
                     "cp", "mv", "chmod", "mkdir", "ln", "rm", "rmdir", "od",
                     "id", "uname", "sha256sum", "shasum"):
            target = shutil.which(name)
            if target:
                (tools / name).symlink_to(target)
        return tools

    @unittest.skipIf(os.geteuid() == 0, "manual user installation requires a non-root account")
    def test_local_main_offline_with_digest_only_checksums(self):
        self.local_pair()
        # Match detect_artifact's actual platform name without executing payloads.
        artifact = self.run_shell('detect_artifact').stdout.strip()
        for suffix in ("", ".sha256", "-monty-worker", "-monty-worker.sha256"):
            shutil.copyfile(self.download / ("artifact" + suffix), self.download / (artifact + suffix))
        original = {p.name: p.read_bytes() for p in self.download.iterdir()}
        tools = self.without_curl()
        for version_args, expected in (("", "version unknown"), ("-v v1.7.0-rc.4", "operator-supplied version: v1.7.0-rc.4")):
            with self.subTest(version=version_args):
                result = self.run_shell('main --local-dir "$TEST_DOWNLOAD" ' + version_args, path=tools)
                self.assertIn(expected, result.stdout)
                self.assertEqual({p.name: p.read_bytes() for p in self.download.iterdir()}, original)
                installed = self.root / "home" / ".local" / "bin"
                self.assertEqual((installed / "brassclaw-reborn").read_bytes(), original[artifact])
                self.assertEqual((installed / "monty_worker").read_bytes(), original[artifact + "-monty-worker"])
                self.assertFalse((installed / ".brassclaw-install.lock").exists())

    def test_network_downloads_still_require_curl(self):
        tools = self.without_curl()
        self.run_shell('LOCAL_DIR="$TEST_DOWNLOAD"\nrequire_commands', path=tools)
        result = self.run_shell('require_commands', expected=1, path=tools)
        self.assertIn('Required command missing: curl', result.stderr)

    def test_local_relative_space_path_and_snapshot_isolation(self):
        self.local_pair()
        source = self.root / "local source"
        self.download.rename(source)
        self.download = source
        original = {p.name: p.read_bytes() for p in source.iterdir()}
        result = self.run_shell('cd "$TEST_DOWNLOAD/.."\n'
                                'parse_args --local-dir "local source"\ntrap cleanup EXIT\n'
                                'snapshot_local_pair artifact\n'
                                'printf "%s\\n" "$DOWNLOAD_DIR"\n'
                                'printf changed > "$LOCAL_DIR/artifact"\n'
                                'stage_pair artifact\ncommit_pair')
        snapshot = Path(result.stdout.strip())
        self.assertFalse(snapshot.exists())
        self.assertTrue(source.is_dir())
        self.assertEqual((self.destination / "brassclaw-reborn").read_bytes(), original["artifact"])
        self.assertEqual((source / "artifact").read_bytes(), b"changed")
        for name in original.keys() - {"artifact"}:
            self.assertEqual((source / name).read_bytes(), original[name])

    def test_local_failures_leave_source_and_installation_untouched(self):
        self.local_pair()
        self.old_pair()
        worker = self.download / "artifact-monty-worker"
        checksum = self.download / "artifact-monty-worker.sha256"
        for problem in ("missing-worker", "missing-checksum", "wrong-checksum", "malformed-checksum", "mixed-worker", "symlink", "checksum-symlink", "fifo"):
            with self.subTest(problem=problem):
                worker.unlink(missing_ok=True)
                checksum.unlink(missing_ok=True)
                shutil.copyfile("/usr/bin/env", worker)
                checksum.write_text(hashlib.sha256(worker.read_bytes()).hexdigest() + "\n")
                if problem == "missing-worker":
                    worker.unlink()
                elif problem == "missing-checksum":
                    checksum.unlink()
                elif problem == "wrong-checksum":
                    checksum.write_text("0" * 64 + "\n")
                elif problem == "malformed-checksum":
                    checksum.write_text("v1.7.0 is not a digest\n")
                elif problem == "mixed-worker":
                    shutil.copyfile("/bin/sh", worker)
                elif problem == "checksum-symlink":
                    checksum.unlink()
                    checksum.symlink_to(self.download / "artifact.sha256")
                elif problem == "symlink":
                    worker.unlink()
                    worker.symlink_to("/usr/bin/env")
                else:
                    worker.unlink()
                    os.mkfifo(worker)
                original = {p.name: (p.lstat().st_mode, os.readlink(p) if p.is_symlink()
                                      else p.read_bytes() if p.is_file() else None)
                            for p in self.download.iterdir()}
                result = self.run_shell('LOCAL_DIR="$TEST_DOWNLOAD"\ntrap cleanup EXIT\n'
                                        'snapshot_local_pair artifact\nstage_pair artifact\ncommit_pair', expected=1)
                self.assertTrue(result.stderr)
                self.assertEqual({p.name: (p.lstat().st_mode, os.readlink(p) if p.is_symlink()
                                           else p.read_bytes() if p.is_file() else None)
                                  for p in self.download.iterdir()}, original)
                self.assertTrue(self.download.is_dir())
                self.assertFalse((self.destination / ".brassclaw-install.lock").exists())
                self.assertEqual((self.destination / "brassclaw-reborn").read_bytes(), b"previous application")
                self.assertEqual((self.destination / "monty_worker").read_bytes(), b"previous worker")
                self.assertEqual(list(self.root.glob("brassclaw-download.*")), [])

    def test_local_snapshot_rollback_preserves_source(self):
        self.local_pair()
        self.old_pair()
        original = {p.name: p.read_bytes() for p in self.download.iterdir()}
        self.run_shell('LOCAL_DIR="$TEST_DOWNLOAD"\ntrap cleanup EXIT\n'
                       'snapshot_local_pair artifact\nstage_pair artifact\n'
                       'rm "$STAGE_DIR/monty_worker"\ncommit_pair', expected=1)
        self.assertEqual({p.name: p.read_bytes() for p in self.download.iterdir()}, original)
        self.assertEqual((self.destination / "brassclaw-reborn").read_bytes(), b"previous application")
        self.assertEqual((self.destination / "monty_worker").read_bytes(), b"previous worker")

    def test_empty_and_missing_option_values_fail_before_install(self):
        for arguments in ('--local-dir', '--local-dir ""', '--local-dir --version',
                          '-v', '-v ""', '-v v', '--local-dir /nonexistent',
                          '--local-dir "$TEST_DOWNLOAD" -v invalid'):
            with self.subTest(arguments=arguments):
                self.run_shell('main ' + arguments, expected=1, path=self.root)
                self.assertEqual(list(self.destination.iterdir()), [])
                self.assertTrue(self.download.is_dir())

    def test_valid_checksum_and_corruption(self):
        artifact = self.download / "artifact"
        checksum = self.download / "artifact.sha256"
        checksum.write_text(hashlib.sha256(artifact.read_bytes()).hexdigest() + "\n")
        self.run_shell('sha256_check "$DOWNLOAD_DIR/artifact" "$DOWNLOAD_DIR/artifact.sha256"')
        artifact.write_bytes(b"corrupted download")
        self.run_shell('sha256_check "$DOWNLOAD_DIR/artifact" "$DOWNLOAD_DIR/artifact.sha256"', expected=1)

    def test_release_selection_requires_one_complete_published_pair(self):
        assets = [{'name': name} for name in ('artifact', 'artifact.sha256', 'artifact-monty-worker', 'artifact-monty-worker.sha256')]
        catalog = self.download / 'releases.json'
        catalog.write_text(json.dumps([
            {'tag_name': 'v2.0.0', 'published_at': '2026-10-10', 'draft': True, 'assets': assets},
            {'tag_name': 'v1.8.0', 'published_at': '2026-10-09', 'assets': assets[:2]},
            {'tag_name': 'v1.7.0-rc.4', 'published_at': '2026-10-08', 'prerelease': True, 'assets': assets},
            {'tag_name': 'v1.6.0', 'published_at': '2026-10-07', 'assets': assets},
        ]))
        result = self.run_shell('select_release artifact < "$DOWNLOAD_DIR/releases.json"')
        self.assertEqual(result.stdout.strip(), '1.7.0-rc.4')
        self.assertIn('prerelease', result.stderr)
        self.run_shell('select_release unsupported < "$DOWNLOAD_DIR/releases.json"', expected=1)

    def test_malformed_and_missing_verifier_fail_closed(self):
        checksum = self.download / "artifact.sha256"
        checksum.write_text("not a checksum\n")
        self.run_shell('sha256_check "$DOWNLOAD_DIR/artifact" "$DOWNLOAD_DIR/artifact.sha256"', expected=1)
        checksum.write_text(hashlib.sha256((self.download / "artifact").read_bytes()).hexdigest())
        tools = self.root / "limited-path"
        tools.mkdir()
        # An actual cat executable is sufficient to read the expected digest;
        # neither supported real checksum executable is exposed in this PATH.
        (tools / "cat").symlink_to(shutil.which("cat"))
        self.run_shell('sha256_check "$DOWNLOAD_DIR/artifact" "$DOWNLOAD_DIR/artifact.sha256"', expected=1, path=tools)

    def test_commit_and_backups_under_restrictive_umask(self):
        self.old_pair()
        self.run_shell('umask 077\ntrap cleanup EXIT\nstage_pair artifact\ncommit_pair')
        for installed, asset, old in (
            ("brassclaw-reborn", "artifact", b"previous application"),
            ("monty_worker", "artifact-monty-worker", b"previous worker"),
        ):
            # Cleanup removes downloads; compare with the real original payload.
            source = "/bin/sh" if asset == "artifact" else "/usr/bin/env"
            self.assertEqual((self.destination / installed).read_bytes(), Path(source).read_bytes())
            self.assertEqual((self.destination / installed).stat().st_mode & 0o777, 0o755)
            self.assertEqual((self.destination / (installed + ".bak")).read_bytes(), old)
        self.assertEqual(sorted(p.name for p in self.destination.iterdir()),
                         ["brassclaw-reborn", "brassclaw-reborn.bak", "monty_worker", "monty_worker.bak"])

    def test_interrupted_replacement_restores_exact_pair(self):
        self.old_pair()
        self.run_shell('trap cleanup EXIT\nstage_pair artifact\nPAIR_COMMITTING=true\n'
                       'mv "$STAGE_DIR/monty_worker" "$INSTALL_DIR/monty_worker"\nfalse', expected=1)
        self.assertEqual((self.destination / "brassclaw-reborn").read_bytes(), b"previous application")
        self.assertEqual((self.destination / "monty_worker").read_bytes(), b"previous worker")
        self.assertEqual((self.destination / "brassclaw-reborn").stat().st_mode & 0o777, 0o751)
        self.assertFalse((self.destination / ".brassclaw-install.lock").exists())

    def test_interrupted_install_removes_new_companion_if_previously_absent(self):
        self.old_pair(worker=False)
        self.run_shell('trap cleanup EXIT\nstage_pair artifact\nPAIR_COMMITTING=true\n'
                       'mv "$STAGE_DIR/monty_worker" "$INSTALL_DIR/monty_worker"\nfalse', expected=1)
        self.assertFalse((self.destination / "monty_worker").exists())
        self.assertEqual((self.destination / "brassclaw-reborn").read_bytes(), b"previous application")

    def test_second_replacement_failure_restores_both_executables(self):
        self.old_pair()
        # Remove the staged companion to force a real mv failure after the
        # application replacement succeeds. The installer's EXIT handler must
        # restore both files, not just the one whose replacement failed.
        self.run_shell('trap cleanup EXIT\nstage_pair artifact\n'
                       'rm "$STAGE_DIR/monty_worker"\ncommit_pair', expected=1)
        self.assertEqual((self.destination / "brassclaw-reborn").read_bytes(), b"previous application")
        self.assertEqual((self.destination / "monty_worker").read_bytes(), b"previous worker")
        self.assertEqual((self.destination / "brassclaw-reborn.bak").read_bytes(), b"previous application")
        self.assertEqual((self.destination / "monty_worker.bak").read_bytes(), b"previous worker")

    def test_lock_conflict_leaves_existing_files_alone(self):
        self.old_pair()
        lock = self.destination / ".brassclaw-install.lock"
        lock.mkdir()
        self.run_shell('trap cleanup EXIT\nstage_pair artifact', expected=1)
        self.assertTrue(lock.is_dir())
        self.assertEqual((self.destination / "monty_worker").read_bytes(), b"previous worker")

    def test_legacy_service_alias_and_backup(self):
        self.old_pair()
        legacy = self.destination / "brassclaw"
        legacy.write_bytes(b"legacy executable")
        self.run_shell('trap cleanup EXIT\nstage_pair artifact\ncommit_pair')
        self.assertTrue(legacy.is_symlink())
        self.assertEqual(legacy.resolve(), (self.destination / "brassclaw-reborn").resolve())
        self.assertEqual((self.destination / "brassclaw.bak").read_bytes(), b"legacy executable")

    def test_listener_address_and_version_handling(self):
        result = self.run_shell('loopback_url "http://0.0.0.0:4000"\nprintf "\\n"\n'
                                'loopback_url "http://[::]:5000"\nprintf "\\n"\n'
                                'parse_args -v v1.7.0-rc.4 --startup-timeout 30\n'
                                'printf "%s %s\\n" "$PINNED_VERSION" "$STARTUP_TIMEOUT"')
        self.assertEqual(result.stdout.splitlines(),
                         ["http://127.0.0.1:4000", "http://[::1]:5000", "1.7.0-rc.4 30"])
        self.run_shell('loopback_url "https://example.com"', expected=1)
        self.run_shell('parse_args -v ../../other', expected=1)

    def test_fresh_service_uses_policy_compatible_loopback_listener(self):
        unit = self.run_shell('render_systemd_unit brassclaw /var/lib/brassclaw/.brassclaw/reborn /var/lib/brassclaw')
        self.assertIn('serve --host 127.0.0.1 --port 3000', unit.stdout)
        self.assertIn('Environment=BRASSCLAW_RUNTIME_PROFILE=local_dev', unit.stdout)

    @unittest.skipUnless(shutil.which("systemd-analyze"), "systemd unit validator is unavailable")
    def test_generated_unit_passes_real_systemd_validation(self):
        self.run_shell('trap cleanup EXIT\nstage_pair artifact\ncommit_pair\n'
                       'render_systemd_unit "$(id -un)" "$INSTALL_DIR" "$INSTALL_DIR" '
                       '> "$INSTALL_DIR/brassclaw-installer-check.service"\n'
                       'systemd-analyze verify "$INSTALL_DIR/brassclaw-installer-check.service"')


if __name__ == "__main__":
    unittest.main()
