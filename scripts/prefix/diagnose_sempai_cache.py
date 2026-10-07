#!/usr/bin/env python3
"""Compare identical generation across native reuse and namespace eviction.

Makes serial API requests only; does not change services or reset caches.
Cache statistics establish whether eviction actually occurred.
"""
import argparse
import hashlib
import json
import time
import urllib.error
import urllib.request
from pathlib import Path


def run(server, bundle, salt, output, structured=False):
    if output.exists():
        raise ValueError('Preserve existing evidence; choose a new output path')
    manifest = json.loads((bundle / 'prefix_manifest.json').read_text())
    payload = {'model': manifest['model_profile']['model'],
               'messages': [{'role': 'user', 'content': 'Reply READY.'}],
               'chat_template_kwargs': {'enable_thinking': False},
               'temperature': 0, 'max_tokens': 8,
               'kv_transfer_params': {'cached_token_stats': True}}
    if structured:
        payload['messages'][0]['content'] = 'Return exactly {"status":"READY"}.'
        payload['max_tokens'] = 16
        payload['structured_outputs'] = {'grammar': 'root ::= "{\\\"status\\\":\\\"READY\\\"}"'}
    record = {'generation': manifest['generation'], 'requests': [],
              'service_settings_changed': False, 'structured': structured}
    output.parent.mkdir(parents=True, exist_ok=True)

    def call(path, data):
        req = urllib.request.Request(server + path, data=json.dumps(data).encode(),
                                     headers={'Content-Type': 'application/json'})
        with urllib.request.urlopen(req, timeout=1800) as response:
            return json.load(response)

    tokens = call('/tokenize', payload)['tokens']
    common = manifest['shared_prefix_tokens']
    digest = hashlib.sha256(json.dumps(tokens[:common], separators=(',', ':')).encode()).hexdigest()
    if digest != manifest['shared_prefix_token_sha256']:
        raise ValueError('Unexpected injected prefix')
    block = manifest['model_profile']['cache_block_tokens']
    if (len(tokens) + payload['max_tokens']) // block * block != manifest['cacheable_shared_tokens']:
        raise ValueError('Probe may cross the static recurrent checkpoint')
    record['prefix_fingerprint_verified'] = True
    record['input_tokens'] = len(tokens)
    for label, namespace in [('initial', salt), ('native', salt),
                             ('displacement', salt + '-displacement'),
                             ('after_displacement', salt), ('repeat', salt)]:
        data = dict(payload, cache_salt=namespace)
        started = time.monotonic()
        entry = {'label': label, 'request': data}
        try:
            entry['response'] = call('/v1/chat/completions', data)
        except urllib.error.HTTPError as error:
            entry['http_error'] = error.code
            entry['body'] = error.read().decode(errors='replace')
        entry['seconds'] = round(time.monotonic() - started, 3)
        record['requests'].append(entry)
        output.write_text(json.dumps(record, indent=2) + '\n')
        response = entry.get('response', {})
        print(json.dumps({'label': label, 'seconds': entry['seconds'],
                          'choices': response.get('choices'),
                          'stats': response.get('kv_transfer_params'),
                          'http_error': entry.get('http_error')}), flush=True)
    restored = record['requests'][3]
    stats = restored.get('response', {}).get('kv_transfer_params', {}).get('cached_token_stats', {})
    record['external_restore_exercised'] = (
        stats.get('num_vllm_cached_tokens') == 0 and
        stats.get('num_lmcache_extra_cached_tokens', 0) >= manifest['cacheable_shared_tokens'])
    def correct(entry):
        choices = entry.get('response', {}).get('choices', [])
        if len(choices) != 1 or choices[0].get('finish_reason') != 'stop':
            return False
        content = choices[0]['message'].get('content', '')
        if structured:
            try:
                return json.loads(content) == {'status': 'READY'}
            except (ValueError, TypeError):
                return False
        return content.strip().rstrip('.') == 'READY'
    record['all_answers_correct'] = all(correct(entry) for entry in record['requests'])
    record['external_restore_check_passed'] = record['external_restore_exercised'] and record['all_answers_correct']
    record['scope'] = 'Short sentinel and cache-path check; not a model capability benchmark'
    output.write_text(json.dumps(record, indent=2) + '\n')
    print(json.dumps({key: record[key] for key in ('external_restore_exercised', 'all_answers_correct',
                                                   'external_restore_check_passed')}), flush=True)


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--server', required=True)
    parser.add_argument('--bundle', type=Path, required=True)
    parser.add_argument('--cache-salt', required=True)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--structured', action='store_true')
    args = parser.parse_args()
    run(args.server.rstrip('/'), args.bundle, args.cache_salt, args.output, args.structured)
