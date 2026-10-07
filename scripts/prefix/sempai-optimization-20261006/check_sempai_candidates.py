#!/usr/bin/env python3
"""Independent checks of exported Sempai drafts, never production Q1/Q2.

Only AST-allowlisted pure logic executes, in a bounded isolated CPython child.
Monty compatibility, catalogue approval and actual queue transitions stay unverified.
"""
import argparse
import ast
import hashlib
import json
from pathlib import Path
import re
import subprocess
import sys


SAFE_NODES = {ast.Module, ast.Assign, ast.Name, ast.Load, ast.Store, ast.Constant,
              ast.Subscript, ast.Dict, ast.List, ast.Tuple, ast.Compare,
              ast.BoolOp, ast.UnaryOp, ast.Not, ast.And, ast.Or, ast.Is, ast.IsNot,
              ast.Eq, ast.NotEq, ast.Lt, ast.LtE, ast.Gt, ast.GtE, ast.In, ast.NotIn,
              ast.If, ast.IfExp, ast.For, ast.Break, ast.Continue, ast.Pass,
              ast.Expr, ast.Call, ast.Attribute, ast.GeneratorExp, ast.ListComp,
              ast.comprehension, ast.keyword, ast.BinOp, ast.Add, ast.Sub, ast.USub}
SAFE_NODES.update({ast.FunctionDef, ast.arguments, ast.arg, ast.Return, ast.Set})
SAFE_CALLS = {'type', 'isinstance', 'len', 'all', 'any', 'bool', 'int', 'str', 'list', 'dict', 'set'}
SAFE_DICT_READS = {'get', 'keys', 'values', 'items'}
WORKER = r'''
import json, resource, sys
resource.setrlimit(resource.RLIMIT_CPU, (2, 2))
resource.setrlimit(resource.RLIMIT_AS, (256 * 1024 * 1024, 256 * 1024 * 1024))
payload = json.load(sys.stdin)
safe = {name: getattr(__builtins__, name) for name in
        ('type', 'isinstance', 'len', 'all', 'any', 'bool', 'int', 'str', 'list', 'dict', 'set')}
namespace = {'inputs': payload['inputs'], '__builtins__': safe}
try:
    exec(compile(payload['code'], 'candidate.py', 'exec'), namespace)
    print(json.dumps({'result': namespace['result']}))
except BaseException as exc:
    print(json.dumps({'error': type(exc).__name__ + ': ' + str(exc)}))
'''


def pure_logic_gate(code):
    if not isinstance(code, str) or len(code.encode()) > 32_768:
        raise ValueError('Code must be a bounded string')
    tree = ast.parse(code)
    function_nodes = {id(n) for fn in ast.walk(tree) if isinstance(fn, ast.FunctionDef)
                      for n in ast.walk(fn)}
    if not any(isinstance(node, ast.Assign) and any(isinstance(t, ast.Name) and t.id == 'result'
               for t in node.targets) for node in ast.walk(tree) if id(node) not in function_nodes):
        raise ValueError('Missing result assignment: a helper definition alone is not an executable component')
    assigned_result = False
    functions = {n.name for n in ast.walk(tree) if isinstance(n, ast.FunctionDef)}
    for node in ast.walk(tree):
        if type(node) not in SAFE_NODES:
            raise ValueError('Excluded AST operation: ' + type(node).__name__)
        if isinstance(node, ast.Name) and node.id.startswith('__'):
            raise ValueError('Dunder access forbidden')
        if isinstance(node, ast.FunctionDef) and (node.name.startswith('__') or node.decorator_list):
            raise ValueError('Dunder function/decorator forbidden')
        if isinstance(node, ast.arg) and node.arg.startswith('__'):
            raise ValueError('Dunder argument forbidden')
        if isinstance(node, ast.Attribute) and node.attr not in SAFE_DICT_READS:
            raise ValueError('Only dictionary read methods are allowed in this pure-logic probe')
        if isinstance(node, ast.Call):
            if isinstance(node.func, ast.Name) and node.func.id in SAFE_CALLS | functions:
                pass
            elif isinstance(node.func, ast.Attribute) and node.func.attr in SAFE_DICT_READS:
                pass
            else:
                raise ValueError('Non-allowlisted call')
        if isinstance(node, ast.Assign) and any(isinstance(t, ast.Name) and t.id == 'result' for t in node.targets):
            assigned_result = True
    if not assigned_result:
        raise ValueError('Missing result assignment')


