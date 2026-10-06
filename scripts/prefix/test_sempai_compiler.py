"""Real local source/evidence checks; no provider or tokenizer required."""
import importlib.util
import json
from pathlib import Path
import shutil
import tempfile
import unittest
from unittest.mock import patch


class SempaiCompilerTests(unittest.TestCase):
    def test_complete_sources_and_failed_recollection_preserve_evidence(self):
        path = Path(__file__).with_name('sempai-compiler.py')
        spec = importlib.util.spec_from_file_location('sempai_compiler_test', path)
        compiler = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(compiler)
        checkout = path.resolve().parents[2]
        with tempfile.TemporaryDirectory() as directory:
            temporary = Path(directory)
            compiler.SOURCE_ROOT = (temporary / 'sources').resolve()
            for relative in compiler.SOURCE_DOCUMENTS:
                target = compiler.SOURCE_ROOT / relative
                target.parent.mkdir(parents=True, exist_ok=True)
                shutil.copyfile(checkout / relative, target)
            compiler.BASE = temporary / 'cache'
            compiler.RAW = compiler.BASE / 'raw'
            compiler.DOCUMENTS_JSONL = compiler.RAW / 'documents.jsonl'
            compiler.DB_PATH = compiler.BASE / 'knowledge.db'
            compiler.FINAL = compiler.BASE / 'final_knowledge.md'
            with patch.dict('os.environ', {'SEMPAI_SOURCE_MANIFEST': ''}):
                compiler.collect()
                for lane in ('ACTION', 'REASONING', 'THINKING'):
                    compiler.distill_lane(lane)
                cards = compiler.accepted_cards()
                selected = compiler.required_reference_cards(cards)
                for relative in compiler.SOURCE_DOCUMENTS:
                    excerpts = sorted((c for c in selected if c['path'] == relative),
                                      key=lambda c: c['line_start'])
                    original = (checkout / relative).read_bytes().decode('utf-8')
                    self.assertEqual(''.join(c['excerpt'] for c in excerpts), original)
                    self.assertTrue((compiler.RAW / 'documents' /
                                     (compiler.digest(original) + '.txt')).is_file())
                # Missing even one source unit must fail complete-coverage validation.
                with self.assertRaisesRegex(ValueError, 'omits or adds'):
                    compiler.required_reference_cards(cards[1:])
                historical_policy = json.loads(path.with_name('sempai-reference-policy.json').read_text())
                selection_path = temporary / 'selection.json'
                selection_path.write_text(json.dumps(historical_policy))
                # Published policy is historical evidence, never rewritten to
                # make current source drift look approved.
                historical_units = {
                    (row['path'], row['line_start'], row['line_end']):
                        (row['document_sha256'], row['excerpt_sha256'])
                    for row in historical_policy['units']
                }
                current_units = {
                    (card['path'], card['line_start'], card['line_end']):
                        (card['document_sha256'], card['excerpt_sha256'])
                    for card in cards
                }
                if historical_units != current_units:
                    with patch.dict('os.environ', {'SEMPAI_REFERENCE_POLICY': str(selection_path)}):
                        with self.assertRaisesRegex(ValueError, 'drifted|every original source unit'):
                            compiler.required_reference_cards(cards)
                # Test-only policy for the actual current sources; this creates
                # no publication, review approval or production policy update.
                policy = {'schema': 1, 'status': 'reviewed-source-selection', 'units': [
                    {key: card[key] for key in ('path', 'line_start', 'line_end',
                                               'document_sha256', 'excerpt_sha256')}
                    | {'selected': True}
                    for card in cards
                ]}
                selection_path.write_text(json.dumps(policy))
                with patch.dict('os.environ', {'SEMPAI_REFERENCE_POLICY': str(selection_path)}):
                    selected = compiler.required_reference_cards(cards)
                    self.assertEqual(len(selected), len(cards))
                    for relative in ('recipe.md', 'skills.md', 'tools.md', 'toolskills.md'):
                        self.assertEqual(''.join(c['excerpt'] for c in selected if c['path'] == relative),
                                         (checkout / relative).read_bytes().decode('utf-8'))
                    # Even an intentional policy edit cannot remove a binding unit.
                    binding_unit = next(row for row in policy['units'] if row['path'] == 'recipe.md')
                    binding_unit['selected'] = False
                    selection_path.write_text(json.dumps(policy))
                    with self.assertRaisesRegex(ValueError, 'cannot omit'):
                        compiler.required_reference_cards(cards)
                    binding_unit['selected'] = True
                    binding_unit['document_sha256'] = '0' * 64
                    selection_path.write_text(json.dumps(policy))
                    with self.assertRaisesRegex(ValueError, 'fingerprints drifted'):
                        compiler.required_reference_cards(cards)
                inventory = (compiler.RAW / 'source_manifest.json').read_bytes()
                (compiler.SOURCE_ROOT / 'recipe.md').unlink()
                with self.assertRaisesRegex(ValueError, 'Required local source missing'):
                    compiler.collect()
                self.assertEqual((compiler.RAW / 'source_manifest.json').read_bytes(), inventory)
                self.assertEqual(compiler.accepted_cards(), cards)
                connection = compiler.db()
                compiler.meta_set(connection, 'profile_id', 'homeassistant')
                connection.close()
                with self.assertRaisesRegex(ValueError, 'separate Sempai cache'):
                    compiler.collect()
                self.assertEqual(compiler.accepted_cards(), cards)
                connection = compiler.db()
                compiler.meta_set(connection, 'profile_id', 'sempai')
                connection.close()
                shutil.copyfile(checkout / 'recipe.md', compiler.SOURCE_ROOT / 'recipe.md')
                # A mismatched pinned package also fails before any DB invalidation.
                pinned = temporary / 'pinned.json'
                manifest = json.loads(inventory)
                manifest['documents']['recipe.md'] = '0' * 64
                pinned.write_text(json.dumps(manifest))
                with patch.dict('os.environ', {'SEMPAI_SOURCE_MANIFEST': str(pinned)}):
                    with self.assertRaisesRegex(ValueError, 'differs from pinned manifest'):
                        compiler.collect()
                self.assertEqual(compiler.accepted_cards(), cards)


if __name__ == '__main__':
    unittest.main()
