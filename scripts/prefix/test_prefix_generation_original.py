"""Whole-original projections from an actual shared-compiler package.

These tests do not replace the native reader's package/digest custody checks.
"""
import base64
import copy
import hashlib
import importlib.util
import json
from pathlib import Path
import unittest

HERE = Path(__file__).parent
namespace = {}
exec(compile((HERE / 'prefix_generation_original.py').read_text(), str(HERE / 'prefix_generation_original.py'), 'exec'), namespace)
project = namespace['project_prefix_generation_source']


def canonical(value):
    return json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(',', ':'))


def digest(value):
    return hashlib.sha256(value.encode('utf-8')).hexdigest()


def fixture():
    spec = importlib.util.spec_from_file_location('compiler_checks', HERE / 'test_prefix_compile_logic.py')
    compiler = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(compiler)
    source = compiler.snapshot('base-prompt', 'literal-reference/v5-contextual-sharing')
    text = '\ufeffÄ\r\n\'; host.forbidden() #\r\nComplete original.\r\n'
    reference = 'archive/exact-original.txt'
    source['sources'].append({'path': reference, 'text': text, 'kind': 'document',
                              'disposition': 'historical', 'reason': 'Preserved complete historical original'})
    package, _ = compiler.run_stages(source)
    generation = digest(package)
    inputs = {'profile': 'base-prompt', 'generation': generation, 'reference': reference,
              'detail': {'summary': {'profile': 'base-prompt', 'generation': generation,
                  'profile_revision': 1, 'kind': 'compiler-package', 'qualification': 'quarantined',
                  'created_at': '2026-10-25T00:00:00Z'}, 'generation_bytes': package}}
    return inputs, text


def artifact(package, name):
    return json.loads(base64.b64decode(package['artifacts'][name]['data']).decode('utf-8'))


def put(package, name, text):
    package['artifacts'][name] = {'encoding': 'base64', 'data': base64.b64encode(text.encode('utf-8')).decode('ascii')}


def replace_package(inputs, package):
    text = canonical(package)
    inputs['detail']['generation_bytes'] = text
    inputs['generation'] = digest(text)
    inputs['detail']['summary']['generation'] = inputs['generation']


def historical_fixture(inputs):
    """Rebuild the old indexed layout with complete manifest/hash relationships.

    This is a compatibility fixture derived from a real compiler package, not a
    claim that the currently shipped compiler still emits the old inventory.
    """
    value = copy.deepcopy(inputs)
    package = json.loads(value['detail']['generation_bytes'])
    index = artifact(package, 'sources.json')
    index['format'] = 'brassclaw-prefix-source-package/1'
    del index['input_artifact']
    for row in index['sources']:
        row['artifact'] = 'input-objects/' + row['sha256']
    put(package, 'sources.json', canonical(index))
    manifest = artifact(package, 'manifest.json')
    manifest['files']['sources.json'] = digest(canonical(index))
    identity = {key: manifest[key] for key in ['profile', 'compiler', 'files', 'geometry']}
    manifest['generation'] = digest(canonical(identity))
    put(package, 'manifest.json', canonical(manifest))
    replace_package(value, package)
    return value


class OriginalProjectionTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.inputs, cls.original = fixture()

    def test_whole_historical_original_preserves_utf8_bom_crlf_and_literal_code(self):
        result = project(copy.deepcopy(self.inputs))
        self.assertEqual(result['original_text'].encode('utf-8'), self.original.encode('utf-8'))
        self.assertEqual(result['original_sha256'], digest(self.original))
        self.assertEqual(result['disposition'], 'historical')
        self.assertEqual(result['generation_qualification'], 'quarantined')
        self.assertEqual(result['generation'], self.inputs['generation'])
        self.assertNotIn('availability', result)

    def test_historical_index_uses_raw_hash_without_following_input_artifact_names(self):
        value = historical_fixture(self.inputs)
        before = copy.deepcopy(value)
        result = project(value)
        expected = project(self.inputs)
        expected['generation'] = value['generation']
        self.assertEqual(result, expected)
        self.assertEqual(value, before)
        package = json.loads(value['detail']['generation_bytes'])
        index = artifact(package, 'sources.json')
        row = next(row for row in index['sources'] if row['path'] == value['reference'])
        self.assertNotIn(row['artifact'], package['artifacts'])
        for pointer in ['../../current-file.txt', 'raw/' + '0' * 64, None]:
            row['artifact'] = pointer
            put(package, 'sources.json', canonical(index))
            replace_package(value, package)
            with self.subTest(pointer=pointer):
                self.assertEqual(project(value)['original_text'], self.original)
        # Installation-baseline indexes omit input artifact and kind entirely.
        del row['artifact']; del row['kind']
        put(package, 'sources.json', canonical(index)); replace_package(value, package)
        self.assertEqual(project(value)['original_text'], self.original)
        # A missing original is still an error; an old input name cannot supply it.
        package['artifacts']['input-objects/' + row['sha256']] = package['artifacts'].pop('raw/' + row['sha256'])
        replace_package(value, package)
        with self.assertRaises(ValueError): project(value)

    def test_unknown_index_and_missing_review_metadata_are_not_reinterpreted(self):
        for change in ['unknown-format', 'missing-reason']:
            value = historical_fixture(self.inputs)
            package = json.loads(value['detail']['generation_bytes'])
            index = artifact(package, 'sources.json')
            if change == 'unknown-format': index['format'] = 'unrecognized-index/1'
            else:
                row = next(row for row in index['sources'] if row['path'] == value['reference'])
                del row['reason']
            put(package, 'sources.json', canonical(index)); replace_package(value, package)
            with self.subTest(change=change), self.assertRaises(ValueError): project(value)

    def test_wrong_identity_legacy_and_absent_reference_fail_without_fallback(self):
        for kind in ['profile', 'generation', 'legacy', 'reference', 'missing-detail', 'extra-input']:
            value = copy.deepcopy(self.inputs)
            if kind == 'profile': value['profile'] = 'sempai'
            if kind == 'generation': value['generation'] = '0' * 64
            if kind == 'legacy': value['detail']['summary']['kind'] = 'legacy-bundle'
            if kind == 'reference': value['reference'] = 'not-archived.txt'
            if kind == 'missing-detail': value['detail'] = None
            if kind == 'extra-input': value['availability'] = 'available'
            with self.subTest(kind=kind), self.assertRaises(ValueError): project(value)

    def test_missing_duplicate_or_wrong_raw_artifact_is_rejected(self):
        for kind in ['index', 'duplicate', 'pointer', 'raw', 'encoding', 'invalid-base64']:
            value = copy.deepcopy(self.inputs)
            package = json.loads(value['detail']['generation_bytes'])
            index = artifact(package, 'sources.json')
            row = next(row for row in index['sources'] if row['path'] == value['reference'])
            if kind == 'index': del package['artifacts']['sources.json']
            if kind == 'duplicate':
                index['sources'].append(copy.deepcopy(row)); put(package, 'sources.json', canonical(index))
            if kind == 'pointer':
                row['artifact'] = 'raw/../../not-a-source'; put(package, 'sources.json', canonical(index))
            if kind == 'raw': del package['artifacts'][row['artifact']]
            if kind == 'encoding': package['artifacts'][row['artifact']]['encoding'] = 'file-path'
            if kind == 'invalid-base64': package['artifacts'][row['artifact']]['data'] += '!'
            replace_package(value, package)
            with self.subTest(kind=kind), self.assertRaises(ValueError): project(value)

    def test_binary_original_is_not_silently_decoded_or_replaced(self):
        value = copy.deepcopy(self.inputs)
        package = json.loads(value['detail']['generation_bytes'])
        index = artifact(package, 'sources.json')
        row = next(row for row in index['sources'] if row['path'] == value['reference'])
        checksum = hashlib.sha256(b'\xff').hexdigest()
        row.update(sha256=checksum, artifact='raw/' + checksum)
        put(package, 'sources.json', canonical(index))
        package['artifacts'][row['artifact']] = {'encoding': 'base64', 'data': '/w=='}
        replace_package(value, package)
        with self.assertRaises(UnicodeError): project(value)

    def test_large_index_is_not_cut_before_requested_original(self):
        value = copy.deepcopy(self.inputs)
        package = json.loads(value['detail']['generation_bytes'])
        index = artifact(package, 'sources.json')
        row = next(row for row in index['sources'] if row['path'] == value['reference'])
        # Projection-only index stress fixture, not a native-store-valid package.
        index['sources'] = [{**row, 'path': 'archive/other-' + str(i) + '.txt'} for i in range(1001)] + [row]
        put(package, 'sources.json', canonical(index)); replace_package(value, package)
        self.assertEqual(project(value)['original_text'], self.original)


if __name__ == '__main__':
    import sys
    if sys.argv[1:] == ['--native-fixture']:
        value, _ = fixture()
        historical = historical_fixture(value)
        expected_historical = {**project(value), 'generation': historical['generation']}
        print(canonical({'inputs': value, 'expected': project(value),
                         'historical': {'inputs': historical, 'expected': expected_historical}}))
    else:
        unittest.main()
