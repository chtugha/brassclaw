"""Effect-free compilation stages for Recipe-owned source-only prefix builds.

JSON, Base64 and section/render helpers run inside Monty. Recipes dispatch
ordered UTF-8 digest batches separately; no stage calls a Tool or provider.
Geometry remains unmeasured until a separately qualified target adapter runs.
"""

def prefix_json(value):
    import json
    return json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(',', ':'))


def prefix_decode(text):
    import json
    value = json.loads(text)
    if prefix_json(value) != text:
        raise ValueError('canonical compiler data required')
    return value


def prefix_fields(value, names):
    if not isinstance(value, dict) or sorted(value) != sorted(names):
        raise ValueError('compiler fields differ')


def prefix_path(name):
    if not isinstance(name, str) or not name or '\\' in name or name.startswith('/'):
        raise ValueError('canonical relative path required')
    if any(ord(ch)<32 or 127<=ord(ch)<=159 or ord(ch) in (8232,8233) for ch in name):
        raise ValueError('canonical source path required')
    if any(not part or part in ('.', '..') or part.startswith('.') for part in name.split('/')) or name.split('/')[-1] == 'LOCAL_TEST_ENV.md':
        raise ValueError('private or noncanonical source path')
    return name


def prefix_digest_list(values, count):
    if not isinstance(values, list) or len(values) != count:
        raise ValueError('complete ordered digest batch required')
    for value in values:
        if not isinstance(value, str) or len(value) != 64 or any(ch not in '0123456789abcdef' for ch in value):
            raise ValueError('invalid content digest')


def prefix_prepare(inputs):
    source = prefix_decode(inputs['snapshot_bytes'])
    prefix_fields(source, ['format', 'profile', 'release', 'header', 'evidence_view', 'sources', 'procedures'])
    if source['format'] != 'brassclaw-prefix-source-snapshot/1' or source['profile'] not in ('base-prompt', 'prefix-maker'):
        raise ValueError('unsupported source snapshot')
    if source['evidence_view'] not in ('literal-reference/v1', 'literal-reference/v2-deduplicated'):
        raise ValueError('unsupported evidence view')
    if any(not isinstance(source[key], str) or not source[key].strip() for key in ('release', 'header')):
        raise ValueError('release and accepted header required')
    mandatory = ['skills.md', 'recipe.md', 'tools.md', 'toolskills.md', 'CLAUDE.md', 'AGENTS.md', 'simplified_v3.md']
    if source['profile'] == 'prefix-maker':
        mandatory.append('scripts/prefix/prefix-maker-reference.md')
    if not isinstance(source['sources'], list) or not source['sources']:
        raise ValueError('complete source inventory required')
    records = []
    seen = []
    for record in source['sources']:
        prefix_fields(record, ['path', 'kind', 'disposition', 'reason', 'text'])
        path = prefix_path(record['path'])
        if path in seen or record['kind'] not in ('document', 'component') or record['disposition'] not in ('included', 'historical', 'development-only', 'superseded'):
            raise ValueError('duplicate or unsupported source')
        if any(not isinstance(record[key], str) or not record[key].strip() for key in ('text', 'reason')):
            raise ValueError('empty original or disposition reason')
        if path in mandatory and (record['kind'] != 'document' or record['disposition'] != 'included'):
            raise ValueError('mandatory guide incomplete')
        seen.append(path)
        suffix = '.' + path.split('.')[-1]
        records.append({'source': record, 'units': source_units_core(record['text'], suffix)})
    if any(path not in seen for path in mandatory):
        raise ValueError('mandatory source missing')
    records = sorted(records, key=lambda record: (mandatory.index(record['source']['path']) if record['source']['path'] in mandatory else len(mandatory), record['source']['path']))
    procedures = source['procedures']
    prefix_fields(procedures, ['compiler/entry.py', 'compiler/prefix_render.py', 'compiler/prefix_units.py'])
    if any(not isinstance(text, str) or not text for text in procedures.values()):
        raise ValueError('complete compiler procedures required')
    texts = []
    for record in records:
        texts.append(record['source']['text'])
        texts.extend(unit[2] for unit in record['units'])
    for name in sorted(procedures):
        texts.append(procedures[name])
    state = {'snapshot': source, 'records': records, 'hash_count': len(texts)}
    return {'state_bytes': prefix_json(state), 'texts': texts}


def prefix_reference(inputs):
    import base64
    state = prefix_decode(inputs['state_bytes'])
    digests = inputs['digests']
    prefix_digest_list(digests, state['hash_count'])
    position = 0
    identities = []
    files = {'source_snapshot.json': prefix_json(state['snapshot'])}
    for record in state['records']:
        source = record['source']
        checksum = digests[position]
        position += 1
        record['sha256'] = checksum
        record['sections'] = []
        units = []
        for unit in record['units']:
            first, last, text = unit[0], unit[1], unit[2]
            record['sections'].append({'line_start': first, 'line_end': last, 'excerpt_sha256': digests[position]})
            units.append({'line_start': first, 'line_end': last, 'text': text, 'id': digests[position]})
            position += 1
        record['units'] = units
        record['line_count'] = len(source['text'].splitlines())
        identities.append(prefix_json([source['path'], checksum, 1, record['line_count']]))
        files['raw/' + checksum] = source['text']
    procedure_hashes = {}
    for name in sorted(state['snapshot']['procedures']):
        files[name] = state['snapshot']['procedures'][name]
        procedure_hashes[name] = digests[position]
        position += 1
    state['procedure_hashes'] = procedure_hashes
    state['files'] = files
    state['hash_count'] = len(identities)
    return {'state_bytes': prefix_json(state), 'texts': identities}


