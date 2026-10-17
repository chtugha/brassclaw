"""Procedure preservation checks; no provider, collection or deployment calls."""
from pathlib import Path
import shutil
import tempfile
import unittest

from prefix_procedure import snapshot_procedure, finish_procedure


class ProcedureTests(unittest.TestCase):
    def test_complete_bytes_are_path_independent_and_changes_fail(self):
        with tempfile.TemporaryDirectory() as tmp:
            first = Path(tmp) / 'first'
            first.mkdir()
            (first / 'entry.py').write_bytes('# Original\r\nvalue = "ü"\r\n'.encode('utf-8'))
            (first / 'prefix_evidence.py').write_bytes(b'# Original helper\r\n')
            other = Path(tmp) / 'other'
            shutil.copytree(first, other)
            snapshot = snapshot_procedure(first / 'entry.py')
            identity, provenance, payloads = finish_procedure(snapshot)
            self.assertEqual((identity, provenance, payloads), finish_procedure(snapshot_procedure(other / 'entry.py')))
            self.assertEqual(payloads['compiler/entry.py'].encode(), (first / 'entry.py').read_bytes())
            self.assertEqual(payloads['compiler/prefix_evidence.py'].encode(), (first / 'prefix_evidence.py').read_bytes())
            self.assertEqual(provenance['execution_attestation'], 'unqualified-development-process')
            (first / 'prefix_evidence.py').write_text('# Changed helper\n')
            with self.assertRaisesRegex(ValueError, 'changed during compilation'):
                finish_procedure(snapshot)
            self.assertNotEqual(identity, finish_procedure(snapshot_procedure(first / 'entry.py'))[0])

    def test_helper_references_cannot_escape_or_duplicate(self):
        entry = Path(__file__)
        for helpers in [('../prefix_evidence.py',), ('/tmp/code.py',), ('prefix_procedure.py',)]:
            with self.subTest(helpers=helpers), self.assertRaises(ValueError):
                snapshot_procedure(entry, helpers)


if __name__ == '__main__':
    unittest.main()
