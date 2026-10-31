"""Pure Recipe stages for exact retained-default-template worked cases.

Uses the existing UTF-8 digest and ORIGINAL-template encoding usages. There is no
template replacement, renderer, provider call, source selection or acceptance.
All cases are retained; the existing primitive/resource limits govern the batch.
"""


def pair_case_json(value):
    import json
    return json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(',', ':'))


def pair_case_decode(text):
    import json
    value = json.loads(text)
    if pair_case_json(value) != text:
        raise ValueError('canonical case/observation data required')
    return value


def pair_case_fields(value, fields):
    if not isinstance(value, dict) or sorted(value) != sorted(fields):
        raise ValueError('case data fields differ')


def pair_case_hash(value):
    if not isinstance(value, str) or len(value) != 64 or any(ch not in '0123456789abcdef' for ch in value):
        raise ValueError('exact case identity digest required')
    return value


def pair_case_uint(value):
    # Exact JSON/JavaScript integer bound, not a model token allowance.
    if type(value) is not int or value < 0 or value > 9007199254740991:
        raise ValueError('exact nonnegative observation integer required')
    return value


def pair_case_packet(text):
    import json
    packet = pair_case_decode(text)
    pair_case_fields(packet, ['format', 'proposal', 'specification', 'artifact_set', 'template_sha256', 'cases', 'qualification'])
    if packet['format'] != 'brassclaw-prefix-pair-case-packet/1' or packet['qualification'] != 'unqualified-case-input':
        raise ValueError('source-owned unqualified case packet required')
    for field in ['proposal', 'specification', 'artifact_set', 'template_sha256']:
        pair_case_hash(packet[field])
    if not isinstance(packet['cases'], list) or not packet['cases']:
        raise ValueError('complete nonempty worked cases required')
    names = set()
    fixed = None
    for case in packet['cases']:
        pair_case_fields(case, ['id', 'mode', 'context_json', 'expected_prefix', 'expected_render'])
        if any(not isinstance(case[field], str) or not case[field] for field in case):
            raise ValueError('nonempty literal case fields required')
        if case['id'] in names or not case['expected_render'].startswith(case['expected_prefix']):
            raise ValueError('distinct cases and exact expected leading prefix required')
        names.add(case['id'])
        if fixed is None:
            fixed = case['expected_prefix']
        if case['expected_prefix'] != fixed:
            raise ValueError('one fixed prefix across dynamic cases required')
        # Preserve original nested order/text. The source owner and native
        # renderer validate context syntax; this stage never rebuilds it.
        if not isinstance(json.loads(case['context_json']), dict):
            raise ValueError('ordered complete render context required')
    return packet


def pair_case_prepare(inputs):
    pair_case_fields(inputs, ['case_packet_bytes'])
    packet = pair_case_packet(inputs['case_packet_bytes'])
    texts = [inputs['case_packet_bytes']]
    probes = []
    for case in packet['cases']:
        # Ordered triples bind expected render, original context, fixed prefix.
        texts.extend([case['expected_render'], case['context_json'], case['expected_prefix']])
        probes.append({'kind': 'leading_prefix_chat', 'context_json': case['context_json'], 'leading_prefix': case['expected_prefix']})
        probes.append({'kind': 'text', 'text': case['expected_render']})
    return {'bank_bytes': inputs['case_packet_bytes'], 'texts': texts, 'probes_json': pair_case_json(probes),
            'specification': packet['specification'], 'artifact_set': packet['artifact_set']}


