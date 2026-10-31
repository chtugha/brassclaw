"""Protocol/evidence negative cases; no claim of native dispatch qualification."""
import copy
import hashlib
import json
from pathlib import Path
import re
import unittest

import bootstrap_answer as bootstrap_module
from prefix_model_budget import plan_prefix_model_budget
bootstrap_module.plan_prefix_model_budget = plan_prefix_model_budget

from bootstrap_answer import (
    bootstrap_answer_stage, check_bootstrap_evidence, resolve_bootstrap_pointer,
    validate_bootstrap_answer,
)


class BootstrapAnswerTests(unittest.TestCase):
    def packet(self, field='context_window_tokens', value=32768):
        fact = {'field': field, 'scope': 'endpoint_configuration', 'status': 'proposed',
                'value': value, 'evidence': [{'source_id': 'captured-source',
                                            'locator': '/json/limit', 'quote': '32768'}]}
        answer = {'schema': 'brassclaw-bootstrap-answer/1', 'task': 'extract_bootstrap_facts',
                  'request_id': 'original-call', 'input_sha256': 'a' * 64,
                  'result': {'facts': [fact], 'issues': []}}
        return {'answer': answer, 'expected_task': answer['task'],
                'expected_request_id': answer['request_id'],
                'expected_input_sha256': answer['input_sha256'],
                'required_facts': [{'field': field, 'scope': fact['scope']}],
                'eligible_candidate_ids': []}

    def evidence(self, packet=None, view=None):
        packet = packet or self.packet()
        return {'fact': packet['answer']['result']['facts'][0],
                'source_views': {'captured-source': view or {'json': {'limit': 32768}}}}

    def test_exact_prompts_and_artifact_integrity(self):
        root = Path(__file__).resolve().parents[2]
        plan = (root / 'docs/plans/Exact_V3_definition_update_at_K3-2.md').read_text()
        section = plan.split('### 4.3 Wortlaut der Nachrichten', 1)[1].split('### 4.4', 1)[0]
        prompts = re.findall(r'```text\n(.*?)```', section, re.S)
        artifacts = Path(__file__).parent / 'bootstrap/v1'
        manifest = json.loads((artifacts / 'manifest.json').read_text())
        self.assertEqual(manifest['format'], 'brassclaw-bootstrap-prompts/1')
        names = ('system.txt', 'extract_bootstrap_facts.txt', 'select_bootstrap_prefill.txt')
        self.assertEqual(len(prompts), len(names))
        for name, prompt in zip(names, prompts):
            raw = (artifacts / name).read_bytes()
            self.assertEqual(raw.decode(), prompt)
            self.assertEqual(hashlib.sha256(raw).hexdigest(), manifest['artifacts'][name])

    def test_typed_answer_and_independent_return(self):
        packet = self.packet()
        result = validate_bootstrap_answer(packet)
        self.assertEqual(result, packet['answer'])
        result['result']['facts'][0]['value'] = 1
        self.assertEqual(packet['answer']['result']['facts'][0]['value'], 32768)
        self.assertEqual(check_bootstrap_evidence(self.evidence()), {'state': 'native_scalar_match'})

    def test_canonical_stage_envelope(self):
        data = json.dumps(self.packet(), ensure_ascii=False, sort_keys=True, separators=(',', ':'))
        result = bootstrap_answer_stage({'phase': 'answer', 'data_bytes': data})
        self.assertEqual(json.loads(result), self.packet()['answer'])
        evidence = json.dumps(self.evidence(), ensure_ascii=False, sort_keys=True, separators=(',', ':'))
        self.assertEqual(json.loads(bootstrap_answer_stage({'phase': 'evidence', 'data_bytes': evidence})),
                         {'state': 'native_scalar_match'})
        for phase, text in [('answer', ' ' + data), ('unknown', data),
                            ('answer', '{"answer":null,"answer":null}'),
                            ('answer', 'NaN')]:
            with self.subTest(phase=phase, text=text[:30]), self.assertRaises(ValueError):
                bootstrap_answer_stage({'phase': phase, 'data_bytes': text})

    def test_wrong_types_never_become_token_limits(self):
        for value in (True, False, '32768', 32768.0, 0, -1, None):
            with self.subTest(value=value), self.assertRaises(ValueError):
                validate_bootstrap_answer(self.packet(value=value))

    def test_extra_fields_and_wrong_echoes(self):
        for key, value in [('extra', None), ('request_id', 'other'),
                           ('input_sha256', 'b' * 64), ('schema', 'future/2'),
                           ('task', 'select_bootstrap_prefill')]:
            packet = self.packet()
            packet['answer'][key] = value
            with self.subTest(key=key), self.assertRaises(ValueError):
                validate_bootstrap_answer(packet)
        for object_path in ('result', 'fact', 'evidence'):
            packet = self.packet()
            obj = packet['answer']['result']
            if object_path != 'result':
                obj = obj['facts'][0]
            if object_path == 'evidence':
                obj = obj['evidence'][0]
            obj['extra'] = 'untrusted'
            with self.subTest(path=object_path), self.assertRaises(ValueError):
                validate_bootstrap_answer(packet)

    def test_exact_requirement_coverage(self):
        for facts in ([], [self.packet()['answer']['result']['facts'][0]] * 2):
            packet = self.packet()
            packet['answer']['result']['facts'] = facts
            with self.assertRaises(ValueError):
                validate_bootstrap_answer(packet)

    def test_unknown_is_valid_without_becoming_zero(self):
        packet = self.packet()
        fact = packet['answer']['result']['facts'][0]
        fact.update(status='unknown', value=None, evidence=[])
        validate_bootstrap_answer(packet)
        self.assertEqual(check_bootstrap_evidence(self.evidence(packet)), {'state': 'unknown'})
        fact['value'] = 0
        with self.assertRaises(ValueError):
            validate_bootstrap_answer(packet)

    def test_issues_are_closed_and_unique(self):
        for issues in (['invented'], ['missing_evidence'] * 2, 'missing_evidence', [None]):
            packet = self.packet()
            packet['answer']['result']['issues'] = issues
            with self.subTest(issues=issues), self.assertRaises(ValueError):
                validate_bootstrap_answer(packet)

    def test_scalar_mismatch_quote_and_unknown_source(self):
        for change in ('value', 'quote', 'source'):
            data = self.evidence()
            if change == 'value':
                data['fact']['value'] = 16384
            elif change == 'quote':
                data['fact']['evidence'][0]['quote'] = '32_768'
            else:
                data['fact']['evidence'][0]['source_id'] = 'invented'
            with self.subTest(change=change), self.assertRaises(ValueError):
                check_bootstrap_evidence(data)

    def test_json_boolean_does_not_match_integer(self):
        data = self.evidence(self.packet(field='supports_tools', value=True), {'json': {'limit': 1}})
        data['fact']['evidence'][0]['quote'] = '1'
        with self.assertRaises(ValueError):
            check_bootstrap_evidence(data)

    def test_text_quote_does_not_verify_semantics(self):
        data = self.evidence(view={'text': 'Untrusted documentation says 32768 tokens.'})
        data['fact']['evidence'][0]['locator'] = '/text'
        self.assertEqual(check_bootstrap_evidence(data), {'state': 'semantic_review_required'})
        data['fact']['evidence'][0]['quote'] = '65536'
        with self.assertRaises(ValueError):
            check_bootstrap_evidence(data)

    def test_native_identity_must_match(self):
        data = self.evidence(self.packet(field='template_identity', value='template-a'),
                             {'json': {'limit': 'template-a'}})
        data['fact']['evidence'][0]['quote'] = 'template-a'
        self.assertEqual(check_bootstrap_evidence(data), {'state': 'native_scalar_match'})
        data['fact']['value'] = 'template-b'
        with self.assertRaises(ValueError):
            check_bootstrap_evidence(data)

    def test_conflict_needs_distinct_values_not_just_two_quotes(self):
        data = self.evidence()
        fact = data['fact']
        fact.update(status='conflict', value=None)
        second = copy.deepcopy(fact['evidence'][0])
        second.update(locator='/json/other', quote='16384')
        fact['evidence'].append(second)
        data['source_views']['captured-source']['json']['other'] = 16384
        self.assertEqual(check_bootstrap_evidence(data), {'state': 'conflict'})
        second['quote'] = '32768'
        data['source_views']['captured-source']['json']['other'] = 32768
        self.assertEqual(check_bootstrap_evidence(data), {'state': 'semantic_review_required'})

    def test_pointers_escape_array_and_reject_containers(self):
        self.assertEqual(resolve_bootstrap_pointer({'json': {'a/b': {'~': [7]}}}, '/json/a~1b/~0/0'), 7)
        for pointer in ('https://server', '/json/a~2b', '/json/x/00', '/json/x/-',
                        '/json/x/9', '/json/x/' + '9' * 10000):
            with self.subTest(pointer=pointer[:40]), self.assertRaises(ValueError):
                resolve_bootstrap_pointer({'json': {'x': [1]}}, pointer)
        with self.assertRaises(ValueError):
            check_bootstrap_evidence(self.evidence(view={'json': {'limit': [32768]}}))

    def test_b2_returns_only_known_candidate_without_importing_values(self):
        packet = self.packet()
        packet['expected_task'] = packet['answer']['task'] = 'select_bootstrap_prefill'
        packet['eligible_candidate_ids'] = ['original-option']
        packet['answer']['result'] = {'candidate_id': 'original-option', 'issues': []}
        answer = validate_bootstrap_answer(packet)
        self.assertEqual(answer['result'], {'candidate_id': 'original-option', 'issues': []})
        packet['answer']['result']['candidate_id'] = 'invented-option'
        with self.assertRaises(ValueError):
            validate_bootstrap_answer(packet)
        packet['answer']['result']['candidate_id'] = None
        validate_bootstrap_answer(packet)


