"""Cloud-only bounded smoke for the exact loaded PostgreSQL observer query."""
import json,subprocess,tempfile,time,traceback,uuid
from pathlib import Path
from lock_watch import LockWatch

def validate_snapshots(rows):
 assert len(rows)>=3,'Need three observer snapshots'
 assert any(l.get('native_app_alias')=='app_0' and not l.get('granted') for r in rows for l in r['locks']),'Native app lock waiter was not observed'
 assert any(a.get('blockers') and a.get('wait_event_type')=='Lock' for r in rows for a in r['activities']),'Actual blocker edge was not observed'

def main():
 out=Path('tmp/test-governance/gateway-app-lock-scope/observer-smoke');out.mkdir(parents=True,exist_ok=True)
 container=subprocess.check_output(['docker','ps','--filter','publish=5432','--format','{{.ID}}'],text=True).strip();assert container and all(c in '0123456789abcdef' for c in container)
 db='qadb'+uuid.uuid4().hex;app=str(uuid.uuid4());rows=[];watch=None;children=[];created=False
 result={'status':'fail','scope':'owned synthetic PostgreSQL18.4 DB; same postgres observer role; no Gateway/model requests','statement_timeout_ms':250,'lock_interval_target_ms':200,'sessions':[]}
 def sql(query,database='1flowbase'):
  return subprocess.run(['docker','exec',container,'psql','-U','postgres','-d',database,'-X','-qAt','-v','ON_ERROR_STOP=1','-v','VERBOSITY=verbose','-c',query],capture_output=True,text=True,timeout=10,check=True).stdout
 def child(name,query):
  err=tempfile.TemporaryFile();p=subprocess.Popen(['docker','exec',container,'psql','-U','postgres','-d',db,'-X','-qAt','-v','ON_ERROR_STOP=1','-v','VERBOSITY=verbose','-c',query],stdout=subprocess.DEVNULL,stderr=err);children.append((name,p,err))
 try:
  sql('CREATE DATABASE '+db);created=True;sql('CREATE EXTENSION pg_stat_statements',db)
  result['postgres_environment']=json.loads(sql("SELECT json_build_object('version',version(),'observer_role',current_user,'statement_timeout',current_setting('statement_timeout'),'compute_query_id',current_setting('compute_query_id'))",db))
  key="hashtextextended('native-snapshots:%s',0)"%app
  child('holder',"BEGIN;SELECT pg_advisory_xact_lock(%s);SELECT pg_sleep(4);COMMIT"%key)
  time.sleep(.3)
  child('waiter',"BEGIN;SELECT pg_advisory_xact_lock(%s);COMMIT"%key)
  watch=LockWatch(container,db,[app]);watch.enabled=True;watch.phase='observer-smoke'
  deadline=time.monotonic()+2.5
  with (out/'samples.jsonl').open('w') as stream:
   while time.monotonic()<deadline:
    for row in watch.tick():rows.append(row);stream.write(json.dumps(row)+'\n');stream.flush()
    time.sleep(.02)
  watch.enabled=False
  while watch.pending and time.monotonic()<deadline+1:
   for row in watch.tick():rows.append(row)
   time.sleep(.02)
  validate_snapshots(rows);result['status']='pass'
 except Exception as e:
  result['error']=str(e);result['traceback']=traceback.format_exc()[-16384:]
  if isinstance(e,subprocess.CalledProcessError):result['sql_failure']={'exit':e.returncode,'stderr':e.stderr[-16384:]}
 finally:
  if watch:
   try:result['observer']=watch.close()
   except Exception as e:result['observer_close_error']=str(e);result['status']='fail'
  for name,p,err in children:
   try:p.wait(timeout=6)
   except subprocess.TimeoutExpired:p.terminate();p.wait(timeout=2);result['status']='fail'
   err.seek(0);stderr=err.read(16384).decode(errors='replace');err.close();result['sessions'].append({'role':name,'exit':p.returncode,'stderr':stderr})
   if p.returncode:result['status']='fail'
  if created:
   try:sql('DROP DATABASE '+db+' WITH (FORCE)');result['cleanup']='owned DB dropped'
   except Exception as e:result['cleanup_error']=str(e);result['status']='fail'
  result['snapshots']=len(rows);(out/'result.json').write_text(json.dumps(result,indent=2));print(json.dumps(result),flush=True)
 return 0 if result['status']=='pass' else 1
if __name__=='__main__':raise SystemExit(main())
