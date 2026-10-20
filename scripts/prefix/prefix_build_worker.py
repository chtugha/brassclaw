#!/usr/bin/env python3
"""Bounded offline build protocol over immutable typed artifact bytes.

This development worker provides no deployment, provider, collection, design
acceptance or activation path. Production use still requires retained worker/
dependency qualification, kernel dispatch and durable Recipe job admission.
"""
import base64
import hashlib
import importlib.util
import json
from pathlib import Path
import sys
import tempfile

LIMIT = 64 * 1024 * 1024
TOKENIZER_FILES = {'config.json', 'tokenizer.json', 'tokenizer_config.json',
                   'special_tokens_map.json', 'added_tokens.json', 'vocab.json',
                   'merges.txt', 'chat_template.jinja'}


def fields(value, expected):
    if not isinstance(value, dict) or set(value) != set(expected):
        raise ValueError('unsupported build contract fields')


def digest(data):
    return hashlib.sha256(data).hexdigest()


def blob(value):
    fields(value, ('encoding', 'sha256', 'data'))
    if value['encoding'] != 'base64' or not isinstance(value['data'], str):
        raise ValueError('encoded artifact required')
    data = base64.b64decode(value['data'], validate=True)
    if len(data) > LIMIT or digest(data) != value['sha256']:
        raise ValueError('artifact checksum or capacity differs')
    return data


def load_core():
    # Fixed shipped module only. A request cannot choose a module or entry path.
    spec = importlib.util.spec_from_file_location('brassclaw_prefix_build_core',
                                                 Path(__file__).with_name('base-promt-compiler.py'))
    core = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(core)
    return core


def shape(value):
    pending = [(value, 0)]
    nodes = 0
    while pending:
        current, depth = pending.pop()
        nodes += 1
        if depth > 64 or nodes > 262144:
            raise ValueError('build data structure exceeds worker capacity')
        if isinstance(current, dict):
            pending.extend((child, depth + 1) for child in current.values())
        elif isinstance(current, list):
            pending.extend((child, depth + 1) for child in current)


def tokenizer_contract(core, artifacts):
    # This worker currently supports only the registered Qwen2 fast adapter.
    # Importing a model configuration never authorizes code or file references.
    for name in ('config.json', 'tokenizer_config.json'):
        config = core.strict_json(artifacts[name])
        shape(config)
        if not isinstance(config, dict) or config.get('auto_map') is not None:
            raise ValueError('unsupported remote tokenizer mapping')
        if config.get('tokenizer_class') not in (None, 'Qwen2Tokenizer', 'Qwen2TokenizerFast'):
            raise ValueError('unsupported tokenizer adapter')
        for key in ('tokenizer_file', 'vocab_file', 'merges_file', 'added_tokens_file',
                    'special_tokens_map_file', 'chat_template_file', '_name_or_path', 'name_or_path'):
            if config.get(key) is not None:
                raise ValueError('tokenizer cannot select external file references')
    if core.strict_json(artifacts['tokenizer_config.json']).get('tokenizer_class') not in ('Qwen2Tokenizer', 'Qwen2TokenizerFast'):
        raise ValueError('explicit registered tokenizer class required')


