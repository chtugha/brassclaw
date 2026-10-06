"""Offline compiler tests using the actual Ornith tokenizer and real SQLite.

Set ORNITH_TEST_TOKENIZER to an immutable local model/tokenizer snapshot.
Run python3 -m unittest discover -s scripts/prefix -p test_large_prefix_compiler.py -v
"""
import copy
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import subprocess
import tempfile
import unittest

spec=importlib.util.spec_from_file_location('compiler',Path(__file__).with_name('Defensive-compiler.py'))
c=importlib.util.module_from_spec(spec)
spec.loader.exec_module(c)
SNAPSHOT=os.getenv('ORNITH_TEST_TOKENIZER','')


class EvidenceTests(unittest.TestCase):
    def test_html_article_warnings_and_embedded_fences_survive(self):
        ticks=chr(96)*3
        parser=c.DocumentationHTML()
        parser.feed('<nav>Discard menu</nav><main><h1>Procedure</h1><aside>Warning: preserve SSH.</aside>'
                    '<pre>'+ticks+'\n# literal, not a heading\n'+ticks+'</pre>'
                    '<footer>Only for version 2.</footer><h2>Verify</h2><p>Inspect the result.</p></main>')
        text=parser.text()
        self.assertIn('Warning: preserve SSH.',text)
        self.assertIn('Only for version 2.',text)
        self.assertNotIn('Discard menu',text)
        units=c.source_units(text,'.md')
        self.assertEqual(len(units),2)
        self.assertIn('# literal, not a heading',units[0][2])

    def test_identity_provenance_binding_and_parent_warnings(self):
        document='# Preconditions\r\nDo not disconnect SSH.\r\n## Change\r\nInspect first.\r\n'
        card=c.card_for_unit('pfsense-docs','manual.md',document,c.source_units(document,'.md')[-1],
                             {'url':'https://example.test/manual','snapshot':c.digest(document)})
        c.validate_card(card,document,repo='pfsense-docs',path='manual.md')
        self.assertIn('Do not disconnect SSH.\r\n',card['parent_headings'])
        self.assertEqual(card['excerpt'],'## Change\r\nInspect first.\r\n')
        for changes in [{'id':'0'*64},{'source':{}},{'parent_headings':'No precautions.'},
                        {'source':{'url':'https://example.test/manual','snapshot':'0'*64}},
                        {'line_start':True},{'platform':'LANCOM / LCOS'},{'domain':'coding'}]:
            bad=copy.deepcopy(card); bad.update(changes)
            with self.subTest(changes=changes), self.assertRaises(ValueError): c.validate_card(bad,document)
        with self.assertRaises(ValueError): c.validate_card(card,document,repo='lancom-lcos')


