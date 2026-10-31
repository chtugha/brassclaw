"""Pure bootstrap-answer checks for a retained Recipe's typed inputs.

The native owner must parse raw response bytes with value_contract::strict_json
under captured limits BEFORE calling this stage. This is deliberately not another
JSON parser. Native dispatch correlation, source hashes/target scope, numeric
technical bounds, rule applicability, CAS and persistence remain owner checks.
An accepted answer here is never an approval or permission to import a value.
"""


def bootstrap_fields(value, names):
    if type(value) is not dict or set(value) != set(names):
        raise ValueError('bootstrap object fields differ')


def bootstrap_answer_stage(inputs):
    """Preloadable pure entry point using the existing canonical envelope pattern.

    data_bytes is a native-canonicalized typed stage input, NOT raw model output.
    Raw output first passes the existing bounded strict JSON owner. Canonical
    envelope equality here also rejects lossy or duplicate-key stage inputs.
    """
    import json
    bootstrap_fields(inputs, ['phase', 'data_bytes'])
    data = json.loads(inputs['data_bytes'])
    canonical = json.dumps(data, ensure_ascii=False, sort_keys=True, separators=(',', ':'), allow_nan=False)
    if canonical != inputs['data_bytes']:
        raise ValueError('canonical typed bootstrap stage input required')
    if inputs['phase'] == 'answer':
        result = validate_bootstrap_answer(data)
    elif inputs['phase'] == 'evidence':
        result = check_bootstrap_evidence(data)
    elif inputs['phase'] == 'prefill':
        result = check_bootstrap_prefill(data)
    else:
        raise ValueError('unknown bootstrap stage')
    return json.dumps(result, ensure_ascii=False, sort_keys=True, separators=(',', ':'), allow_nan=False)


def bootstrap_string(value):
    if type(value) is not str or not value.strip():
        raise ValueError('nonempty bootstrap string required')
    return value


def bootstrap_fact_key(value):
    if value['field'] not in (
        'context_window_tokens', 'output_limit_tokens', 'tokenizer_identity',
        'template_identity', 'supports_tools', 'supports_reasoning',
    ) or value['scope'] not in ('endpoint_configuration', 'model_declared'):
        raise ValueError('unknown bootstrap field or scope')
    return value['field'], value['scope']


def validate_bootstrap_answer(inputs):
    """Check the shared B1/B2 protocol; return a fresh typed answer.

    expected_* and required_facts/eligible_candidate_ids come from the captured
    packet, never from the response. Equality of echoes is not native correlation.
    No optional/default inputs and no type coercion are permitted.
    """
    import json
    bootstrap_fields(inputs, ['answer', 'expected_task', 'expected_request_id',
                             'expected_input_sha256', 'required_facts',
                             'eligible_candidate_ids'])
    task = inputs['expected_task']
    if task not in ('extract_bootstrap_facts', 'select_bootstrap_prefill'):
        raise ValueError('unknown bootstrap task')
    bootstrap_string(inputs['expected_request_id'])
    digest = inputs['expected_input_sha256']
    if type(digest) is not str or len(digest) != 64 or any(c not in '0123456789abcdef' for c in digest):
        raise ValueError('exact bootstrap input SHA-256 required')
    required = inputs['required_facts']
    candidates = inputs['eligible_candidate_ids']
    if type(required) is not list or type(candidates) is not list:
        raise ValueError('bootstrap requirements and candidates must be lists')
    keys = []
    for requirement in required:
        bootstrap_fields(requirement, ['field', 'scope'])
        keys.append(bootstrap_fact_key(requirement))
    for candidate in candidates:
        bootstrap_string(candidate)
    if len(set(keys)) != len(keys) or len(set(candidates)) != len(candidates):
        raise ValueError('duplicate bootstrap requirement or candidate')
    if task == 'extract_bootstrap_facts' and candidates:
        raise ValueError('B1 must not receive candidate options')
    answer = inputs['answer']
    bootstrap_fields(answer, ['schema', 'task', 'request_id', 'input_sha256', 'result'])
    if (answer['schema'] != 'brassclaw-bootstrap-answer/1' or answer['task'] != task
            or answer['request_id'] != inputs['expected_request_id']
            or answer['input_sha256'] != digest):
        raise ValueError('bootstrap answer contract or echo mismatch')
    result = answer['result']
    bootstrap_fields(result, ['facts', 'issues'] if task == 'extract_bootstrap_facts'
                     else ['candidate_id', 'issues'])
    issues = result['issues']
    allowed = ('missing_evidence', 'conflicting_evidence', 'target_mismatch',
               'unsupported_capability', 'no_applicable_option', 'ambiguous_options',
               'protected_existing_value')
    if type(issues) is not list or any(type(issue) is not str or issue not in allowed for issue in issues):
        raise ValueError('unknown bootstrap issue')
    if len(set(issues)) != len(issues):
        raise ValueError('duplicate bootstrap issue')
    if task == 'select_bootstrap_prefill':
        candidate = result['candidate_id']
        if candidate is not None and (type(candidate) is not str or candidate not in candidates):
            raise ValueError('unknown bootstrap candidate')
        # Applicability/protected values are independently checked by the owner.
        return json.loads(json.dumps(answer, ensure_ascii=False, allow_nan=False))
    facts = result['facts']
    if type(facts) is not list:
        raise ValueError('bootstrap facts must be a list')
    found = []
    for fact in facts:
        bootstrap_fields(fact, ['field', 'scope', 'status', 'value', 'evidence'])
        found.append(bootstrap_fact_key(fact))
        status, value, evidence = fact['status'], fact['value'], fact['evidence']
        if type(evidence) is not list:
            raise ValueError('bootstrap evidence must be a list')
        if status == 'unknown':
            if value is not None or evidence:
                raise ValueError('unknown facts cannot supply a value or evidence')
        elif status == 'conflict':
            if value is not None or len(evidence) < 2:
                raise ValueError('conflict requires distinct contradictory evidence')
        elif status == 'proposed':
            if not evidence:
                raise ValueError('proposed fact requires evidence')
            if fact['field'] in ('context_window_tokens', 'output_limit_tokens'):
                if type(value) is not int or value <= 0:
                    raise ValueError('positive exact token integer required')
            elif fact['field'] in ('supports_tools', 'supports_reasoning'):
                if type(value) is not bool:
                    raise ValueError('native boolean capability required')
            else:
                bootstrap_string(value)
        else:
            raise ValueError('unknown bootstrap status')
        evidence_keys = []
        for item in evidence:
            bootstrap_fields(item, ['source_id', 'locator', 'quote'])
            for name in ('source_id', 'locator', 'quote'):
                bootstrap_string(item[name])
            evidence_keys.append((item['source_id'], item['locator'], item['quote']))
        if len(set(evidence_keys)) != len(evidence_keys):
            raise ValueError('duplicate bootstrap evidence')
    if len(found) != len(keys) or set(found) != set(keys):
        raise ValueError('bootstrap facts must cover each requirement exactly once')
    return json.loads(json.dumps(answer, ensure_ascii=False, allow_nan=False))


