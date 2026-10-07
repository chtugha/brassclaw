#!/usr/bin/env python3
"""Verify authored teaching artifacts with existing offline checks, never Q1/Q2."""
import argparse
import copy
import hashlib
import json
from pathlib import Path
import re
import sys

from check_sempai_candidates import pure_logic_gate, recipe_checks, run_case
from evaluate_sempai_prefix import FIELDS


def equal(actual, expected):
    return json.dumps(actual, sort_keys=True) == json.dumps(expected, sort_keys=True)


def check(source, manifest_path):
    if sys.platform != 'linux':
        raise ValueError('Behavior verification requires the existing Linux bounded-child checker')
    text = source.read_text()
    manifest = json.loads(manifest_path.read_text())
    if manifest['schema'] != 1 or hashlib.sha256(source.read_bytes()).hexdigest() != manifest['source_sha256']:
        raise ValueError('Teaching source/manifest drift')
    rows = []
    for example in manifest['examples']:
        response = example['response']
        if set(response) != FIELDS or not all(response[k] == [] for k in
                ('bridge_messages','proposed_recipe_updates','proposed_intent_examples','settings_adjustments')):
            raise ValueError('Invalid complete teaching envelope')
        if json.dumps(response, indent=2, ensure_ascii=False) not in text:
            raise ValueError('Sidecar artifact not present literally in source')
        row = {'id': example['id'], 'kind': example['kind']}
        if example['kind'] == 'python':
            code = example['code']
            pure_logic_gate(code)
            if response['proposed_components'][0]['payload']['content'] != code:
                raise ValueError('Decoded constructor source differs from verified program')
            if code not in re.findall(r'```python\n(.*?)\n```', text, re.S):
                raise ValueError('Decoded complete program missing from source')
            failures = []
            for probe in example['probes']:
                actual = run_case(code, probe['inputs'])
                if 'error' in actual or not equal(actual.get('result'), probe['expected']):
                    failures.append({'probe':probe,'actual':actual})
            row.update({'program_sha256':hashlib.sha256(code.encode()).hexdigest(),
                        'behavior_cases':len(example['probes']), 'failures':failures})
            if failures:
                raise ValueError('Incorrect teaching program: '+json.dumps(row))
        elif example['kind'] == 'recipe':
            row['design_checks'] = recipe_checks(response['proposed_components'][0]['payload'], example['fixture'])
            row['tool_execution_verified'] = False
            if not all(row['design_checks'].values()):
                raise ValueError('Incorrect offline Recipe design: '+json.dumps(row))
        else:
            if response['adjusted_volatile_messages'] != example['expected_messages'] or response['proposed_components']:
                raise ValueError('Incorrect review preservation/edit')
        row['passed'] = True
        rows.append(row)
    # Verify that the oracles reject observed defect families rather than merely
    # accepting our authored programs. No untrusted Tool code executes here.
    examples = {e['id']:e for e in manifest['examples']}
    mutations = []
    for identifier, old, new in [
            ('helper-range', "return {'valid':", "{'valid':"),
            ('recursive-shape', 'len(devices) <= 2', 'len(devices) <= 20'),
            ('flag-both', 'type(value) is bool', 'isinstance(value, int)')]:
        example = examples[identifier]
        code = example['code'].replace(old, new)
        assert code != example['code']
        pure_logic_gate(code)
        rejected = any(not equal(run_case(code,p['inputs']).get('result'),p['expected']) for p in example['probes'])
        mutations.append({'mutation':identifier, 'rejected':rejected})
    example = examples['exact-output']
    code = example['code']+"\nresult['debug'] = True"
    pure_logic_gate(code)
    mutations.append({'mutation':'extra-result-field','rejected':not equal(run_case(code,{})['result'], {'eligible':False})})
    for identifier, defect in [('ready-data','duplicate-intent'),('ready-reply','missing-binding')]:
        example = examples[identifier]
        payload = copy.deepcopy(example['response']['proposed_components'][0]['payload'])
        if defect == 'duplicate-intent':
            intents = payload['intent_examples']
            intents[-1] = intents[0]
            payload['variants'][0]['intent_examples'] = intents
        else:
            payload['step_descriptions'][0]['steps'].pop(0)
        mutations.append({'mutation':defect,'rejected':not all(recipe_checks(payload,example['fixture']).values())})
    if not all(m['rejected'] for m in mutations):
        raise ValueError('Oracle mutation survived')
    return {'schema':1,'source_sha256':manifest['source_sha256'],
            'manifest_sha256':hashlib.sha256(manifest_path.read_bytes()).hexdigest(),
            'checker_sha256':hashlib.sha256(Path(__file__).with_name('check_sempai_candidates.py').read_bytes()).hexdigest(),
            'validator_sha256':hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
            'engine':'bounded isolated Linux CPython and offline design checks, not Monty/Q1/Q2',
            'passed':True,'examples':rows,'mutation_checks':mutations,
            'q1_completed':False,'q2_completed':False,'components_activated':False}


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--source', required=True, type=Path)
    parser.add_argument('--manifest', required=True, type=Path)
    parser.add_argument('--receipt', required=True, type=Path)
    args = parser.parse_args()
    receipt = check(args.source,args.manifest)
    args.receipt.write_text(json.dumps(receipt,indent=2)+'\n')
    print(json.dumps({'examples_passed':len(receipt['examples']),
        'behavior_cases':sum(r.get('behavior_cases',0) for r in receipt['examples']),
        'mutations_rejected':len(receipt['mutation_checks'])}))
