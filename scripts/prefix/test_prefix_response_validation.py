"""Negative evidence tests and real installed-HA schema regressions."""
import copy
import os
import unittest

from prefix_response_validation import check_citations, check_configuration, resolve_card, validated_generate


class ValidationTests(unittest.TestCase):
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
