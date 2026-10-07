import importlib.util,json
from pathlib import Path
root=Path('/Volumes/SSDE/brassclaw');p=root/'scripts/prefix/sempai-reference-policy.json'
spec=importlib.util.spec_from_file_location('review',root/'scripts/prefix/sempai-compiler.py');m=importlib.util.module_from_spec(spec);spec.loader.exec_module(m)
old=json.loads(p.read_text());prior={(u['path'],u['heading']):u for u in old['units']}
complete={'recipe.md','skills.md','tools.md','toolskills.md','scripts/prefix/sempai-authoring-reference.md','scripts/prefix/sempai-worked-examples.md','crates/brassclaw_interceptor/src/packet.rs'}
units=[]
for path in m.SOURCE_DOCUMENTS:
 text=(root/path).read_text()
 for start,end,excerpt,parents in m.source_units(text,Path(path).suffix):
  heading=excerpt.splitlines()[0];prev=prior.get((path,heading))
  if path not in complete and prev is None:raise ValueError('Review new unit: '+path+' '+heading)
  units.append({'path':path,'line_start':start,'line_end':end,'heading':heading,'document_sha256':m.digest(text),'excerpt_sha256':m.digest(excerpt),'selected':True if path in complete else prev['selected'],'reason':'Complete authoritative guide or verified tutorial, source reviewed' if path in complete else prev['reason']})
old.update(units=units,source_units=len(units),selected_units=sum(u['selected'] for u in units),review_revision='2026-10-06-sempai-verified-examples-1',review_notes='Preserved prior guide/partial-unit selections. Added the complete 24-example authored tutorial with matching actual Linux verification and mutation receipts. This is source review, not Q1/Q2 component approval.')
p.write_text(json.dumps(old,indent=2,ensure_ascii=False)+'\n');print(len(units),'preserved;',old['selected_units'],'selected')
