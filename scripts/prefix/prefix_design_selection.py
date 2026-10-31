"""Pure whole-unit model-design selection for Recipe composition.

Prepare -> existing JSON digest usage -> existing model-encoding usage -> select.
The encoder measures complete candidate reference texts, never additive unit costs.
This stage does not establish chat-envelope fit, accepted design/source provenance,
quality, cache correctness or serving eligibility. Those are independent build gates.
All numeric limits and priorities are input data; no model-specific defaults exist.
"""


def design_json(value):
    import json
    return json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(',', ':'))


def design_decode(text):
    import json
    value = json.loads(text)
    if design_json(value) != text:
        raise ValueError('canonical design data required')
    return value


def design_fields(value, fields):
    if not isinstance(value, dict) or sorted(value) != sorted(fields):
        raise ValueError('design fields differ')


def design_integer(value, minimum):
    # Exact JSON/JavaScript integer representation bound, not a model token cap.
    if type(value) is not int or value < minimum or value > 9007199254740991:
        raise ValueError('exact supported design integer required')
    return value


def design_name(value):
    if not isinstance(value, str) or not value or value.strip() != value or any(ord(ch) < 32 or 127 <= ord(ch) <= 159 or ord(ch) in (8232, 8233) for ch in value):
        raise ValueError('nonempty design identity required')
    return value


def design_hash(value):
    if not isinstance(value, str) or len(value) != 64 or any(ch not in '0123456789abcdef' for ch in value):
        raise ValueError('exact design digest required')
    return value


def design_closure(roots, groups):
    # Iterative expansion retains dependency membership without Python recursion.
    selected = list(roots)
    seen = set(selected)
    position = 0
    while position < len(selected):
        for dependency in groups[selected[position]]['dependencies']:
            if dependency not in seen:
                seen.add(dependency)
                selected.append(dependency)
        position += 1
    return selected


def design_groups(band, units):
    if not isinstance(band['groups'], list) or not band['groups']:
        raise ValueError('explicit required/optional teaching groups required')
    groups = {}
    owned = set()
    for group in band['groups']:
        design_fields(group, ['id', 'required', 'priority', 'dependencies', 'units'])
        name = design_name(group['id'])
        if name in groups or type(group['required']) is not bool:
            raise ValueError('distinct typed teaching groups required')
        design_integer(group['priority'], 0)
        if not isinstance(group['dependencies'], list):
            raise ValueError('distinct dependency names required')
        for dependency in group['dependencies']:
            design_name(dependency)
        if len(set(group['dependencies'])) != len(group['dependencies']):
            raise ValueError('distinct dependency names required')
        if not isinstance(group['units'], list) or not group['units']:
            raise ValueError('nonempty complete-unit group required')
        for key in group['units']:
            design_name(key)
            if key not in units or key in owned:
                raise ValueError('unknown or multiply assigned source unit')
            owned.add(key)
        groups[name] = group
    for group in groups.values():
        if any(name not in groups for name in group['dependencies']):
            raise ValueError('missing teaching dependency')
    # Preserve the original alphabetically ordered *waves*, not a heap that can
    # insert a newly-ready child ahead of an independent member of this wave.
    # Every edge is visited once; long dependency chains need no repeated full
    # graph scan or linear membership search through the resolved prefix.
    remaining = {name: len(group['dependencies']) for name, group in groups.items()}
    dependents = {name: [] for name in groups}
    for name, group in groups.items():
        for dependency in group['dependencies']:
            dependents[dependency].append(name)
    ready = sorted(name for name in groups if remaining[name] == 0)
    resolved = []
    while ready:
        following = []
        for name in ready:
            resolved.append(name)
            for dependent in dependents[name]:
                remaining[dependent] -= 1
                if remaining[dependent] == 0:
                    following.append(dependent)
        ready = sorted(following)
    if len(resolved) != len(groups):
        raise ValueError('cyclic teaching dependencies')
    return groups, resolved


