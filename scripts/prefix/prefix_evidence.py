"""Shared pure evidence segmentation, literal formatting and exact token probes.

Extracted from the existing compilers. No network, configuration or process
inspection occurs on import. PDF segmentation remains in its domain adapter.
"""
from __future__ import annotations
import re
from functools import lru_cache
from prefix_units import source_units_core
from prefix_template import inject_reference

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
