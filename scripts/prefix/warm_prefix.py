#!/usr/bin/env python3
"""Prefill an exported prefix on an existing vLLM endpoint; verify native reuse.

Renders the exported template locally and sends token IDs to /v1/completions.
Does not change the server template, restart services, or reset caches.
"""
import argparse
import hashlib
import json
import re
import time
from pathlib import Path

import httpx
from transformers import AutoTokenizer


def main():
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument('--bundle', required=True, type=Path)
    ap.add_argument('--tokenizer', required=True, type=Path)
    ap.add_argument('--server', required=True)
    ap.add_argument('--receipt', required=True, type=Path)
    args = ap.parse_args()
    bundle = args.bundle.resolve()
    manifest = json.loads((bundle / 'prefix_manifest.json').read_text())
    template = (bundle / 'vllm_prefix_template.jinja').read_text()
    reference = (bundle / 'context_100k.md').read_text()
    for content, expected in ((template, manifest['template_sha256']),
                              (reference, manifest['sha256'])):
        if hashlib.sha256(content.encode()).hexdigest() != expected:
            raise ValueError('Bundle hash mismatch')
    for name, expected in manifest['model_profile']['tokenizer_files_sha256'].items():
        if hashlib.sha256((args.tokenizer / name).read_bytes()).hexdigest() != expected:
            raise ValueError('Tokenizer hash mismatch: ' + name)
    tok = AutoTokenizer.from_pretrained(args.tokenizer, local_files_only=True)
    prompts = [tok.encode(tok.apply_chat_template(
        [{'role': 'user', 'content': task}], chat_template=template,
        tokenize=False, add_generation_prompt=True, enable_thinking=False),
        add_special_tokens=False)
        for task in ('Warm engineering reference. Reply OK.',
                     'Confirm engineering reference warm-up.')]
    common = 0
    for left, right in zip(*prompts):
        if left != right:
            break
        common += 1
    if common < manifest['shared_prefix_tokens']:
        raise ValueError('Warm-up prompts do not share the verified prefix')
    if not tok.decode(prompts[0], skip_special_tokens=False).startswith(
            '<|im_start|>system\n' + reference):
        raise ValueError('Rendered reference is not the leading system prefix')
    receipt = {'server': args.server, 'generation': manifest['generation'],
               'shared_prefix_tokens': common, 'requests': [],
               'automatic_server_injection_configured': False,
               'external_cache_replay_verified': False}
    with httpx.Client(timeout=httpx.Timeout(1800, connect=10)) as client:
        base = args.server.rstrip('/')
        models = client.get(base + '/v1/models')
        models.raise_for_status()
        served = next(m for m in models.json()['data']
                      if m['id'] == manifest['model_profile']['model'])
        if max(map(len, prompts)) + 1 > served['max_model_len']:
            raise ValueError('Prefix exceeds serving context limit')

        def metrics():
            response = client.get(base + '/metrics')
            response.raise_for_status()
            names = ('prefix_cache_hits_total', 'prefix_cache_queries_total',
                     'external_prefix_cache_hits_total')
            return {name: sum(float(value) for value in re.findall(
                r'^vllm:' + name + r'(?:\{[^\n]*\})?\s+([0-9.eE+-]+)',
                response.text, re.M)) for name in names}

        for index, prompt in enumerate(prompts):
            before = metrics()
            print(f'Request {index + 1}: prefilling {len(prompt):,} tokens', flush=True)
            started = time.monotonic()
            response = client.post(base + '/v1/completions', json={
                'model': served['id'], 'prompt': prompt, 'max_tokens': 1,
                'temperature': 0, 'add_special_tokens': False})
            response.raise_for_status()
            after = metrics()
            entry = {'seconds': round(time.monotonic() - started, 3),
                     'usage': response.json().get('usage'),
                     'metric_deltas': {name: after[name] - before[name] for name in before}}
            receipt['requests'].append(entry)
            args.receipt.write_text(json.dumps(receipt, indent=2) + '\n')
            print(json.dumps(entry), flush=True)
    receipt['native_hybrid_cache_reuse_verified'] = (
        receipt['requests'][1]['metric_deltas']['prefix_cache_hits_total'] > 0)
    args.receipt.write_text(json.dumps(receipt, indent=2) + '\n')
    if not receipt['native_hybrid_cache_reuse_verified']:
        raise RuntimeError('Prefill completed but native cache reuse was not observed')
    print('Prefill and native cache reuse verified.', flush=True)


if __name__ == '__main__':
    main()