def design_source_units(source_bytes):
    sources = design_decode(source_bytes)
    design_fields(sources, ['format', 'sources'])
    if sources['format'] != 'brassclaw-prefix-design-sources/1' or not isinstance(sources['sources'], list) or not sources['sources']:
        raise ValueError('complete original source inventory required')
    units = {}
    originals = []
    source_names = set()
    for source in sources['sources']:
        design_fields(source, ['path', 'sha256', 'text'])
        path = design_name(source['path'])
        if path in source_names or path.startswith('/') or '\\' in path or any(part in ('', '.', '..') or part.startswith('.') for part in path.split('/')) or path.split('/')[-1] == 'LOCAL_TEST_ENV.md':
            raise ValueError('duplicate, private or noncanonical source path')
        source_names.add(path)
        design_hash(source['sha256'])
        if not isinstance(source['text'], str) or not source['text'].strip():
            raise ValueError('complete nonempty original required')
        originals.append(source['text'])
        suffix = '.' + path.split('.')[-1]
        for index, unit in enumerate(source_units_core(source['text'], suffix)):
            key = path + '#' + str(index)
            units[key] = {'source': path, 'source_sha256': source['sha256'], 'line_start': unit[0], 'line_end': unit[1], 'text': unit[2], 'parent_context': unit[3]}
    return units, originals


def design_selector_units(selector, units):
    design_fields(selector, ['path', 'match', 'first_lines'])
    path = selector['path']
    mode = selector['match']
    if not isinstance(path, str) or mode not in ('exact', 'prefix'):
        raise ValueError('explicit source selector required')
    if path:
        design_name(path)
    if (mode == 'exact' and not path) or (mode == 'prefix' and path and not path.endswith('/')):
        raise ValueError('exact source or namespace prefix required')
    parts = path[:-1].split('/') if mode == 'prefix' and path else path.split('/')
    if path.startswith('/') or '\\' in path or (path and any(part in ('', '.', '..') or part.startswith('.') for part in parts)):
        raise ValueError('canonical public source selector required')
    first_lines = selector['first_lines']
    if not isinstance(first_lines, list):
        raise ValueError('explicit complete-unit first-line selectors required')
    for line in first_lines:
        design_name(line)
    if len(set(first_lines)) != len(first_lines):
        raise ValueError('distinct complete-unit first lines required')
    matches = []
    for key in units:
        unit = units[key]
        path_matches = unit['source'] == path if mode == 'exact' else unit['source'].startswith(path)
        if path_matches and (not first_lines or unit['text'].splitlines()[0] in first_lines):
            matches.append(key)
    return matches


def design_resolve_rules(design, units):
    design_fields(design, ['format', 'profile', 'header', 'bands', 'required_coverage'])
    if not isinstance(design['bands'], list) or not design['bands']:
        raise ValueError('explicit supported rule bands required')
    coverage = design['required_coverage']
    if not isinstance(coverage, list) or not coverage:
        raise ValueError('explicit mandatory source coverage required')
    required_units = set()
    for selector in coverage:
        matches = design_selector_units(selector, units)
        if not matches:
            raise ValueError('required source coverage has no current units')
        required_units.update(matches)
    bands = []
    for band in design['bands']:
        design_fields(band, ['minimum_tokens', 'maximum_tokens', 'groups'])
        if not isinstance(band['groups'], list) or not band['groups']:
            raise ValueError('explicit teaching rule groups required')
        groups = []
        for group in band['groups']:
            design_fields(group, ['id', 'required', 'priority', 'dependencies', 'selectors'])
            if not isinstance(group['selectors'], list) or not group['selectors']:
                raise ValueError('explicit teaching source selectors required')
            included = set()
            for selector in group['selectors']:
                matches = design_selector_units(selector, units)
                if not matches:
                    raise ValueError('teaching selector has no current units')
                included.update(matches)
            selected = sorted(included, key=lambda key: (units[key]['source'], units[key]['line_start'], key))
            groups.append({'id': group['id'], 'required': group['required'], 'priority': group['priority'], 'dependencies': group['dependencies'], 'units': selected})
        resolved = {'minimum_tokens': band['minimum_tokens'], 'maximum_tokens': band['maximum_tokens'], 'groups': groups}
        checked, order = design_groups(resolved, units)
        mandatory = design_closure([name for name in order if checked[name]['required']], checked)
        covered = {key for name in mandatory for key in checked[name]['units']}
        if any(key not in covered for key in required_units):
            raise ValueError('mandatory source units are outside the required dependency closure')
        bands.append(resolved)
    return {'format': 'brassclaw-prefix-unit-design/1', 'profile': design['profile'], 'header': design['header'], 'bands': bands}