def pair_case_compare(inputs):
    pair_case_fields(inputs, ['bank_bytes', 'digests', 'observation_bytes'])
    packet = pair_case_packet(inputs['bank_bytes'])
    digests = inputs['digests']
    if not isinstance(digests, list) or len(digests) != 1 + 3 * len(packet['cases']):
        raise ValueError('complete ordered native digest results required')
    for digest in digests:
        pair_case_hash(digest)
    observed = pair_case_decode(inputs['observation_bytes'])
    pair_case_fields(observed, ['format', 'specification', 'artifact_set', 'preparation_revision', 'model_chat_template_sha256',
                                'template_origin', 'worker_checksum', 'report', 'qualification'])
    if observed['format'] != 'brassclaw-model-encoding-observation/1' or observed['qualification'] != 'offline-measurement-only':
        raise ValueError('actual offline encoding observation required')
    if observed['specification'] != packet['specification'] or observed['artifact_set'] != packet['artifact_set']:
        raise ValueError('case observation model identity differs')
    if observed['template_origin'] != 'model-artifact-template' or observed['model_chat_template_sha256'] != packet['template_sha256']:
        raise ValueError('retained original template required; no override or fallback')
    if pair_case_uint(observed['preparation_revision']) == 0:
        raise ValueError('acknowledged preparation revision required')
    pair_case_hash(observed['worker_checksum'])
    report = observed['report']
    pair_case_fields(report, ['format', 'tokenizer_sha256', 'tokenizer_config_sha256', 'chat_template_sha256',
                             'truncation_disabled', 'padding_disabled', 'probes', 'common_prefix_tokens', 'common_prefix_sha256'])
    if report['format'] != 'brassclaw-model-encoding/2' or report['truncation_disabled'] is not True or report['padding_disabled'] is not True:
        raise ValueError('complete unpadded untruncated partition observations required')
    for field in ['tokenizer_sha256', 'tokenizer_config_sha256', 'chat_template_sha256', 'common_prefix_sha256']:
        pair_case_hash(report[field])
    if report['chat_template_sha256'] != packet['template_sha256']:
        raise ValueError('effective template differs from retained original')
    common = pair_case_uint(report['common_prefix_tokens'])
    if not isinstance(report['probes'], list) or len(report['probes']) != 2 * len(packet['cases']):
        raise ValueError('complete ordered render/text probe pairs required')
    results = []
    fixed = None
    stable = True
    probe_index = 0
    digest_index = 1
    for case in packet['cases']:
        rendered = report['probes'][probe_index]
        literal = report['probes'][probe_index + 1]
        probe_index += 2
        pair_case_fields(rendered, ['rendered_sha256', 'rendered_bytes', 'token_count', 'token_ids_sha256', 'partition'])
        pair_case_fields(literal, ['rendered_sha256', 'rendered_bytes', 'token_count', 'token_ids_sha256'])
        expected_hash, context_hash, prefix_hash = digests[digest_index:digest_index + 3]
        digest_index += 3
        for probe in [rendered, literal]:
            pair_case_hash(probe['rendered_sha256'])
            pair_case_hash(probe['token_ids_sha256'])
            pair_case_uint(probe['rendered_bytes'])
            if pair_case_uint(probe['token_count']) < common:
                raise ValueError('common token sequence exceeds a complete probe')
        if literal['rendered_sha256'] != expected_hash:
            raise ValueError('literal probe is not the expected complete render')
        partition = rendered['partition']
        pair_case_fields(partition, ['prefix_start', 'prefix_end', 'prefix_bytes_sha256', 'prefix_tokens', 'prompt_tokens',
                                    'prefix_token_ids_sha256', 'prompt_token_ids_sha256', 'qualification', 'render_context_sha256'])
        if partition['qualification'] != 'verified-leading-render-bytes-only' or partition['render_context_sha256'] != context_hash or partition['prefix_bytes_sha256'] != prefix_hash:
            raise ValueError('case context/leading-prefix partition differs')
        if pair_case_uint(partition['prefix_start']) != 0 or pair_case_uint(partition['prefix_end']) == 0 or partition['prefix_end'] > rendered['rendered_bytes']:
            raise ValueError('nonempty prefix must begin at byte zero')
        prefix_tokens = pair_case_uint(partition['prefix_tokens'])
        prompt_tokens = pair_case_uint(partition['prompt_tokens'])
        if prefix_tokens + prompt_tokens != rendered['token_count']:
            raise ValueError('partition does not account for complete encoding')
        pair_case_hash(partition['prefix_token_ids_sha256'])
        pair_case_hash(partition['prompt_token_ids_sha256'])
        identity = [prefix_tokens, partition['prefix_token_ids_sha256']]
        if fixed is None:
            fixed = identity
        if identity != fixed:
            stable = False
        matches = (rendered['rendered_sha256'] == expected_hash and rendered['rendered_bytes'] == literal['rendered_bytes']
                   and rendered['token_count'] == literal['token_count'] and rendered['token_ids_sha256'] == literal['token_ids_sha256'])
        results.append({'id': case['id'], 'mode': case['mode'], 'matches': matches,
                        'expected_rendered_sha256': expected_hash, 'rendered_sha256': rendered['rendered_sha256'],
                        'token_ids_sha256': rendered['token_ids_sha256'], 'prefix_token_ids_sha256': partition['prefix_token_ids_sha256'],
                        'prefix_tokens': prefix_tokens, 'prompt_tokens': prompt_tokens, 'total_tokens': rendered['token_count']})
    return pair_case_json({'format': 'brassclaw-prefix-pair-case-comparison/1', 'case_packet_sha256': digests[0],
                           'proposal': packet['proposal'], 'specification': packet['specification'], 'artifact_set': packet['artifact_set'],
                           'template_sha256': packet['template_sha256'], 'worker_checksum': observed['worker_checksum'],
                           'preparation_revision': observed['preparation_revision'], 'cases': results,
                           'all_cases_match': all(case['matches'] for case in results), 'stable_prefix_encoding': stable,
                           'qualification': 'retained-template-cases-only', 'provider_wire_qualified': False,
                           'context_fit': None, 'cache_quality': 'unmeasured', 'model_quality': 'unmeasured'})


def pair_case_stage(inputs):
    pair_case_fields(inputs, ['phase', 'data_bytes'])
    data = pair_case_decode(inputs['data_bytes'])
    if inputs['phase'] == 'prepare':
        return pair_case_json(pair_case_prepare(data))
    if inputs['phase'] == 'compare':
        return pair_case_compare(data)
    raise ValueError('unsupported pair-case stage')
