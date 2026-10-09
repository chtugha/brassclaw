#!/usr/bin/env python3
"""Qualify pure-logic teaching probes against upstream Monty, separate from Q1/Q2."""
import argparse
import hashlib
import importlib.metadata
import json
from pathlib import Path
from check_sempai_candidates import pure_logic_gate


def check(source, manifest_path, receipt_path):
    if receipt_path.exists():
        raise ValueError('Preserve previous receipt')
    manifest_bytes = manifest_path.read_bytes()
    manifest = json.loads(manifest_bytes)
    if manifest['schema'] != 1 or hashlib.sha256(source.read_bytes()).hexdigest() != manifest['source_sha256']:
        raise ValueError('Teaching source/manifest drift')
    if importlib.metadata.version('pydantic-monty') != '1.0.0':
        raise ValueError('Expected pinned Monty 1.0.0')
    from pydantic_monty import Monty
    receipt = {'engine':'upstream Monty 1.0.0', 'source_sha256':manifest['source_sha256'],
               'manifest_sha256':hashlib.sha256(manifest_bytes).hexdigest(),
               'custom_brassclaw_host_verified':False, 'q1_q2_verified':False, 'examples':[]}
    with Monty() as pool:
        for example in manifest['examples']:
            if example['kind'] != 'python':
                continue
            code = example['code']; pure_logic_gate(code)
            results=[]
            for probe in example['probes']:
                try:
                    with pool.checkout(limits={'max_memory':16_000_000,'max_feed_duration_secs':2,'max_recursion_depth':64}) as session:
                        actual=session.feed_run(code+'\nresult\n',inputs={'inputs':probe['inputs']})
                    results.append({'inputs':probe['inputs'],'expected':probe['expected'],'actual':actual,
                                    'passed':json.dumps(actual,sort_keys=True)==json.dumps(probe['expected'],sort_keys=True)})
                except Exception as exc:
                    results.append({'inputs':probe['inputs'],'error':str(exc),'passed':False})
            receipt['examples'].append({'id':example['id'],'code_sha256':hashlib.sha256(code.encode()).hexdigest(),
                                       'passed':all(r['passed'] for r in results),'probes':results})
    receipt['passed']=all(e['passed'] for e in receipt['examples'])
    receipt['probe_count']=sum(len(e['probes']) for e in receipt['examples'])
    receipt_path.write_text(json.dumps(receipt,indent=2)+'\n')
    print(json.dumps({k:v for k,v in receipt.items() if k!='examples'}))
    if not receipt['passed']:
        raise RuntimeError('Teaching Monty behavior failed')


if __name__=='__main__':
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('--source',type=Path,required=True);p.add_argument('--manifest',type=Path,required=True);p.add_argument('--receipt',type=Path,required=True)
    a=p.parse_args();check(a.source,a.manifest,a.receipt)