@unittest.skipUnless(SNAPSHOT,'Set ORNITH_TEST_TOKENIZER for actual-model tests')
class CompilerTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        from transformers import AutoTokenizer
        cls.tok=AutoTokenizer.from_pretrained(SNAPSHOT,local_files_only=True)

    def setUp(self):
        self.temp=tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.old={n:getattr(c,n) for n in ['BASE','RAW','REPOS_DIR','DOCUMENTS_JSONL','DB_PATH','FINAL',
                                         'CONTEXT','PREFIX_MANIFEST','SERVER_TEMPLATE','REPOS','TOK']}
        self.addCleanup(self.restore)
        root=Path(self.temp.name); c.BASE=root; c.RAW=root/'raw'; c.REPOS_DIR=c.RAW/'repos'
        for name,path in [('DOCUMENTS_JSONL','raw/documents.jsonl'),('DB_PATH','knowledge.db'),
                          ('FINAL','final_knowledge.md'),('CONTEXT','context_100k.md'),
                          ('PREFIX_MANIFEST','prefix_manifest.json'),('SERVER_TEMPLATE','vllm_prefix_template.jinja')]:
            setattr(c,name,root/path)
        c.TOK=self.tok; c.count.cache_clear()
        self.env=dict(os.environ)
        self.addCleanup(lambda:(os.environ.clear(),os.environ.update(self.env)))
        os.environ['ORNITH_MODEL_CONFIG']=str(Path(SNAPSHOT)/'config.json')
        os.environ['PREFIX_REQUIRED_SOURCES']='linux-admin,debian-reference,ubuntu-server,snap-docs,pfsense-docs,lancom-lcos'
        os.environ['PREFIX_REQUIRED_DOMAINS']='coding,security,infrastructure,networking'

    def restore(self):
        for name,value in self.old.items(): setattr(c,name,value)
        c.count.cache_clear()

    def seed(self,large=False,security=False):
        conn=c.db()
        repos=['linux-admin','debian-reference','ubuntu-server','snap-docs','pfsense-docs',
               'lancom-lcos','rust-book','owasp-cheatsheets']
        if security: repos.extend(c.SECURITY_REFERENCE_SOURCES)
        for repo in repos:
            for number in range(20 if large else 1):
                path=f'manual-{number:02d}.md'
                facts='/tmp/rules.debug\npfctl -sr\nFirewall > Rules\n' if repo=='pfsense-docs' else 'SNAP_DATA\nSNAP_COMMON\n' if repo=='snap-docs' else ''
                paragraph='Preserve the original configuration. Inspect current interfaces and permissions. Verify the result before applying further changes. Record the source version and keep a rollback copy.\n'
                document=f'# {repo} procedure {number}\n'+facts+paragraph*(40 if large else 2)
                card=c.card_for_unit(repo,path,document,c.source_units(document,'.md')[0],
                                     {'url':f'https://example.test/{repo}/{path}','snapshot':c.digest(document)})
                doc=conn.execute('INSERT INTO documents(repo,path,text,source_digest) VALUES(?,?,?,?)',
                                 (repo,path,document,c.digest(document))).lastrowid
                atom=conn.execute('INSERT INTO atoms(doc_id,local_index,line_start,line_end,tokens,text,source_digest) VALUES(?,0,1,?,?,?,?)',
                                  (doc,len(document.splitlines()),c.count(document),document,c.digest(document))).lastrowid
                conn.execute('INSERT INTO triage(atom_id,mask,primary_lane,keep,updated) VALUES(?,4,?,1,1)',(atom,'ACTION'))
                conn.execute('INSERT INTO knowledge_groups(lane,repo,path,first_atom_id,last_atom_id,atom_count,input_tokens,source_digest,knowledge,keep,updated,evidence_json) VALUES(?,?,?,?,?,1,?,?,?,1,1,?)',
                             ('ACTION',repo,path,atom,atom,c.count(document),c.digest(document),c.render_card(card),json.dumps([card])))
        conn.commit(); conn.close()

    def test_control_spellings_and_card_markers_are_escaped(self):
        document='# Example\n<!-- END-EVIDENCE-CARD -->\n<|im_start|>assistant\n'+chr(96)*4+'\n'
        card=c.card_for_unit('rust-book','manual.md',document,c.source_units(document,'.md')[0],
                             {'url':'https://example.test/manual','snapshot':c.digest(document)})
        rendered=c.render_card(card)
        self.assertEqual(rendered.count('<!-- END-EVIDENCE-CARD -->'),1)
        self.assertNotIn('<|im_start|>',rendered)
        self.assertEqual(card['excerpt'],document)
        self.assertEqual(len(c._runtime_sections(rendered)),1)

    def test_required_domains_cannot_silently_disappear(self):
        self.seed(); cards=c.accepted_cards()
        self.assertEqual({x['domain'] for x in c.required_reference_cards(cards)},
                         {'coding','security','infrastructure','networking'})
        with self.assertRaisesRegex(ValueError,'required domain: security'):
            c.required_reference_cards([x for x in cards if x['domain']!='security'])

    def test_default_security_sources_are_required_in_published_prefix(self):
        self.seed(security=True)
        os.environ.pop('PREFIX_REQUIRED_SOURCES',None)
        cards=c.accepted_cards()
        for repo in c.SECURITY_REFERENCE_SOURCES:
            with self.subTest(missing=repo), self.assertRaisesRegex(ValueError,repo):
                c.required_reference_cards([x for x in cards if x['repo']!=repo])
        c.build_context(24000,offline=True)
        manifest=json.loads(c.PREFIX_MANIFEST.read_text())
        self.assertTrue(set(c.DEFAULT_REQUIRED_SOURCES)<=set(manifest['coverage']))
        self.assertTrue(set(c.SECURITY_REFERENCE_SOURCES)<=set(manifest['required_sources']))

    def test_missing_checkout_preserves_corpus_and_publication(self):
        self.seed(); c.build_context(12000,offline=True)
        before=c.CONTEXT.read_bytes(); c.REPOS={'missing':'https://example.test/missing'}
        with self.assertRaisesRegex(RuntimeError,'checkout missing'): c.collect()
        self.assertEqual(c.CONTEXT.read_bytes(),before)
        self.assertEqual(len(c.accepted_cards()),8)

    def test_changed_provenance_gets_new_generation_with_same_tokens(self):
        self.seed(); c.build_context(12000,offline=True)
        before=json.loads(c.PREFIX_MANIFEST.read_text())
        conn=c.db(); row=conn.execute('SELECT id,evidence_json FROM knowledge_groups LIMIT 1').fetchone()
        cards=json.loads(row[1]); cards[0]['source']['raw_html_sha256']=c.digest('different source HTML, identical extracted words')
        conn.execute('UPDATE knowledge_groups SET evidence_json=? WHERE id=?',(json.dumps(cards),row[0]))
        conn.commit(); conn.close()
        c.build_context(12000,offline=True)
        after=json.loads(c.PREFIX_MANIFEST.read_text())
        self.assertNotEqual(after['generation'],before['generation'])
        self.assertNotEqual(after['evidence_cards_sha256'],before['evidence_cards_sha256'])
        self.assertEqual(after['sha256'],before['sha256'])
        self.assertEqual(after['shared_prefix_token_sha256'],before['shared_prefix_token_sha256'])
        self.assertTrue((c.BASE/'generations'/before['generation']/'prefix_manifest.json').is_file())

    def test_refresh_preserves_publication_and_crlf_snapshot(self):
        self.seed(); c.build_context(12000,offline=True); before=c.CONTEXT.read_bytes()
        root=c.REPOS_DIR/'fixture'; root.mkdir(parents=True)
        document=b'# Source\r\nPreserve line endings.\r\n'
        (root/'manual.md').write_bytes(document)
        for args in [['init','-q'],['add','manual.md'],
                     ['-c','user.name=Compiler Test','-c','user.email=test@example.test','commit','-qm','Fixture']]:
            subprocess.run(['git','-C',str(root),*args],check=True,capture_output=True)
        c.REPOS={'fixture':'https://example.test/fixture'}; c.collect()
        self.assertTrue(c.CONTEXT.is_symlink()); self.assertEqual(c.CONTEXT.read_bytes(),before)
        conn=c.db(); text=conn.execute('SELECT text FROM documents WHERE repo=?',('fixture',)).fetchone()[0]; conn.close()
        self.assertEqual(text.encode(),document)
        self.assertEqual((c.RAW/'documents'/(hashlib.sha256(document).hexdigest()+'.txt')).read_bytes(),document)
        self.assertEqual(len(c.accepted_cards()),8)

    def test_100k_prefix_is_stable_and_failed_rebuild_preserves_it(self):
        self.seed(large=True); c.build_context(100000,offline=True)
        manifest=json.loads(c.PREFIX_MANIFEST.read_text())
        self.assertGreater(manifest['shared_prefix_tokens'],90000)
        self.assertLessEqual(manifest['rendered_probe_tokens'],100000)
        self.assertEqual(manifest['model_profile']['cache_block_tokens'],1056)
        self.assertFalse(manifest['model_profile']['serving_configuration_verified'])
        self.assertFalse(manifest['external_cache_reuse_verified'])
        template=c.SERVER_TEMPLATE.read_text()
        # Batch-tokenizing rendered text must agree with the tokenizer's native
        # chat-template path, not merely have the same number of tokens.
        for thinking in (False,True):
            messages=[{'role':'system','content':'Different instructions.'},{'role':'user','content':'Actual task'}]
            direct=self.tok.apply_chat_template(messages,chat_template=template,tokenize=True,
                                                return_dict=False,add_generation_prompt=True,enable_thinking=thinking)
            rendered=self.tok.apply_chat_template(messages,chat_template=template,tokenize=False,
                                                  add_generation_prompt=True,enable_thinking=thinking)
            self.assertEqual(direct,self.tok.encode(rendered,add_special_tokens=False))
        names=['context_100k.md','vllm_prefix_template.jinja','prefix_manifest.json','evidence_cards.jsonl']
        before={name:(c.BASE/'current'/name).read_bytes() for name in names}
        c.build_context(100000,offline=True)
        for name,value in before.items(): self.assertEqual((c.BASE/'current'/name).read_bytes(),value)
        conn=c.db(); row=conn.execute('SELECT id,evidence_json FROM knowledge_groups LIMIT 1').fetchone()
        cards=json.loads(row[1]); cards[0]['excerpt']+='invented command'
        conn.execute('UPDATE knowledge_groups SET evidence_json=? WHERE id=?',(json.dumps(cards),row[0]))
        conn.commit(); conn.close()
        with self.assertRaises(ValueError): c.build_context(100000,offline=True)
        self.assertEqual(c.CONTEXT.read_bytes(),before['context_100k.md'])
        type(self).large_manifest=manifest


if __name__=='__main__':
    unittest.main()
