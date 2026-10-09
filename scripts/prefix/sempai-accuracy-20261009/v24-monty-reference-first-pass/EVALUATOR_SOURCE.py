#!/usr/bin/env python3
"""Fixed Monty/BrassClaw compatibility probes; local oracles never reach provider."""
import argparse
import hashlib
import json
import re
from pathlib import Path
import time
import urllib.request
from prefix_response_validation import load_bundle
from evaluate_sempai_prefix import verify_prefix

CASES = [
 ('eval-policy', 'May a BrassClaw authored PythonCode component call eval because upstream Monty supports restricted eval?', False, r'Policy is stricter|Never use.*eval'),
 ('snapshot-effects', 'Does restoring a Monty snapshot by itself authorize replaying an external operation whose effect is unresolved?', False, r'effect ledger|unresolved effects|confirmed effects'),
 ('snapshot-trust', 'Can a snapshot from an untrusted user be loaded merely because its byte format parses?', False, r'trusted compatible producer|trusted.*snapshot|provenance and integrity'),
 ('generator-function', 'Can a Monty component implement a generator function using yield?', False, r'yield|generator functions'),
 ('match-language', 'Can a Monty component use Python match/case statements?', False, r'match.*statements|structural pattern matching'),
 ('inheritance', 'Can a Monty component inherit a Python base class?', False, r'inheritance|base classes'),
 ('typed-proof', 'Does upstream static type-check success prove recursive runtime validation and behavior?', False, r'Annotations/type-check|not.*runtime validation|not.*behavior|static only|not runtime enforcement'),
 ('private-frame', 'Is the upstream 256 MiB transport allowance available as the BrassClaw private transport frame limit?', False, '64 MiB'),
 ('task-clock', 'Does upstream feed_run automatically impose one cumulative BrassClaw task time budget over all feeds?', False, 'not automatically'),
 ('global-lifecycle', 'May an ordinary completed turn terminate the one global BrassClaw Monty orchestrator?', False, r'another Monty VM|lifetime of the BrassClaw|Completing or cancelling a turn'),
 ('seed-q2', 'Must reviewed bundled system_seed installation components receive a human Q2 click on every installed instance?', False, 'installed-instance human Q2'),
 ('binding-policy', 'Must current instance-wide Tool policy be checked independently before dispatch even for an approved pinned component?', True, 'current global Tool policy'),
 ('upstream-eval', 'Does upstream Monty support restricted eval/exec, despite those operations being forbidden in BrassClaw authored components?', True, r'restricted eval/exec'),
 ('eager-generator', 'Do Monty generator expressions currently materialize eagerly rather than providing lazy generator functions?', True, r'materiali[sz](?:e|es|ed|ing).*list|eager'),
 ('host-inputs', 'Must BrassClaw PythonCode use its step-local inputs mapping rather than construct an upstream Monty host pool inside the component?', True, r'step-local(?: typed)?\s+inputs'),
 ('snapshot-ledger', 'Does BrassClaw need retained continuation/effect records beyond interpreter snapshot bytes to reconcile interrupted operations?', True, r'continuation records|effect ledger'),
 ('preload-effects', 'May loading a Skill export call its Tool to initialize a shared cache?', False, r'No Tool calls|effect-free|without Tool/model/I/O'),
 ('preload-not-invoke', 'Is loading a qualified function definition distinct from invoking its retained export?', True, r'Loading a definition is not invocation|Preloading.*invoking|loading.*invocation|Composition preloads|Preloading occurs in assembly preparation'),
 ('mcp-source', 'May MCP invoke a real Skill by submitting arbitrary Python source instead of its canonical matching command and typed data?', False, r'not Python code|no arbitrary Python source|accepts no\s+(?:arbitrary\s+)?Python\s+source'),
 ('shared-defaults', 'May different tasks share mutable Skill function defaults or closures capturing another task inputs?', False, r'Forbid shared mutable|isolated mutable|mutable defaults.*isolat'),
 ('export-api', 'Does the new Skill interface documentation prove that arbitrary export fields can already be inserted through current Recipe APIs?', False, r'not new fields|not.*fields.*INSERT|not.*export fields|not.*implemented.*loader'),
 ('exports-live-policy', 'Do preloaded Skill exports still require compatible ToolSkill bindings and current kernel policy checks before actual Tool calls?', True, r'Preloaded code does not automatically|Prepared ToolSkill bindings'),
 ('mcp-listing', 'Is a raw Skill row sufficient to advertise an MCP tool even without an available activated approved mcp-call-skill-recipe?', False, r'raw Skill presence is not|not raw\s+Skill rows'),
 ('mcp-connect-order', 'Must Kohai establish the provider MCP connection only after final prefix addition, immediately before sending the model request?', True, r'only after final prefix|after final prefix addition'),
 ('mcp-server-lifetime', 'Does disconnecting a completed provider request stop the instance MCP server?', False, r'server stays\s+alive|server is always running|server always running'),
 ('mcp-refresh-pins', 'May a catalogue discovery refresh silently replace the contract already advertised to an in-flight call?', False, r'existing\s+advertised contracts|Existing calls\s+keep their advertised contract'),
 ('monty-memory-ceiling', 'Does the documented 512 MiB minimum Monty budget mean that amount of physical RAM is reserved in advance?', False, r'ceiling, not reserved memory'),
 ('monty-upgrade-floor', 'Does the current guide require an upgrade to raise smaller configured Monty memory budgets to the documented floor with a new revision while preserving larger values and modes?', True, r'Upgrade raises smaller configured values'),
]