def resolve_bootstrap_pointer(source_view, pointer):
    """RFC-6901 pointer into an owner-captured source view, never a file/URL."""
    if type(pointer) is not str or not pointer.startswith('/'):
        raise ValueError('source-view JSON pointer required')
    value = source_view
    for encoded in pointer[1:].split('/'):
        index = 0
        while index < len(encoded):
            if encoded[index] == '~':
                if index + 1 == len(encoded) or encoded[index + 1] not in '01':
                    raise ValueError('invalid JSON pointer escape')
                index += 1
            index += 1
        key = encoded.replace('~1', '/').replace('~0', '~')
        if type(value) is dict and key in value:
            value = value[key]
        elif (type(value) is list and key and key.isascii() and key.isdigit()
              and (key == '0' or not key.startswith('0'))):
            # Compare the decimal index to the list bound before int conversion.
            # A huge untrusted index must not cause arbitrary big-int work.
            bound = str(len(value))
            if len(key) > len(bound) or (len(key) == len(bound) and key >= bound):
                raise ValueError('source-view index out of range')
            value = value[int(key)]
        else:
            raise ValueError('source-view pointer does not resolve')
    return value


def check_bootstrap_evidence(inputs):
    """Check quotes/scalars; text remains an unverified semantic candidate.

    source_views is supplied by the native source owner AFTER hash, target and
    scope verification. This pure helper cannot establish that provenance.
    Text-only conflicts remain unresolved until an existing extraction rule or
    operator resolves them. Neither quotes nor two different sentences prove a
    contradiction. The returned state is evidence analysis, never import consent.
    """
    import json
    bootstrap_fields(inputs, ['fact', 'source_views'])
    fact, sources = inputs['fact'], inputs['source_views']
    bootstrap_fields(fact, ['field', 'scope', 'status', 'value', 'evidence'])
    bootstrap_fact_key(fact)
    if type(sources) is not dict:
        raise ValueError('captured source views required')
    scalars = []
    text = False
    for item in fact['evidence']:
        bootstrap_fields(item, ['source_id', 'locator', 'quote'])
        if item['source_id'] not in sources:
            raise ValueError('uncaptured bootstrap source')
        quote = bootstrap_string(item['quote'])
        value = resolve_bootstrap_pointer(sources[item['source_id']], item['locator'])
        if type(value) is str:
            if quote not in value:
                raise ValueError('bootstrap quote is not an exact substring')
            if item['locator'].startswith('/json/'):
                if fact['status'] == 'proposed' and (type(fact['value']) is not str or fact['value'] != value):
                    raise ValueError('bootstrap proposed identity differs from native string')
                scalars.append(value)
            else:
                text = True
        elif type(value) in (bool, int):
            if quote != json.dumps(value, separators=(',', ':')):
                raise ValueError('bootstrap scalar quote differs')
            scalars.append(value)
            if fact['status'] == 'proposed' and (type(value) is not type(fact['value']) or value != fact['value']):
                raise ValueError('bootstrap proposed value differs from native scalar')
        else:
            raise ValueError('bootstrap evidence must point to a supported scalar')
    if fact['status'] == 'unknown':
        if fact['value'] is not None or fact['evidence']:
            raise ValueError('unknown fact cannot have evidence')
        return {'state': 'unknown'}
    if fact['status'] == 'conflict':
        if fact['value'] is not None or len(fact['evidence']) < 2:
            raise ValueError('conflict requires contradictory evidence')
        distinct = {(type(value).__name__, str(value)) for value in scalars}
        return {'state': 'conflict' if len(distinct) > 1 else 'semantic_review_required'}
    if fact['status'] != 'proposed' or not fact['evidence']:
        raise ValueError('proposed fact requires evidence')
    return {'state': 'semantic_review_required' if text else 'native_scalar_match'}


