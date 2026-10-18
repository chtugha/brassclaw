"""Pure, versioned Ornith/Qwen template injection; no rendering or effects.

This adapter prepares template bytes only. Its syntax anchor is deliberately
exact; unsupported templates require a reviewed adapter instead of a fallback.
"""

def inject_reference(full, original):
    """The existing Ornith/Qwen server-owned rendering adapter, unchanged."""
    import json
    wrapper='{% set _engineering_reference = '+json.dumps(full+'\n\n# REQUEST CONTEXT\n',ensure_ascii=False)+' %}\n'
    wrapper+="""{%- if not (tools and tools is iterable and tools is not mapping) -%}
{%- if messages and messages[0]['role'] == 'system' -%}
{%- if messages[0]['content'] is string -%}
{%- set _system_content = _engineering_reference ~ '\n\n' ~ messages[0]['content'] -%}
{%- elif messages[0]['content'] is iterable and messages[0]['content'] is not mapping -%}
{%- set _system_content = [{'type':'text','text':_engineering_reference ~ '\n\n'}] + messages[0]['content'] -%}
{%- elif messages[0]['content'] is none -%}
{%- set _system_content = _engineering_reference -%}
{%- else -%}
{{- raise_exception('Unexpected system content type.') -}}
{%- endif -%}
{%- set messages = [{'role':'system','content':_system_content}] + messages[1:] -%}
{%- else -%}
{%- set messages = [{'role':'system','content':_engineering_reference}] + messages -%}
{%- endif -%}
{%- endif -%}
"""
    # Original Qwen puts changing tools before system content. Insert the
    # static reference immediately after that system opening instead.
    opening="{{- '<|im_start|>system\\n' }}"
    if original.count(opening)!=1:
        raise ValueError('Deployed chat template tool-system opening changed; inspect before exporting')
    tool_first=original.replace(opening,"{{- '<|im_start|>system\\n' + _engineering_reference + '\\n\\n' }}",1)
    return wrapper+tool_first
