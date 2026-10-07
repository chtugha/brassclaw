#!/usr/bin/env python3
"""Apply the inspected brassclaw LMCache hybrid fix on its existing installation.

Run on brassclaw with --apply. Preserves base units and the prefix drop-in.
Stops inference briefly so the existing engine re-registers the new cache layout.
No packages, model files, virtual environments or Compose files are changed.
"""
import argparse
import hashlib
import json
import shutil
import subprocess
from datetime import datetime, timezone
from pathlib import Path

BASE_SHA = '0a61271c8e7901796101d2a2ca8abcb97bb06e8ffb18168bbbb89da826101192'
COMMAND = ['/opt/vLLM/.venv/bin/python3', '/opt/vLLM/.venv/bin/lmcache',
           'server', '--host', '127.0.0.1', '--port', '5555', '--l1-size-gb', '24',
           '--eviction-policy', 'LRU', '--chunk-size', '1056',
           '--worker-reap-timeout-seconds', '0']


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def apply():
    base = Path('/etc/systemd/system/vllm.service')
    lm_base = Path('/etc/systemd/system/lmcache.service')
    prefix = Path('/etc/systemd/system/vllm.service.d/90-reference-prefix.conf')
    dropin = Path('/etc/systemd/system/lmcache.service.d/91-hybrid-object-groups.conf')
    if digest(base) != BASE_SHA:
        raise RuntimeError('Base vLLM unit changed; inspect before proceeding')
    if dropin.exists():
        raise RuntimeError('Fix drop-in already exists; preserve and inspect it')
    pid = subprocess.check_output(['systemctl', 'show', 'lmcache.service',
                                   '--property=MainPID', '--value'], text=True).strip()
    argv = Path('/proc', pid, 'cmdline').read_bytes().decode().rstrip('\0').split('\0')
    if argv != COMMAND:
        raise RuntimeError('LMCache arguments differ from inspected deployment')
    help_text = subprocess.check_output([COMMAND[0], COMMAND[1], 'server', '--help'],
                                        text=True, stderr=subprocess.STDOUT)
    if '--separate-object-groups' not in help_text:
        raise RuntimeError('Installed CLI does not support hybrid separation')
    originals = {str(path): digest(path) for path in (base, lm_base, prefix)}
    stamp = datetime.now(timezone.utc).strftime('%Y%m%dT%H%M%SZ')
    backup = Path('/root/lmcache-hybrid-fix-' + stamp)
    backup.mkdir(mode=0o700)
    shutil.copy2(lm_base, backup / 'lmcache.service')
    receipt = {'original_hashes': originals, 'dropin': str(dropin),
               'backup': str(backup), 'status': 'prepared'}
    receipt_path = backup / 'receipt.json'
    def save():
        receipt_path.write_text(json.dumps(receipt, indent=2) + '\n')
    save()
    subprocess.run(['systemctl', 'stop', 'vllm.service'], check=True)
    subprocess.run(['systemctl', 'stop', 'lmcache.service'], check=True)
    try:
        dropin.parent.mkdir(exist_ok=True)
        dropin.write_text('[Service]\nExecStart=\nExecStart=' +
                          ' '.join(COMMAND[1:]) + ' --separate-object-groups\n')
        subprocess.run(['systemctl', 'daemon-reload'], check=True)
        subprocess.run(['systemctl', 'start', 'lmcache.service'], check=True)
        subprocess.run(['systemctl', 'start', 'vllm.service'], check=True)
        for name, original in originals.items():
            if digest(Path(name)) != original:
                raise RuntimeError('Original configuration changed: ' + name)
        receipt['status'] = 'applied; readiness and external-restore tests still required'
        receipt['dropin_sha256'] = digest(dropin)
        save()
    except Exception:
        # Restore only this newly created override, preserving unrelated files.
        subprocess.run(['systemctl', 'stop', 'vllm.service'])
        subprocess.run(['systemctl', 'stop', 'lmcache.service'])
        if dropin.exists():
            dropin.rename(backup / 'failed-91-hybrid-object-groups.conf')
        subprocess.run(['systemctl', 'daemon-reload'])
        subprocess.run(['systemctl', 'start', 'lmcache.service'])
        subprocess.run(['systemctl', 'start', 'vllm.service'])
        receipt['status'] = 'application failed; override rolled back'
        save()
        raise
    print(json.dumps(receipt), flush=True)


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--apply', action='store_true', required=True)
    parser.parse_args()
    apply()
