#!/usr/bin/env python3
"""Run already gated pure-logic draft probes in pinned upstream Monty 1.0.0.

This verifies upstream language behavior, not BrassClaw's custom host, Q1/Q2,
Tool dispatch or catalogue activation. Each probe has a fresh isolated session.
"""
import argparse
import hashlib
import importlib.metadata
import json
from pathlib import Path
from check_sempai_candidates import pure_logic_gate


def check(directory, output):
    if output.exists():
        raise ValueError('Preserve the previous Monty receipt')
    version = importlib.metadata.version('pydantic-monty')
    if version != '1.0.0':
        raise ValueError('Expected pinned upstream Monty 1.0.0')
    from pydantic_monty import Monty
    source = directory / 'candidate-checks.json'
    receipt = {'engine': 'upstream Monty', 'version': version,
               'input_receipt_sha256': hashlib.sha256(source.read_bytes()).hexdigest(),
               'custom_brassclaw_host_verified': False, 'q1_q2_verified': False,
               'activated': False, 'candidates': []}
    with Monty() as pool:
        for row in json.loads(source.read_text())['proposals']:
            if row.get('class_code') != 22 or not row.get('behavior_results'):
                continue
            raw = json.loads((directory / row['response']).read_text())
            proposals = json.loads(raw['choices'][0]['message']['content'])['proposed_components']
            proposal = next(p for p in proposals if p['payload']['name'] == row['name'])
            code = proposal['payload']['content']
            if hashlib.sha256(code.encode()).hexdigest() != row['content_sha256']:
                raise ValueError('Candidate changed after the pure-logic check')
            pure_logic_gate(code)
            results = []
            for probe in row['behavior_results']:
                try:
                    with pool.checkout(limits={'max_memory': 16_000_000,
                            'max_feed_duration_secs': 2, 'max_recursion_depth': 64}) as session:
                        actual = session.feed_run(code + '\nresult\n', inputs={'inputs': probe['inputs']})
                    passed = json.dumps(actual, sort_keys=True) == json.dumps(probe['expected'], sort_keys=True)
                    results.append({'inputs': probe['inputs'], 'expected': probe['expected'],
                                    'actual': actual, 'passed': passed})
                except Exception as exc:
                    results.append({'inputs': probe['inputs'], 'expected': probe['expected'],
                                    'error': str(exc), 'passed': False})
            receipt['candidates'].append({'name': row['name'], 'response': row['response'],
                'content_sha256': row['content_sha256'], 'passed': all(p['passed'] for p in results),
                'probes': results})
            output.write_text(json.dumps(receipt, indent=2) + '\n')
    receipt['candidate_count'] = len(receipt['candidates'])
    receipt['candidate_passed'] = sum(c['passed'] for c in receipt['candidates'])
    receipt['probe_count'] = sum(len(c['probes']) for c in receipt['candidates'])
    receipt['probe_passed'] = sum(p['passed'] for c in receipt['candidates'] for p in c['probes'])
    output.write_text(json.dumps(receipt, indent=2) + '\n')
    print(json.dumps({k: v for k, v in receipt.items() if k != 'candidates'}))


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('directory', type=Path)
    parser.add_argument('--output', required=True, type=Path)
    args = parser.parse_args()
    check(args.directory, args.output)
