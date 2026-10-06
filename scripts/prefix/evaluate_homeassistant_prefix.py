#!/usr/bin/env python3
"""Paired source-grounding and cache measurements against a deployed HA prefix.

Chat requests rely on the server prefix. Controls use /v1/completions with the
original tokenizer template, bypassing reference injection without changing
services. Rubrics are recorded for human review; citations alone are not accuracy.
"""
import argparse
import hashlib
import json
from pathlib import Path
import re
import time
import httpx
from transformers import AutoTokenizer

CASES = [
 ('ha-secret-visibility',
  'A Home Assistant administrator claims that !secret hides secret values from every UI. Is that true when an automation uses a secret? Name two relevant UI locations and distinguish configuration reuse from confidentiality.',
  ['Secrets are exposed to administrators in YAML source and trace viewers.', '!secret is not a confidentiality boundary.'], ['af8bb373315b']),
 ('ha-include-directory',
  'With automation: !include_dir_list automation/presence/, should each file contain an automation mapping or a list of mappings? Can a file contain two automations? Show a tiny valid file and name the appropriate directory include form for files containing lists.',
  ['Each file is one automation mapping, without a leading list item.', 'Exactly one entry per file; !include_dir_merge_list combines files containing lists.'], ['c7bdc8aa18d2']),
 ('mqtt5-session-expiry',
  'Under MQTT 5 section 4.1.1, can the server discard session state merely because the Session Expiry Interval elapses while the Network Connection remains open? State the normal normative open/closed rules and the separate administrative/resource-limit qualification.',
  ['Client and Server MUST NOT discard while the Network Connection is open.', 'Server MUST discard after the connection is closed and the Session Expiry Interval has passed.', 'Administrative termination and storage/failure limits are a separately identified qualification.'], ['7a923030035d']),
 ('mqtt5-subscribe-ordering',
  'Under MQTT 5 section 3.8.4, is receiving a matching PUBLISH before SUBACK a protocol violation? Which Packet Identifier must SUBACK carry? For a published QoS 2 message with granted subscription maximum QoS 1, what delivery QoS applies?',
  ['PUBLISH before SUBACK is permitted.', 'SUBACK uses the same Packet Identifier as SUBSCRIBE.', 'Delivered QoS is min(2,1)=1.'], ['e7b3c9609d1d']),
 ('modbus-serial-addressing',
  'For Modbus serial line V1.02, classify addresses 0, 1 through 247, and 248 through 255. Does the master need its own slave address? Can two slaves share an individual address? Do not generalize serial addressing rules to every Modbus TCP gateway.',
  ['0 is broadcast; 1..247 are individual slave addresses; 248..255 are reserved.', 'Master has no specific slave address.', 'Slave addresses must be unique on the serial bus.'], ['19f728cfc940']),
 ('modbus-exception-vs-timeout',
  'A Modbus server receives a serial request with a bad CRC. Should it return an exception or remain silent? Separately, for a valid request using function 0x03 to a nonexistent register, explain the exception function code and illegal-data-address exception code.',
  ['Bad CRC: no response, client eventually times out.', 'Valid request that cannot be handled: exception response.', 'Exception function 0x83; illegal data address 0x02.'], ['b585e7d79812']),
 ('yaml-quoting-and-duplicates',
  "In YAML 1.2 single-quoted scalars, does 'C:\\new\\test' contain newline/tab escapes? How do you encode an apostrophe in a single-quoted scalar? With ruamel.yaml new API, are duplicate mapping keys accepted by default? If allow_duplicate_keys=True is explicitly enabled, is the first or subsequent duplicate value discarded?",
  ['Backslashes are literal in single-quoted style.', "An apostrophe is doubled, e.g. 'it''s'.", 'Duplicate keys are rejected by default in the new API.', 'When duplicates are allowed, the later duplicate key/value is discarded.'], ['f3e83e371366','a449a40d2a1a']),
]


def metrics(client,server):
    response=client.get(server+'/metrics');response.raise_for_status()
    return {name:sum(map(float,re.findall(r'^vllm:'+name+r'(?:\{[^\n]*\})?\s+([0-9.eE+-]+)',response.text,re.M)))
            for name in ('prefix_cache_hits_total','prefix_cache_queries_total','external_prefix_cache_hits_total')}


