"""Independent digest inputs and original-byte checks for Recipe-owned stages."""
import base64
import hashlib
import importlib.util
import json
from pathlib import Path
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]
HERE = Path(__file__).parent
namespace = {}
for name in ['prefix_units.py', 'prefix_render.py', 'prefix_compile_logic.py']:
    exec(compile((HERE / name).read_text(), str(HERE / name), 'exec'), namespace)
spec = importlib.util.spec_from_file_location('offline_core', HERE / 'base-promt-compiler.py')
core = importlib.util.module_from_spec(spec)
spec.loader.exec_module(core)


def snapshot(profile='base-prompt', view='literal-reference/v1'):
    paths = list(core.MANDATORY)
    header = core.HEADER
    if profile == 'prefix-maker':
        spec = importlib.util.spec_from_file_location('maker_core', HERE / 'prefix-maker-compiler.py')
        maker = importlib.util.module_from_spec(spec); spec.loader.exec_module(maker)
        paths = list(maker.REQUIRED); header = maker.HEADER
    return {'format': 'brassclaw-prefix-source-snapshot/1', 'profile': profile,
        'release': 'actual-current-source-stage-check', 'header': header, 'evidence_view': view,
        'sources': [{'path': name, 'kind': 'document', 'disposition': 'included',
                     'reason': 'complete current packaged original', 'text': (ROOT / name).read_bytes().decode('utf-8')} for name in paths],
        'procedures': {'compiler/entry.py': (HERE / 'prefix_compile_logic.py').read_text(),
            'compiler/prefix_render.py': (HERE / 'prefix_render.py').read_text(),
            'compiler/prefix_units.py': (HERE / 'prefix_units.py').read_text()}}


def run_stages(source):
    stage = namespace['prefix_prepare']({'snapshot_bytes': namespace['prefix_json'](source)})
    trace = []
    for name in ['prefix_reference', 'prefix_render', 'prefix_identity', 'prefix_finalize']:
        trace.append(stage)
        digests = [hashlib.sha256(text.encode()).hexdigest() for text in stage['texts']]
        stage = namespace[name]({'state_bytes': stage['state_bytes'], 'digests': digests})
    return stage, trace


class CompileLogicTests(unittest.TestCase):
    def test_actual_profiles_and_views_preserve_every_original_and_match_shared_renderer(self):
        for profile in ['base-prompt', 'prefix-maker']:
            for view in ['literal-reference/v1', 'literal-reference/v2-deduplicated', 'literal-reference/v3-trailing-inventory', 'literal-reference/v4-stable-sharing']:
                with self.subTest(profile=profile, view=view):
                    source = snapshot(profile, view)
                    compiled, _ = run_stages(source)
                    self.assertEqual(compiled, run_stages(source)[0])
                    package = json.loads(compiled)
                    files = {name: base64.b64decode(value['data'], validate=True) for name, value in package['artifacts'].items()}
                    self.assertEqual(files['source_snapshot.json'].decode(), namespace['prefix_json'](source))
                    archive = json.loads(files['sources.json'])
                    self.assertEqual(archive['format'], 'brassclaw-prefix-generation-sources/1')
                    self.assertEqual(archive['input_artifact'], 'source_snapshot.json')
                    for record in archive['sources']:
                        self.assertEqual(record['artifact'], 'raw/' + record['sha256'])
                        self.assertEqual(hashlib.sha256(files[record['artifact']]).hexdigest(), record['sha256'])
                    evidence = json.loads(files['evidence.json'])
                    for record in source['sources']:
                        original = record['text'].encode()
                        checksum = hashlib.sha256(original).hexdigest()
                        self.assertEqual(files['raw/' + checksum], original)
                        self.assertEqual(next(e['excerpt'] for e in evidence if e['path'] == record['path']).encode(), original)
                    rendering = [{**record, 'sha256': hashlib.sha256(record['text'].encode()).hexdigest(),
                        'evidence_id': next(e['id'] for e in evidence if e['path'] == record['path']),
                        'units': [{'line_start':a,'line_end':b,'text':text,'id':hashlib.sha256(text.encode()).hexdigest()} for a,b,text,_ in core.source_units(record['text'], '.'+record['path'].split('.')[-1])]} for record in source['sources']]
                    expected = core.render_prefix_reference(source['header'], rendering, []) if view == 'literal-reference/v1' else (core.render_trailing_inventory_prefix_reference if view == 'literal-reference/v3-trailing-inventory' else core.render_deduplicated_prefix_reference)(source['header'], rendering, [])['context']
                    if view == 'literal-reference/v4-stable-sharing':
                        expected = core.render_stable_sharing_prefix_reference(source['header'], rendering, [])['context']
                    self.assertEqual(files['context.md'].decode(), expected)
                    manifest = json.loads(files['manifest.json'])
                    self.assertFalse(manifest['activation_eligible'])
                    self.assertEqual(manifest['geometry']['status'], 'unmeasured')
                    identity = {key:manifest[key] for key in ['profile','compiler','files','geometry']}
                    self.assertEqual(manifest['generation'],hashlib.sha256(core.canonical(identity)).hexdigest())

    def test_missing_and_unsafe_sources_and_bad_digest_batches_stop(self):
        value = snapshot()
        value['sources'].pop()
        with self.assertRaisesRegex(ValueError,'mandatory'):
            namespace['prefix_prepare']({'snapshot_bytes': namespace['prefix_json'](value)})
        value = snapshot(); value['sources'][0]['path'] = '../private'
        with self.assertRaisesRegex(ValueError,'source path'):
            namespace['prefix_prepare']({'snapshot_bytes': namespace['prefix_json'](value)})
        _, stages = run_stages(snapshot())
        with self.assertRaisesRegex(ValueError,'digest batch'):
            namespace['prefix_reference']({'state_bytes':stages[0]['state_bytes'],'digests':[]})
        with self.assertRaisesRegex(ValueError,'canonical'):
            namespace['prefix_prepare']({'snapshot_bytes':'{"profile":"base-prompt","profile":"prefix-maker"}'})


def native_fixture(profile, view):
    source = snapshot(profile, view)
    inputs = {'snapshot_bytes': namespace['prefix_json'](source)}
    cases = []
    for name in ['prefix_prepare', 'prefix_reference', 'prefix_render', 'prefix_identity', 'prefix_finalize']:
        result = namespace[name](inputs)
        cases.append({'function': name, 'inputs': inputs, 'expected': result})
        if isinstance(result, dict):
            inputs = {'state_bytes': result['state_bytes'],
                      'digests': [hashlib.sha256(text.encode()).hexdigest() for text in result['texts']]}
    return cases


if __name__ == '__main__':
    unittest.main()
