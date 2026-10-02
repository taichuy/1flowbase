"""Recompute diagnostics from saved synthetic observations; no network or collector."""
import json,sys
from pathlib import Path
from ci_observation_schema import review
root=Path(sys.argv[1]);rows=[]
for file in sorted(root.glob('O_*/result.json')):
 data=json.loads(file.read_text())
 for phase in data.get('phases',[]):
  for request in phase.get('requests',[]):
   if not request.get('observation_trace'):continue
   result=review(request['observation'],request['observation_trace'],data.get('eventloop_lag',[]),data.get('eventloop_probe_interval_ms',50))
   rows.append({'fixture':data['label'],'phase':phase['name'],'probe_id':request['observation']['probe_id'],'review':result})
(root/'boundary-review.json').write_text(json.dumps({'lane':'observation only; never pooled with performance','rows':rows},indent=2))
print(json.dumps({'reviewed':len(rows),'invalid':sum(r['review']['status']!='boundary_review' for r in rows)}))
if any(r['review']['status']!='boundary_review' for r in rows):raise SystemExit(1)