def validate_prefix_design_coverage(inputs):
    # The caller must resolve this floor independently of the proposed design.
    # Comparing supplied data cannot authenticate that caller or approve sources.
    design_fields(inputs, ['design_bytes', 'sources_bytes', 'floor_bytes'])
    floor = design_decode(inputs['floor_bytes'])
    design_fields(floor, ['format', 'profile', 'required_coverage'])
    if floor['format'] != 'brassclaw-prefix-coverage-floor/1':
        raise ValueError('independent profile coverage floor required')
    design_name(floor['profile'])
    if not isinstance(floor['required_coverage'], list) or not floor['required_coverage']:
        raise ValueError('nonempty independent mandatory coverage required')
    units, _ = design_source_units(inputs['sources_bytes'])
    required = set()
    for selector in floor['required_coverage']:
        matched = design_selector_units(selector, units)
        if not matched:
            raise ValueError('independent required source coverage has no current units')
        required.update(matched)
    rules = design_decode(inputs['design_bytes'])
    if not isinstance(rules, dict) or rules.get('format') != 'brassclaw-prefix-generation-rules/1' or rules.get('profile') != floor['profile']:
        raise ValueError('generation rules must match the independent profile')
    resolved = design_resolve_rules(rules, units)
    if not isinstance(resolved['header'], str) or not resolved['header'].strip():
        raise ValueError('nonempty design header required')
    previous = -1
    bands = []
    for band in resolved['bands']:
        low = design_integer(band['minimum_tokens'], 0)
        high = design_integer(band['maximum_tokens'], low)
        if low <= previous:
            raise ValueError('ordered nonoverlapping budget bands required')
        previous = high
        groups, order = design_groups(band, units)
        mandatory = design_closure([name for name in order if groups[name]['required']], groups)
        covered = {key for name in mandatory for key in groups[name]['units']}
        missing = sorted(required - covered)
        if missing:
            raise ValueError('independent mandatory coverage missing from design band: ' + design_json(missing))
        bands.append({'minimum_tokens': low, 'maximum_tokens': high, 'mandatory_groups': [name for name in order if name in mandatory]})
    return {'format': 'brassclaw-prefix-coverage-comparison/1', 'profile': floor['profile'],
            'qualification': 'supplied-floor-compared-only; no source authority, approval, token fit or activation',
            'floor_bytes': inputs['floor_bytes'], 'design_bytes': inputs['design_bytes'],
            'sources_bytes': inputs['sources_bytes'], 'required_units': sorted(required), 'bands': bands}