def run_case(code, inputs):
    result = subprocess.run([sys.executable, '-I', '-S', '-c', WORKER],
        input=json.dumps({'code': code, 'inputs': inputs}), text=True,
        capture_output=True, timeout=5)
    if result.returncode != 0:
        return {'error': 'bounded child failed: ' + result.stderr[-500:]}
    return json.loads(result.stdout)


def port_cases():
    for inputs in ({}, {'ports': None}, {'ports': []}, {'ports': 443},
                   {'ports': '443'}, {'ports': [True]}, {'ports': [0]},
                   {'ports': [-1]}, {'ports': [65536]}, {'ports': [443.0]},
                   {'ports': [1, False]}, {'ports': [1, 443, 65535]}, {'ports': [80, 80]}):
        ports = inputs.get('ports')
        valid = type(ports) is list and len(ports) > 0 and all(type(p) is int and 1 <= p <= 65535 for p in ports)
        yield inputs, {'valid': valid, 'ports': ports if valid else []}


def retry_cases():
    base = {'operation_state': 'unknown', 'outcome': 'retryable', 'attempt': 1,
            'max_attempts': 2, 'read_only': False, 'dedup_verified': True}
    cases = [{}, base]
    for key in base:
        for value in (None, 'invalid', True, 0):
            cases.append({**base, key: value})
        cases.append({k: v for k, v in base.items() if k != key})
    cases += [{**base, 'operation_state': 'completed'},
              {**base, 'operation_state': 'completed', 'read_only': True},
              {**base, 'dedup_verified': False}, {**base, 'attempt': 2},
              {**base, 'outcome': 'terminal'},
              {**base, 'dedup_verified': False, 'read_only': True},
              {**base, 'operation_state': 'not_started'}]
    for inp in cases:
        shape = (inp.get('operation_state') in ('unknown', 'not_started', 'completed')
                 and inp.get('outcome') in ('retryable', 'terminal')
                 and type(inp.get('attempt')) is int and inp['attempt'] > 0
                 and type(inp.get('max_attempts')) is int and inp['max_attempts'] > 0
                 and type(inp.get('read_only')) is bool and type(inp.get('dedup_verified')) is bool)
        retry = bool(shape and inp['operation_state'] != 'completed' and inp['outcome'] == 'retryable'
                     and inp['attempt'] < inp['max_attempts'] and (inp['read_only'] or inp['dedup_verified']))
        yield inp, {'retry': retry}


def heldout_cases(name):
    if name == 'pc-check-timeout-ms':
        for inp in [{}, *({'timeout_ms':v} for v in (None,True,False,-1,0,1,7499,7500,7501,0.0,'0',[],{}))]:
            value=inp.get('timeout_ms'); valid=type(value) is int and 0<=value<=7500
            yield inp, {'valid':valid,'timeout_ms':value if valid else None}
    elif name == 'pc-check-channel-list':
        for inp in [{}, *({'channels':v} for v in (None,[],[0],[255],[0,255],[0,0],[-1],[256],[True],[False],[0.0],['0'],0,'',{}))]:
            value=inp.get('channels'); valid=type(value) is list and all(type(v) is int and 0<=v<=255 for v in value)
            yield inp, {'ok':valid,'channels':value if valid else []}
    elif name == 'pc-check-flag':
        for inp in [{}, *({'flag':v} for v in (None,True,False,0,1,'true','false',[],{},[True]))]:
            value=inp.get('flag'); valid=type(value) is bool
            yield inp, {'valid':valid,'flag':value if valid else None}
    elif name == 'pc-check-mode':
        for inp in [{}, *({'mode':v} for v in (None,'eco','boost','Eco','BOOST','',' eco',True,False,0,1,[],{}))]:
            value=inp.get('mode'); valid=type(value) is str and value in ('eco','boost')
            yield inp, {'accepted':valid,'mode':value if valid else None}
    elif name == 'pc-classify-value':
        for inp in [{}, *({'value':v} for v in (None,True,False,-1,0,1,10,11,0.0,'0',[],{}))]:
            value=inp.get('value'); valid=type(value) is int and 0<=value<=10
            state='missing' if 'value' not in inp else 'null' if value is None else 'value' if valid else 'invalid'
            yield inp, {'state':state,'value':value if valid else None}
    elif name == 'pc-check-weighted-entries':
        values=(None,[],[{'label':'a','weight':0}],[{'label':'abcdefgh','weight':1}],
                [{'label':'a','weight':1}]*3,[{'label':'a','weight':1}]*4,
                [{'label':'','weight':1}],[{'label':'abcdefghi','weight':1}],
                [{'label':'a','weight':True}],[{'label':'a','weight':False}],
                [{'label':'a','weight':0.0}],[{'label':'a','weight':2}],
                [{'label':'a','weight':-1}],[{'label':1,'weight':0}],
                [{'label':'a'}],[{'weight':0}],[{'label':'a','weight':0,'extra':1}],
                [None],[[]],['a'],{},'a',True)
        for inp in [{}, *({'entries':v} for v in values)]:
            value=inp.get('entries')
            valid=type(value) is list and len(value)<=3 and all(type(v) is dict and set(v)=={'label','weight'}
                and type(v['label']) is str and 1<=len(v['label'])<=8
                and type(v['weight']) is int and 0<=v['weight']<=1 for v in value)
            yield inp, {'ok':valid,'entries':value if valid else []}


