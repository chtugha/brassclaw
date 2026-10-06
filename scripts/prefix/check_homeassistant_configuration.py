#!/usr/bin/env python3
"""Read YAML on stdin; run safe syntax and installed HA static schema checks.

Never starts Home Assistant, resolves includes/secrets, renders templates, or
dispatches actions. Run in an isolated environment with the exact target release.
Static schemas are a subset of Home Assistant's full installation config check.
"""
import argparse
import importlib.metadata
import json
import sys


def check(text, kind, expected_version=None):
    result = {'syntax': 'not_checked', 'application_schema': 'not_checked',
              'runtime': 'not_checked', 'errors': [], 'version': None}
    try:
        from ruamel.yaml import YAML
        from ruamel.yaml.nodes import MappingNode, SequenceNode
    except ImportError:
        result['errors'].append('Install ruamel.yaml in the checker interpreter'); return result
    if len(text.encode()) > 128000:
        result['syntax'] = 'failed'; result['errors'].append('Configuration exceeds 128000 bytes'); return result
    yaml = YAML(typ='safe', pure=True)
    yaml.version = (1, 1)  # HA uses YAML 1.1 resolution, not generic YAML 1.2.
    yaml.allow_duplicate_keys = False
    try:
        node = yaml.compose(text)
        seen, custom_tags = set(), set()
        def visit(node, depth=0):
            if depth > 80:
                raise ValueError('YAML nesting exceeds 80')
            if id(node) in seen:
                raise ValueError('YAML aliases are not supported by this isolated checker')
            seen.add(id(node))
            if len(seen) > 10000:
                raise ValueError('YAML node limit exceeded')
            if not node.tag.startswith('tag:yaml.org,2002:'):
                custom_tags.add(node.tag)
            if isinstance(node, MappingNode):
                for key, value in node.value:
                    visit(key, depth+1); visit(value, depth+1)
            elif isinstance(node, SequenceNode):
                for value in node.value:
                    visit(value, depth+1)
        if node is None:
            raise ValueError('Empty YAML document')
        visit(node)
        if custom_tags:
            permitted = {'!secret', '!include', '!include_dir_list', '!include_dir_named',
                         '!include_dir_merge_list', '!include_dir_merge_named', '!input'}
            if not custom_tags <= permitted:
                raise ValueError('Unknown/unsafe YAML tag')
            result['errors'].append('Application tags require original files/secrets/blueprint context; not resolved')
            return result  # Composition alone does not establish duplicate-key or full syntax checks.
        data = yaml.load(text)
        result['syntax'] = 'passed'
    except Exception as exc:
        # Parser errors can embed secret values: retain class and location, not input snippets.
        mark = getattr(exc, 'problem_mark', None)
        location = f' at line {mark.line+1}, column {mark.column+1}' if mark is not None else ''
        result['syntax'] = 'failed'; result['errors'].append(type(exc).__name__+location); return result
    try:
        version = importlib.metadata.version('homeassistant')
        result['version'] = version
        if not expected_version:
            result['errors'].append('Exact target Home Assistant version is required'); return result
        if version != expected_version:
            result['errors'].append('Installed Home Assistant '+version+' differs from target '+expected_version); return result
        # Use HA's own loader for scalar types and its shipped static schema APIs.
        from homeassistant.util.yaml import parse_yaml
        data = parse_yaml(text)
        from homeassistant.components.modbus import CONFIG_SCHEMA as modbus_schema
        from homeassistant.components.automation.config import PLATFORM_SCHEMA as automation_schema
        from homeassistant.components.script.config import SCRIPT_ENTITY_SCHEMA as script_schema
        if kind == 'ha-modbus':
            if not isinstance(data, dict) or set(data) != {'modbus'}:
                raise ValueError('ha-modbus requires exactly a top-level modbus key')
            modbus_schema(data)
        elif kind == 'ha-automation':
            automation_schema(data)
        elif kind == 'ha-script':
            script_schema(data)
        elif kind == 'ha-configuration':
            if not isinstance(data, dict) or not data:
                raise ValueError('configuration.yaml must be a nonempty mapping')
            unsupported = set(data) - {'automation', 'script', 'modbus'}
            if unsupported:
                result['errors'].append('No static adapter for these configuration domains; use installation config check'); return result
            if 'modbus' in data:
                modbus_schema({'modbus': data['modbus']})
            if 'automation' in data:
                if not isinstance(data['automation'], list):
                    raise ValueError('automation must be a list')
                for automation in data['automation']:
                    automation_schema(automation)
            if 'script' in data:
                if not isinstance(data['script'], dict):
                    raise ValueError('script must be a mapping')
                from homeassistant.helpers.config_validation import slug
                for key, script in data['script'].items():
                    slug(key); script_schema(script)
        else:
            raise ValueError('Unsupported configuration kind')
        result['application_schema'] = 'passed'
    except (ImportError, importlib.metadata.PackageNotFoundError) as exc:
        result['errors'].append('Home Assistant checker dependency unavailable: '+type(exc).__name__)
    except Exception as exc:
        result['application_schema'] = 'failed'
        path = getattr(exc, 'path', None)
        # Voluptuous errors describe requirements without echoing provided secret values.
        message = getattr(exc, 'msg', None) or (str(exc) if isinstance(exc, ValueError) else type(exc).__name__)
        result['errors'].append(message + (' at '+str(path) if path else ''))
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--kind', required=True, choices=['ha-modbus', 'ha-automation', 'ha-script', 'ha-configuration'])
    parser.add_argument('--expected-version')
    args = parser.parse_args()
    result = check(sys.stdin.read(128001), args.kind, args.expected_version)
    print(json.dumps(result))
    return 0 if result['application_schema'] == 'passed' else 1


if __name__ == '__main__':
    sys.exit(main())
