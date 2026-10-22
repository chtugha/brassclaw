"""Exercise the actual offline byte protocol, not a declared worker success flag."""
import base64
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

import prefix_build_worker as worker

ROOT = Path(__file__).resolve().parents[2]


def artifact(data):
    return {'encoding': 'base64', 'sha256': hashlib.sha256(data).hexdigest(),
            'data': base64.b64encode(data).decode('ascii')}


def request(profile='base-prompt'):
    core = worker.load_core()
    names = list(core.MANDATORY)
    if profile == 'prefix-maker':
        names.append('scripts/prefix/prefix-maker-reference.md')
    sources, originals = [], {}
    for name in names:
        data = (ROOT / name).read_bytes()
        checksum = worker.digest(data)
        ref = 'objects/' + checksum
        originals[ref] = artifact(data)
        sources.append({'path': name, 'artifact': ref, 'sha256': checksum,
                        'kind': 'document', 'disposition': 'included',
                        'reason': 'complete current packaged guide; development protocol check'})
    return {'format': 'brassclaw-prefix-build-request/1', 'profile': profile,
            'source_manifest': {'format': core.FORMAT, 'release': 'current-development-byte-snapshot',
                                'sources': sources}, 'artifacts': originals,
            'design': {'evidence_view': 'literal-reference/v1', 'reserve_tokens': 8192},
            'target': None}


def large_source_request():
    value = request()
    size = sum(len(base64.b64decode(v['data'])) for v in value['artifacts'].values())
    # Real current implementation sources, never padding or fabricated guides.
    for path in sorted((ROOT / 'crates/brassclaw_reborn_composition/src').glob('*.rs')):
        data = path.read_bytes()
        name = str(path.relative_to(ROOT))
        checksum = worker.digest(data)
        ref = 'objects/' + checksum
        value['artifacts'][ref] = artifact(data)
        value['source_manifest']['sources'].append({'path': name, 'artifact': ref,
            'sha256': checksum, 'kind': 'document', 'disposition': 'included',
            'reason': 'literal implementation-source observation; not complete approved library coverage'})
        size += len(data)
        if size >= 2500000:
            break
    return value


class BuildWorkerTests(unittest.TestCase):
    def test_real_offline_process_is_deterministic_and_retains_its_procedure(self):
        with tempfile.TemporaryDirectory() as temporary:
            guard = Path(temporary) / 'sitecustomize.py'
            guard.write_text("import sys\ndef guard(event, args):\n    if event == 'socket.connect':\n        raise RuntimeError('network forbidden in compiler check')\nsys.addaudithook(guard)\n")
            env = dict(os.environ, PYTHONPATH=temporary, HF_HUB_OFFLINE='1', TRANSFORMERS_OFFLINE='1', HF_HUB_DISABLE_TELEMETRY='1')
            for view in ['literal-reference/v1', 'literal-reference/v3-trailing-inventory', 'literal-reference/v4-stable-sharing', 'literal-reference/v5-contextual-sharing']:
                for profile in ['base-prompt', 'prefix-maker']:
                    value = request(profile)
                    value['design']['evidence_view'] = view
                    data = json.dumps(value, ensure_ascii=False).encode()
                    results = [subprocess.run([sys.executable, str(Path(worker.__file__))],
                        input=data, capture_output=True, env=env, check=True, timeout=60).stdout for _ in range(2)]
                    self.assertEqual(results[0], results[1])
                    result = json.loads(results[0])
                    self.assertEqual(result['qualification'], 'development-worker-unqualified')
                    self.assertFalse(result['receipt']['activation_eligible'])
                    self.assertEqual(result['receipt']['geometry']['status'], 'unmeasured')
                    package = json.loads(result['generation_bytes'])
                    procedure = base64.b64decode(package['artifacts']['compiler/worker.py']['data'], validate=True)
                    self.assertEqual(procedure, Path(worker.__file__).read_bytes())
                    self.assertEqual(result['receipt']['compiler']['worker_sha256'], worker.digest(procedure))
                    destination = os.getenv('BRASSCLAW_TEST_COMPILER_WORKER_EXPORT')
                    if profile == 'base-prompt' and destination:
                        Path(destination).write_text(result['generation_bytes'])
    @unittest.skipUnless(os.getenv('ORNITH_TEST_TOKENIZER'), 'local exact model/tokenizer artifacts required')
    def test_local_qwen_geometry_has_no_provider_or_deployment_effect(self):
        directory = Path(os.environ['ORNITH_TEST_TOKENIZER'])
        value = request()
        value['design']['evidence_view'] = 'literal-reference/v3-trailing-inventory'
        value['target'] = {'adapter': 'hf-qwen2-fast/1', 'max_model_len': 131072,
            'artifacts': {name: artifact((directory / name).read_bytes()) for name in worker.TOKENIZER_FILES if (directory / name).is_file()}}
        result = json.loads(worker.build(json.dumps(value).encode()))
        geometry = result['receipt']['geometry']
        self.assertEqual(geometry['status'], 'local-tokenizer-validated')
        self.assertGreater(geometry['shared_prefix_tokens'], 50000)
        self.assertLessEqual(geometry['rendered_probe_tokens'] + geometry['reserve_tokens'], geometry['max_model_len'])
        self.assertFalse(result['receipt']['activation_eligible'])
        value['target']['artifacts']['tokenizer_config.json'] = artifact(b'{"tokenizer_class":"Qwen2Tokenizer","tokenizer_file":"/etc/passwd"}')
        with self.assertRaisesRegex(ValueError, 'external file'):
            worker.build(json.dumps(value).encode())

    def test_invalid_contract_artifacts_and_paths_produce_no_result(self):
        cases = []
        original = request()
        unknown = json.loads(json.dumps(original)); unknown['execute'] = 'host.shell(...)'; cases.append(unknown)
        traversal = json.loads(json.dumps(original)); traversal['source_manifest']['sources'][0]['artifact'] = '../outside'; cases.append(traversal)
        altered = json.loads(json.dumps(original)); next(iter(altered['artifacts'].values()))['data'] = base64.b64encode(b'changed').decode(); cases.append(altered)
        missing = json.loads(json.dumps(original)); missing['artifacts'].pop(next(iter(missing['artifacts']))); cases.append(missing)
        invalid = json.loads(json.dumps(original)); invalid['design']['reserve_tokens'] = True; cases.append(invalid)
        for value in cases:
            with self.subTest(case=value.keys()), self.assertRaises(ValueError):
                worker.build(json.dumps(value).encode())
        failed = subprocess.run([sys.executable, worker.__file__], input=b'{"profile":"base-prompt","profile":"prefix-maker"}', capture_output=True, timeout=30)
        self.assertEqual(failed.returncode, 2)
        self.assertEqual(failed.stdout, b'')
        self.assertEqual(failed.stderr, b'prefix_build_invalid_or_unavailable\n')


if __name__ == '__main__':
    unittest.main()
