#!/usr/bin/env python3
"""Verify automatic chat-template injection and native cache reuse on vLLM.

Clients send ordinary messages, without a template override or client prefix.
Checks exact server token IDs against the immutable compiler template.
"""
import argparse
import hashlib
import json
from pathlib import Path
import re
import time

import httpx
from transformers import AutoTokenizer


def verify(bundle,tokenizer,server,receipt):
    manifest=json.loads((bundle/'prefix_manifest.json').read_text())
    template=(bundle/'vllm_prefix_template.jinja').read_text()
    if hashlib.sha256(template.encode()).hexdigest()!=manifest['template_sha256']:
        raise ValueError('Template fingerprint mismatch')
    for name,expected in manifest['model_profile']['tokenizer_files_sha256'].items():
        if hashlib.sha256((tokenizer/name).read_bytes()).hexdigest()!=expected:
            raise ValueError('Tokenizer fingerprint mismatch: '+name)
    tok=AutoTokenizer.from_pretrained(tokenizer,local_files_only=True)
    record={'generation':manifest['generation'],'server':server,'tokenization_probes':[],
            'requests':[],'automatic_server_injection_verified':False,
            'native_hybrid_cache_reuse_verified':False}
    model=manifest['model_profile']['model']
    tool=[{'type':'function','function':{'name':'read_configuration','description':'Read configuration',
                                       'parameters':{'type':'object','properties':{}}}}]
    with httpx.Client(timeout=httpx.Timeout(1800,connect=10)) as client:
        for thinking,system,tools in [(False,None,None),(True,'Operator instructions.',None),
                                       (False,[{'type':'text','text':'Operator instructions.'}],tool)]:
            messages=([{'role':'system','content':system}] if system is not None else [])+[
                {'role':'user','content':'Inspect a Home Assistant YAML script.'}]
            expected=tok.encode(tok.apply_chat_template(messages,tools=tools,chat_template=template,
                tokenize=False,add_generation_prompt=True,enable_thinking=thinking),add_special_tokens=False)
            payload={'model':model,'messages':messages,'chat_template_kwargs':{'enable_thinking':thinking}}
            if tools: payload['tools']=tools
            response=client.post(server+'/tokenize',json=payload); response.raise_for_status()
            actual=response.json()['tokens']
            if actual!=expected: raise ValueError('Server token IDs differ from the exported template')
            common=manifest['shared_prefix_tokens']
            prefix_sha=hashlib.sha256(json.dumps(actual[:common],separators=(',',':')).encode()).hexdigest()
            if prefix_sha!=manifest['shared_prefix_token_sha256']:
                raise ValueError('Server reference prefix differs from its manifest')
            record['tokenization_probes'].append({'thinking':thinking,'tools':bool(tools),
                                                  'tokens':len(actual),'exact_match':True})
        record['automatic_server_injection_verified']=True
        receipt.write_text(json.dumps(record,indent=2)+'\n')
        print('Automatic injection: exact token IDs verified across three request variants.',flush=True)
        def metrics():
            r=client.get(server+'/metrics'); r.raise_for_status()
            return {name:sum(map(float,re.findall(r'^vllm:'+name+r'(?:\{[^\n]*\})?\s+([0-9.eE+-]+)',r.text,re.M)))
                    for name in ('prefix_cache_hits_total','prefix_cache_queries_total','external_prefix_cache_hits_total')}
        for task in ('Reply OK.','Reply READY.'):
            before=metrics(); started=time.monotonic()
            response=client.post(server+'/v1/chat/completions',json={'model':model,
                'messages':[{'role':'user','content':task}],'max_tokens':1,'temperature':0,
                'chat_template_kwargs':{'enable_thinking':False}})
            response.raise_for_status(); after=metrics()
            entry={'seconds':round(time.monotonic()-started,3),'usage':response.json()['usage'],
                   'metric_deltas':{key:after[key]-before[key] for key in before}}
            if entry['usage']['prompt_tokens']<manifest['shared_prefix_tokens']:
                raise ValueError('Chat request did not receive the reference prefix')
            record['requests'].append(entry); print(json.dumps(entry),flush=True)
            receipt.write_text(json.dumps(record,indent=2)+'\n')
        record['native_hybrid_cache_reuse_verified']=record['requests'][1]['metric_deltas']['prefix_cache_hits_total']>0
        receipt.write_text(json.dumps(record,indent=2)+'\n')
        if not record['native_hybrid_cache_reuse_verified']: raise RuntimeError('Native cache reuse not observed')
        print('Automatic chat prefix and native hybrid cache reuse verified.',flush=True)


if __name__=='__main__':
    ap=argparse.ArgumentParser(description=__doc__)
    ap.add_argument('--bundle',required=True,type=Path)
    ap.add_argument('--tokenizer',required=True,type=Path)
    ap.add_argument('--server',required=True)
    ap.add_argument('--receipt',required=True,type=Path)
    args=ap.parse_args()
    verify(args.bundle,args.tokenizer,args.server.rstrip('/'),args.receipt)
