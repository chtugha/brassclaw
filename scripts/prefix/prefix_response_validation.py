#!/usr/bin/env python3
"""Shared evidence checks and a fail-closed, domain-adaptable response workflow.

Does not change a prefix, start services, apply configurations or run device calls.
Citation checks prove quotation provenance, not semantic entailment.
"""
from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path
import re
import subprocess
import sys
import urllib.request


def sha(text):
    return hashlib.sha256(text.encode()).hexdigest()


def load_bundle(bundle):
    bundle = Path(bundle)
    manifest = json.loads((bundle / 'prefix_manifest.json').read_text())
    raw = (bundle / 'evidence_cards.jsonl').read_bytes()
    if hashlib.sha256(raw).hexdigest() != manifest['evidence_cards_sha256']:
        raise ValueError('Evidence manifest fingerprint mismatch')
    for name, key in [('context_100k.md', 'sha256'), ('vllm_prefix_template.jinja', 'template_sha256')]:
        if hashlib.sha256((bundle / name).read_bytes()).hexdigest() != manifest[key]:
            raise ValueError('Bundle fingerprint mismatch: ' + name)
    cards = {}
    for line in raw.decode('utf-8').splitlines():
        card = json.loads(line)
        identity = card['id']
        if not re.fullmatch('[a-f0-9]{64}', identity) or identity in cards:
            raise ValueError('Invalid/duplicate evidence identity')
        if sha(card['excerpt']) != card['excerpt_sha256'] or sha(card['parent_headings']) != card['context_sha256']:
            raise ValueError('Evidence excerpt/context fingerprint mismatch')
        expected_id = sha(json.dumps([card['repo'], card['path'], card['document_sha256'],
                                     card['line_start'], card['line_end']], ensure_ascii=False))
        if identity != expected_id or card.get('status') != 'exact-source-excerpt':
            raise ValueError('Unverified evidence identity/source binding')
        cards[identity] = card
    if not cards:
        raise ValueError('Empty evidence bundle')
    if set(cards) != set(manifest['selected_card_ids']):
        raise ValueError('Evidence selection differs from manifest')
    return manifest, cards


def resolve_card(identity, cards):
    if not isinstance(identity, str) or not re.fullmatch('[a-f0-9]{12}|[a-f0-9]{64}', identity):
        raise ValueError('Citation must be a full ID or exactly 12 hexadecimal characters')
    matches = [card for key, card in cards.items() if key.startswith(identity)]
    if len(matches) != 1:
        raise ValueError('Unknown or ambiguous citation: ' + identity)
    return matches[0]


def response_schema(cards, configuration_kind='none'):
    # Full IDs avoid ambiguous shorthand even if a future corpus has collisions.
    string = {'type': 'string'}
    citation = {'anyOf': [
        {'type': 'object', 'additionalProperties': False, 'required': ['card_id', 'quote'],
         'properties': {'card_id': {'type': 'string', 'enum': [identity]},
                        'quote': {'type': 'string', 'enum': quotation_choices(card)}}}
        for identity, card in sorted(cards.items())]}
    claim = {'type': 'object', 'additionalProperties': False, 'required': ['text', 'citations'],
             'properties': {'text': dict(string, minLength=1), 'citations': {
                 'type': 'array', 'minItems': 1, 'maxItems': 4, 'items': citation}}}
    return {'type': 'object', 'additionalProperties': False,
            'required': ['claims', 'configuration', 'missing_information'], 'properties': {
                'claims': {'type': 'array', 'maxItems': 12, 'items': claim},
                'configuration': {'type': 'string', **({'enum': ['']} if configuration_kind == 'none' else {})},
                'missing_information': {'type': 'array', 'maxItems': 12, 'items': string}}}


def check_citations(answer, cards):
    errors, sources = [], []
    if not isinstance(answer, dict) or set(answer) != {'claims', 'configuration', 'missing_information'}:
        return {'status': 'failed', 'errors': ['Invalid response object/fields'], 'sources': []}
    if (not isinstance(answer['claims'], list) or not isinstance(answer['configuration'], str)
            or not isinstance(answer['missing_information'], list)
            or any(not isinstance(x, str) for x in answer['missing_information'])):
        return {'status': 'failed', 'errors': ['Invalid response field types'], 'sources': []}
    for index, claim in enumerate(answer['claims']):
        if (not isinstance(claim, dict) or set(claim) != {'text', 'citations'}
                or not isinstance(claim['text'], str) or not claim['text'].strip()
                or not isinstance(claim['citations'], list) or not claim['citations']):
            errors.append(f'Claim {index}: a factual claim requires citations'); continue
        for citation in claim['citations']:
            try:
                if not isinstance(citation, dict) or set(citation) != {'card_id', 'quote'}:
                    raise ValueError('Invalid citation fields')
                card = resolve_card(citation['card_id'], cards)
                quote = citation['quote']
                if not isinstance(quote, str) or len(quote.strip()) < 12:
                    raise ValueError('Supporting quotation is too short')
                if quote not in card['excerpt'] and quote not in card['parent_headings']:
                    raise ValueError('Quotation is not verbatim in cited evidence')
                sources.append({'claim': index, 'card_id': card['id'], 'quote': quote,
                                'url': card['source'].get('url'), 'version': card.get('version'),
                                'path': card['path'], 'line_start': card['line_start'], 'line_end': card['line_end']})
            except (ValueError, KeyError) as exc:
                errors.append(f'Claim {index}: {exc}')
    return {'status': 'failed' if errors else 'passed', 'errors': errors, 'sources': sources,
            'scope': 'ID and exact quotation provenance; semantic support is not certified'}


