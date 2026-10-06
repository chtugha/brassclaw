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