def bootstrap_prefill_value(field, value):
    # Shape only: all actual budget arithmetic and exact representation bounds
    # are delegated to the existing retained model-budget export below.
    if field in ('context_window_tokens', 'output_limit_tokens'):
        if type(value) is not int or value <= 0:
            raise ValueError('positive bootstrap token observation required')
    elif field in ('supports_tools', 'supports_reasoning'):
        if type(value) is not bool:
            raise ValueError('boolean bootstrap capability required')
    else:
        bootstrap_string(value)


def bootstrap_prefill_option(option, observations, existing, policy_ref, context):
    bootstrap_fields(option, ['candidate_id', 'rule_ref', 'rule_revision', 'values', 'requires'])
    for name in ('candidate_id', 'rule_ref', 'rule_revision'):
        bootstrap_string(option[name])
    names = ['max_prefix_tokens', 'max_prompt_tokens',
             'reserved_output_tokens', 'additional_framing_tokens']
    bootstrap_fields(option['values'], names)
    values = option['values']
    # Reuse the actual planner; no parallel token-budget implementation or
    # guessed framing/output reservation. Every option includes all four values.
    budget = plan_prefix_model_budget({
        'policy_ref': policy_ref,
        'max_prefix_tokens': values['max_prefix_tokens'],
        'max_prompt_tokens': values['max_prompt_tokens'],
        'context_window_tokens': context,
        'reserved_output_tokens': values['reserved_output_tokens'],
        'additional_framing_tokens': values['additional_framing_tokens'],
        'requested_prefix_tokens': values['max_prefix_tokens'],
    })
    requirements = option['requires']
    if type(requirements) is not list:
        raise ValueError('complete option prerequisites required')
    keys = []
    met = True
    for required in requirements:
        bootstrap_fields(required, ['field', 'scope', 'value', 'evidence_ref'])
        key = bootstrap_fact_key(required)
        if key in keys:
            raise ValueError('duplicate option prerequisite')
        keys.append(key)
        bootstrap_string(required['evidence_ref'])
        bootstrap_prefill_value(required['field'], required['value'])
        observation = observations.get(key)
        if (observation is None or observation['status'] != 'independently_verified'
                or type(observation['value']) is not type(required['value'])
                or observation['value'] != required['value']
                or observation['evidence_ref'] != required['evidence_ref']):
            met = False
    state = 'candidate_checked'
    if not met:
        state = 'requirements_not_met'
    elif budget['reservation_shortfall_tokens'] or budget['requested_shortfall_tokens']:
        # A bootstrap candidate is an exact proposed setting, not a requested
        # rendered size to silently shrink. Generation uses the planner normally.
        state = 'budget_not_fit'
    output = observations.get(('output_limit_tokens', 'endpoint_configuration'))
    if (output is not None and output['status'] == 'independently_verified'
            and values['reserved_output_tokens'] > output['value']):
        state = 'budget_not_fit'
    updates = []
    for field in names:
        previous = existing[field]
        if previous['value'] != values[field]:
            if previous['protection'] == 'preserve':
                state = 'protected_existing_value'
            updates.append({'field': field, 'value': values[field],
                            'expected_owner_revision': previous['owner_revision']})
    return {'candidate_id': option['candidate_id'], 'rule_ref': option['rule_ref'],
            'rule_revision': option['rule_revision'], 'state': state,
            'budget': budget, 'field_updates': updates if state == 'candidate_checked' else []}


