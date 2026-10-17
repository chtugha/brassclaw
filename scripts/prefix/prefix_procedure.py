"""Retain exact development compiler procedures without claiming execution trust.

Snapshots contain bytes, not executable imports or deployment paths. Each caller
owns an independent snapshot; publication fails if a procedure changes meanwhile.
"""
import hashlib
import json
import os
from pathlib import Path


def exact_procedure_read(path):
    with Path(path).open('rb') as source:
        before = os.fstat(source.fileno())
        data = source.read()
        after = os.fstat(source.fileno())
    if (before.st_ino, before.st_size, before.st_mtime_ns) != (after.st_ino, after.st_size, after.st_mtime_ns):
        raise ValueError('compiler procedure changed during read')
    data.decode('utf-8')
    return data


def snapshot_procedure(entry, helpers=('prefix_evidence.py',)):
    entry = Path(entry)
    paths = {'compiler/entry.py': entry,
             'compiler/prefix_procedure.py': Path(__file__)}
    for name in helpers:
        if Path(name).name != name or not name.endswith('.py') or name.startswith('.'):
            raise ValueError('registered procedure helper filename required')
        key = 'compiler/' + name
        if key in paths:
            raise ValueError('duplicate procedure helper')
        paths[key] = entry.with_name(name)
    return {key: (path, exact_procedure_read(path)) for key, path in paths.items()}


def finish_procedure(snapshot):
    for path, original in snapshot.values():
        if exact_procedure_read(path) != original:
            raise ValueError('compiler procedure changed during compilation; no generation published')
    files = {key: hashlib.sha256(original).hexdigest()
             for key, (_, original) in sorted(snapshot.items())}
    provenance = {'format': 'brassclaw-development-prefix-procedure/1',
                  'execution_attestation': 'unqualified-development-process',
                  'files': files}
    identity = hashlib.sha256(json.dumps(provenance, ensure_ascii=False, sort_keys=True,
                                        separators=(',', ':')).encode()).hexdigest()
    return identity, provenance, {key: original.decode('utf-8')
                                  for key, (_, original) in snapshot.items()}