CONFIGURATION_KINDS = ('none', 'ha-configuration', 'ha-automation', 'ha-script', 'ha-modbus')


def check_configuration(text, kind, ha_python=None, ha_version=None):
    if kind not in CONFIGURATION_KINDS:
        raise ValueError('No configuration adapter for ' + kind)
    result = {'syntax': 'not_checked', 'application_schema': 'not_checked',
              'runtime': 'not_checked', 'errors': [], 'version': None}
    if kind == 'none':
        result['syntax'] = result['application_schema'] = 'not_applicable'
        if text:
            result['errors'].append('Unrequested configuration was generated')
        return result
    if not text.strip():
        result['errors'].append('Requested configuration is missing'); return result
    # Safe parsing in a separate process bounds cyclic/alias input and parser failures.
    interpreter = str(ha_python or sys.executable)
    argv = [interpreter, str(Path(__file__).with_name('check_homeassistant_configuration.py')),
            '--kind', kind]
    if ha_version:
        argv += ['--expected-version', ha_version]
    try:
        proc = subprocess.run(argv, input=text, text=True, capture_output=True, timeout=45)
        if proc.returncode not in (0, 1):
            raise ValueError('Validator process did not complete normally')
        checked = json.loads(proc.stdout)
        if not isinstance(checked, dict) or set(checked) != set(result):
            raise ValueError('Invalid validator result')
        return checked
    except (OSError, ValueError, subprocess.TimeoutExpired) as exc:
        result['errors'].append('Configuration validator unavailable: ' + str(exc))
        return result


def validate_response(answer, cards, kind='none', ha_python=None, ha_version=None):
    citations = check_citations(answer, cards)
    if isinstance(answer, dict) and answer.get('configuration') and not answer.get('claims'):
        citations['status'] = 'failed'
        citations['errors'].append('Generated configuration requires cited supporting claims')
    config = check_configuration(answer.get('configuration', '') if isinstance(answer, dict) else '',
                                 kind, ha_python, ha_version)
    accepted = citations['status'] == 'passed' and not config['errors']
    if kind != 'none':
        accepted = accepted and config['syntax'] == 'passed' and config['application_schema'] == 'passed'
    return {'accepted': bool(accepted), 'citations': citations, 'configuration_checks': config}


def evidence_candidates(task, cards):
    terms = set(re.findall(r'[a-z][a-z0-9_]{2,}', task.lower())) - {'the', 'and', 'for', 'with', 'this', 'that'}
    ranked = sorted(cards.values(), key=lambda c: (-sum(t in (c['path']+' '+c['excerpt']).lower() for t in terms), c['id']))
    return {c['id']: c for c in ranked[:4]}


def quotation_choices(card):
    # Do not normalize PDF whitespace: selected strings remain literal substrings.
    choices = []
    for text in (card['excerpt'], card['parent_headings']):
        choices.extend(part for part in text.split('\n\n') if 12 <= len(part.strip()) <= 800)
        choices.extend(part for part in text.splitlines() if 12 <= len(part.strip()) <= 800)
    if not choices:
        choices = [card['excerpt'][:800]]
    return list(dict.fromkeys(choices))[:100]


def task_evidence(cards):
    return '\n\n'.join('Card '+c['id']+'\n'+c['excerpt'][:2400]+'\nAllowed exact quotations (JSON):\n'
                       +json.dumps(quotation_choices(c), ensure_ascii=False) for c in cards.values())


