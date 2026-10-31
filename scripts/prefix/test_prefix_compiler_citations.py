"""Exercise real compiler entry points and their literal evidence contracts."""
import importlib.util
import json
from pathlib import Path
import unittest
import sqlite3
import subprocess
import tempfile
from contextlib import contextmanager, ExitStack
from unittest.mock import patch

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


class CollectorTests(unittest.TestCase):
    """Real local Git/files/SQLite; no tokenizer, provider or network required."""
    filenames = ('Defensive-compiler.py', 'Homeassistant-compiler.py', 'tomedo-compiler.py')

    @contextmanager
    def fixture(self, filename):
        spec = importlib.util.spec_from_file_location('collector_' + filename,
            Path(__file__).with_name(filename))
        compiler = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(compiler)
        with tempfile.TemporaryDirectory() as directory, ExitStack() as stack:
            base = Path(directory)
            paths = dict(BASE=base, RAW=base/'raw', REPOS_DIR=base/'raw/repos',
                DOCUMENTS_JSONL=base/'raw/documents.jsonl', DB_PATH=base/'knowledge.db',
                FINAL=base/'final_knowledge.md', CONTEXT=base/'context_100k.md',
                REPOS={'first': 'https://example.invalid/first', 'later': 'https://example.invalid/later'})
            stack.enter_context(patch.multiple(compiler, **paths))
            for repo in compiler.REPOS:
                root = compiler.REPOS_DIR/repo
                root.mkdir(parents=True)
                for name in ('keep.md', 'removed.md'):
                    (root/name).write_bytes(b'# Source\r\nPreserve existing evidence.\r\n')
                for args in (['init', '-q'], ['add', '.'],
                    ['-c', 'user.name=Collector Test', '-c', 'user.email=test@example.invalid',
                     'commit', '-qm', 'Local source fixture']):
                    subprocess.run(['git', '-C', str(root), *args], check=True, capture_output=True)
            compiler.collect()
            connection = compiler.db()
            for ident, repo, path, text, digest in connection.execute(
                'SELECT id,repo,path,text,source_digest FROM documents').fetchall():
                # Valid existing checkpoint rows; not tokenizer/serving evidence.
                connection.execute('INSERT INTO atoms(id,doc_id,local_index,line_start,line_end,tokens,text,source_digest) VALUES(?,?,1,1,2,1,?,?)',
                    (ident, ident, text, digest))
                connection.execute('INSERT INTO triage(atom_id,mask,primary_lane,keep,updated) VALUES(?,1,?,1,1)',
                    (ident, 'ACTION'))
                connection.execute('INSERT INTO knowledge(atom_id,lane,knowledge,source_digest,updated) VALUES(?,?,?,?,1)',
                    (ident, 'ACTION', text, digest))
                connection.execute('INSERT INTO knowledge_groups(lane,repo,path,first_atom_id,last_atom_id,atom_count,input_tokens,source_digest,knowledge,keep,updated,evidence_json) VALUES(?,?,?,?,?,1,1,?,?,1,1,?)',
                    ('ACTION', repo, path, ident, ident, digest, text, '[]'))
            connection.commit()
            connection.close()
            compiler.FINAL.write_bytes(b'existing working reduction\r\n')
            compiler.CONTEXT.write_bytes(b'published-prefix sentinel, not a serving qualification\r\n')
            yield compiler

    def snapshot(self, compiler):
        connection = compiler.db()
        try:
            rows = {table: connection.execute('SELECT * FROM ' + table + ' ORDER BY rowid').fetchall()
                for table in ('documents', 'atoms', 'triage', 'knowledge', 'knowledge_groups', 'meta')}
            rows['fts'] = connection.execute(
                'SELECT repo,path,knowledge FROM knowledge_search ORDER BY rowid').fetchall()
        finally:
            connection.close()
        files = {name: getattr(compiler, name).read_bytes()
            for name in ('DOCUMENTS_JSONL', 'FINAL', 'CONTEXT')}
        originals = {path.name: path.read_bytes() for path in (compiler.RAW/'documents').iterdir()}
        return rows, files, originals

    def assert_preserved(self, compiler, before, connections):
        rows, files, originals = self.snapshot(compiler)
        self.assertEqual((rows, files), before[:2])
        for name, data in before[2].items():
            self.assertEqual(originals[name], data)
        self.assertFalse(list(compiler.DOCUMENTS_JSONL.parent.glob('.documents.jsonl.*.tmp')))
        self.assertTrue(connections)
        for connection in connections:
            with self.assertRaises(sqlite3.ProgrammingError):
                connection.execute('SELECT 1')

    def fail_after_first_repository(self, compiler, before, mutate, expected):
        # Earlier updates must also roll back when a later repository fails.
        (compiler.REPOS_DIR/'first/keep.md').write_bytes(b'# Changed\r\nKeep earlier checkpoints on failure.\r\n')
        (compiler.REPOS_DIR/'first/removed.md').unlink()
        connections = []
        original_db = compiler.db
        def capture_connection():
            connection = original_db()
            connections.append(connection)
            return connection
        with patch.object(compiler, 'db', capture_connection), mutate():
            with self.assertRaisesRegex(RuntimeError, expected):
                compiler.collect()
        self.assert_preserved(compiler, before, connections)

    def test_invalid_utf8_rolls_back_all_repositories_and_preserves_files(self):
        for filename in self.filenames:
            with self.subTest(compiler=filename), self.fixture(filename) as compiler:
                before = self.snapshot(compiler)
                (compiler.REPOS_DIR/'later/keep.md').write_bytes(b'\xffinvalid UTF-8')
                self.fail_after_first_repository(compiler, before, ExitStack, 'Source read failed')

    def test_real_read_error_preserves_prior_documents_and_derived_rows(self):
        for filename in self.filenames:
            with self.subTest(compiler=filename), self.fixture(filename) as compiler:
                before = self.snapshot(compiler)
                target = compiler.REPOS_DIR/'later/keep.md'
                original_score = compiler.source_score
                def replace_after_stat(path):
                    score = original_score(path)
                    if path == target:
                        # The real read now raises IsADirectoryError. No fake I/O result.
                        target.unlink()
                        target.mkdir()
                    return score
                self.fail_after_first_repository(compiler, before,
                    lambda: patch.object(compiler, 'source_score', replace_after_stat), 'Source read failed')

    def test_real_stat_error_preserves_the_complete_working_catalogue(self):
        for filename in self.filenames:
            with self.subTest(compiler=filename), self.fixture(filename) as compiler:
                before = self.snapshot(compiler)
                target = compiler.REPOS_DIR/'later/keep.md'
                original_stat = Path.stat
                removed = False
                def remove_before_stat(path, *args, **kwargs):
                    nonlocal removed
                    if path == target and not removed:
                        target.unlink()
                        removed = True
                    return original_stat(path, *args, **kwargs)
                self.fail_after_first_repository(compiler, before,
                    lambda: patch.object(Path, 'stat', remove_before_stat), 'Source stat failed')

    def test_programming_error_is_not_swallowed_and_connection_closes(self):
        for filename in self.filenames:
            with self.subTest(compiler=filename), self.fixture(filename) as compiler:
                before = self.snapshot(compiler)
                original_score = compiler.source_score
                def broken_score(path):
                    if path.parent.name == 'later':
                        raise RuntimeError('deliberate programming failure')
                    return original_score(path)
                self.fail_after_first_repository(compiler, before,
                    lambda: patch.object(compiler, 'source_score', broken_score), 'deliberate programming failure')

    def test_complete_scan_prunes_confirmed_deletion_and_preserves_crlf(self):
        for filename in self.filenames:
            with self.subTest(compiler=filename), self.fixture(filename) as compiler:
                (compiler.REPOS_DIR/'first/removed.md').unlink()
                compiler.collect()
                connection = compiler.db()
                try:
                    rows = connection.execute('SELECT repo,path,text FROM documents').fetchall()
                    self.assertEqual(len(rows), 3)
                    self.assertNotIn(('first', 'removed.md'), [(repo, path) for repo, path, _ in rows])
                    for _, _, text in rows:
                        self.assertEqual(text, '# Source\r\nPreserve existing evidence.\r\n')
                    for table in ('atoms', 'triage', 'knowledge', 'knowledge_groups'):
                        self.assertEqual(connection.execute('SELECT count(*) FROM ' + table).fetchone()[0], 3)
                    self.assertEqual(connection.execute('SELECT count(*) FROM knowledge_search').fetchone()[0], 3)
                finally:
                    connection.close()
                self.assertFalse(compiler.FINAL.exists())
                manifest = compiler.DOCUMENTS_JSONL.read_bytes()
                self.assertEqual(len([json.loads(line) for line in manifest.splitlines()]), 3)
                compiler.collect()
                self.assertEqual(compiler.DOCUMENTS_JSONL.read_bytes(), manifest)


if __name__ == '__main__':
    unittest.main()