class BootstrapPrefillTests(unittest.TestCase):
    def packet(self):
        values = {'max_prefix_tokens': 1024, 'max_prompt_tokens': 2048,
                  'reserved_output_tokens': 1024, 'additional_framing_tokens': 0}
        context = {'field': 'context_window_tokens', 'scope': 'endpoint_configuration',
                   'status': 'independently_verified', 'value': 8192,
                   'evidence_ref': 'original-endpoint-observation'}
        required = {key: value for key, value in context.items() if key != 'status'}
        return {'decision': {'method': 'deterministic'},
                'policy_ref': 'original-provider-model-policy',
                'eligible_options': [{'candidate_id': 'original-option', 'rule_ref': 'retained-rule',
                                      'rule_revision': 'original-rule-revision',
                                      'values': values, 'requires': [required]}],
                'checked_observations': [context],
                'existing_values': {field: {'value': None, 'owner_revision': 'original-field-revision',
                                            'protection': 'replaceable'} for field in values}}

    def model(self, packet, selected='original-option'):
        protocol = {'expected_task': 'select_bootstrap_prefill', 'expected_request_id': 'original-call',
                    'expected_input_sha256': 'a' * 64, 'required_facts': [],
                    'eligible_candidate_ids': [option['candidate_id'] for option in packet['eligible_options']]}
        protocol['answer'] = {'schema': 'brassclaw-bootstrap-answer/1',
                              'task': protocol['expected_task'], 'request_id': protocol['expected_request_id'],
                              'input_sha256': protocol['expected_input_sha256'],
                              'result': {'candidate_id': selected, 'issues': []}}
        packet['decision'] = {'method': 'model', 'protocol': protocol}
        return packet

    def run_prefill(self, packet):
        return json.loads(bootstrap_answer_stage({'phase': 'prefill', 'data_bytes':
                          json.dumps(packet, ensure_ascii=False, sort_keys=True,
                                     separators=(',', ':'), allow_nan=False)}))

    def test_unique_option_skips_model_and_preserves_original_rule_and_revisions(self):
        packet = self.packet()
        result = self.run_prefill(packet)
        self.assertEqual(result['state'], 'candidate_checked')
        self.assertEqual(result['selection_method'], 'deterministic')
        self.assertIsNone(result['model_answer_echo'])
        self.assertEqual(result['candidate_id'], 'original-option')
        self.assertEqual(result['rule_revision'], 'original-rule-revision')
        self.assertEqual(result['budget']['effective_prefix_budget'], 1024)
        self.assertEqual(len(result['field_updates']), 4)
        self.assertEqual(len(result['field_preconditions']), 4)
        self.assertTrue(all(value['expected_owner_revision'] == 'original-field-revision'
                            for value in result['field_updates']))
        self.assertFalse(any(name in result for name in ('eligible_for_prefill', 'correlation_valid', 'accepted')))
        result['field_updates'][0]['value'] = 999
        self.assertEqual(packet['eligible_options'][0]['values']['max_prefix_tokens'], 1024)

    def test_model_selection_uses_original_values_and_no_candidate_is_not_default(self):
        result = self.run_prefill(self.model(self.packet()))
        self.assertEqual(result['selection_method'], 'model')
        self.assertEqual(result['model_answer_echo'], {'request_id': 'original-call', 'input_sha256': 'a' * 64})
        self.assertEqual(result['state'], 'candidate_checked')
        self.assertEqual(result['budget']['max_prefix_tokens'], 1024)
        result = self.run_prefill(self.model(self.packet(), None))
        self.assertEqual(result['state'], 'no_candidate')
        self.assertEqual(result['field_updates'], [])
        self.assertEqual(result['field_preconditions'], [])
        self.assertIsNone(result['candidate_id'])

    def test_protected_zero_is_preserved_and_fences_include_unchanged_manual_fields(self):
        packet = self.model(self.packet())
        packet['existing_values']['max_prefix_tokens'].update(value=0, protection='preserve')
        result = self.run_prefill(packet)
        self.assertEqual(result['state'], 'protected_existing_value')
        self.assertEqual(result['field_updates'], [])
        packet['existing_values']['max_prefix_tokens']['value'] = 1024
        result = self.run_prefill(packet)
        self.assertEqual(result['state'], 'candidate_checked')
        self.assertNotIn('max_prefix_tokens', [row['field'] for row in result['field_updates']])
        self.assertIn({'field': 'max_prefix_tokens', 'value': 1024,
                       'owner_revision': 'original-field-revision', 'protection': 'preserve'},
                      result['field_preconditions'])

    def test_zero_prefix_is_valid_and_reservations_are_not_silently_clamped(self):
        packet = self.model(self.packet())
        packet['eligible_options'][0]['values']['max_prefix_tokens'] = 0
        result = self.run_prefill(packet)
        self.assertEqual(result['state'], 'candidate_checked')
        self.assertEqual(result['budget']['status'], 'prefix-disabled')
        for field in ('max_prefix_tokens', 'max_prompt_tokens', 'reserved_output_tokens',
                      'additional_framing_tokens'):
            packet = self.model(self.packet())
            packet['eligible_options'][0]['values'][field] = 8192
            result = self.run_prefill(packet)
            self.assertEqual(result['state'], 'budget_not_fit')
            self.assertEqual(result['field_updates'], [])

    def test_output_bound_and_scope_cannot_be_replaced_by_model_declaration(self):
        packet = self.model(self.packet())
        packet['checked_observations'].append({'field': 'output_limit_tokens',
             'scope': 'endpoint_configuration', 'value': 512, 'status': 'independently_verified',
             'evidence_ref': 'original-endpoint-output-limit'})
        self.assertEqual(self.run_prefill(packet)['state'], 'budget_not_fit')
        for change in ('missing', 'model_declared', 'llm_proposed', 'unknown', 'conflict'):
            packet = self.packet()
            observation = packet['checked_observations'][0]
            if change == 'missing':
                packet['checked_observations'] = []
            elif change == 'model_declared':
                observation['scope'] = change
            else:
                observation['status'] = change
                if change in ('unknown', 'conflict'):
                    observation['value'] = None
            with self.subTest(change=change), self.assertRaises(ValueError):
                self.run_prefill(packet)

    def test_requires_use_exact_evidence_type_value_and_verification(self):
        for change in ('missing', 'ref', 'value', 'scope', 'status'):
            packet = self.model(self.packet())
            required = packet['eligible_options'][0]['requires'][0]
            if change == 'missing':
                required['field'] = 'output_limit_tokens'
            elif change == 'ref':
                required['evidence_ref'] = 'foreign-observation'
            elif change == 'value':
                required['value'] = 8191
            elif change == 'scope':
                required['scope'] = 'model_declared'
            else:
                packet['checked_observations'].append({'field': 'supports_tools',
                     'scope': 'endpoint_configuration', 'status': 'llm_proposed',
                     'value': True, 'evidence_ref': 'unverified-tool-support'})
                packet['eligible_options'][0]['requires'].append({'field': 'supports_tools',
                     'scope': 'endpoint_configuration', 'value': True, 'evidence_ref': 'unverified-tool-support'})
            result = self.run_prefill(packet)
            self.assertEqual(result['state'], 'requirements_not_met')
            self.assertEqual(result['field_updates'], [])

    def test_ambiguity_empty_options_and_whole_packet_validation(self):
        packet = self.packet()
        second = copy.deepcopy(packet['eligible_options'][0]); second['candidate_id'] = 'second'
        packet['eligible_options'].append(second)
        self.assertEqual(self.run_prefill(packet)['state'], 'ambiguous_options')
        result = self.run_prefill(self.model(packet, 'second'))
        self.assertEqual(result['candidate_id'], 'second')
        packet = self.packet(); packet['eligible_options'] = []
        self.assertEqual(self.run_prefill(packet)['state'], 'no_applicable_option')
        for change in ('duplicate_option', 'malformed_sibling', 'duplicate_requirement', 'duplicate_observation'):
            packet = self.packet()
            if change == 'duplicate_option':
                packet['eligible_options'].append(copy.deepcopy(packet['eligible_options'][0]))
            elif change == 'malformed_sibling':
                sibling = copy.deepcopy(second); sibling['values']['max_prompt_tokens'] = True
                packet['eligible_options'].append(sibling)
            elif change == 'duplicate_requirement':
                requirements = packet['eligible_options'][0]['requires']; requirements.append(copy.deepcopy(requirements[0]))
            else:
                packet['checked_observations'].append(copy.deepcopy(packet['checked_observations'][0]))
            with self.subTest(change=change), self.assertRaises(ValueError):
                self.run_prefill(packet)

    def test_no_coercion_overflow_extra_fields_or_forged_choices(self):
        for field, invalid in [('max_prefix_tokens', True), ('max_prefix_tokens', -1),
                               ('max_prompt_tokens', 0), ('reserved_output_tokens', 0),
                               ('additional_framing_tokens', '0'), ('max_prefix_tokens', 2**53),
                               ('max_prompt_tokens', 2**53-1)]:
            packet = self.packet(); packet['eligible_options'][0]['values'][field] = invalid
            with self.subTest(field=field, invalid=invalid), self.assertRaises(ValueError):
                self.run_prefill(packet)
        for change in ('model_invents_value', 'wrong_option_packet', 'unknown_method', 'deterministic_invents_choice', 'bad_revision'):
            packet = self.model(self.packet())
            if change == 'model_invents_value':
                packet['decision']['protocol']['answer']['result']['max_prefix_tokens'] = 1
            elif change == 'wrong_option_packet':
                packet['decision']['protocol']['eligible_candidate_ids'].append('foreign')
            elif change == 'unknown_method':
                packet['decision']['method'] = 'model_says_approved'
            elif change == 'deterministic_invents_choice':
                packet['decision'] = {'method': 'deterministic', 'candidate_id': 'original-option'}
            else:
                packet['existing_values']['max_prefix_tokens']['owner_revision'] = ''
            with self.subTest(change=change), self.assertRaises(ValueError):
                self.run_prefill(packet)


if __name__ == '__main__':
    unittest.main()
