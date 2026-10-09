#!/usr/bin/env python3
"""Preserve an identical task across cold, native and external hybrid cache paths.

No service edits or cache-reset API calls. A different namespace displaces the
native cache; telemetry must prove whether external restoration actually happened.
Behavioral scoring belongs to the task's independent validator, not text equality.
"""
import argparse
import hashlib
import json
import time
import urllib.request
from pathlib import Path

from prefix_response_validation import load_bundle


def run(server, bundle, request_file, salt, output):
    if output.exists():
        raise ValueError('Choose a new evidence path; existing results are immutable')
    manifest, _ = load_bundle(bundle)
    payload = json.loads(request_file.read_text())
    if payload['model'] != manifest['model_profile']['model']:
        raise ValueError('Request model differs from bundle')
    payload.pop('cache_salt', None)
    payload['kv_transfer_params'] = dict(payload.get('kv_transfer_params') or {}, cached_token_stats=True)
    sentinel = {'model': payload['model'], 'messages': [{'role': 'user', 'content': 'Reply READY.'}],
                'temperature': 0, 'max_tokens': 8,
                'chat_template_kwargs': {'enable_thinking': False},
                'kv_transfer_params': {'cached_token_stats': True}}
    record = {'generation': manifest['generation'],
              'request_sha256': hashlib.sha256(request_file.read_bytes()).hexdigest(),
              'service_settings_changed': False, 'requests': [],
              'scope': 'Paired task outputs; text equality is not behavioral correctness'}
    output.parent.mkdir(parents=True, exist_ok=True)

    def call(path, data):
        request = urllib.request.Request(server + path, data=json.dumps(data).encode(),
                                         headers={'Content-Type': 'application/json'})
        with urllib.request.urlopen(request, timeout=1800) as response:
            return json.load(response)

    common = manifest['shared_prefix_tokens']
    for label in ('cold', 'native', 'displacement', 'external', 'restored_native'):
        data = dict(sentinel if label == 'displacement' else payload,
                    cache_salt=salt + '-displacement' if label == 'displacement' else salt)
        tokens = call('/tokenize', data)['tokens']
        digest = hashlib.sha256(json.dumps(tokens[:common], separators=(',', ':')).encode()).hexdigest()
        if digest != manifest['shared_prefix_token_sha256']:
            raise ValueError('Active injected prefix changed before ' + label)
        if len(tokens) + data['max_tokens'] > manifest['model_profile']['max_model_len']:
            raise ValueError('Request exceeds model context')
        entry = {'label': label, 'request': data, 'input_tokens': len(tokens),
                 'prefix_fingerprint_verified': True}
        record['requests'].append(entry)
        output.write_text(json.dumps(record, indent=2) + '\n')
        started = time.monotonic()
        entry['response'] = call('/v1/chat/completions', data)
        entry['seconds'] = round(time.monotonic() - started, 3)
        stats = entry['response'].get('kv_transfer_params', {}).get('cached_token_stats', {})
        print(json.dumps({'label': label, 'seconds': entry['seconds'], 'stats': stats,
                          'finish_reasons': [c.get('finish_reason') for c in entry['response'].get('choices', [])]}), flush=True)
        output.write_text(json.dumps(record, indent=2) + '\n')
    def stats(label):
        return next(e for e in record['requests'] if e['label'] == label)['response'].get(
            'kv_transfer_params', {}).get('cached_token_stats', {})
    cold, native, external = (stats(label) for label in ('cold', 'native', 'external'))
    root = manifest['cacheable_shared_tokens']
    record['cache_paths_verified'] = {
        'cold': cold.get('num_vllm_cached_tokens') == 0 and cold.get('num_lmcache_extra_cached_tokens') == 0,
        'native': native.get('num_vllm_cached_tokens', 0) >= root,
        'external': external.get('num_vllm_cached_tokens') == 0 and external.get('num_lmcache_extra_cached_tokens', 0) >= root}
    output.write_text(json.dumps(record, indent=2) + '\n')
    print(json.dumps(record['cache_paths_verified']), flush=True)


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--server', required=True)
    parser.add_argument('--bundle', type=Path, required=True)
    parser.add_argument('--request-file', type=Path, required=True)
    parser.add_argument('--cache-salt', required=True)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    run(args.server.rstrip('/'), args.bundle, args.request_file, args.cache_salt, args.output)
