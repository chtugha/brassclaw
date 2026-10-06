#!/usr/bin/env python3
"""Live LMCache reload check using normal requests, without service restarts.

An unrelated long request pressures the native GPU prefix cache, then the exact
engineering reference is requested again. Run only on a dedicated test server.
"""
import argparse
import json
import re
import time
from pathlib import Path

import httpx
from transformers import AutoTokenizer


def counters(client, url, names):
    response = client.get(url)
    response.raise_for_status()
    return {name: sum(float(value) for value in re.findall(
        '^' + re.escape(name) + r'(?:\{[^\n]*\})?\s+([0-9.eE+-]+)',
        response.text, re.M)) for name in names}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--bundle', type=Path, required=True)
    parser.add_argument('--tokenizer', type=Path, required=True)
    parser.add_argument('--server', required=True)
    parser.add_argument('--lmcache', required=True)
    parser.add_argument('--receipt', type=Path, required=True)
    args = parser.parse_args()
    manifest = json.loads((args.bundle / 'prefix_manifest.json').read_text())
    tok = AutoTokenizer.from_pretrained(args.tokenizer, local_files_only=True)
    text = tok.apply_chat_template(
        [{'role': 'user', 'content': 'Confirm engineering reference warm-up.'}],
        chat_template=(args.bundle / 'vllm_prefix_template.jinja').read_text(),
        tokenize=False, add_generation_prompt=True, enable_thinking=False)
    prefix = tok.encode(text, add_special_tokens=False)
    # This is a transient cache-pressure probe, never prefix reference content.
    unrelated = tok.encode('Unrelated cache-pressure test.\n', add_special_tokens=False)
    unit = tok.encode(' neutral', add_special_tokens=False)
    unrelated = (unrelated + unit * 130000)[:130000]
    receipt = {'generation': manifest['generation'], 'requests': [],
               'service_restarts': False, 'disk_persistence_verified': False}
    native = ('vllm:prefix_cache_hits_total', 'vllm:external_prefix_cache_hits_total')
    external = ('lmcache_mp_lookup_hit_tokens_total', 'lmcache_mp_lookup_hit_l1_tokens_total',
                'lmcache_mp_num_finished_retrieves_total', 'lmcache_mp_l1_write_chunks_total')
    with httpx.Client(timeout=httpx.Timeout(1800, connect=10)) as client:
        models = client.get(args.server + '/v1/models')
        models.raise_for_status()
        served = next(m for m in models.json()['data'] if m['id'] == manifest['model_profile']['model'])
        if max(len(unrelated), len(prefix)) + 1 > served['max_model_len']:
            raise ValueError('Cache-pressure request exceeds server context limit')
        for label, tokens in [('cache_pressure', unrelated), ('prefix_reload', prefix)]:
            before = counters(client, args.server + '/metrics', native)
            before.update(counters(client, args.lmcache + '/metrics', external))
            print(label, len(tokens), 'tokens', flush=True)
            started = time.monotonic()
            response = client.post(args.server + '/v1/completions', json={
                'model': served['id'], 'prompt': tokens, 'max_tokens': 1,
                'temperature': 0, 'add_special_tokens': False})
            response.raise_for_status()
            after = counters(client, args.server + '/metrics', native)
            after.update(counters(client, args.lmcache + '/metrics', external))
            entry = {'stage': label, 'seconds': round(time.monotonic() - started, 3),
                     'usage': response.json().get('usage'),
                     'metric_deltas': {k: after[k] - before[k] for k in before}}
            receipt['requests'].append(entry)
            args.receipt.write_text(json.dumps(receipt, indent=2) + '\n')
            print(json.dumps(entry), flush=True)
    delta = receipt['requests'][-1]['metric_deltas']
    receipt['external_prefix_reload_verified'] = (
        delta['vllm:external_prefix_cache_hits_total'] > 0
        and delta['lmcache_mp_lookup_hit_l1_tokens_total'] > 0
        and delta['lmcache_mp_num_finished_retrieves_total'] > 0)
    args.receipt.write_text(json.dumps(receipt, indent=2) + '\n')
    if not receipt['external_prefix_reload_verified']:
        raise RuntimeError('External prefix reload was not observed; inspect receipt and logs')
    print('External prefix reload verified.', flush=True)


if __name__ == '__main__':
    main()
