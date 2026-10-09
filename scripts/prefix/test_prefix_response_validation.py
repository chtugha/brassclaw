"""Negative evidence tests and real installed-HA schema regressions."""
import copy
import os
import unittest
import hashlib
import json
from pathlib import Path
import tempfile

from prefix_response_validation import check_citations, check_configuration, resolve_card, materialize_quotes, load_bundle


class ValidationTests(unittest.TestCase):
    def test_view_manifest_cannot_hide_changed_original_evidence(self):
        sha = lambda b: hashlib.sha256(b).hexdigest()
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            card = {'repo':'test','path':'source.md','document_sha256':'a'*64,
                    'line_start':1,'line_end':1,'excerpt':'original source',
                    'parent_headings':'','status':'exact-source-excerpt'}
            card['id'] = sha(json.dumps([card[k] for k in
                ('repo','path','document_sha256','line_start','line_end')],ensure_ascii=False).encode())
            card['excerpt_sha256'] = sha(card['excerpt'].encode())
            card['context_sha256'] = sha(b'')
            raw = (json.dumps(card)+'\n').encode()
            view = {'cards':[{'id':card['id'],'original_excerpt_sha256':card['excerpt_sha256']}]}
            view_raw = json.dumps(view).encode()
            files = {'evidence_cards.jsonl':raw,'context_100k.md':b'context',
                     'vllm_prefix_template.jinja':b'template','evidence_view_manifest.json':view_raw}
            for name, value in files.items(): (root/name).write_bytes(value)
            manifest = {'evidence_cards_sha256':sha(raw),'sha256':sha(b'context'),
                        'template_sha256':sha(b'template'),'selected_card_ids':[card['id']],
                        'evidence_view_manifest_sha256':sha(view_raw)}
            (root/'prefix_manifest.json').write_text(json.dumps(manifest))
            load_bundle(root)
            (root/'evidence_view_manifest.json').write_text('{}')
            with self.assertRaisesRegex(ValueError,'view manifest fingerprint'):
                load_bundle(root)
            view['cards'][0]['original_excerpt_sha256'] = 'b'*64
            view_raw = json.dumps(view).encode()
            (root/'evidence_view_manifest.json').write_bytes(view_raw)
            manifest['evidence_view_manifest_sha256'] = sha(view_raw)
            (root/'prefix_manifest.json').write_text(json.dumps(manifest))
            with self.assertRaisesRegex(ValueError,'original source selection'):
                load_bundle(root)

    def setUp(self):
        self.identity = 'a' * 64
        self.cards = {self.identity: {'id': self.identity, 'excerpt': 'The server MUST NOT discard session state while the connection is open.',
            'parent_headings': '# MQTT Session State', 'source': {'url': 'https://docs.oasis-open.org/mqtt/mqtt/v5.0/os/mqtt-v5.0-os.html'},
            'path': 'mqtt.md', 'version': '5.0', 'line_start': 1, 'line_end': 2}}
        self.answer = {'claims': [{'text': 'An open connection preserves session state.', 'citations': [{
            'card_id': self.identity, 'quote': 'MUST NOT discard session state while the connection is open.'}]}],
            'configuration': '', 'missing_information': []}

    def test_exact_quote_and_generated_source_metadata(self):
        result = check_citations(self.answer, self.cards)
        self.assertEqual(result['status'], 'passed')
        self.assertEqual(result['sources'][0]['url'], self.cards[self.identity]['source']['url'])

    def test_reject_forged_quote_unknown_id_and_uncited_claim(self):
        for update in ({'quote': 'The server may discard everything at any time.'}, {'card_id': 'b'*64}):
            answer = copy.deepcopy(self.answer)
            answer['claims'][0]['citations'][0].update(update)
            self.assertEqual(check_citations(answer, self.cards)['status'], 'failed')
        self.answer['claims'][0]['citations'] = []
        self.assertEqual(check_citations(self.answer, self.cards)['status'], 'failed')

    def test_real_but_unrelated_quote_is_rejected(self):
        self.answer['claims'][0]['text'] = 'The router firewall allows incoming SSH packets.'
        self.assertEqual(check_citations(self.answer, self.cards)['status'], 'failed')

    def test_ambiguous_short_ids_and_unsolicited_yaml(self):
        duplicate = copy.deepcopy(self.cards[self.identity]); duplicate['id'] = 'a'*12+'b'*52
        self.cards[duplicate['id']] = duplicate
        with self.assertRaises(ValueError):
            resolve_card('a'*12, self.cards)
        self.assertTrue(check_configuration('modbus: []', 'none')['errors'])

    def test_failed_checker_does_not_claim_success(self):
        checks = check_configuration('modbus: []', 'ha-modbus', '/nonexistent/python', '2026.2.3')
        self.assertEqual(checks['application_schema'], 'not_checked')
        self.assertTrue(checks['errors'])
        self.assertTrue(check_configuration([], 'ha-modbus')['errors'])

    def test_quote_selection_copies_source_and_rejects_invalid_index(self):
        answer = copy.deepcopy(self.answer)
        citation = answer['claims'][0]['citations'][0]
        citation.pop('quote'); citation['quote_index'] = 0
        converted = materialize_quotes(answer, self.cards)
        self.assertEqual(converted['claims'][0]['citations'][0]['quote'], self.cards[self.identity]['excerpt'])
        citation.pop('quote'); citation['quote_index'] = 99
        with self.assertRaises(ValueError):
            materialize_quotes(answer, self.cards)

    @unittest.skipUnless(os.getenv('PREFIX_TEST_HA_PYTHON'), 'Set PREFIX_TEST_HA_PYTHON for real HA schemas')
    def test_real_schema_and_safe_yaml_regressions(self):
        python = os.environ['PREFIX_TEST_HA_PYTHON']
        version = os.environ['PREFIX_TEST_HA_VERSION']
        cases = [
            ('modbus:\n  - name: example\n    type: tcp\n    host: 192.0.2.1\n    port: 502\n', 'ha-modbus', 'passed', 'passed'),
            ('modbus:\n  - type: rtu\n    host: 192.0.2.1\n    port: 502\n', 'ha-modbus', 'passed', 'failed'),
            ('alias: Wait\nsequence:\n  - delay: "00:00:01"\n', 'ha-script', 'passed', 'passed'),
            ('alias: Broken\nsequence: [42]\n', 'ha-script', 'passed', 'failed'),
            ('alias: Periodic\ntriggers:\n  - trigger: time_pattern\n    minutes: "/5"\nactions:\n  - delay: "00:00:01"\n', 'ha-automation', 'passed', 'passed'),
            ('alias: Broken\nactions: []\n', 'ha-automation', 'passed', 'failed'),
            ('modbus: []\nmodbus: []\n', 'ha-modbus', 'failed', 'not_checked'),
            ('modbus: [\n', 'ha-modbus', 'failed', 'not_checked'),
            ('x: !!python/object/apply:os.system ["touch /tmp/unsafe-prefix-check"]\n', 'ha-configuration', 'failed', 'not_checked'),
            ('script: !include scripts.yaml\n', 'ha-configuration', 'not_checked', 'not_checked'),
            ('unsupported_domain: {}\n', 'ha-configuration', 'passed', 'not_checked'),
        ]
        for text, kind, syntax, schema in cases:
            with self.subTest(kind=kind, text=text):
                result = check_configuration(text, kind, python, version)
                self.assertEqual(result['syntax'], syntax, result)
                self.assertEqual(result['application_schema'], schema, result)
                self.assertEqual(result['runtime'], 'not_checked')
        wrong = check_configuration('modbus: []', 'ha-modbus', python, '0.0.0')
        self.assertEqual(wrong['application_schema'], 'not_checked')
        self.assertTrue(wrong['errors'])


if __name__ == '__main__':
    unittest.main()
