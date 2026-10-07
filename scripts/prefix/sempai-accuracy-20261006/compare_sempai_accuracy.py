#!/usr/bin/env python3
"""Compare complete frozen offline runs; never merge repairs into first-pass rates."""
import argparse
import hashlib
import json
from pathlib import Path


def summarize(directory):
    manifest_bytes = (directory/'case-manifest.json').read_bytes()
    cases = json.loads(manifest_bytes)['cases']
    evaluation = json.loads((directory/'model-evaluation.json').read_text())
    if evaluation.get('repair_from'):
        raise ValueError('Repair run cannot count as first-pass accuracy')
    by_id = {row['id']:row for row in evaluation['cases']}
    if set(by_id) != {c['id'] for c in cases} or len(by_id) != len(evaluation['cases']):
        raise ValueError('Run is incomplete or contains repeated cases')
    digest = hashlib.sha256(manifest_bytes).hexdigest()
    if evaluation.get('case_manifest_sha256') != digest:
        raise ValueError('Case manifest fingerprint mismatch')
    proposals = json.loads((directory/'candidate-checks.json').read_text())['proposals']
    rows = []
    for case in cases:
        result = by_id[case['id']]
        checks = [r for r in proposals if r['response'] == case['id']+'-response.json']
        artifact_ok = (len(checks)==1 and checks[0].get('offline_acceptance_passed') is True
                       if case['expected_class'] is not None else not checks)
        rows.append({'id':case['id'],'family':case['family'],
                     'response_passed':result.get('response_contract_passed',False),
                     'artifact_passed':artifact_ok,
                     'accepted':result.get('response_contract_passed',False) and artifact_ok,
                     'conversation_preserved':result.get('conversation_contract_passed',False),
                     'requested_draft_emitted':case['expected_class'] is None or result.get('proposal_count')==1,
                     'behavior_passed':sum(r.get('behavior_passed',0) for r in checks),
                     'behavior_cases':sum(r.get('behavior_cases',0) for r in checks)})
    families = {}
    for family in sorted({r['family'] for r in rows}):
        subset = [r for r in rows if r['family']==family]
        families[family] = {'accepted':sum(r['accepted'] for r in subset),'total':len(subset),
                            'conversation_passed':sum(r['conversation_preserved'] for r in subset)}
    return {'manifest_sha256':digest,'accepted':sum(r['accepted'] for r in rows),
            'total':len(rows),'families':families,'cases':rows,
            'first_pass_only':True,'production_q1_q2_verified':False,
            'summary_semantics_fully_verified':False}


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--baseline', required=True, type=Path)
    parser.add_argument('--candidate', required=True, type=Path)
    parser.add_argument('--output', required=True, type=Path)
    args = parser.parse_args()
    baseline, candidate = summarize(args.baseline), summarize(args.candidate)
    if baseline['manifest_sha256'] != candidate['manifest_sha256']:
        raise ValueError('Compare exactly the same frozen cases')
    changes = [{'id':a['id'],'before':a['accepted'],'after':b['accepted']}
               for a,b in zip(baseline['cases'],candidate['cases'],strict=True) if a['accepted']!=b['accepted']]
    report = {'baseline':baseline,'candidate':candidate,'paired_changes':changes,
              'note':'One temperature-zero trial per case; finite offline checks, not a statistical reliability guarantee.'}
    args.output.write_text(json.dumps(report,indent=2)+'\n')
    print(json.dumps({'baseline':baseline['accepted'],'candidate':candidate['accepted'],
                     'cases':baseline['total'],'candidate_families':candidate['families']}))
