#!/usr/bin/env python3
"""Report bounded repair acceptance separately from complete first-pass accuracy."""
import argparse
import hashlib
import json
from pathlib import Path
from compare_sempai_accuracy import case_verdict, summarize


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def report(first_pass, rounds, output, max_rounds=2):
    if output.exists():
        raise ValueError('Report exists; preserve prior evidence')
    if len(rounds)>max_rounds or not 0<=max_rounds<=3:
        raise ValueError('Repair bound exceeded')
    initial=summarize(first_pass)
    original_evaluation=json.loads((first_pass/'model-evaluation.json').read_text())
    controls=('json_response_mode_requested','schema_response_mode_requested',
              'thinking_requested','message_count_constrained','compact_grammar_requested',
              'strict_draft_schema_requested','message_content_protected',
              'editable_message_cases','direct_contract_requested',
              'at_most_one_proposal_requested','model_derived_construction_plan_requested',
              'single_layout_requested')
    manifest=json.loads((first_pass/'case-manifest.json').read_text())
    cases={c['id']:c for c in manifest['cases']}
    verdicts={r['id']:r for r in initial['cases']}
    accepted={i:{'stage':0,'directory':str(first_pass),'response_sha256':digest(first_pass/(i+'-response.json'))}
              for i,v in verdicts.items() if v['accepted']}
    stages=[{'stage':0,'accepted':len(accepted),'total':initial['total'],
             'model_evaluation_sha256':digest(first_pass/'model-evaluation.json'),
             'candidate_checks_sha256':digest(first_pass/'candidate-checks.json')}]
    parent=first_pass
    for ordinal,directory in enumerate(rounds,1):
        evaluation=json.loads((directory/'model-evaluation.json').read_text())
        if any(evaluation.get(key)!=original_evaluation.get(key) for key in controls):
            raise ValueError('Repair changed the declared decoder/presentation controls')
        if not evaluation.get('repair_from') or Path(evaluation['repair_from']).resolve()!=parent.resolve():
            raise ValueError('Repair parent does not match the retained preceding stage')
        if digest(directory/'case-manifest.json')!=initial['manifest_sha256']:
            raise ValueError('Repair changed the frozen contracts')
        for name in ('model-evaluation.json','case-manifest.json','candidate-checks.json'):
            if evaluation.get('repair_source_sha256',{}).get(name)!=digest(parent/name):
                raise ValueError('Repair parent evidence drift: '+name)
        proposals=json.loads((directory/'candidate-checks.json').read_text())['proposals']
        pending=set(cases)-set(accepted)
        seen=set(); newly=[]
        for row in evaluation['cases']:
            identifier=row['id']
            if identifier not in pending or identifier in seen:
                raise ValueError('Repair repeated an accepted case or unknown/duplicate case')
            seen.add(identifier)
            if evaluation.get('repair_response_sha256',{}).get(identifier)!=digest(parent/(identifier+'-response.json')):
                raise ValueError('Original rejected response was not pinned')
            verdict=case_verdict(cases[identifier],row,proposals)
            if verdict['accepted']:
                accepted[identifier]={'stage':ordinal,'directory':str(directory),
                                     'response_sha256':digest(directory/(identifier+'-response.json'))}
                newly.append(identifier)
        stages.append({'stage':ordinal,'attempted':len(seen),'newly_accepted':newly,
                       'repair_feedback_presentation':'system' if evaluation.get('direct_repair_feedback_requested') else 'packet',
                       'accepted_after_stage':len(accepted),
                       'model_evaluation_sha256':digest(directory/'model-evaluation.json'),
                       'candidate_checks_sha256':digest(directory/'candidate-checks.json')})
        parent=directory
    result={'schema':1,'manifest_sha256':initial['manifest_sha256'],
            'first_pass_accepted':initial['accepted'],'total':initial['total'],
            'accepted_after_bounded_repairs':len(accepted),'repair_rounds':len(rounds),
            'remaining_rejected':sorted(set(cases)-set(accepted)),
            'stages':stages,'accepted_sources':accepted,
            'first_pass_score_includes_repairs':False,'production_q1_q2_verified':False,
            'base_decoder_and_presentation_controls_unchanged':True,
            'summary_semantics_fully_verified':False}
    output.write_text(json.dumps(result,indent=2)+'\n')
    print(json.dumps({k:result[k] for k in ('first_pass_accepted','total','accepted_after_bounded_repairs','repair_rounds','remaining_rejected')}))
    return result


if __name__=='__main__':
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--first-pass',required=True,type=Path)
    parser.add_argument('--repair',action='append',type=Path,default=[])
    parser.add_argument('--max-repair-rounds',type=int,default=2)
    parser.add_argument('--output',required=True,type=Path)
    args=parser.parse_args()
    report(args.first_pass,args.repair,args.output,args.max_repair_rounds)
