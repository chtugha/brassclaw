#!/usr/bin/env python3
"""Check documented language/preload facts in upstream Monty, not BrassClaw Q1."""
import argparse
import importlib.metadata
import json
from pathlib import Path


def check(output):
    if output.exists():
        raise ValueError('Preserve previous evidence')
    if importlib.metadata.version('pydantic-monty') != '1.0.0':
        raise ValueError('Expected Monty 1.0.0')
    from pydantic_monty import Monty
    rows=[]
    limits={'max_memory':16_000_000,'max_feed_duration_secs':2,'max_recursion_depth':64}
    with Monty() as pool:
        for name, code, expected in [
            ('exact-bool-int', '[type(True) is int, type(True) is bool]', [False,True]),
            ('eager-generator', 'type((x for x in [1,2])) is list', True),
            ('upstream-restricted-eval', 'eval("1 + 2")', 3),
        ]:
            with pool.checkout(limits=limits) as session:
                actual=session.feed_run(code)
            rows.append({'id':name,'actual':actual,'expected':expected,'passed':actual==expected})
        for name, code in [
            ('reject-yield','def gen():\n    yield 1\ngen()'),
            ('reject-match','match 1:\n    case 1:\n        x=1'),
            ('reject-inheritance','class Child(object):\n    pass'),
            ('reject-package','import requests'),
        ]:
            try:
                with pool.checkout(limits=limits) as session:
                    session.feed_run(code)
                rows.append({'id':name,'passed':False,'error':'unexpectedly accepted'})
            except Exception as exc:
                rows.append({'id':name,'passed': 'NotImplementedError' in str(exc) or
                             (name=='reject-package' and 'ModuleNotFoundError' in str(exc)),
                             'observed_error':str(exc)})
        calls=[]
        def external(item):
            calls.append(item)
            return {'item':item}
        with pool.checkout(limits=limits) as session:
            session.feed_run('def retained_usage(item):\n    return primitive(item)',
                             external_lookup={'primitive':external})
            no_preload_effect=not calls
            actual=session.feed_run('retained_usage(value)',inputs={'value':7},
                                    external_lookup={'primitive':external})
        rows.append({'id':'definition-before-invocation','preload_effect_free':no_preload_effect,
                     'calls':calls,'actual':actual,
                     'passed':no_preload_effect and calls==[7] and actual=={'item':7}})
    receipt={'engine':'upstream Monty 1.0.0','checks':rows,'passed':all(r['passed'] for r in rows),
             'brassclaw_export_loader_verified':False,'tool_registration_policy_verified':False,
             'production_q1_q2_verified':False}
    output.write_text(json.dumps(receipt,indent=2)+'\n')
    print(json.dumps({'checks':len(rows),'passed':sum(r['passed'] for r in rows)}))
    if not receipt['passed']:
        raise RuntimeError('Upstream reference verification failed')


if __name__=='__main__':
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('--output',type=Path,required=True)
    check(p.parse_args().output)
