import fcntl
import importlib.util
import os
os.environ['CODING_DISTILL_CACHE']='/root/ornith_evidence_v3'
os.environ['CODING_WEB_SOURCES']='linux-admin'
spec=importlib.util.spec_from_file_location('p','/root/coding_mode_prefix_candidate.py')
p=importlib.util.module_from_spec(spec); spec.loader.exec_module(p)
with (p.BASE/'pipeline.lock').open('a') as lock:
    fcntl.flock(lock,fcntl.LOCK_EX | fcntl.LOCK_NB)
    p.fetch_web_documents()
    p.ensure_pipeline_version(); p.atoms_build(); p.triage()
    for lane in ('ACTION','REASONING','THINKING'): p.distill_lane(lane)
    p.reduce_all(); p.build_context()
