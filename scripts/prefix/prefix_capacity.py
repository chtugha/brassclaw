"""Pure measured-capacity comparison; no selection, truncation or approval.

The consuming pinned Recipe supplies exact native observation bytes and its
retained target/design limits. These calculations establish no trust or cache
acceptance, and never change the source generation or serving configuration.
"""

def capacity_fields(value, names):
    if not isinstance(value, dict) or sorted(value) != sorted(names):
        raise ValueError('capacity observation fields differ')


def capacity_integer(value, minimum=0):
    if type(value) is not int or value < minimum or value > 9007199254740991:
        raise ValueError('exact supported capacity integer required')
    return value


def capacity_hash(value):
    if not isinstance(value, str) or len(value) != 64 or any(ch not in '0123456789abcdef' for ch in value):
        raise ValueError('exact capacity identity required')
    return value


def assess_prefix_capacity(inputs):
    import json
    capacity_fields(inputs, ['observation_bytes', 'expected_specification', 'expected_artifact_set', 'expected_template_sha256', 'limits_ref', 'context_window_tokens', 'reserved_output_tokens', 'reserved_additional_input_tokens'])
    observation = json.loads(inputs['observation_bytes'])
    if json.dumps(observation, ensure_ascii=False, sort_keys=True, separators=(',', ':')) != inputs['observation_bytes']:
        raise ValueError('canonical complete native observation required')
    capacity_fields(observation, ['format', 'specification', 'artifact_set', 'preparation_revision', 'worker_checksum', 'model_chat_template_sha256', 'template_origin', 'report', 'qualification'])
    if observation['format'] != 'brassclaw-model-encoding-observation/1' or observation['qualification'] != 'offline-measurement-only':
        raise ValueError('unsupported measurement contract')
    for key in ['specification', 'artifact_set']:
        if capacity_hash(observation[key]) != capacity_hash(inputs['expected_' + key]):
            raise ValueError('measurement belongs to another target input')
    capacity_integer(observation['preparation_revision'], 1)
    capacity_hash(observation['worker_checksum'])
    original_template = capacity_hash(observation['model_chat_template_sha256'])
    if observation['template_origin'] not in ['supplied-template', 'model-artifact-template']:
        raise ValueError('unknown template origin')
    report = observation['report']
    capacity_fields(report, ['format', 'tokenizer_sha256', 'tokenizer_config_sha256', 'chat_template_sha256', 'truncation_disabled', 'padding_disabled', 'probes', 'common_prefix_tokens', 'common_prefix_sha256'])
    if report['format'] != 'brassclaw-model-encoding/1' or report['truncation_disabled'] is not True or report['padding_disabled'] is not True:
        raise ValueError('full unpadded measurement required')
    for name in ['tokenizer_sha256', 'tokenizer_config_sha256', 'chat_template_sha256', 'common_prefix_sha256']:
        capacity_hash(report[name])
    if report['chat_template_sha256'] != capacity_hash(inputs['expected_template_sha256']):
        raise ValueError('measurement belongs to another effective template')
    if observation['template_origin'] == 'model-artifact-template' and report['chat_template_sha256'] != original_template:
        raise ValueError('original template identity differs')
    probes = report['probes']
    if not isinstance(probes, list) or not probes or len(probes) > 64:
        raise ValueError('complete supported probe bank required')
    shared = capacity_integer(report['common_prefix_tokens'])
    largest = 0
    for probe in probes:
        capacity_fields(probe, ['rendered_sha256', 'rendered_bytes', 'token_count', 'token_ids_sha256'])
        capacity_hash(probe['rendered_sha256'])
        capacity_hash(probe['token_ids_sha256'])
        capacity_integer(probe['rendered_bytes'])
        tokens = capacity_integer(probe['token_count'])
        if shared > tokens:
            raise ValueError('shared prefix exceeds a complete request')
        largest = max(largest, tokens)
    context = capacity_integer(inputs['context_window_tokens'], 1)
    output = capacity_integer(inputs['reserved_output_tokens'], 1)
    additional = capacity_integer(inputs['reserved_additional_input_tokens'])
    required = capacity_integer(largest + output + additional)
    limits_ref = inputs['limits_ref']
    if not isinstance(limits_ref, str) or not limits_ref:
        raise ValueError('retained capacity limits reference required')
    return {
        'format': 'brassclaw-prefix-capacity-comparison/1',
        'qualification': 'calculation-only; not approval or target verification',
        'specification': observation['specification'],
        'artifact_set': observation['artifact_set'],
        'limits_ref': limits_ref,
        'template_sha256': report['chat_template_sha256'],
        'worker_checksum': observation['worker_checksum'],
        'preparation_revision': observation['preparation_revision'],
        'context_window_tokens': context,
        'largest_probe_tokens': largest,
        'reserved_output_tokens': output,
        'reserved_additional_input_tokens': additional,
        'required_tokens': required,
        'fits': required <= context,
        'remaining_tokens': max(0, context - required),
        'shortfall_tokens': max(0, required - context),
        'common_prefix_tokens': shared,
        'cache_qualification': 'unverified',
        'model_quality': 'unmeasured',
    }
