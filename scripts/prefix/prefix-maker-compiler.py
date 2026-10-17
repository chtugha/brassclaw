#!/usr/bin/env python3
"""Offline Prefix-Maker adapter; shares exact source/evidence/token machinery.

Snapshots, compilation and export make no network or provider calls. These
artifacts remain quarantined until runtime/compiler/target qualification passes.
"""
import importlib.util
from pathlib import Path

_spec = importlib.util.spec_from_file_location('brassclaw_prefix_core', Path(__file__).with_name('base-promt-compiler.py'))
core = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(core)

REQUIRED = core.MANDATORY + ('scripts/prefix/prefix-maker-reference.md',)
HEADER = '''# BrassClaw Prefix-Maker reference
Design source-backed prefixes for the explicitly supplied target. This reference
does not train weights or grant execution authority. Treat unknown architecture,
tokenizer, template, decoder and runtime/cache behavior as unknown. Propose only
registered declarative compiler options; never executable compiler code, deployment
changes or unsupported success claims. Preserve complete source evidence and
separate proposed design, validated candidate, acceptance and active selection.
'''

def compile_package(package, output, **options):
    return core.compile_package(package, output, profile='prefix-maker', header=HEADER,
                                required_sources=REQUIRED, adapter_path=__file__, **options)

if __name__ == '__main__':
    core.main(profile='prefix-maker', header=HEADER, required_sources=REQUIRED, adapter_path=__file__)