def build(request_bytes):
    if not isinstance(request_bytes, bytes) or len(request_bytes) > LIMIT:
        raise ValueError('bounded build request required')
    core = load_core()
    request = core.strict_json(request_bytes)
    shape(request)
    fields(request, ('format', 'profile', 'source_manifest', 'artifacts', 'design', 'target'))
    if request['format'] != 'brassclaw-prefix-build-request/1' or request['profile'] not in {'base-prompt', 'prefix-maker'}:
        raise ValueError('unsupported build profile')
    fields(request['design'], ('evidence_view', 'reserve_tokens'))
    view = request['design']['evidence_view']
    reserve = request['design']['reserve_tokens']
    if view not in {'literal-reference/v1', 'literal-reference/v2-deduplicated', 'literal-reference/v3-trailing-inventory', 'literal-reference/v4-stable-sharing'} or type(reserve) is not int or not 8192 <= reserve <= 9007199254740991:
        raise ValueError('unsupported build design')
    manifest = request['source_manifest']
    fields(manifest, ('format', 'release', 'sources'))
    if not isinstance(manifest['sources'], list) or not isinstance(request['artifacts'], dict):
        raise ValueError('complete source artifact inventory required')
    wanted = set()
    for source in manifest['sources']:
        if not isinstance(source, dict) or not isinstance(source.get('artifact'), str):
            raise ValueError('invalid source entry')
        name = str(core.relative(source['artifact']))
        if name == 'manifest.json':
            raise ValueError('source cannot replace its manifest')
        wanted.add(name)
    if set(request['artifacts']) != wanted:
        raise ValueError('source artifact inventory differs')
    sources = {name: blob(value) for name, value in request['artifacts'].items()}
    target = request['target']
    tokenizer = {}
    context = None
    if target is not None:
        fields(target, ('adapter', 'max_model_len', 'artifacts'))
        context = target['max_model_len']
        if target['adapter'] != 'hf-qwen2-fast/1' or type(context) is not int or not reserve < context <= 9007199254740991 or not isinstance(target['artifacts'], dict):
            raise ValueError('explicit model context required')
        names = set(target['artifacts'])
        if not {'config.json', 'tokenizer.json', 'tokenizer_config.json'} <= names or not names <= TOKENIZER_FILES:
            raise ValueError('complete registered local tokenizer required')
        tokenizer = {name: blob(value) for name, value in target['artifacts'].items()}
        tokenizer_contract(core, tokenizer)
    if sum(map(len, sources.values())) + sum(map(len, tokenizer.values())) > LIMIT:
        raise ValueError('complete input artifacts exceed worker capacity')
    with tempfile.TemporaryDirectory(prefix='brassclaw-prefix-build-') as temporary:
        root = Path(temporary)
        package = root / 'sources'
        package.mkdir()
        (package / 'manifest.json').write_bytes(core.canonical(manifest))
        for name, data in sources.items():
            destination = package / name
            destination.parent.mkdir(parents=True, exist_ok=True)
            destination.write_bytes(data)
        tokenizer_dir = None
        if target is not None:
            tokenizer_dir = root / 'tokenizer'
            tokenizer_dir.mkdir()
            for name, data in tokenizer.items():
                (tokenizer_dir / name).write_bytes(data)
        options = dict(tokenizer_dir=tokenizer_dir, max_model_len=context,
                       reserve_tokens=reserve, evidence_view=view, worker_path=__file__)
        if request['profile'] == 'prefix-maker':
            spec = importlib.util.spec_from_file_location('brassclaw_prefix_build_maker',
                                                         Path(__file__).with_name('prefix-maker-compiler.py'))
            adapter = importlib.util.module_from_spec(spec)
            spec.loader.exec_module(adapter)
            receipt = adapter.compile_package(package, root / 'output', **options)
            exporter = adapter.core
        else:
            receipt = core.compile_package(package, root / 'output', **options)
            exporter = core
        generation = exporter.export_generation(root / 'output' / receipt['generation'])
        response = core.canonical({'format': 'brassclaw-prefix-build-result/1',
            'request_identity': digest(core.canonical(request)), 'receipt': receipt,
            'generation_bytes': generation.decode('utf-8'), 'qualification': 'development-worker-unqualified'})
        if len(response) > LIMIT:
            raise ValueError('complete result exceeds worker capacity; no artifacts omitted')
        return response


def main():
    data = sys.stdin.buffer.read(LIMIT + 1)
    try:
        result = build(data)
    except (ValueError, OSError):
        # No source snippets, credentials, artifact content or host paths in errors.
        sys.stderr.write('prefix_build_invalid_or_unavailable\n')
        return 2
    sys.stdout.buffer.write(result + b'\n')
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
