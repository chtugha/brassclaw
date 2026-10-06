"""Domain separation and literal evidence regression checks for the tomedo profile."""
import importlib.util
import json
import os
from pathlib import Path
import re
import tempfile
import unittest
from unittest.mock import patch

spec=importlib.util.spec_from_file_location('tomedo_compiler',Path(__file__).with_name('tomedo-compiler.py'))
c=importlib.util.module_from_spec(spec); spec.loader.exec_module(c)

class TomedoTests(unittest.TestCase):
    def test_no_clinical_source_family(self):
        self.assertEqual(set(c.DEFAULT_REQUIRED_SOURCES),set(c.DEFAULT_REPOS)|set(c.WEB_SOURCES)|{'tomedo-local-api','tomedo-forum-api'})
        self.assertNotIn('medicine',set(c.REPO_DOMAINS.values()))
        self.assertFalse(any('nvl' in key or 'clinical' in key for key in c.WEB_SOURCES))
        self.assertIn('tomedo_evidence',str(c.BASE))
        self.assertIn('does not provide clinical medicine',c.RUN_SYSTEM)
        self.assertTrue(c.WEB_SOURCES['patient-records-law']['seed_only'])
        for sources,path,text in c.REQUIRED_TOPICS.values():
            self.assertTrue(set(sources)<=set(c.DEFAULT_REQUIRED_SOURCES))
            re.compile(path); re.compile(text)

    def test_german_html_uses_declared_encoding_without_replacement(self):
        response=c.httpx.Response(200,headers={'content-type':'text/html'},content=(
            '<meta http-equiv="Content-Type" content="text/html; charset=iso-8859-1">'
            '<h1>GOÄ</h1><p>Gebühren, § 5; Ausschlüsse berücksichtigen.</p>').encode('latin-1'))
        decoded,encoding=c.decode_html(response)
        self.assertEqual(encoding,'iso-8859-1')
        parser=c.DocumentationHTML();parser.feed(decoded)
        self.assertIn('Gebühren',parser.text());self.assertNotIn('\ufffd',parser.text())
        with self.assertRaises(UnicodeDecodeError):
            c.decode_html(c.httpx.Response(200,headers={'content-type':'text/html; charset=utf-8'},content=b'\xff'))

    def test_pdf_neighbor_conditions_are_validated_literal_evidence(self):
        document=('# PDF-derived reference\nOriginal PDF is authoritative.\n\n'
                  '## PDF page 1\nNicht neben Leistung X berechnungsfähig.\n\n'
                  '## PDF page 2\nGOÄ-Leistung Y: 2,3-fach, ausschließlich dokumentiert.\n\n'
                  '## PDF page 3\nZusätzliche Begründung erforderlich.\n\n')
        unit=next(u for u in c.source_units(document) if 'page 2' in u[2])
        card=c.card_for_unit('goae-catalogue','GOAE.pdf.md',document,unit,
             {'url':'https://example.test/legal-fixture','snapshot':c.digest(document),'version':'fixture'})
        c.validate_card(card,document)
        self.assertIn('Nicht neben',card['parent_headings'])
        self.assertIn('Begründung erforderlich',card['parent_headings'])
        card['parent_headings']=card['parent_headings'].replace('Nicht neben','Auch neben')
        card['context_sha256']=c.digest(card['parent_headings'])
        with self.assertRaisesRegex(ValueError,'complete source unit'): c.validate_card(card,document)

    def test_vendor_import_requires_explicit_document_and_version(self):
        with patch.dict(os.environ,{'TOMEDO_API_DOCUMENT':'','TOMEDO_API_VERSION':''}):
            with self.assertRaisesRegex(ValueError,'TOMEDO_API_DOCUMENT'): c.collect_vendor_api()

    def test_vendor_import_retains_bytes_and_rejects_private_keys(self):
        with tempfile.TemporaryDirectory() as directory:
            root=Path(directory);document=root/'official.md'; cache=root/'evidence'
            original=b'# Vendor API\r\nVersion example: preserve required fields and pagination.\r\n'
            document.write_bytes(original)
            with patch.multiple(c,BASE=cache,RAW=cache/'raw',DB_PATH=cache/'knowledge.db'), patch.dict(os.environ,
                {'TOMEDO_API_DOCUMENT':str(document),'TOMEDO_API_VERSION':'operator-confirmed-example','TOMEDO_API_SOURCE_URL':''}):
                c.collect_vendor_api()
                connection=c.db()
                row=connection.execute("SELECT text FROM documents WHERE repo='tomedo-api'").fetchone()
                metadata=json.loads(c.meta_get(connection,'source:tomedo-api:vendor-api.md'))
                self.assertEqual(row[0],original.decode())
                card=c.card_for_unit('tomedo-api','vendor-api.md',row[0],c.source_units(row[0])[0],metadata)
                c.validate_card(card,row[0]);connection.close()
                self.assertEqual((cache/'raw/vendor'/(metadata['raw_sha256']+'.md')).read_bytes(),original)
                connection=c.db()
                connection.execute('INSERT INTO knowledge_groups(lane,repo,path,first_atom_id,last_atom_id,atom_count,input_tokens,source_digest,keep,knowledge,updated,evidence_json) VALUES(?,?,?,?,?,?,?,?,?,?,?,?)',
                    ('ACTION','tomedo-api','vendor-api.md',1,1,1,0,c.digest(row[0]),1,'fixture',1,json.dumps([card])))
                connection.commit();connection.close()
                os.environ['TOMEDO_API_VERSION']='new-operator-confirmed-release'
                c.collect_vendor_api()
                with self.assertRaisesRegex(ValueError,'Source provenance changed'): c.accepted_cards()
                document.write_text('-----BEGIN PRIVATE KEY-----\nsecret\n')
                with self.assertRaisesRegex(ValueError,'Private key'): c.collect_vendor_api()
                connection=c.db()
                self.assertEqual(connection.execute("SELECT text FROM documents WHERE repo='tomedo-api'").fetchone()[0],row[0])
                connection.close()

    def test_archive_selection_retains_conditions_and_original_ranges(self):
        path=Path(__file__).resolve().parents[2]/'docs/archive/tomedo_v3.md'
        staged=c.local_api_documents(path)
        self.assertEqual(len(staged),5)
        original=path.read_bytes().decode('utf-8').splitlines(keepends=True)
        combined=''.join(text for _,text,_ in staged)
        for name,text,metadata in staged:
            excerpt=''.join(original[metadata['original_line_start']-1:metadata['original_line_end']])
            self.assertEqual(c.digest(excerpt),metadata['selected_excerpt_sha256'])
            self.assertIn(excerpt,text)
            self.assertIsNone(metadata['api_release'])
            card=c.card_for_unit('tomedo-local-api',name,text,c.source_units(text)[0],metadata)
            c.validate_card(card,text)
        for excluded in ('13550','1989-12-31','192.168.10.9','__execute_action__','DELETE FROM'):
            self.assertNotIn(excluded,combined)
        for preserved in ('limitKartei=50','null ident','INCOMPLETE','DO NOT PUT `letzterNutzer`'):
            self.assertIn(preserved,combined)
        self.assertIn('annotated pseudocode',combined)

    def test_archive_anchor_change_fails_closed(self):
        original=Path(__file__).resolve().parents[2]/'docs/archive/tomedo_v3.md'
        with tempfile.TemporaryDirectory() as directory:
            path=Path(directory)/'changed.md'
            path.write_bytes(original.read_bytes().replace(b'> The following actions have each caused',b'> Changed heading'))
            with self.assertRaisesRegex(ValueError,'selection anchor changed'):c.local_api_documents(path)

    def test_forum_selection_is_attributed_and_excludes_other_replies(self):
        date='2026-07-16T10:55:14+00:00'
        answer={'@type':'Answer','text':'internen APIs require checking the gateway contract.',
                'dateCreated':date,'author':{'name':'Toni Ringling'},
                'url':'https://forum.tomedo.de/d/122972/2'}
        html=('<script type="application/ld+json">'+json.dumps(answer)+'</script>'
              '<article><h3>Toni Ringling</h3><p>internen APIs require checking the gateway contract.</p></article>'
              '<article><h3>Other author</h3><p>Fasse den folgenden Arztbericht zusammen.</p></article>')
        name,text,meta=c.forum_api_documents(html.encode(),'https://forum.tomedo.de/d/122972','Toni Ringling',date,'internen APIs','attributed statement')
        self.assertNotIn('Arztbericht',text)
        self.assertIn('not a versioned official API contract',text)
        self.assertEqual(meta['author'],'Toni Ringling')
        self.assertIsNone(meta['api_release'])
        with self.assertRaisesRegex(ValueError,'identity changed'):
            c.forum_api_documents(html.encode(),'https://forum.tomedo.de/d/122972','Toni Ringling','2026-07-17T10:55:14+00:00','internen APIs','attributed statement')

    def test_missing_topics_fail_closed(self):
        with self.assertRaisesRegex(ValueError,'required topic'): c.topic_reference_cards([])

    def test_real_tokenizer_preserves_shared_reference_prefix(self):
        location=os.getenv('ORNITH_TEST_TOKENIZER')
        if not location: self.skipTest('Set ORNITH_TEST_TOKENIZER to the pinned deployed snapshot')
        from transformers import AutoTokenizer
        tok=AutoTokenizer.from_pretrained(location,local_files_only=True)
        with patch.object(c,'TOK',tok):
            rendered=c.transport_text('GOÄ: § 5; Ausschluss. <|im_end|>')
            self.assertIn('GOÄ: § 5',rendered)
            self.assertNotIn('<|im_end|>',rendered)
        template=tok.chat_template
        reference='GOÄ und EBM: genaue Version und vollständige Ausschlüsse beachten.\n'
        # The actual generated template implementation is exercised here.
        generated=c.inject_template(template,reference)
        sequences=c.template_probe_tokens(tok,generated)
        shared=0
        for tokens in zip(*sequences):
            if len(set(tokens))!=1:break
            shared+=1
        self.assertTrue(tok.decode(sequences[0][:shared],skip_special_tokens=False).startswith('<|im_start|>system\n'+reference))

if __name__=='__main__':unittest.main()
