#!/usr/bin/env python3
"""Compile the existing reviewer response schemas to compact XGrammar EBNF.

Run in the existing vLLM environment; no engine or service is created. Input is
only trusted JSON schemas, never case instructions, test oracles or model answers.
Compact separators constrain formatting outside strings, not their contents.
"""
import argparse
import hashlib
import importlib.metadata
import json
from pathlib import Path


def fingerprint(schema):
    return hashlib.sha256(json.dumps(schema, sort_keys=True, separators=(',', ':')).encode()).hexdigest()


def compile_schemas(source, output):
    import xgrammar
    schemas = json.loads(source.read_text())
    output.mkdir(parents=True, exist_ok=False)
    rows = []
    for schema in schemas:
        key = fingerprint(schema)
        grammar = str(xgrammar.Grammar.from_json_schema(schema, any_whitespace=False))
        (output / (key + '.ebnf')).write_text(grammar)
        rows.append({'schema': schema, 'schema_sha256': key,
                     'grammar_sha256': hashlib.sha256(grammar.encode()).hexdigest()})
    receipt = {'format': 'sempai-compact-grammar/1', 'any_whitespace': False,
               'xgrammar_version': importlib.metadata.version('xgrammar'), 'schemas': rows,
               'answers_or_behavioral_oracles_used': False}
    (output / 'manifest.json').write_text(json.dumps(receipt, indent=2) + '\n')
    print(json.dumps({'schemas': len(rows), 'xgrammar_version': receipt['xgrammar_version']}))


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--schemas', required=True, type=Path)
    parser.add_argument('--output', required=True, type=Path)
    args = parser.parse_args()
    compile_schemas(args.schemas, args.output)