def prefix_render(inputs):
    state = prefix_decode(inputs['state_bytes'])
    prefix_digest_list(inputs['digests'], state['hash_count'])
    snapshot = state['snapshot']
    records = state['records']
    rendering = []
    evidence = []
    sources = []
    compact = []
    for index in range(len(records)):
        record = records[index]
        source = record['source']
        identity = inputs['digests'][index]
        sources.append({'path': source['path'], 'artifact': 'raw/' + record['sha256'], 'sha256': record['sha256'], 'kind': source['kind'], 'disposition': source['disposition'], 'reason': source['reason']})
        rendering.append({'path': source['path'], 'sha256': record['sha256'], 'disposition': source['disposition'], 'reason': source['reason'], 'text': source['text'], 'evidence_id': identity, 'units': record['units']})
        if source['disposition'] == 'included':
            if identity[:12] in compact:
                raise ValueError('compact evidence identity collision')
            compact.append(identity[:12])
            evidence.append({'id': identity, 'path': source['path'], 'document_sha256': record['sha256'], 'line_start': 1, 'line_end': record['line_count'], 'excerpt_sha256': record['sha256'], 'excerpt': source['text'], 'disposition': source['disposition'], 'sections': record['sections']})
    files = state['files']
    if snapshot['evidence_view'] == 'literal-reference/v2-deduplicated':
        rendered = render_deduplicated_prefix_reference(snapshot['header'], rendering, [])
        files['context.md'] = rendered['context']
        files['render_map.json'] = prefix_json(rendered['render_map'])
    else:
        files['context.md'] = render_prefix_reference(snapshot['header'], rendering, [])
    files['sources.json'] = prefix_json({'format': 'brassclaw-prefix-generation-sources/1', 'release': snapshot['release'], 'input_artifact': 'source_snapshot.json', 'sources': sources})
    files['evidence.json'] = prefix_json(evidence)
    files['profile_design.json'] = prefix_json({'format': 'brassclaw-prefix-compiler-design/1', 'profile': snapshot['profile'], 'header': snapshot['header'], 'required_sources': [record['source']['path'] for record in records if record['source']['path'] in ['skills.md', 'recipe.md', 'tools.md', 'toolskills.md', 'CLAUDE.md', 'AGENTS.md', 'simplified_v3.md', 'scripts/prefix/prefix-maker-reference.md'] and (snapshot['profile'] == 'prefix-maker' or record['source']['path'] != 'scripts/prefix/prefix-maker-reference.md')], 'evidence_view': snapshot['evidence_view'], 'source_selection': 'complete-mandatory'})
    hashes = state['procedure_hashes']
    compiler = {'format': 'brassclaw-monty-prefix-compiler/1', 'execution_attestation': 'unqualified-development-process', 'entry_sha256': hashes['compiler/entry.py'], 'shared_evidence_sha256': hashes['compiler/prefix_units.py'], 'renderer_sha256': hashes['compiler/prefix_render.py'], 'unit_segmenter_sha256': hashes['compiler/prefix_units.py']}
    if snapshot['profile'] == 'prefix-maker':
        compiler['profile_adapter_sha256'] = hashes['compiler/entry.py']
    state = {'files': files, 'profile': snapshot['profile'], 'compiler': compiler, 'source_count': len(records), 'evidence_count': len(evidence)}
    return {'state_bytes': prefix_json(state), 'texts': [files[name] for name in sorted(files)]}


def prefix_identity(inputs):
    state = prefix_decode(inputs['state_bytes'])
    names = sorted(state['files'])
    prefix_digest_list(inputs['digests'], len(names))
    files = {}
    for index in range(len(names)):
        files[names[index]] = inputs['digests'][index]
    identity = {'profile': state['profile'], 'compiler': state['compiler'], 'files': files, 'geometry': {'status': 'unmeasured'}}
    state['identity'] = identity
    return {'state_bytes': prefix_json(state), 'texts': [prefix_json(identity)]}


def prefix_finalize(inputs):
    import base64
    state = prefix_decode(inputs['state_bytes'])
    prefix_digest_list(inputs['digests'], 1)
    manifest = dict(state['identity'])
    manifest.update({'generation': inputs['digests'][0], 'source_count': state['source_count'], 'evidence_count': state['evidence_count'], 'activation_eligible': False, 'qualification': 'development artifact; qualified Recipe admission pending', 'model_quality': 'unmeasured', 'cache_reuse': 'unverified', 'prefix_injection': 'unassigned source-only reference; target qualification required'})
    files = state['files']
    files['manifest.json'] = prefix_json(manifest)
    artifacts = {}
    for name in sorted(files):
        artifacts[name] = {'encoding': 'base64', 'data': base64.b64encode(files[name].encode('utf-8')).decode('ascii')}
    return prefix_json({'format': 'brassclaw-prefix-generation-package/1', 'profile': state['profile'], 'artifacts': artifacts})


def compile_stage(inputs):
    phase = inputs['phase']
    if phase == 'prepare':
        return prefix_json(prefix_prepare({'snapshot_bytes': inputs['snapshot_bytes']}))
    arguments = {'state_bytes': inputs['state_bytes'], 'digests': inputs['digests']}
    if phase == 'reference':
        return prefix_json(prefix_reference(arguments))
    if phase == 'render':
        return prefix_json(prefix_render(arguments))
    if phase == 'identity':
        return prefix_json(prefix_identity(arguments))
    if phase == 'finalize':
        return prefix_finalize(arguments)
    raise ValueError('unsupported compiler stage')