def check_bootstrap_prefill(inputs):
    """Pure candidate projection, never native provenance, CAS or import consent.

    Options, observations and field revisions come from the captured native
    packet. The native owner MUST independently verify that origin, target,
    source/rule revisions and current field CAS before using field_updates.
    Model output supplies only a candidate ID under the existing B2 protocol.
    Deterministic selection needs exactly one locally admissible option and
    creates no synthetic model response or dispatch record.
    """
    import json
    bootstrap_fields(inputs, ['decision', 'eligible_options', 'checked_observations',
                             'existing_values', 'policy_ref'])
    policy = bootstrap_string(inputs['policy_ref'])
    options, checked, existing = (inputs['eligible_options'], inputs['checked_observations'],
                                  inputs['existing_values'])
    if type(options) is not list or type(checked) is not list:
        raise ValueError('captured options and observations must be lists')
    observations = {}
    for observation in checked:
        bootstrap_fields(observation, ['field', 'scope', 'status', 'value', 'evidence_ref'])
        key = bootstrap_fact_key(observation)
        if key in observations:
            raise ValueError('observations must be consolidated per field and scope')
        bootstrap_string(observation['evidence_ref'])
        if observation['status'] in ('independently_verified', 'llm_proposed'):
            bootstrap_prefill_value(observation['field'], observation['value'])
        elif observation['status'] in ('unknown', 'conflict'):
            if observation['value'] is not None:
                raise ValueError('unknown/conflict observation cannot supply a scalar')
        else:
            raise ValueError('unknown bootstrap observation verification status')
        observations[key] = observation
    context = observations.get(('context_window_tokens', 'endpoint_configuration'))
    if context is None or context['status'] != 'independently_verified':
        raise ValueError('independently verified endpoint context is required before prefill selection')
    names = ['max_prefix_tokens', 'max_prompt_tokens',
             'reserved_output_tokens', 'additional_framing_tokens']
    bootstrap_fields(existing, names)
    for field in names:
        previous = existing[field]
        bootstrap_fields(previous, ['value', 'owner_revision', 'protection'])
        bootstrap_string(previous['owner_revision'])
        if previous['protection'] not in ('preserve', 'replaceable'):
            raise ValueError('explicit field protection required')
        value = previous['value']
        if value is not None and (type(value) is not int or value < 0):
            raise ValueError('existing draft token value must be an exact nonnegative integer or null')
    candidates = {}
    for option in options:
        candidate = bootstrap_prefill_option(option, observations, existing, policy, context['value'])
        if candidate['candidate_id'] in candidates:
            raise ValueError('duplicate bootstrap candidate identity')
        candidates[candidate['candidate_id']] = candidate
    decision = inputs['decision']
    if type(decision) is not dict:
        raise ValueError('explicit bootstrap decision source required')
    method = decision.get('method')
    chosen = None
    issues = []
    model_answer_echo = None
    if method == 'deterministic':
        bootstrap_fields(decision, ['method'])
        viable = [value for value in candidates.values() if value['state'] == 'candidate_checked']
        if len(viable) == 1:
            chosen = viable[0]
            state = chosen['state']
        else:
            state = 'no_applicable_option' if not viable else 'ambiguous_options'
    elif method == 'model':
        bootstrap_fields(decision, ['method', 'protocol'])
        protocol = decision['protocol']
        answer = validate_bootstrap_answer(protocol)
        if (protocol['expected_task'] != 'select_bootstrap_prefill'
                or set(protocol['eligible_candidate_ids']) != set(candidates)):
            raise ValueError('B2 must use the exact captured option identities')
        issues = answer['result']['issues']
        model_answer_echo = {'request_id': answer['request_id'], 'input_sha256': answer['input_sha256']}
        selected = answer['result']['candidate_id']
        if selected is None:
            state = 'no_candidate'
        else:
            chosen = candidates[selected]
            state = chosen['state']
    else:
        raise ValueError('unknown bootstrap decision source')
    result = {'format': 'brassclaw-bootstrap-prefill-candidate/1',
              'selection_method': method, 'state': state,
              'candidate_id': chosen['candidate_id'] if chosen is not None else None,
              'rule_ref': chosen['rule_ref'] if chosen is not None else None,
              'rule_revision': chosen['rule_revision'] if chosen is not None else None,
              'budget': chosen['budget'] if chosen is not None else None,
              'field_updates': chosen['field_updates'] if chosen is not None else [],
              'policy_ref': policy,
              'model_answer_echo': model_answer_echo,
              'field_preconditions': [dict({'field': field}, **existing[field]) for field in names]
                  if chosen is not None and state == 'candidate_checked' else [],
              'issues': issues}
    return json.loads(json.dumps(result, ensure_ascii=False, allow_nan=False))
