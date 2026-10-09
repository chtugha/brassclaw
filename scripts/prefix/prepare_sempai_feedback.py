#!/usr/bin/env python3
"""Prepare bounded validation feedback for rejected offline drafts.

This is explicitly a REPAIR input, never an unseen/first-pass experiment. Original
responses/checks remain intact. It does not synthesize code or approve components.
"""
import argparse
import ast
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
        corrections=[]
        for row in rows:
            details['artifact_checks'].append({
                'error': row.get('error'),
                'failed_design_checks': [k for k, v in row.get('design_checks', {}).items() if not v],
                'failed_behavior_witnesses': [r for r in row.get('behavior_results', []) if not r['passed']][:4]})
            # Explain an observed execution failure without synthesizing a fix.
            # This is invocation-body repair evidence, not a preload validator.
            if row.get('class_code') == 22 and row.get('behavior_results') and not row.get('offline_acceptance_passed'):
                raw = json.loads((directory / row['response']).read_text())
                outcome = json.loads(raw['choices'][0]['message']['content'])
                proposal = next(p for p in outcome['proposed_components'] if p['payload']['name'] == row['name'])
                try:
                    tree = ast.parse(proposal['payload']['content'])
                except SyntaxError:
                    tree = None
                if tree is not None:
                    helpers = {n.name: n for n in tree.body if isinstance(n, ast.FunctionDef)}
                    for statement in tree.body:
                        if not isinstance(statement, ast.Assign) or not any(isinstance(t, ast.Name) and t.id == 'result' for t in statement.targets):
                            continue
                        call = statement.value
                        if not isinstance(call, ast.Call) or not isinstance(call.func, ast.Name):
                            continue
                        helper = helpers.get(call.func.id)
                        if helper is not None and not any(isinstance(n, ast.Return) for n in ast.walk(helper)):
                            corrections.append('Observed invocation-body defect: helper ' + helper.name + ' has no return statement, so the caller receives null. Assigning its local result variable does not return that value. Return the complete typed object from the helper and capture it at the invocation boundary; recheck every branch against the original contract. This diagnostic is not a new preload-artifact requirement.')
            if row.get('design_checks',{}).get('retains_review_and_sink_gap') is False:
                corrections.append('Repair payload.prior_knowledge_content itself: retain unapproved dependencies, Q1 and human Q2, and the current live sink limitation for full v3 fields. A warning only in composition_summary does not travel with the Recipe and does not satisfy its contract.')
            if 'Missing result assignment' in row.get('error',''):
                corrections.append('Repair payload.content itself: actually invoke the helper at module scope and assign its returned object to result. A definition alone runs no validation. Recheck every returned value against the original current contract, including invalid-container results; prose is not execution.')
                corrections.append('If the original contract does NOT explicitly require a helper, replace the optional function with a plain module body: initialize the complete required invalid result, validate the raw input, and assign the complete valid result only when all checks pass. Do not keep a definition-only artifact while claiming it was called. Required helpers in other contracts must still be implemented and invoked.')
        details['artifact_specific_corrections']=corrections
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
