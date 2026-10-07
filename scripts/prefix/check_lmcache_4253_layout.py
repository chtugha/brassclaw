#!/usr/bin/env python3
"""Check the exact LMCache backport with real CPU tensors and vLLM specs."""
import argparse
import importlib.util
import json
from pathlib import Path

import torch
from vllm.v1.kv_cache_interface import FullAttentionSpec


def load(path, name):
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def check(original, patched):
    before = load(original, 'layout_before')
    after = load(patched, 'layout_after')
    spec = FullAttentionSpec(block_size=1056, num_kv_heads=4,
                             head_size=256, dtype=torch.uint8)
    n = 66 * 32 * 4 * 512
    raw = (torch.arange(n, dtype=torch.int32) % 251).to(torch.uint8)
    views = {'rank4_contiguous': raw.view(66, 4, 32, 512),
             'rank4_permuted': raw.view(66, 32, 4, 512).permute(0, 2, 1, 3),
             'rank5_legacy': raw.view(66, 2, 32, 4, 256)}
    results = []
    for name, view in views.items():
        old_matches = before._SubpagedAttentionViewEdit().matches(spec, view)
        assert old_matches == (name == 'rank5_legacy')
        rule = after._SubpagedAttentionViewEdit()
        assert rule.matches(spec, view)
        edited = rule.apply(spec, view, {'kv_layout': 'NHD'})
        assert tuple(edited.shape) == (2, 2, 1056, 1, 1024)
        assert edited.untyped_storage().data_ptr() == raw.untyped_storage().data_ptr()
        assert torch.equal(edited.reshape(-1), raw)
        assert torch.equal(edited[1].reshape(-1), raw[spec.page_size_bytes:])
        results.append({'case': name, 'before_matches': old_matches,
                        'after_matches': True, 'full_page_byte_coverage': True,
                        'shape': list(edited.shape)})
    rule = after._SubpagedAttentionViewEdit()
    # A tensor already using logical pages must not be edited again.
    assert not rule.matches(spec, raw.view(2, 4, 1056, 512))
    rejected = []
    for name, view in [('page_count_not_divisible', views['rank4_contiguous'][:65]),
                       ('noncontiguous_page_bytes', views['rank4_contiguous'][::2])]:
        try:
            rule.apply(spec, view, {'kv_layout': 'NHD'})
        except ValueError:
            rejected.append(name)
        else:
            raise AssertionError('Invalid layout accepted: ' + name)
    return {'passed': True, 'real_vllm_spec': True, 'cpu_only': True,
            'logical_block': 1056, 'physical_block': 32, 'pages_per_logical_block': 33,
            'cases': results, 'invalid_layouts_rejected': rejected,
            'scope': 'Byte layout only; live external-restore checks still required'}


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--original', type=Path, required=True)
    parser.add_argument('--patched', type=Path, required=True)
    args = parser.parse_args()
    print(json.dumps(check(args.original, args.patched), indent=2))
