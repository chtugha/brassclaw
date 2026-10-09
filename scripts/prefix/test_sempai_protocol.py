"""Protocol regressions: decoder order and source-anchored construction plans."""
import copy
import hashlib
import json
from pathlib import Path
import tempfile
import unittest

from compile_sempai_grammars import fingerprint, compile_schemas
from evaluate_sempai_prefix import (FIELDS, compact_grammar, construction_plan_schema,
                                   reviewer_schema, validate_plan_contract)


def summary_first(schema):
    old = copy.deepcopy(schema)
    old['properties'] = {key: old['properties'][key] for key in sorted(FIELDS)}
    return old


class ProtocolTests(unittest.TestCase):
    def test_summary_last_for_every_schema_variant(self):
        for strict in (False, True):
            for protect in (False, True):
                schema = reviewer_schema([['user', 'Draft']], strict_drafts=strict,
                    single_layout=strict, single_proposal=True,
                    preserve_count=True, preserve_content=protect)
                keys = list(schema['properties'])
                self.assertEqual(keys[-1], 'composition_summary')
                self.assertLess(keys.index('proposed_components'), keys.index('composition_summary'))
                self.assertEqual(set(keys), FIELDS)
                # Reordering alone keeps the JSON validation constraints identical.
                self.assertEqual(schema, summary_first(schema))
                self.assertNotEqual(fingerprint(schema), fingerprint(summary_first(schema)))

    def test_legacy_grammar_cannot_restore_old_order(self):
        schema = reviewer_schema([])
        old = summary_first(schema)
        digest = hashlib.sha256(json.dumps(old, sort_keys=True, separators=(',', ':')).encode()).hexdigest()
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary)
            grammar = 'root ::= "{}"'
            (directory / (digest + '.ebnf')).write_text(grammar)
            row = {'schema': old, 'schema_sha256': digest,
                   'grammar_sha256': hashlib.sha256(grammar.encode()).hexdigest()}
            (directory / 'manifest.json').write_text(json.dumps({
                'format': 'sempai-compact-grammar/1', 'any_whitespace': False, 'schemas': [row]}))
            with self.assertRaisesRegex(ValueError, 'decoder layout'):
                compact_grammar(schema, directory)
            self.assertEqual(compact_grammar(old, directory), grammar)

    def test_existing_plan_grammar_is_rejected_after_contract_change(self):
        old_dir = Path(__file__).parent / 'sempai-accuracy-20261007/sempai-bounded-plan-grammar-20261007'
        with self.assertRaisesRegex(ValueError, 'decoder layout'):
            compact_grammar(construction_plan_schema(), old_dir)

    def test_plan_quotes_cannot_replace_default_with_invented_branch(self):
        instruction = 'Absent channel defaults to read. Supplied null is invalid.'
        plan = {'artifact_kind': 'invocation-body', 'requirements': [
            {'aspect': 'input', 'contract_quote': 'Absent channel defaults to read.'}], 'workflow': []}
        validate_plan_contract(plan, instruction)
        bad = copy.deepcopy(plan)
        bad['requirements'][0]['contract_quote'] = 'Absent channel is invalid.'
        with self.assertRaisesRegex(ValueError, 'exact current host contract'):
            validate_plan_contract(bad, instruction)
        bad = copy.deepcopy(plan); bad['requirements'] = []
        with self.assertRaisesRegex(ValueError, 'incomplete'):
            validate_plan_contract(bad, instruction)
        bad = copy.deepcopy(plan); bad['branches'] = []
        with self.assertRaisesRegex(ValueError, 'keys mismatch'):
            validate_plan_contract(bad, instruction)

    def test_workflow_references_are_bound_to_host_contract(self):
        identity = '11111111-1111-4111-8111-111111111111'
        instruction = 'Recipe uses ' + identity
        plan = {'artifact_kind': 'recipe-constructor', 'requirements': [
            {'aspect': 'artifact', 'contract_quote': instruction}],
            'workflow': [{'knowledge': 'orchestrator', 'uuid': identity}]}
        validate_plan_contract(plan, instruction)
        bad = copy.deepcopy(plan); bad['artifact_kind'] = 'preload-definition'
        with self.assertRaisesRegex(ValueError, 'Only Recipe'):
            validate_plan_contract(bad, instruction)
        bad = copy.deepcopy(plan); bad['workflow'][0]['uuid'] = '22222222-2222-4222-8222-222222222222'
        with self.assertRaisesRegex(ValueError, 'invented'):
            validate_plan_contract(bad, instruction)


class XGrammarOrderTests(unittest.TestCase):
    def test_actual_decoder_accepts_payload_first_and_rejects_summary_first(self):
        try:
            from xgrammar.testing import _is_grammar_accept_string
        except ImportError:
            self.skipTest('Run in the existing vLLM environment for actual XGrammar matching')
        for strict in (False, True):
            schema = reviewer_schema([['user', 'Draft']], strict_drafts=strict,
                single_layout=strict, preserve_content=True, single_proposal=True)
            values = {key: [] for key in FIELDS}
            values['adjusted_volatile_messages'] = [['user', 'Draft']]
            values['proposed_components'] = [{'class_code': 22, 'payload': {
                'name': 'test-draft', 'description': 'Unapproved test', 'content': 'result = {}'}}]
            values['composition_summary'] = 'One unapproved draft.'
            # Preserve XGrammar's existing default outer separators. Its const
            # arrays are literal compact JSON independently of that formatting.
            def encoded(keys):
                return '{' + ', '.join(json.dumps(key) + ': ' + json.dumps(values[key],
                    separators=(',', ':') if 'const' in schema['properties'][key]
                    else (', ', ': ')) for key in keys) + '}'
            good = encoded(schema['properties'])
            bad = encoded(sorted(FIELDS))
            with tempfile.TemporaryDirectory() as temporary:
                source = Path(temporary) / 'schemas.json'; source.write_text(json.dumps([schema]))
                output = Path(temporary) / 'grammars'; compile_schemas(source, output)
                grammar = compact_grammar(schema, output)
                self.assertTrue(_is_grammar_accept_string(grammar, good))
                self.assertFalse(_is_grammar_accept_string(grammar, bad))
                self.assertFalse(_is_grammar_accept_string(grammar, good[:-1]))


if __name__ == '__main__':
    unittest.main()