def request(client,url,payload):
    start=time.monotonic(); first=None; pieces=[]; usage=None; finish=None
    payload=dict(payload,stream=True,stream_options={'include_usage':True})
    with client.stream('POST',url,json=payload) as response:
        response.raise_for_status()
        for line in response.iter_lines():
            if not line.startswith('data: '):continue
            raw=line[6:]
            if raw=='[DONE]':break
            item=json.loads(raw)
            if item.get('error'):raise RuntimeError(str(item['error']))
            if item.get('usage'):usage=item['usage']
            for choice in item.get('choices',[]):
                text=choice.get('delta',{}).get('content') or choice.get('text') or ''
                if text:
                    if first is None:first=time.monotonic()-start
                    pieces.append(text)
                if choice.get('finish_reason'):finish=choice['finish_reason']
    return {'answer':''.join(pieces),'usage':usage,'seconds':round(time.monotonic()-start,3),
            'time_to_first_text_seconds':round(first,3) if first is not None else None,'finish_reason':finish}


def evaluate(bundle,tokenizer,server,receipt):
    manifest=json.loads((bundle/'prefix_manifest.json').read_text())
    cards=[json.loads(line) for line in (bundle/'evidence_cards.jsonl').read_text().splitlines()]
    if hashlib.sha256((bundle/'evidence_cards.jsonl').read_bytes()).hexdigest()!=manifest['evidence_cards_sha256']:
        raise ValueError('Evidence fingerprint mismatch')
    ids={card['id'][:12] for card in cards}
    for _,_,_,expected in CASES:
        if not set(expected)<=ids:raise ValueError('Question evidence not present in this generation')
    tok=AutoTokenizer.from_pretrained(tokenizer,local_files_only=True)
    for name,digest in manifest['model_profile']['tokenizer_files_sha256'].items():
        if hashlib.sha256((tokenizer/name).read_bytes()).hexdigest()!=digest:raise ValueError('Tokenizer mismatch: '+name)
    record={'generation':manifest['generation'],'server':server,'cases':[],
            'design':'Same deterministic question with automatic chat prefix versus raw completion using original tokenizer template; no service changes.',
            'limitations':['Seven targeted examples do not prove general expertise.','Cache hits measure reuse, not accuracy.','Semantic rubrics require human review.','Control prompts are short; latency comparisons are not equivalent-work benchmarks.']}
    instruction=('Answer in English, within 180 words, with a small YAML example if relevant. '
                 'If reference cards are available, cite their exact 12-character IDs. '
                 'Otherwise explicitly say reference cards are unavailable. Do not invent citations. ')
    with httpx.Client(timeout=httpx.Timeout(1800,connect=10)) as client:
        for name,question,rubric,expected in CASES:
            entry={'name':name,'question':question,'rubric':rubric,'expected_reference_ids':expected}
            record['cases'].append(entry)
            messages=[{'role':'user','content':instruction+question}]
            print('Testing '+name,flush=True)
            for variant in ('prefixed','control'):
                before=metrics(client,server)
                payload={'model':manifest['model_profile']['model'],'temperature':0,'seed':7,'max_tokens':700}
                if variant=='prefixed':
                    payload.update(messages=messages,chat_template_kwargs={'enable_thinking':False})
                    url=server+'/v1/chat/completions'
                else:
                    payload['prompt']=tok.apply_chat_template(messages,tokenize=False,add_generation_prompt=True,enable_thinking=False)
                    url=server+'/v1/completions'
                result=request(client,url,payload);after=metrics(client,server)
                result['metric_deltas']={key:after[key]-before[key] for key in before}
                cited=set(re.findall(r'\b[0-9a-f]{12}\b',result['answer']))
                result['cited_reference_ids']=sorted(cited)
                result['invalid_reference_ids']=sorted(cited-ids)
                result['expected_reference_ids_cited']=sorted(cited&set(expected))
                if result['usage'] is None:raise RuntimeError('Missing actual server token usage')
                tokens=result['usage']['prompt_tokens']
                if variant=='prefixed' and tokens<manifest['shared_prefix_tokens']:raise ValueError('Prefix was not injected')
                if variant=='control' and tokens>5000:raise ValueError('Control unexpectedly received the reference')
                result['other_requests_may_affect_metrics']=abs(result['metric_deltas']['prefix_cache_queries_total']-tokens)>1
                entry[variant]=result
                receipt.write_text(json.dumps(record,ensure_ascii=False,indent=2)+'\n')
                print(f"{variant}: {result['seconds']}s; prompt={tokens}; cache_hits={result['metric_deltas']['prefix_cache_hits_total']}; cited={result['cited_reference_ids']}",flush=True)
    record['completed']=True;receipt.write_text(json.dumps(record,ensure_ascii=False,indent=2)+'\n')


if __name__=='__main__':
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--bundle',type=Path,required=True)
    parser.add_argument('--tokenizer',type=Path,required=True)
    parser.add_argument('--server',required=True)
    parser.add_argument('--receipt',type=Path,required=True)
    args=parser.parse_args();evaluate(args.bundle,args.tokenizer,args.server.rstrip('/'),args.receipt)
