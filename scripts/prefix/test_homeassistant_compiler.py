"""Profile and evidence tests; use ORNITH_TEST_TOKENIZER for exact card budgets."""
import importlib.util
import os
from pathlib import Path
import re
import unittest
from unittest.mock import patch

spec=importlib.util.spec_from_file_location('ha_compiler',Path(__file__).with_name('Homeassistant-compiler.py'))
c=importlib.util.module_from_spec(spec); spec.loader.exec_module(c)

class HomeAssistantTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.tok=None
        if os.getenv("ORNITH_TEST_TOKENIZER"):
            from transformers import AutoTokenizer
            cls.tok=AutoTokenizer.from_pretrained(os.environ["ORNITH_TEST_TOKENIZER"],local_files_only=True)

    def test_profile_and_topic_rules(self):
        self.assertEqual(set(c.DEFAULT_REQUIRED_SOURCES),set(c.DEFAULT_REPOS)|set(c.WEB_SOURCES))
        self.assertIn('homeassistant_evidence',str(c.BASE))
        for sources,path,content in c.REQUIRED_TOPICS.values():
            self.assertTrue(set(sources)<=set(c.DEFAULT_REQUIRED_SOURCES))
            re.compile(path); re.compile(content)
        for topic in ('ha-yaml','ha-includes','ha-secrets','ha-blueprints','yaml-parser-safety'):
            self.assertIn(topic,c.REQUIRED_TOPICS)

    def test_markdown_yaml_and_jinja_are_literal_evidence(self):
        document='''# Configuration
Preserve the application tags and sequence.
## YAML script
```yaml
script: !include scripts.yaml
password: !secret mqtt_password
sequence:
  - variables:
      target: !input target
      message: >-
        {{ states('sensor.temperature') }}
# This is a YAML comment, not a document section.
```
'''
        units=c.source_units(document,'.markdown')
        self.assertEqual(len(units),2)
        card=c.card_for_unit('ha-user','source/_docs/scripts/example.markdown',document,units[-1],
                             {'url':'https://example.test/fixture','snapshot':c.digest(document)})
        c.validate_card(card,document)
        for literal in ('!include scripts.yaml','!secret mqtt_password','!input target',
                        "{{ states('sensor.temperature') }}",'      message: >-'):
            self.assertIn(literal,card['excerpt'])
            if os.getenv("ORNITH_TEST_TOKENIZER"):
                with patch.object(c,"TOK",self.tok): self.assertIn(literal,c.render_card(card))

    def test_web_directory_document_has_a_markdown_suffix(self):
        path=c.web_document_path("https://yaml.org/spec/1.2.2/")
        self.assertEqual(Path(path).suffix,".md")
        self.assertEqual(len(c.source_units("# One\nLiteral.\n## Two\nNested.\n",Path(path).suffix)),2)

    def test_mdx_yields_complete_sections_without_execution(self):
        document="import Sample from './sample';\n# MQTT\n```yaml\nmqtt:\n  broker: 192.0.2.1\n```\n## Discovery\nSet discovery appropriately.\n"
        units=c.source_units(document,".mdx")
        self.assertEqual(len(units),3)
        self.assertIn("  broker: 192.0.2.1",units[1][2])

    def test_missing_yaml_topic_fails_closed(self):
        with self.assertRaisesRegex(ValueError,'required topic'):
            c.topic_reference_cards([])

    @unittest.skipUnless(os.getenv('ORNITH_TEST_TOKENIZER'),'Requires actual Ornith tokenizer')
    def test_required_topics_select_complete_cards(self):
        text='# YAML configuration\nUse indentation and quoted mappings.\n'+('Preserve nested sequences and application-specific schema rules.\n'*8)
        card=c.card_for_unit('ha-user','source/_docs/configuration/yaml.markdown',text,c.source_units(text,'.markdown')[0],
                             {'snapshot':c.digest(text),'url':'https://example.test/fixture'})
        with patch.object(c,'TOK',self.tok),patch.object(c,'REQUIRED_TOPICS',{'ha-yaml':c.REQUIRED_TOPICS['ha-yaml']}):
            c.count.cache_clear()
            self.assertEqual(c.topic_reference_cards([card])['ha-yaml']['excerpt'],text)
        c.count.cache_clear()

    def test_unreadable_pdf_is_not_silently_accepted(self):
        with self.assertRaises(Exception):
            c.pdf_document(b'not a PDF')

if __name__=='__main__': unittest.main()