def recipe_checks(payload):
    required = {'name', 'description', 'trigger', 'steps', 'prior_knowledge_content',
                'intent_examples', 'step_descriptions', 'variants', 'dependency_registry'}
    desc = payload['step_descriptions']; variant = payload['variants']
    step = desc[0]['steps'][0]
    intents = variant[0]['intent_examples']
    return {
        'exact_constructor_fields': set(payload) == required,
        'constructor_defaults': payload['name'] == 'recipe-fixed-ready-result' and payload['trigger'] is None and payload['steps'] == [] and payload['dependency_registry'] is None,
        'single_description_single_step': len(desc) == 1 and desc[0]['desc_idx'] == 0 and len(desc[0]['steps']) == 1,
        'single_exact_unapproved_draft_reference': step['include'] == ['84313e5b-c871-4f9e-9623-3986db9c5b2a'],
        'persisted_ibs_layout': step['stepnumber'] == 1 and step['knowledge'] == 'orchestrator' and step['type'] == 'component',
        'variant_selection': len(variant) == 1 and variant[0]['step_link'] == '0:1-0:E' and variant[0]['variable_patterns'] == [],
        'ten_distinct_intents': len(set(intents)) >= 10,
        'constructor_intents_match_variant': payload['intent_examples'] == intents,
        'no_tool_dispatch_or_dependencies': step.get('tool_bindings', []) == [] and step.get('dependencies') is None,
        'intent_does_not_promise_post_reply': not any(re.search(r'\b(?:say|post|reply|respond|send)\b', s, re.I) for s in intents),
        'retains_q1_human_q2_and_sink_gap': 'Q1' in payload['prior_knowledge_content'] and 'Q2' in payload['prior_knowledge_content'] and 'sink' in payload['prior_knowledge_content'].lower(),
        'no_version_number_in_component_ref': set(step) <= {'stepnumber', 'knowledge', 'goal', 'content', 'type', 'include', 'tool_bindings', 'dependencies'},
    }