def prepare_prefix_design_candidates(inputs):
    design_fields(inputs, ['design_bytes', 'sources_bytes', 'prefix_budget', 'policy_ref', 'special_tokens'])
    budget = design_integer(inputs['prefix_budget'], 0)
    policy_ref = design_name(inputs['policy_ref'])
    special_tokens = inputs['special_tokens']
    if not isinstance(special_tokens, list):
        raise ValueError('pinned model special tokens required')
    for token in special_tokens:
        if not isinstance(token, str) or not token:
            raise ValueError('nonempty model special token required')
    if len(set(special_tokens)) != len(special_tokens):
        raise ValueError('distinct model special tokens required')
    units, originals = design_source_units(inputs['sources_bytes'])
    design = design_decode(inputs['design_bytes'])
    if isinstance(design, dict) and design.get('format') == 'brassclaw-prefix-generation-rules/1':
        design = design_resolve_rules(design, units)
    resolved_design_bytes = design_json(design)
    design_fields(design, ['format', 'profile', 'header', 'bands'])
    if design['format'] != 'brassclaw-prefix-unit-design/1':
        raise ValueError('unsupported unit design')
    design_name(design['profile'])
    if not isinstance(design['header'], str) or not design['header'].strip():
        raise ValueError('nonempty accepted design header required')
    if not isinstance(design['bands'], list) or not design['bands']:
        raise ValueError('explicit supported budget bands required')
    band = None
    previous = -1
    for item in design['bands']:
        design_fields(item, ['minimum_tokens', 'maximum_tokens', 'groups'])
        low = design_integer(item['minimum_tokens'], 0)
        high = design_integer(item['maximum_tokens'], low)
        if low <= previous:
            raise ValueError('ordered nonoverlapping budget bands required')
        previous = high
        if low <= budget <= high:
            band = item
    if band is None:
        raise ValueError('needs redesign: unsupported prefix budget')
    # An accepted design must be sound in every declared band, including those
    # not currently selected. New sources never repair an invalid hidden band.
    for declared_band in design['bands']:
        checked_groups, checked_order = design_groups(declared_band, units)
        if not any(group['required'] for group in checked_groups.values()):
            raise ValueError('at least one mandatory teaching group required')
        if declared_band['minimum_tokens'] == band['minimum_tokens']:
            groups, resolved = checked_groups, checked_order
    required = [name for name in resolved if groups[name]['required']]
    if not required:
        raise ValueError('at least one mandatory teaching group required')
    mandatory = design_closure(required, groups)
    selected = [name for name in resolved if name in mandatory]
    optional = sorted([name for name in groups if name not in selected], key=lambda name: (groups[name]['priority'], name))
    selections = [list(selected)]
    for name in optional:
        if name not in selected:
            closure = design_closure([name], groups)
            selected = selected + [dependency for dependency in resolved if dependency in closure and dependency not in selected]
            selections.append(list(selected))
    candidates = []
    for selection in selections:
        included = [key for name in selection for key in groups[name]['units']]
        parts = [design['header'], '\n']
        for key in included:
            unit = units[key]
            if unit['parent_context']:
                parts.append('\nAncestor context from ' + prefix_escape_controls(unit['source'], special_tokens) + ':\n' + prefix_literal(prefix_escape_controls(unit['parent_context'], special_tokens)) + '\n')
            parts.append('\nSource: ' + prefix_escape_controls(key, special_tokens) + '\n' + prefix_literal(prefix_escape_controls(unit['text'], special_tokens)) + '\n')
        candidates.append({'groups': selection, 'units': included, 'text': ''.join(parts)})
    bank = {'format': 'brassclaw-prefix-design-candidates/1', 'design_bytes': inputs['design_bytes'], 'resolved_design_bytes': resolved_design_bytes, 'sources_bytes': inputs['sources_bytes'], 'policy_ref': policy_ref, 'prefix_budget': budget, 'special_tokens': special_tokens, 'units': units, 'candidates': candidates}
    texts = [inputs['design_bytes'], inputs['sources_bytes']] + originals + [candidate['text'] for candidate in candidates] + [resolved_design_bytes]
    probes = [{'kind': 'text', 'text': candidate['text']} for candidate in candidates]
    return {'bank_bytes': design_json(bank), 'texts': texts, 'probes_json': design_json(probes)}


