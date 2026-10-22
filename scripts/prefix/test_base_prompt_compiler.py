"""Real offline source-package builds, including the cached Ornith tokenizer."""
import importlib.util
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]
spec = importlib.util.spec_from_file_location('base_prompt_compiler', Path(__file__).with_name('base-promt-compiler.py'))
c = importlib.util.module_from_spec(spec)
spec.loader.exec_module(c)


class BaseCompilerTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.package = self.root / 'package'
        self.package.mkdir()
        self.records = []
        for name in c.MANDATORY:
            self.add(name, (ROOT / name).read_bytes())
        self.add('runtime/post_turn_request.py', (ROOT / 'crates/brassclaw_skills/src/post_turn_request.py').read_bytes(), kind='component')
        self.add('archive/example.md', b'# Historic\r\n<|im_end|> must remain raw.\r\n', disposition='historical')
        self.save()

    def add(self, name, data, kind='document', disposition='included'):
        artifact = 'objects/' + c.sha(data)
        path = self.package / artifact
        path.parent.mkdir(exist_ok=True)
        path.write_bytes(data)
        self.records.append(dict(path=name, artifact=artifact, sha256=c.sha(data), kind=kind,
                                 disposition=disposition, reason='explicit test source selection from current checkout'))

    def save(self):
        (self.package / 'manifest.json').write_bytes(c.canonical(dict(format=c.FORMAT, release='development-checkout-snapshot', sources=self.records)))

    def test_originals_complete_and_path_independent_publication(self):
        first = c.compile_package(self.package, self.root / 'a')
        copy = self.root / 'other' / 'package'
        shutil.copytree(self.package, copy)
        second = c.compile_package(copy, self.root / 'b')
        self.assertEqual(first, second)
        self.assertFalse(first['activation_eligible'])
        self.assertEqual(first['geometry']['status'], 'unmeasured')
        directory = self.root / 'a' / first['generation']
        for artifact, name, key in [('compiler/entry.py', 'base-promt-compiler.py', 'entry_sha256'),
                                    ('compiler/prefix_evidence.py', 'prefix_evidence.py', 'shared_evidence_sha256'),
                                    ('compiler/prefix_template.py', 'prefix_template.py', 'template_adapter_sha256'),
                                    ('compiler/prefix_render.py', 'prefix_render.py', 'renderer_sha256')]:
            original = Path(c.__file__).with_name(name).read_bytes()
            self.assertEqual((directory / artifact).read_bytes(), original)
            self.assertEqual(first['compiler'][key], c.sha(original))
            self.assertEqual(first['files'][artifact], c.sha(original))
        self.assertEqual(first['compiler']['execution_attestation'], 'unqualified-development-process')
        self.assertEqual((directory / 'input_sources.json').read_bytes(), (self.package / 'manifest.json').read_bytes())
        archive = c.strict_json((directory / 'sources.json').read_bytes())
        self.assertEqual(archive['format'], 'brassclaw-prefix-generation-sources/1')
        self.assertEqual(archive['input_artifact'], 'input_sources.json')
        for record in archive['sources']:
            self.assertEqual(record['artifact'], 'raw/' + record['sha256'])
            self.assertEqual(c.sha((directory / record['artifact']).read_bytes()), record['sha256'])
        evidence = c.strict_json((directory / 'evidence.json').read_bytes())
        for record in self.records:
            data = (self.package / record['artifact']).read_bytes()
            self.assertEqual((directory / 'raw' / record['sha256']).read_bytes(), data)
            if record['disposition'] == 'included':
                reconstructed = ''.join(e['excerpt'] for e in evidence if e['path'] == record['path']).encode()
                self.assertEqual(reconstructed, data)
        self.assertNotIn('<|im_end|>', (directory / 'context.md').read_text())
        self.assertEqual(c.compile_package(self.package, self.root / 'a'), first)
        (directory / 'context.md').write_text('tampered')
        with self.assertRaisesRegex(ValueError, 'immutable generation differs'):
            c.compile_package(self.package, self.root / 'a')

    def test_input_manifest_change_during_render_never_publishes_or_changes_last_good(self):
        from unittest.mock import patch
        first = c.compile_package(self.package, self.root / 'stable')
        directory = self.root / 'stable' / first['generation']
        before = {name: (directory / name).read_bytes() for name in first['files']}
        entries = {path.name for path in (self.root / 'stable').iterdir()}
        manifest = self.package / 'manifest.json'
        original = manifest.read_bytes()
        render = c.render_prefix_reference
        def changed(*args, **kwargs):
            manifest.write_bytes(original + b'\n')
            return render(*args, **kwargs)
        try:
            with patch.object(c, 'render_prefix_reference', side_effect=changed):
                with self.assertRaisesRegex(ValueError, 'manifest changed'):
                    c.compile_package(self.package, self.root / 'stable')
        finally:
            manifest.write_bytes(original)
        self.assertEqual({name: (directory / name).read_bytes() for name in first['files']}, before)
        self.assertEqual({path.name for path in (self.root / 'stable').iterdir()}, entries)

    def test_missing_changed_duplicate_and_escaping_sources_rejected(self):
        self.records[0]['artifact'] = '../outside'
        self.save()
        with self.assertRaisesRegex(ValueError, 'relative path'):
            c.compile_package(self.package, self.root / 'a')
        self.records.pop(0)
        self.save()
        with self.assertRaisesRegex(ValueError, 'missing mandatory guides'):
            c.compile_package(self.package, self.root / 'a')
        self.records.append(self.records[0])
        self.save()
        with self.assertRaisesRegex(ValueError, 'duplicate logical source'):
            c.compile_package(self.package, self.root / 'a')
        self.records.pop()
        path = self.package / self.records[0]['artifact']
        path.write_bytes(path.read_bytes() + b'\nchanged')
        with self.assertRaisesRegex(ValueError, 'checksum differs'):
            c.compile_package(self.package, self.root / 'a')
        with self.assertRaisesRegex(ValueError, 'duplicate JSON key'):
            c.strict_json('{"release":"a","release":"b"}')

    def test_exact_unit_deduplication_preserves_all_original_ranges(self):
        from prefix_render import render_deduplicated_prefix_reference
        common = '# Shared exact contract\n' + ('Complete common qualification.\n' * 100)
        self.add('alpha/reference.md', (common + '# Unique A\nKeep A qualifications.\n').encode())
        self.add('beta/reference.md', (common + '# Unique B\nKeep B qualifications.\n').encode())
        self.save()
        original = c.compile_package(self.package, self.root / 'original')
        first = c.compile_package(self.package, self.root / 'dedup', evidence_view='literal-reference/v2-deduplicated')
        second = c.compile_package(self.package, self.root / 'other', evidence_view='literal-reference/v2-deduplicated')
        self.assertEqual(first, second)
        self.assertNotEqual(first['generation'], original['generation'])
        directory = self.root / 'dedup' / first['generation']
        evidence = {card['path']: card for card in c.strict_json((directory / 'evidence.json').read_bytes())}
        mapping = c.strict_json((directory / 'render_map.json').read_bytes())
        units = {unit['unit_id']: unit for unit in mapping['units']}
        for source in mapping['sources']:
            original_bytes = evidence[source['source']]['excerpt'].encode()
            restored = []
            for part in source['parts']:
                canonical = units[part['unit_id']]['canonical']
                lines = evidence[canonical['source']]['excerpt'].splitlines(keepends=True)
                text = ''.join(lines[canonical['line_start'] - 1:canonical['line_end']])
                self.assertEqual(c.sha(text.encode()), part['unit_id'])
                restored.append(text)
            self.assertEqual(''.join(restored).encode(), original_bytes)
        text = (directory / 'context.md').read_text()
        self.assertEqual(text.count(common), 1)
        self.assertIn('Keep A qualifications.', text)
        self.assertIn('Keep B qualifications.', text)
        self.assertLess(len(text.encode()), (self.root / 'original' / original['generation'] / 'context.md').stat().st_size)
        with self.assertRaisesRegex(ValueError, 'partition'):
            render_deduplicated_prefix_reference('header', [{'path':'wrong','disposition':'included','reason':'test','text':'missing','units':[]}], [])

    def test_trailing_inventory_keeps_metadata_changes_after_complete_reference(self):
        view = 'literal-reference/v3-trailing-inventory'
        def build(name, selected):
            result = c.compile_package(self.package, self.root / name, evidence_view=selected)
            directory = self.root / name / result['generation']
            return result, directory, (directory / 'context.md').read_bytes()
        first, directory, before = build('before', view)
        _, _, old_before = build('old-before', 'literal-reference/v2-deduplicated')
        self.records[-1]['reason'] = 'Re-reviewed historical source; original remains excluded.'
        self.save()
        second, after_directory, after = build('after', view)
        _, _, old_after = build('old-after', 'literal-reference/v2-deduplicated')
        boundary = before.index(b'# Source inventory\n')
        self.assertGreater(boundary, before.rindex(b'<!-- END-EVIDENCE-CARD -->'))
        self.assertEqual(before[:boundary], after[:boundary])
        self.assertNotEqual(old_before.split(b'# Complete current references')[0], old_after.split(b'# Complete current references')[0])
        self.assertNotEqual(first['generation'], second['generation'])
        self.assertNotEqual(before[boundary:], after[boundary:])
        self.assertEqual((directory / 'evidence.json').read_bytes(), (after_directory / 'evidence.json').read_bytes())
        self.assertEqual((directory / 'render_map.json').read_bytes(), (after_directory / 'render_map.json').read_bytes())
        for source in self.records:
            raw = 'raw/' + source['sha256']
            self.assertEqual((after_directory / raw).read_bytes(), (self.package / source['artifact']).read_bytes())

    def test_stable_sharing_does_not_relabel_existing_units_for_appended_duplicate(self):
        def build(name, view):
            receipt = c.compile_package(self.package, self.root / name, evidence_view=view)
            directory = self.root / name / receipt['generation']
            return directory, (directory / 'context.md').read_bytes()
        _, before = build('stable-before', 'literal-reference/v4-stable-sharing')
        _, prior_before = build('trailing-before', 'literal-reference/v3-trailing-inventory')
        self.add('zz/reused-guide.md', (ROOT / c.MANDATORY[0]).read_bytes())
        self.save()
        directory, after = build('stable-after', 'literal-reference/v4-stable-sharing')
        _, prior_after = build('trailing-after', 'literal-reference/v3-trailing-inventory')
        boundary = before.index(b'# Source inventory\n')
        self.assertEqual(before[:boundary], after[:boundary])
        self.assertNotEqual(prior_before[:prior_before.index(b'# Source inventory\n')], prior_after[:prior_before.index(b'# Source inventory\n')])
        mapping = c.strict_json((directory / 'render_map.json').read_bytes())
        cards = {card['path']: card for card in c.strict_json((directory / 'evidence.json').read_bytes())}
        units = {unit['unit_id']: unit for unit in mapping['units']}
        for source in mapping['sources']:
            recovered = []
            for part in source['parts']:
                origin = units[part['unit_id']]['canonical']
                original = cards[origin['source']]['excerpt'].splitlines(keepends=True)
                text = ''.join(original[origin['line_start'] - 1:origin['line_end']])
                self.assertEqual(c.sha(text.encode()), part['unit_id'])
                recovered.append(text)
            self.assertEqual(''.join(recovered), cards[source['source']]['excerpt'])
        self.assertEqual(cards['zz/reused-guide.md']['excerpt'], cards[c.MANDATORY[0]]['excerpt'])

    def test_contextual_sharing_keeps_equal_bytes_in_different_contexts_separate(self):
        common = 'Same complete reference bytes in two distinct source contexts.\n'
        self.add('fixture/shared-a.md', common.encode())
        self.add('fixture/shared-b.md', common.encode())
        self.add('scripts/prefix/monty-reference/v1.0.0/docs/api/python/fixture.md', common.encode())
        self.save()
        result = c.compile_package(self.package, self.root / 'contextual', evidence_view='literal-reference/v5-contextual-sharing')
        directory = self.root / 'contextual' / result['generation']
        rendered = (directory / 'context.md').read_text()
        self.assertEqual(rendered.count(common), 2)
        self.assertIn('Host embedding APIs are not guest functions or BrassClaw host callables', rendered)
        mapping = c.strict_json((directory / 'render_map.json').read_bytes())
        design = c.strict_json((directory / 'profile_design.json').read_bytes())
        self.assertEqual(mapping['format'], 'brassclaw-prefix-render-map/2')
        self.assertEqual(design['format'], 'brassclaw-prefix-compiler-design/2')
        groups = [unit for unit in mapping['units'] if unit['unit_id'] == c.sha(common.encode())]
        self.assertEqual({unit['context'] for unit in groups}, {'reference', 'upstream-api'})
        cards = {card['path']: card for card in c.strict_json((directory / 'evidence.json').read_bytes())}
        units = {(unit['context'], unit['unit_id']): unit for unit in mapping['units']}
        for source in mapping['sources']:
            self.assertEqual(design['source_contexts'][source['source']], source['context'])
            recovered = []
            for part in source['parts']:
                origin = units[(source['context'], part['unit_id'])]['canonical']
                lines = cards[origin['source']]['excerpt'].splitlines(keepends=True)
                recovered.append(''.join(lines[origin['line_start'] - 1:origin['line_end']]))
            self.assertEqual(''.join(recovered), cards[source['source']]['excerpt'])

    def test_snapshot_uses_explicit_inventory_and_preserves_original_bytes(self):
        inventory = self.root / 'inventory.json'
        inventory.write_bytes(c.canonical({'format': 'brassclaw-prefix-source-inventory/1',
                                          'release': 'current-development-documents',
                                          'sources': [{'path': name, 'kind': 'document', 'disposition': 'included',
                                                       'reason': 'complete mandatory guide'} for name in c.MANDATORY]}))
        result = c.prepare_package(ROOT, inventory, self.root / 'snapshots')
        self.assertEqual(c.prepare_package(ROOT, inventory, self.root / 'snapshots'), result)
        manifest, records = c.load_package(self.root / 'snapshots' / result['source_package'])
        self.assertEqual(len(records), len(c.MANDATORY))
        for record, data, _text in records:
            self.assertEqual(data, (ROOT / record['path']).read_bytes())
        self.assertNotIn(str(ROOT), c.canonical(manifest).decode())

    @unittest.skipUnless(os.getenv('BRASSCLAW_TEST_CATALOGUE_EXPORT'), 'actual authenticated native export required')
    def test_actual_catalogue_export_preserves_sources_and_rejects_transfer_corruption(self):
        export_path = Path(os.environ['BRASSCLAW_TEST_CATALOGUE_EXPORT'])
        export = c.strict_json(export_path.read_bytes())
        inventory = self.root / 'inventory.json'
        inventory.write_bytes(c.canonical({'format': 'brassclaw-prefix-source-inventory/1',
                                          'release': 'native-export-development-documents',
                                          'sources': [{'path': name, 'kind': 'document', 'disposition': 'included',
                                                       'reason': 'complete mandatory guide'} for name in c.MANDATORY]}))
        result = c.prepare_package(ROOT, inventory, self.root / 'snapshots', catalogue_export=export_path)
        package = self.root / 'snapshots' / result['source_package']
        manifest, records = c.load_package(package)
        bodies = {record['path']: data for record, data, _ in records}
        for component in export['components']:
            ref = component['reference']
            self.assertEqual(bodies[f"catalogue/components/{ref['uuid']}/v{ref['version']}.json"], component['revision_bytes'].encode())
        self.assertEqual(bodies[f"catalogue/provenance/{export['generation']}.json"], export_path.read_bytes())
        built = c.compile_package(package, self.root / 'built')
        context = (self.root / 'built' / built['generation'] / 'context.md').read_text()
        self.assertFalse(built['activation_eligible'])
        self.assertNotIn('complete transfer evidence retained raw', context.split('# Complete current references')[1])
        corrupt = self.root / 'corrupt.json'
        altered = json.loads(json.dumps(export))
        altered['components'][0]['revision_bytes'] += ' '
        corrupt.write_bytes(c.canonical(altered))
        with self.assertRaisesRegex(ValueError, 'source checksum differs'):
            c.prepare_package(ROOT, inventory, self.root / 'rejected', catalogue_export=corrupt)
        self.assertFalse((self.root / 'rejected').exists())
        altered = json.loads(json.dumps(export))
        altered['components'].pop()
        corrupt.write_bytes(c.canonical(altered))
        with self.assertRaisesRegex(ValueError, 'missing|conflicting'):
            c.catalogue_records(corrupt)
        altered = json.loads(json.dumps(export))
        altered['components'].append(altered['components'][0])
        corrupt.write_bytes(c.canonical(altered))
        with self.assertRaisesRegex(ValueError, 'duplicate component (UUID|revision)'):
            c.catalogue_records(corrupt)
        altered = json.loads(json.dumps(export))
        altered['catalogue_bytes'] += ' '
        corrupt.write_bytes(c.canonical(altered))
        with self.assertRaisesRegex(ValueError, 'catalogue checksum differs'):
            c.catalogue_records(corrupt)
        altered = json.loads(json.dumps(export))
        altered['selections'][0] += ' '
        # A structurally identical selection with distinct bytes is still pinned
        # evidence: source identity changes, without manufacturing approval.
        corrupt.write_bytes(c.canonical(altered))
        self.assertNotEqual(c.catalogue_records(corrupt), c.catalogue_records(export_path))

    @unittest.skipUnless(os.getenv('BRASSCLAW_TEST_CATALOGUE_EXPORT'), 'actual source transfer fixture required')
    def test_versioned_transfer_preserves_revisions_without_approving_new_combinations(self):
        # Parser-only mutation of actual transfer data. This does not manufacture
        # approval, an activated catalogue or an execution result.
        export = c.strict_json(Path(os.environ['BRASSCLAW_TEST_CATALOGUE_EXPORT']).read_bytes())
        export['format'] = 'brassclaw-catalogue-source-export/2'
        export['coverage'] = 'active-ordinary-plus-source-compiler-and-measurement-with-retained-root'
        selections = [c.strict_json(source) for source in export['selections']]
        counts = {}
        for selection in selections:
            for ref in selection['components']:
                counts[ref['uuid']] = counts.get(ref['uuid'], 0) + 1
        original = next(entry for entry in export['components']
                        if counts.get(entry['reference']['uuid'], 0) > 1
                        and entry['reference']['class_code'] == 0
                        and not c.strict_json(entry['revision_bytes'])['dependencies'])
        successor = json.loads(json.dumps(original))
        document = c.strict_json(successor['revision_bytes'])
        document['document']['transfer_fixture_note'] = 'new unapproved metadata, parser evidence only'
        successor['revision_bytes'] = c.canonical(document).decode()
        successor['reference']['version'] = 1 + max(entry['reference']['version'] for entry in export['components'] if entry['reference']['uuid'] == original['reference']['uuid'])
        successor['reference']['checksum'] = c.sha(successor['revision_bytes'].encode())
        export['components'].append(successor)
        selected = next(selection for selection in selections
                        if original['reference'] in selection['components'])
        selected['components'] = [successor['reference'] if ref == original['reference'] else ref
                                  for ref in selected['components']]
        export['selections'] = [c.canonical(selection).decode() for selection in selections]
        path = self.root / 'versioned-export.json'
        path.write_bytes(c.canonical(export))
        records = {name: body for name, body, *_ in c.catalogue_records(path)}
        for entry in [original, successor]:
            ref = entry['reference']
            self.assertEqual(records[f"catalogue/components/{ref['uuid']}/v{ref['version']}.json"],
                             entry['revision_bytes'].encode())
        altered = json.loads(json.dumps(export))
        altered['components'].append(successor)
        path.write_bytes(c.canonical(altered))
        with self.assertRaisesRegex(ValueError, 'duplicate component revision'):
            c.catalogue_records(path)
        altered = json.loads(json.dumps(export))
        target = next(i for i, source in enumerate(altered['selections'])
                      if successor['reference'] in c.strict_json(source)['components'])
        mixed = c.strict_json(altered['selections'][target])
        mixed['components'].append(original['reference'])
        altered['selections'][target] = c.canonical(mixed).decode()
        path.write_bytes(c.canonical(altered))
        with self.assertRaisesRegex(ValueError, 'selected component source missing or conflicting'):
            c.catalogue_records(path)
        # The old monoversion contract still fails closed on this transfer.
        altered = json.loads(json.dumps(export))
        altered['format'] = 'brassclaw-catalogue-source-export/1'
        altered['coverage'] = 'active-ordinary-workflows-and-retained-root'
        path.write_bytes(c.canonical(altered))
        with self.assertRaisesRegex(ValueError, 'duplicate component UUID'):
            c.catalogue_records(path)

    @unittest.skipUnless(os.getenv('ORNITH_TEST_TOKENIZER'), 'actual local tokenizer required')
    def test_actual_cli_token_geometry_capacity_and_no_publication_on_failure(self):
        args = [sys.executable, str(Path(c.__file__)), '--package', str(self.package), '--output', str(self.root / 'target'),
                '--tokenizer-dir', os.environ['ORNITH_TEST_TOKENIZER'], '--max-model-len', '131072']
        env = {**os.environ, 'HF_HUB_OFFLINE': '1', 'TRANSFORMERS_OFFLINE': '1'}
        result = subprocess.run(args, env=env, capture_output=True, text=True)
        self.assertEqual(result.returncode, 0, result.stderr)
        receipt = json.loads(result.stdout)
        self.assertGreater(receipt['geometry']['shared_prefix_tokens'], 1000)
        self.assertLessEqual(receipt['geometry']['rendered_probe_tokens'] + receipt['geometry']['reserve_tokens'], 131072)
        print('Actual Ornith geometry:', receipt['geometry']['rendered_probe_tokens'], 'rendered tokens;', receipt['geometry']['reserve_tokens'], 'workspace reserve', flush=True)
        self.assertEqual(receipt['geometry']['status'], 'local-tokenizer-validated')
        self.assertFalse(receipt['activation_eligible'])
        repeated = subprocess.run(args, env=env, capture_output=True, text=True)
        self.assertEqual(repeated.returncode, 0, repeated.stderr)
        self.assertEqual(receipt, json.loads(repeated.stdout))
        before = sorted(path.name for path in (self.root / 'target').iterdir())
        args[-1] = '32000'
        failed = subprocess.run(args, env=env, capture_output=True, text=True)
        self.assertNotEqual(failed.returncode, 0)
        self.assertIn('no sources omitted', failed.stderr)
        self.assertEqual(before, sorted(path.name for path in (self.root / 'target').iterdir()))


if __name__ == '__main__':
    unittest.main()
