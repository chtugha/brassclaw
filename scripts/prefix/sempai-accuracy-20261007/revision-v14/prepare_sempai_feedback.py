#!/usr/bin/env python3
"""Prepare bounded validation feedback for rejected offline drafts.

This is explicitly a REPAIR input, never an unseen/first-pass experiment. Original
responses/checks remain intact. It does not synthesize code or approve components.
"""
import argparse
import json
from pathlib import Path


def prepare(directory, output):
    evaluation = json.loads((directory / 'model-evaluation.json').read_text())
    checks = json.loads((directory / 'candidate-checks.json').read_text())['proposals']
    feedback = {}
    for case in evaluation['cases']:
        identifier = case['id']
        rows = [r for r in checks if r['response'] == identifier + '-response.json']
        if case.get('response_contract_passed') and all(r.get('offline_acceptance_passed') for r in rows):
            continue
        if not (directory / (identifier + '-response.json')).exists():
            raise ValueError('Serving failure has no draft to repair: ' + identifier)
        details = {'response_checks': {k: case.get(k) for k in (
            'finish_reason', 'schema_valid', 'conversation_contract_passed',
            'compatibility_arrays_empty', 'proposal_contract_passed')}}
        details['artifact_checks'] = []
        for row in rows:
            details['artifact_checks'].append({
                'error': row.get('error'),
                'failed_design_checks': [k for k, v in row.get('design_checks', {}).items() if not v],
                'failed_behavior_witnesses': [r for r in row.get('behavior_results', []) if not r['passed']][:4]})
        feedback[identifier] = (
            'The offline host rejected the previous outcome. Repair the actual emitted artifact, '
            'using the original current contract and these observed checks; do not merely describe a fix. '
            'Preserve original messages outside explicitly authorized edits. New designs belong in '
            'proposed_components with class_code and payload, never compatibility update arrays. '
            'Use complete result objects on every code path; preserve missing/null and exact types. '
            'Invoke a helper when requested. Do not replay Tools or claim production Q1/Q2/activation. '
            'These are offline validation observations, not a component approval: ' + json.dumps(details))
    if output.exists():
        raise ValueError('Keep previous feedback; choose a new output path')
    output.write_text(json.dumps(feedback, indent=2) + '\n')
    print(json.dumps({'rejected_cases': list(feedback), 'first_pass': False}))


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('directory', type=Path)
    parser.add_argument('--output', required=True, type=Path)
    args = parser.parse_args()
    prepare(args.directory, args.output)
