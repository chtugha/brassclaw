#!/usr/bin/env python3
"""Activate a verified bundle via a backed-up vLLM systemd override.

Keeps the base unit and environment intact. Restores the previous override if
activation or the bounded health check fails. Run on the model host as root.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import subprocess
import time
import urllib.request


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def run(*args):
    return subprocess.check_output(args,text=True,stderr=subprocess.STDOUT).strip()


def restart_stack(use_lmcache):
    if use_lmcache:
        # LMCache imports the engine GPU pool through CUDA IPC. Stop its owner
        # and release those imports before vLLM checks free startup memory.
        run('systemctl','stop','vllm')
        run('systemctl','restart','lmcache')
        run('systemctl','start','vllm')
    else:
        run('systemctl','restart','vllm')


def activate(bundle,tokenizer,receipt):
    bundle=bundle.resolve(); tokenizer=tokenizer.resolve()
    manifest=json.loads((bundle/'prefix_manifest.json').read_text())
    for name,key in [('context_100k.md','sha256'),('vllm_prefix_template.jinja','template_sha256'),
                     ('evidence_cards.jsonl','evidence_cards_sha256')]:
        if sha(bundle/name)!=manifest[key]: raise ValueError('Bundle fingerprint mismatch: '+name)
    if sha(tokenizer/'config.json')!=manifest['model_profile']['model_config_sha256']:
        raise ValueError('Serving model config differs from the compiled profile')
    for name,expected in manifest['model_profile']['tokenizer_files_sha256'].items():
        if sha(tokenizer/name)!=expected: raise ValueError('Serving tokenizer differs: '+name)
    with urllib.request.urlopen('http://127.0.0.1:8000/v1/models',timeout=10) as response:
        models=json.load(response)['data']
    model=next(m for m in models if m['id']==manifest['model_profile']['model'])
    if model['max_model_len']<manifest['rendered_probe_tokens']+manifest['workspace_reserve_tokens']:
        raise ValueError('Serving context cannot fit this prefix and workspace reserve')
    fragment=Path(run('systemctl','show','vllm','--property=FragmentPath','--value'))
    override=Path('/etc/systemd/system/vllm.service.d/90-reference-prefix.conf')
    dropins=run('systemctl','show','vllm','--property=DropInPaths','--value').split()
    if any(path!=str(override) for path in dropins):
        raise ValueError('Additional vLLM overrides need inspection before replacing ExecStart')
    text=fragment.read_text(); lines=text.splitlines(); commands=[]
    for index,line in enumerate(lines):
        if not line.startswith('ExecStart='): continue
        command=line[len('ExecStart='):]
        while command.rstrip().endswith('\\'):
            command=command.rstrip()[:-1]+' '+lines[index+1].strip(); index+=1
        commands.append(command)
    if len(commands)!=1 or '--chat-template' in commands[0]:
        raise ValueError('Expected one base ExecStart without an existing template; inspect unit first')
    if not re.fullmatch(r'[A-Za-z0-9_./-]+',str(bundle)):
        raise ValueError('Bundle path needs explicit systemd escaping')
    use_lmcache='LMCacheMPConnector' in commands[0]
    if use_lmcache and run('systemctl','show','lmcache','--property=ActiveState','--value')!='active':
        raise ValueError('Expected the existing LMCache service to be active')
    previous=override.read_bytes() if override.exists() else None
    stamp=time.strftime('%Y%m%dT%H%M%SZ',time.gmtime())
    backup=bundle/'deployment-backups'/stamp; backup.mkdir(parents=True,mode=0o700)
    saved=backup/'vllm.service'; saved.write_bytes(fragment.read_bytes()); saved.chmod(0o600)
    if previous is not None:
        saved=backup/'90-reference-prefix.conf'; saved.write_bytes(previous); saved.chmod(0o600)
    record={'generation':manifest['generation'],'bundle':str(bundle),'base_unit':str(fragment),
            'base_unit_sha256':sha(fragment),'override':str(override),'backup':str(backup),
            'activated':False,'health_verified':False,'automatic_server_injection_verified':False,
            'lmcache_restarted_to_release_gpu_ipc':use_lmcache}
    receipt.write_text(json.dumps(record,indent=2)+'\n')
    override.parent.mkdir(parents=True,exist_ok=True)
    try:
        temp=override.with_suffix('.tmp')
        temp.write_text('[Service]\nExecStart=\nExecStart='+commands[0]+' --chat-template '+str(bundle/'vllm_prefix_template.jinja')+'\n')
        temp.chmod(0o600); temp.replace(override)
        run('systemctl','daemon-reload'); restart_stack(use_lmcache)
        print('vLLM restarted; waiting for readiness.',flush=True)
        deadline=time.monotonic()+300
        while time.monotonic()<deadline:
            try:
                with urllib.request.urlopen('http://127.0.0.1:8000/health',timeout=5) as response:
                    if response.status==200: break
            except OSError: pass
            state=subprocess.run(['systemctl','is-failed','vllm'],capture_output=True,text=True)
            if state.stdout.strip()=='failed': raise RuntimeError('vLLM startup failed')
            time.sleep(5)
        else: raise TimeoutError('vLLM readiness timed out')
        if sha(fragment)!=record['base_unit_sha256']: raise RuntimeError('Base unit changed during activation')
        record.update(activated=True,health_verified=True)
        receipt.write_text(json.dumps(record,indent=2)+'\n')
        print('Template activated; vLLM is healthy.',flush=True)
    except Exception:
        if previous is None: override.unlink(missing_ok=True)
        else: override.write_bytes(previous)
        run('systemctl','daemon-reload'); restart_stack(use_lmcache)
        record['rolled_back']=True; receipt.write_text(json.dumps(record,indent=2)+'\n')
        raise


if __name__=='__main__':
    ap=argparse.ArgumentParser(description=__doc__)
    ap.add_argument('--bundle',required=True,type=Path)
    ap.add_argument('--tokenizer',required=True,type=Path)
    ap.add_argument('--receipt',required=True,type=Path)
    args=ap.parse_args()
    if os.geteuid()!=0: raise SystemExit('Run as root on the model host')
    activate(args.bundle,args.tokenizer,args.receipt)
