"""Exercise real compiler entry points and their literal evidence contracts."""
import importlib.util
import json
from pathlib import Path
import unittest

from prefix_response_validation import check_citations


class CompilerCitationTests(unittest.TestCase):
    def test_real_fast_tokenizer_specials_preserve_unicode_in_every_domain_renderer(self):
        from tokenizers import Tokenizer, models
        from transformers import PreTrainedTokenizerFast
        from prefix_render import prefix_escape_controls
        # Actual tokenizer implementation and added-token API, no inference or
        # monkeypatched token counts. This is not a deployed-model fixture.
        backend = Tokenizer(models.WordLevel({'[UNK]': 0}, unk_token='[UNK]'))
        tokenizer = PreTrainedTokenizerFast(tokenizer_object=backend, unk_token='[UNK]',
            bos_token='🧭', eos_token='\U00010000', sep_token='ä', pad_token='<\U0010ffff>')
        for token in ('ä', '\U00010000', '🧭', '<\U0010ffff>'):
            self.assertEqual(len(tokenizer.encode(token, add_special_tokens=False)), 1)
        for filename in ('Defensive-compiler.py', 'tomedo-compiler.py',
                         'Homeassistant-compiler.py', 'sempai-compiler.py'):
            with self.subTest(compiler=filename):
                spec = importlib.util.spec_from_file_location(filename, Path(__file__).with_name(filename))
                compiler = importlib.util.module_from_spec(spec)
                spec.loader.exec_module(compiler)
                compiler.TOK = tokenizer
                original = '{"glyph": "🧭", "word": "ä", "plane": "\U00010000"}\r\n'
                escaped = compiler.transport_text(original)
                self.assertEqual(json.loads(escaped), json.loads(original))
                self.assertEqual(escaped, prefix_escape_controls(original, tokenizer.all_special_tokens))
                self.assertEqual(compiler.transport_text('ordinary é\r\n'), 'ordinary é\r\n')
                self.assertEqual(compiler.transport_text('ä'), r'\u00e4')
                self.assertEqual(compiler.transport_text('<|im_start|>'), r'\u003c|im_start|\u003e')
                repo = next(iter(compiler.REPOS))
                card = compiler.card_for_unit(repo, 'unicode.json', original,
                    compiler.source_units(original, '.json')[0],
                    {'url': 'https://example.test/unicode-fixture', 'snapshot': compiler.digest(original)})
                compiler.validate_card(card, original)
                before = json.dumps(card, ensure_ascii=False, sort_keys=True)
                self.assertIn(escaped, compiler.render_card(card))
                self.assertEqual(json.dumps(card, ensure_ascii=False, sort_keys=True), before)

    def test_all_run_modes_require_verified_active_prefix(self):
        for filename in ('Defensive-compiler.py', 'tomedo-compiler.py', 'Homeassistant-compiler.py'):
            with self.subTest(compiler=filename):
                spec = importlib.util.spec_from_file_location(filename, Path(__file__).with_name(filename))
                compiler = importlib.util.module_from_spec(spec)
                spec.loader.exec_module(compiler)
                compiler.ACTIVE_SERVER_PREFIX = ''
                with self.assertRaisesRegex(ValueError, 'VLLM_SERVER_PREFIX_FILE'):
                    compiler.run_task('Explain source-grounded maintenance.')
                document = ('# Maintenance\n\nPreserve firewall backups and verify management access '
                            'before applying changes. PostgreSQL-Transaktionen müssen dokumentiert werden.\n')
                repo = next(iter(compiler.REPOS))
                unit = (3, 3, document.splitlines(keepends=True)[2], '# Maintenance\n')
                card = compiler.card_for_unit(repo, 'reference.md', document, unit,
                                              {'url': 'https://example.org/test-fixture', 'revision': 'fixture-v1'})
                answer = {'claims': [{'text': 'Preserve firewall backups and verify management access.',
                    'citations': [{'card_id': card['id'], 'quote': unit[2]}]}],
                    'configuration': '', 'missing_information': []}
                self.assertEqual(check_citations(answer, {card['id']: card})['status'], 'passed')
                answer['claims'][0]['citations'][0]['quote'] = 'All firewall rules are correct and tested successfully.'
                self.assertEqual(check_citations(answer, {card['id']: card})['status'], 'failed')
                answer['claims'][0]['text'] = 'PostgreSQL-Transaktionen müssen dokumentiert werden.'
                answer['claims'][0]['citations'][0]['quote'] = unit[2]
                self.assertEqual(check_citations(answer, {card['id']: card})['status'], 'passed')


if __name__ == '__main__':
    unittest.main()