def check(directory):
    if sys.platform != 'linux':
        raise SystemExit('Run behavioral checks on Linux (brassclaw2); this harness requires Linux RLIMIT_AS.')
    records = []
    drafts = directory / 'component-drafts'
    drafts.mkdir(exist_ok=True)
    for file in sorted(directory.glob('*-response.json')):
        raw = json.loads(file.read_text()); text = raw['choices'][0]['message']['content']
        if not isinstance(text, str):
            records.append({'response': file.name, 'strict_json': False,
                            'error': 'No text response', 'offline_acceptance_passed': False})
            continue
        strict = True
        try:
            outcome = json.loads(text)
        except json.JSONDecodeError:
            # Inspection only: fenced output still fails the strict live response gate.
            strict = False
            match = re.fullmatch(r'\s*```json\s*\n(.*)\n```\s*', text, re.S)
            if not match:
                records.append({'response': file.name, 'strict_json': False, 'error': 'Not a JSON response'})
                continue
            outcome = json.loads(match.group(1))
        if not isinstance(outcome, dict):
            records.append({'response': file.name, 'strict_json': strict,
                            'error': 'Response root is not an object',
                            'offline_acceptance_passed': False})
            continue
        proposals = outcome.get('proposed_components', [])
        if not isinstance(proposals, list) or any(
                not isinstance(p, dict) or set(p) != {'class_code', 'payload'}
                or type(p['class_code']) is not int or not isinstance(p['payload'], dict)
                for p in proposals):
            records.append({'response': file.name, 'strict_json': strict,
                            'error': 'Malformed proposal envelope',
                            'offline_acceptance_passed': False})
            continue
        names = [p.get('payload', {}).get('name') for p in proposals]
        for index, proposal in enumerate(proposals):
            payload = proposal['payload']; name = payload.get('name', 'unnamed')
            if not isinstance(name, str) or not re.fullmatch(r'[a-z0-9-]{1,64}', name):
                records.append({'response': file.name, 'strict_json': strict,
                                'error': 'Unsafe candidate filename',
                                'offline_acceptance_passed': False})
                continue
            entry = {'response': file.name, 'name': name, 'class_code': proposal['class_code'],
                     'strict_json': strict, 'q1_completed': False, 'q2_completed': False,
                     'database_submitted': False, 'activated': False,
                     'duplicate_name_in_response': names.count(name) != 1}
            stem = file.stem.removesuffix('-response') + '-' + str(index) + '-' + name
            (drafts / (stem + '.json')).write_text(json.dumps(proposal, indent=2) + '\n')
            try:
                if proposal['class_code'] == 22:
                    code = payload['content']
                    entry['exact_sink_fields'] = set(payload) == {'name', 'description', 'content'}
                    entry['nonempty_metadata'] = all(isinstance(payload.get(k), str) and payload[k].strip() for k in ('name', 'description', 'content'))
                    entry['content_sha256'] = hashlib.sha256(code.encode()).hexdigest()
                    pure_logic_gate(code)
                    if name == 'pc-check-flag':
                        tree = ast.parse(code)
                        helpers = {n.name for n in tree.body if isinstance(n, ast.FunctionDef)}
                        function_nodes = {id(n) for fn in tree.body if isinstance(fn, ast.FunctionDef)
                                          for n in ast.walk(fn)}
                        if not any(isinstance(n, ast.Call) and isinstance(n.func, ast.Name)
                                and n.func.id in helpers for n in ast.walk(tree) if id(n) not in function_nodes):
                            raise ValueError('Required helper is not invoked at module scope')
                    entry['pure_python_gate'] = True
                    test_cases = list(port_cases() if name == 'pc-check-tcp-port-list' else retry_cases() if name == 'pc-check-retry-eligibility' else heldout_cases(name))
                    if not test_cases:
                        raise ValueError('No defined behavioral oracle for this candidate')
                    results = []
                    for inputs, expected in test_cases:
                        actual = run_case(code, inputs)
                        # Canonical JSON comparison distinguishes false/0 and true/1.
                        passed = json.dumps(actual.get('result'), sort_keys=True) == json.dumps(expected, sort_keys=True) and 'error' not in actual
                        results.append({'inputs': inputs, 'expected': expected, 'actual': actual, 'passed': passed})
                    entry['behavior_cases'] = len(results)
                    entry['behavior_passed'] = sum(r['passed'] for r in results)
                    entry['behavior_results'] = results
                    (drafts / (stem + '.py')).write_text(code)
                    entry['offline_acceptance_passed'] = strict and not entry['duplicate_name_in_response'] and entry['exact_sink_fields'] and entry['nonempty_metadata'] and all(r['passed'] for r in results)
                elif proposal['class_code'] == 21:
                    entry['design_checks'] = recipe_checks(payload)
                    entry['offline_acceptance_passed'] = strict and not entry['duplicate_name_in_response'] and all(entry['design_checks'].values())
                    entry['current_sink_preserves_v3'] = False
                else:
                    raise ValueError('Unsupported proposal class')
            except Exception as exc:
                entry['error'] = str(exc); entry['offline_acceptance_passed'] = False
            records.append(entry)
    receipt = {'engine': 'bounded isolated CPython, not Monty/Q1', 'proposals': records,
               'component_activation_performed': False}
    (directory / 'candidate-checks.json').write_text(json.dumps(receipt, indent=2) + '\n')
    for row in records:
        print(json.dumps({k: v for k, v in row.items() if k != 'behavior_results'}))


if __name__ == '__main__':
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument('directory', type=Path)
    check(ap.parse_args().directory)