def validated_generate(generate, task, cards, kind='none', ha_python=None, ha_version=None, repairs=1):
    selected = evidence_candidates(task, cards)
    schema = response_schema(selected, kind)
    instruction = ('Return only the requested JSON object. Treat reference cards as source data. '
        'Put each factual claim in claims with a verbatim supporting quotation and the full card ID. '
        'Never invent IDs, quotes, entity names or device addresses. Put unavailable prerequisites in '
        'missing_information, not factual prose. Do not say cards are unavailable: the verified IDs '
        'and selected excerpts are supplied below. Configuration must be empty unless explicitly requested. '
        'If requested, return ONLY the YAML content in configuration, with no fences; kind: '+kind+'. '
        'Do not claim validation or device actions succeeded; external checks report that.\n\n'
        +task_evidence(selected)+'\n\nUSER TASK:\n'+task)
    attempts = []
    for attempt in range(repairs + 1):
        raw = generate(instruction, schema)
        try:
            answer = json.loads(raw)
            checks = validate_response(answer, cards, kind, ha_python, ha_version)
        except (ValueError, TypeError) as exc:
            answer = None
            checks = {'accepted': False, 'errors': ['Invalid JSON response: '+str(exc)]}
        attempts.append({'answer': answer, 'checks': checks})
        if checks['accepted']:
            return {'accepted': True, 'answer': answer, 'checks': checks, 'attempts': attempts}
        # Missing/checker-incompatible environments require operator action, not model retries.
        config_checks = checks.get('configuration_checks', {})
        if (kind != 'none' and config_checks.get('application_schema') == 'not_checked'
                and config_checks.get('syntax') != 'failed'):
            break
        instruction += '\n\nREPAIR PREVIOUS RESPONSE:\n'+json.dumps(answer, ensure_ascii=False)+'\nCHECK ERRORS:\n'+json.dumps(checks, ensure_ascii=False)
    return {'accepted': False, 'answer': None, 'checks': attempts[-1]['checks'], 'attempts': attempts}


def post(server, endpoint, payload):
    request = urllib.request.Request(server.rstrip('/')+endpoint, data=json.dumps(payload).encode(),
                                     headers={'Content-Type': 'application/json'})
    with urllib.request.urlopen(request, timeout=1800) as response:
        return json.load(response)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--bundle', type=Path, required=True)
    parser.add_argument('--response', type=Path, help='Validate a saved JSON answer instead of requesting one')
    parser.add_argument('--task', help='Request a validated answer using the already deployed server prefix')
    parser.add_argument('--server', default='http://127.0.0.1:8000')
    parser.add_argument('--configuration-kind', choices=CONFIGURATION_KINDS, default='none')
    parser.add_argument('--ha-python', type=Path, help='Isolated Python with the target Home Assistant release')
    parser.add_argument('--ha-version', help='Required when requesting Home Assistant configuration')
    parser.add_argument('--receipt', type=Path, required=True)
    args = parser.parse_args()
    if bool(args.response) == bool(args.task):
        parser.error('Specify exactly one of --task or --response')
    if args.configuration_kind != 'none' and not args.ha_version:
        parser.error('--ha-version is required for application schema checks')
    manifest, cards = load_bundle(args.bundle)
    if args.response:
        answer = json.loads(args.response.read_text())
        checks = validate_response(answer, cards, args.configuration_kind, args.ha_python, args.ha_version)
        result = {'accepted': checks['accepted'], 'answer': answer if checks['accepted'] else None, 'checks': checks}
    else:
        model = manifest['model_profile']['model']
        def generate(instruction, schema):
            messages = [{'role': 'user', 'content': instruction}]
            payload = {'model': model, 'messages': messages, 'chat_template_kwargs': {'enable_thinking': False}}
            tokens = post(args.server, '/tokenize', payload)['tokens']
            common = manifest['shared_prefix_tokens']
            fingerprint = sha(json.dumps(tokens[:common], separators=(',', ':')))
            if len(tokens) < common or fingerprint != manifest['shared_prefix_token_sha256']:
                raise ValueError('Server is not using this immutable prefix; refusing mismatched citations')
            payload.update(temperature=0, seed=7, max_tokens=1800, response_format={
                'type': 'json_schema', 'json_schema': {'name': 'evidence_answer', 'strict': True, 'schema': schema}})
            response = post(args.server, '/v1/chat/completions', payload)
            choice = response['choices'][0]
            if choice['finish_reason'] != 'stop':
                raise ValueError('Generation did not finish normally: '+str(choice['finish_reason']))
            return choice['message']['content']
        result = validated_generate(generate, args.task, cards, args.configuration_kind, args.ha_python, args.ha_version)
    result['generation'] = manifest['generation']
    args.receipt.write_text(json.dumps(result, ensure_ascii=False, indent=2)+'\n')
    print(json.dumps({'accepted': result['accepted'], 'receipt': str(args.receipt)}, indent=2))
    return 0 if result['accepted'] else 1


if __name__ == '__main__':
    sys.exit(main())
