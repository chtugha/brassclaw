#!/usr/bin/env python3
"""Verify and prime one hybrid-prefix namespace without changing server settings."""
import argparse
import hashlib
import json
import re
import time
import urllib.request
from pathlib import Path


def request(url, payload=None):
    data=None if payload is None else json.dumps(payload).encode()
    with urllib.request.urlopen(urllib.request.Request(url, data=data,
            headers={'Content-Type':'application/json'}), timeout=1800) as response:
        body=response.read().decode()
        return body if payload is None else json.loads(body)


def prime(server, bundle, salt, output):
    if output.exists():
        raise ValueError('Receipt exists; preserve previous evidence')
    manifest=json.loads((bundle/'prefix_manifest.json').read_text())
    record={'generation':manifest['generation'], 'cache_salt':salt, 'requests':[],
            'native_root_verified':False, 'service_settings_changed':False}
    output.parent.mkdir(parents=True, exist_ok=True)
    def save(): output.write_text(json.dumps(record,indent=2)+'\n')
    def metrics():
        raw=request(server+'/metrics')
        return {name:sum(map(float,re.findall(r'^vllm:'+name+r'(?:\{[^\n]*\})?\s+([0-9.eE+-]+)',raw,re.M)))
                for name in ('prefix_cache_hits_total','external_prefix_cache_hits_total')}
    for text in ('Reply OK.', 'Reply READY.'):
        payload={'model':manifest['model_profile']['model'],
                 'messages':[{'role':'user','content':text}],
                 'chat_template_kwargs':{'enable_thinking':False}}
        tokens=request(server+'/tokenize',payload)['tokens']
        common=manifest['shared_prefix_tokens']
        digest=hashlib.sha256(json.dumps(tokens[:common],separators=(',',':')).encode()).hexdigest()
        if digest!=manifest['shared_prefix_token_sha256']:
            raise ValueError('Injected prefix fingerprint differs')
        block=manifest['model_profile']['cache_block_tokens']
        if len(tokens)//block*block!=manifest['cacheable_shared_tokens']:
            raise ValueError('Probe crosses beyond shared-prefix checkpoint')
        payload.update(max_tokens=1,temperature=0,cache_salt=salt,
                       kv_transfer_params={'cached_token_stats':True})
        before=metrics(); started=time.monotonic()
        raw=request(server+'/v1/chat/completions',payload)
        after=metrics()
        entry={'input_tokens':len(tokens),'prefix_fingerprint_verified':True,
               'seconds':round(time.monotonic()-started,3), 'response':raw,
               'metric_deltas':{key:after[key]-before[key] for key in before}}
        record['requests'].append(entry); save(); print(json.dumps(entry),flush=True)
    stats=record['requests'][-1]['response'].get('kv_transfer_params',{}).get('cached_token_stats',{})
    choices=record['requests'][-1]['response'].get('choices',[])
    record['root_content_verified']=(len(choices)==1 and
        (choices[0].get('message',{}).get('content') or '').strip().rstrip('.')=='READY')
    # Prometheus counters include concurrent clients; acceptance uses this request.
    record['metric_scope']='instance counters; concurrent requests may contribute'
    record['native_root_verified']=(stats.get('num_vllm_cached_tokens',0)>=manifest['cacheable_shared_tokens']
                                    and stats.get('num_lmcache_extra_cached_tokens',-1)==0
                                    and record['root_content_verified'])
    save()
    if not record['native_root_verified']:
        raise RuntimeError('Native shared-root reuse/content not verified; do not score accuracy')


if __name__=='__main__':
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--server',required=True)
    parser.add_argument('--bundle',type=Path,required=True)
    parser.add_argument('--cache-salt',required=True)
    parser.add_argument('--output',type=Path,required=True)
    args=parser.parse_args()
    prime(args.server.rstrip('/'),args.bundle,args.cache_salt,args.output)