def run(server, bundle, output, salt, constrain_citations=False):
    if output.exists():
        raise ValueError('Preserve previous evaluation')
    output.mkdir(parents=True)
    manifest, verified_cards = load_bundle(bundle)
    cards = list(verified_cards.values())
    known = {c['id'][:12]: c for c in cards}
    if len(known) != len(cards):
        raise ValueError('Ambiguous citation shorthand in this bundle')
    (output/'EVALUATOR_SOURCE.py').write_bytes(Path(__file__).read_bytes())
    receipt = {'cases': [], 'temperature': 0, 'repair': False, 'citations_constrained_to_manifest':constrain_citations,
               'generation': manifest['generation'],
               'suite_sha256': hashlib.sha256(json.dumps(CASES).encode()).hexdigest(),
               'production_q1_q2_verified': False, 'support_check': 'Topic regex on cited originals; reasons additionally require semantic review. Not a general entailment verifier.'}
    for identifier, question, expected, support in CASES:
        verify_prefix(server, manifest['model_profile']['model'], manifest)
        schema = {'type': 'object', 'additionalProperties': False, 'required': ['answer', 'reason', 'citations'],
                  'properties': {'answer': {'type': 'boolean'}, 'reason': {'type': 'string'},
                    'citations': {'type': 'array', 'items': {'type': 'string'}, 'minItems': 1, 'maxItems': 3}}}
        if constrain_citations:
            schema['properties']['citations']['items']['enum']=sorted(known)
            schema['properties']['reason']['maxLength']=600
        payload = {'model': 'cyankiwi/Ornith-1.5-9B-AWQ-INT4', 'temperature': 0,
             'messages': [{'role':'system','content': 'Review the precise selected Monty and BrassClaw contracts. Answer the question with a boolean and a brief explanatory reason of at most two sentences. Cite exact 12-character evidence-card IDs from the supplied reference. No invented APIs or permissions.'},
                          {'role':'user','content': question}],
             'max_tokens': 512, 'cache_salt': salt, 'chat_template_kwargs': {'enable_thinking':False},
             'kv_transfer_params': {'cached_token_stats':True},
             'response_format': {'type':'json_schema','json_schema':{'name':'compatibility','strict':True,'schema':schema}}}
        (output/(identifier+'-request.json')).write_text(json.dumps(payload,indent=2)+'\n')
        started = time.monotonic()
        with urllib.request.urlopen(urllib.request.Request(server+'/v1/chat/completions', data=json.dumps(payload).encode(),headers={'Content-Type':'application/json'}),timeout=900) as response:
            raw=json.load(response)
        (output/(identifier+'-response.json')).write_text(json.dumps(raw,indent=2)+'\n')
        try:
            answer=json.loads(raw['choices'][0]['message']['content'])
        except (ValueError, TypeError) as exc:
            receipt['cases'].append({'id':identifier,'passed':False,'error':str(exc),'finish_reason':raw['choices'][0]['finish_reason']})
            (output/'reference-evaluation.json').write_text(json.dumps(receipt,indent=2)+'\n')
            continue
        citations=answer.get('citations',[])
        resolved=bool(citations) and all(c in known for c in citations)
        supported=resolved and any(re.search(support, known[c]['excerpt'], re.I) for c in citations)
        row={'id':identifier,'expected':expected,'answer':answer,'citations_resolved':resolved,
             'citations_support_expected_contract':supported,'passed':answer.get('answer') is expected and supported and raw['choices'][0]['finish_reason']=='stop',
             'finish_reason':raw['choices'][0]['finish_reason'], 'seconds':round(time.monotonic()-started,3),
             'cached_token_stats':raw.get('kv_transfer_params')}
        receipt['cases'].append(row)
        receipt['passed']=sum(c['passed'] for c in receipt['cases'])
        (output/'reference-evaluation.json').write_text(json.dumps(receipt,indent=2)+'\n')
        print(identifier, row['passed'], flush=True)


if __name__ == '__main__':
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('--server',required=True);p.add_argument('--bundle',type=Path,required=True);p.add_argument('--output',type=Path,required=True);p.add_argument('--cache-salt',required=True);p.add_argument('--constrain-citations',action='store_true',help='Use all retained card IDs as decoder choices; no question-specific oracle or feedback.')
    a=p.parse_args();run(a.server.rstrip('/'),a.bundle,a.output,a.cache_salt,a.constrain_citations)
