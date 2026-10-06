"""Exercise real compiler entry points and their literal evidence contracts."""
import importlib.util
from pathlib import Path
import unittest

from prefix_response_validation import check_citations


class CompilerCitationTests(unittest.TestCase):
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
