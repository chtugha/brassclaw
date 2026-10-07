#!/usr/bin/env python3
"""Backport upstream PR 4253's one adapter class to the inspected LMCache 0.5.5.

Requires the pinned backported.py and --apply. Keeps an exact server-side backup.
No package upgrade or base-unit edit; existing services re-register fresh caches.
"""
import argparse
import hashlib
import json
import shutil
import subprocess
from datetime import datetime, timezone
from pathlib import Path

ORIGINAL = '909bc3bf5ef87741f623c19aedd12499bea3f399ea79e416911449cbea197b70'
PATCHED = 'bb8d04b6676315ee8253639b702f22dcd259700e340a2e71c1563193ea52f9af'
TARGET = Path('/opt/vLLM/.venv/lib/python3.13/site-packages/lmcache/integration/vllm/kv_cache_group_edits.py')
VLLM_BASE = '0a61271c8e7901796101d2a2ca8abcb97bb06e8ffb18168bbbb89da826101192'


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def apply(source):
    units = [Path('/etc/systemd/system/vllm.service'),
             Path('/etc/systemd/system/lmcache.service'),
             Path('/etc/systemd/system/vllm.service.d/90-reference-prefix.conf'),
             Path('/etc/systemd/system/lmcache.service.d/91-hybrid-object-groups.conf')]
    if sha(TARGET) != ORIGINAL or sha(source) != PATCHED:
        raise RuntimeError('Adapter/source differs from reviewed pinned patch')
    if sha(units[0]) != VLLM_BASE:
        raise RuntimeError('Original vLLM base configuration changed')
    hashes = {str(path): sha(path) for path in units}
    backup = Path('/root/lmcache-4253-backport-' + datetime.now(timezone.utc).strftime('%Y%m%dT%H%M%SZ'))
    backup.mkdir(mode=0o700)
    shutil.copy2(TARGET, backup / 'kv_cache_group_edits.original.py')
    record = {'upstream_pr': 'https://github.com/LMCache/LMCache/pull/4253',
              'upstream_commit': '1847435fa401330a5cf7caa6e86aaf873b2a043b',
              'target': str(TARGET), 'original_sha256': ORIGINAL,
              'patched_sha256': PATCHED, 'unit_hashes': hashes,
              'backup': str(backup), 'status': 'prepared'}
    def save():
        (backup / 'receipt.json').write_text(json.dumps(record, indent=2) + '\n')
    save()
    subprocess.run(['systemctl', 'stop', 'vllm.service'], check=True)
    subprocess.run(['systemctl', 'stop', 'lmcache.service'], check=True)
    try:
        TARGET.write_bytes(source.read_bytes())
        subprocess.run(['systemctl', 'start', 'lmcache.service'], check=True)
        subprocess.run(['systemctl', 'start', 'vllm.service'], check=True)
        if sha(TARGET) != PATCHED or any(sha(Path(path)) != value for path, value in hashes.items()):
            raise RuntimeError('Post-application fingerprints differ')
        record['status'] = 'applied; live external-restore validation still required'
        save()
    except Exception:
        subprocess.run(['systemctl', 'stop', 'vllm.service'])
        subprocess.run(['systemctl', 'stop', 'lmcache.service'])
        shutil.copy2(backup / 'kv_cache_group_edits.original.py', TARGET)
        subprocess.run(['systemctl', 'start', 'lmcache.service'])
        subprocess.run(['systemctl', 'start', 'vllm.service'])
        record['status'] = 'application failed; adapter rolled back'
        save()
        raise
    print(json.dumps(record), flush=True)


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--source', type=Path, required=True)
    parser.add_argument('--apply', action='store_true', required=True)
    args = parser.parse_args()
    apply(args.source)