def select_prefix_design_candidate(inputs):
    design_fields(inputs, ['bank_bytes', 'digests', 'observation_bytes', 'expected_specification', 'expected_artifact_set'])
    bank = design_decode(inputs['bank_bytes'])
    design_fields(bank, ['format', 'design_bytes', 'resolved_design_bytes', 'sources_bytes', 'policy_ref', 'prefix_budget', 'special_tokens', 'units', 'candidates'])
    if bank['format'] != 'brassclaw-prefix-design-candidates/1':
        raise ValueError('unsupported candidate bank')
    # Recompute pure structure rather than trusting a caller-edited candidate bank.
    rebuilt = prepare_prefix_design_candidates({'design_bytes': bank['design_bytes'], 'sources_bytes': bank['sources_bytes'], 'prefix_budget': bank['prefix_budget'], 'policy_ref': bank['policy_ref'], 'special_tokens': bank['special_tokens']})
    if rebuilt['bank_bytes'] != inputs['bank_bytes']:
        raise ValueError('candidate bank differs from pinned design/source inputs')
    digests = inputs['digests']
    if not isinstance(digests, list) or len(digests) != len(rebuilt['texts']):
        raise ValueError('complete actual digest batch required')
    for digest in digests:
        design_hash(digest)
    sources = design_decode(bank['sources_bytes'])['sources']
    for index, source in enumerate(sources):
        if digests[index + 2] != source['sha256']:
            raise ValueError('original source checksum differs')
    observation = design_decode(inputs['observation_bytes'])
    design_fields(observation, ['format', 'specification', 'artifact_set', 'preparation_revision', 'worker_checksum', 'model_chat_template_sha256', 'template_origin', 'report', 'qualification'])
    if observation['format'] != 'brassclaw-model-encoding-observation/1' or observation['qualification'] != 'offline-measurement-only' or observation['template_origin'] != 'model-artifact-template':
        raise ValueError('actual original-template encoding observation required')
    design_integer(observation['preparation_revision'], 1)
    design_hash(observation['worker_checksum'])
    for name in ['specification', 'artifact_set']:
        if design_hash(observation[name]) != design_hash(inputs['expected_' + name]):
            raise ValueError('measurement belongs to another model target')
    report = observation['report']
    design_fields(report, ['format', 'tokenizer_sha256', 'tokenizer_config_sha256', 'chat_template_sha256', 'truncation_disabled', 'padding_disabled', 'probes', 'common_prefix_tokens', 'common_prefix_sha256'])
    if report['format'] != 'brassclaw-model-encoding/1' or report['truncation_disabled'] is not True or report['padding_disabled'] is not True:
        raise ValueError('complete unpadded untruncated encoding required')
    for name in ['tokenizer_sha256', 'tokenizer_config_sha256', 'chat_template_sha256', 'common_prefix_sha256']:
        design_hash(report[name])
    if report['chat_template_sha256'] != design_hash(observation['model_chat_template_sha256']):
        raise ValueError('model template identity differs')
    probes = report['probes']
    if not isinstance(probes, list) or len(probes) != len(bank['candidates']):
        raise ValueError('complete candidate measurements required')
    shared = design_integer(report['common_prefix_tokens'], 0)
    chosen = None
    for index, probe in enumerate(probes):
        design_fields(probe, ['rendered_sha256', 'rendered_bytes', 'token_count', 'token_ids_sha256'])
        design_hash(probe['token_ids_sha256'])
        if design_hash(probe['rendered_sha256']) != digests[len(sources) + 2 + index] or design_integer(probe['rendered_bytes'], 0) != len(bank['candidates'][index]['text'].encode('utf-8')):
            raise ValueError('measurement differs from exact rendered candidate')
        tokens = design_integer(probe['token_count'], 0)
        if shared > tokens:
            raise ValueError('invalid common-prefix measurement')
        if tokens <= bank['prefix_budget']:
            chosen = index
    # No monotonic-token-count assumption: every complete candidate was measured.
    selected = bank['candidates'][chosen] if chosen is not None else None
    selected_units = set(selected['units']) if selected is not None else set()
    # The final candidate contains every group in this accepted size band.
    band_units = set(bank['candidates'][-1]['units'])
    dispositions = [{'unit': key, 'source': bank['units'][key]['source'], 'source_sha256': bank['units'][key]['source_sha256'], 'line_start': bank['units'][key]['line_start'], 'line_end': bank['units'][key]['line_end'], 'disposition': 'selected' if key in selected_units else 'not-selected', 'reason': 'design-selected-unit' if key in selected_units else 'not-in-selected-design-band' if key not in band_units else 'no-complete-candidate-fits' if selected is None else 'outside-selected-priority-prefix'} for key in bank['units']]
    return design_json({'format': 'brassclaw-prefix-design-selection/1', 'status': 'reference-selected' if selected is not None else 'capacity-blocked', 'qualification': 'reference-text-measured-only; no envelope fit, approval, quality or activation', 'design_sha256': digests[0], 'design_format': design_decode(bank['design_bytes'])['format'], 'resolved_design_bytes': bank['resolved_design_bytes'], 'resolved_design_sha256': digests[-1], 'sources_sha256': digests[1], 'policy_ref': bank['policy_ref'], 'prefix_budget': bank['prefix_budget'], 'selected': selected, 'token_count': probes[chosen]['token_count'] if chosen is not None else None, 'mandatory_candidate_tokens': probes[0]['token_count'], 'dispositions': dispositions, 'observation_bytes': inputs['observation_bytes']})


def prefix_design_stage(inputs):
    design_fields(inputs, ['phase', 'data_bytes'])
    data = design_decode(inputs['data_bytes'])
    if inputs['phase'] == 'prepare':
        return design_json(prepare_prefix_design_candidates(data))
    if inputs['phase'] == 'select':
        return select_prefix_design_candidate(data)
    if inputs['phase'] == 'coverage':
        return design_json(validate_prefix_design_coverage(data))
    raise ValueError('unsupported design stage')
