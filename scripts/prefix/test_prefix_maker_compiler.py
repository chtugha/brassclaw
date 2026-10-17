"""Prefix-Maker reuses the complete offline compiler, never copied pipelines."""
import importlib.util
import json
from pathlib import Path
import shutil
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]
spec = importlib.util.spec_from_file_location('prefix_maker_compiler', Path(__file__).with_name('prefix-maker-compiler.py'))
p = importlib.util.module_from_spec(spec)
spec.loader.exec_module(p)

class PrefixMakerCompilerTests(unittest.TestCase):
    def test_complete_design_reference_and_path_independent_export(self):
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary)
            inventory = directory / 'inventory.json'
            inventory.write_bytes(p.core.canonical({'format': 'brassclaw-prefix-source-inventory/1', 'release': 'prefix-maker-development-check',
                'sources': [{'path': name, 'kind': 'document', 'disposition': 'included', 'reason': 'complete required original'} for name in p.REQUIRED]}))
            result = p.core.prepare_package(ROOT, inventory, directory / 'sources')
            package = directory / 'sources' / result['source_package']
            first = p.compile_package(package, directory / 'first')
            moved = directory / 'different' / 'package'
            shutil.copytree(package, moved)
            second = p.compile_package(moved, directory / 'second')
            self.assertEqual(first, second)
            self.assertEqual(first['profile'], 'prefix-maker')
            self.assertFalse(first['activation_eligible'])
            self.assertEqual(first['geometry']['status'], 'unmeasured')
            generated = directory / 'first' / first['generation']
            design = p.core.strict_json((generated / 'profile_design.json').read_bytes())
            self.assertEqual(design['required_sources'], list(p.REQUIRED))
            self.assertEqual(first['compiler']['profile_adapter_sha256'], p.core.sha(Path(p.__file__).read_bytes()))
            self.assertEqual((generated / 'compiler/profile_adapter.py').read_bytes(), Path(p.__file__).read_bytes())
            evidence = p.core.strict_json((generated / 'evidence.json').read_bytes())
            self.assertEqual({card['path'] for card in evidence}, set(p.REQUIRED))
            for card in evidence:
                self.assertEqual(card['excerpt'].encode(), (ROOT / card['path']).read_bytes())
            exported = p.core.export_generation(generated)
            self.assertEqual(p.core.strict_json(exported)['profile'], 'prefix-maker')
            self.assertEqual(exported, p.core.export_generation(directory / 'second' / second['generation']))
            manifest = p.core.strict_json((package / 'manifest.json').read_bytes())
            manifest['sources'] = [source for source in manifest['sources'] if source['path'] != p.REQUIRED[-1]]
            (package / 'manifest.json').write_bytes(p.core.canonical(manifest))
            with self.assertRaisesRegex(ValueError, 'missing mandatory guides'):
                p.compile_package(package, directory / 'rejected')
            self.assertFalse((directory / 'rejected').exists())

if __name__ == '__main__':
    unittest.main()
