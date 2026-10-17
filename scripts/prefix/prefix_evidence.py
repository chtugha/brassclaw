"""Shared pure evidence segmentation, literal formatting and exact token probes.

Extracted from the existing compilers. No network, configuration or process
inspection occurs on import. PDF segmentation remains in its domain adapter.
"""
from __future__ import annotations
import re
import json
from functools import lru_cache
from prefix_units import source_units_core

def markdown_literal(excerpt):
    fence='`'*max(3,max((len(m.group()) for m in re.finditer(r'`+',excerpt)),default=0)+1)
    return f'{fence}text\n{excerpt}' + ('' if excerpt.endswith('\n') else '\n') + fence


@lru_cache(maxsize=64)
def source_units(text: str, suffix: str = '.md') -> list[tuple[int, int, str, str]]:
    return source_units_core(text, suffix)


def template_probe_tokens(tok, template, *, complete=True):
    """Render real request variants; deduplicate before exact batch tokenization.

    Never concatenate separately tokenized reference and suffix: BPE merges at
    their boundary would make the token count and cache-prefix claim unreliable.
    """
    requests=[]
    for thinking in (False,True):
        system='Operator instructions.'
        requests.append(([{'role':'system','content':system},{'role':'user','content':'CACHE-PROBE-A'}],None,thinking))
        if not complete: continue
        for content in (system,[{'type':'text','text':system}],None,''):
            for tool_name in (None,'read_configuration','list_interfaces'):
                tools=None if tool_name is None else [{'type':'function','function':{
                    'name':tool_name,'description':'Read-only diagnostic '+tool_name,
                    'parameters':{'type':'object','properties':{}}}}]
                requests.append(([{'role':'system','content':content},
                                  {'role':'user','content':'CACHE-PROBE-B'}],tools,thinking))
        requests.append(([{'role':'user','content':'CACHE-PROBE-A'}],None,thinking))
        requests.append(([{'role':'user','content':[{'type':'text','text':'CACHE-PROBE-B'}]}],None,thinking))
        requests.append(([{'role':'system','content':system},{'role':'user','content':'Earlier question'},
                          {'role':'assistant','content':'Earlier answer'},{'role':'user','content':'CACHE-PROBE-B'}],None,thinking))
    rendered=list(dict.fromkeys(tok.apply_chat_template(messages,tools=tools,chat_template=template,
        tokenize=False,add_generation_prompt=True,enable_thinking=thinking)
        for messages,tools,thinking in requests))
    return tok(rendered,add_special_tokens=False,return_attention_mask=False)['input_ids']



def inject_reference(full, original):
    """The existing Ornith/Qwen server-owned rendering adapter, unchanged."""
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
