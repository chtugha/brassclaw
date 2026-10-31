"""Pure-stage checks and an independent real retained-template HF fixture."""
import importlib.util
import json
import os
from pathlib import Path
import sys
import unittest

SPEC = importlib.util.spec_from_file_location('pair_cases', Path(__file__).with_name('prefix_pair_cases.py'))
MODULE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(MODULE)


class PairCasePreparationTests(unittest.TestCase):
    def packet(self):
        return {'format': 'brassclaw-prefix-pair-case-packet/1', 'proposal': 'a' * 64,
                'specification': 'b' * 64, 'artifact_set': 'c' * 64, 'template_sha256': 'd' * 64,
                'qualification': 'unqualified-case-input', 'cases': [
                    {'id': 'unicode', 'mode': 'plain', 'context_json': '{"z":2,"messages":[{"role":"user","content":"ü\\r\\n"}],"a":1}',
                     'expected_prefix': 'Fixed ü\n', 'expected_render': 'Fixed ü\nDynamic\r\n'}]}

    def test_complete_case_data_and_original_order_are_preserved(self):
        packet = MODULE.pair_case_json(self.packet())
        prepared = MODULE.pair_case_prepare({'case_packet_bytes': packet})
        self.assertEqual(prepared['bank_bytes'], packet)
        probes = json.loads(prepared['probes_json'])
        self.assertEqual(len(probes), 2)
        self.assertEqual(probes[0]['context_json'], self.packet()['cases'][0]['context_json'])
        self.assertEqual(prepared['texts'][1:], ['Fixed ü\nDynamic\r\n', self.packet()['cases'][0]['context_json'], 'Fixed ü\n'])
        self.assertNotIn('template_utf8', prepared)
        prepared['texts'].clear()
        self.assertEqual(len(MODULE.pair_case_prepare({'case_packet_bytes': packet})['texts']), 4)

    def test_ambiguous_partial_or_relabelled_packets_fail_without_omission(self):
        packet = self.packet()
        for field, value in [('qualification', 'accepted'), ('cases', []), ('proposal', 'not-a-digest')]:
            invalid = dict(packet, **{field: value})
            with self.assertRaises(ValueError):
                MODULE.pair_case_prepare({'case_packet_bytes': MODULE.pair_case_json(invalid)})
        with self.assertRaises(ValueError):
            MODULE.pair_case_prepare({'case_packet_bytes': json.dumps(packet)})
        duplicate = MODULE.pair_case_json(packet).replace('{', '{"format":"forged",', 1)
        with self.assertRaises(ValueError):
            MODULE.pair_case_prepare({'case_packet_bytes': duplicate})


def native_fixture():
    from transformers import AutoTokenizer
    root = Path(os.environ['BRASSCLAW_TEST_MODEL_ARTIFACT_ROOT'])
    tokenizer = AutoTokenizer.from_pretrained(root, local_files_only=True, trust_remote_code=False)
    original = (root / 'chat_template.jinja').read_text()
    assert tokenizer.chat_template == original, 'original template must remain unchanged'
    sample = 'Reference ü'
    fixed = '<|im_start|>system\n' + sample + '<|im_end|>\n'
    base = [{'role': 'system', 'content': sample}]
    contexts = [
        ('plain', {'messages': base + [{'role': 'user', 'content': 'Frage'}], 'add_generation_prompt': True}),
        ('unicode', {'messages': base + [{'role': 'user', 'content': ' ä\r\n😀\r\n'}], 'add_generation_prompt': True}),
        ('history', {'messages': base + [{'role': 'user', 'content': 'alt'}, {'role': 'assistant', 'content': 'Antwort'}, {'role': 'user', 'content': 'neu'}], 'add_generation_prompt': True}),
        ('thinking_off', {'messages': base + [{'role': 'user', 'content': 'Frage'}], 'add_generation_prompt': True, 'enable_thinking': False}),
    ]
    cases = []
    for name, context in contexts:
        rendered = tokenizer.apply_chat_template(context['messages'], tokenize=False, **{key: value for key, value in context.items() if key != 'messages'})
        assert rendered.startswith(fixed)
        cases.append({'id': name, 'mode': name, 'render_context_json': json.dumps(context, ensure_ascii=False, separators=(',', ':')),
                      'expected_fixed_prefix': fixed, 'formatted_prompt': rendered[len(fixed):],
                      'expected_request': rendered, 'evidence': ['template']})
    print(json.dumps({'sample_reference': sample, 'cases': cases}, ensure_ascii=False, sort_keys=True, separators=(',', ':')))


if __name__ == '__main__':
    if sys.argv[1:] == ['--native-fixture']:
        native_fixture()
    else:
        unittest.main()
