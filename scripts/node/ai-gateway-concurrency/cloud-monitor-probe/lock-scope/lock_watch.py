"""Bounded PG read-only observer hosted inside the existing resource sampler."""
import hashlib,hmac,json,os,select,subprocess,time,tempfile

def key_tag(salt,row):
 fields=['locktype','database','relation','page','tuple','virtualxid','transactionid','classid','objid','objsubid']
 return hmac.new(salt,json.dumps([row.get(k) for k in fields],separators=(',',':')).encode(),hashlib.sha256).hexdigest()

def safe_export(data,salt,aliases):
 def alias(pid):
  if pid is None:return None
  if pid==0:return 'prepared-transaction-unknown'
  return aliases.setdefault(pid,'backend_'+str(len(aliases)))
 activities=[]
 for a in data.get('activities',[]):
  activities.append({k:a.get(k) for k in ['query_id','state','wait_event_type','wait_event','xact_age_ms','query_age_ms']}|{'backend':alias(a['pid']),'blockers':[alias(p) for p in a.get('blockers',[])]})
 locks=[]
 for row in data.get('locks',[]):
  locks.append({'backend':alias(row.get('pid')),'key_tag':key_tag(salt,row),'locktype':row.get('locktype'),'mode':row.get('mode'),'granted':row.get('granted'),'waitstart_epoch_ns':row.get('waitstart_epoch_ns'),'native_app_alias':row.get('native_app_alias'),'relation_name':row.get('relation_name')})
 return {'server_epoch_ns':data.get('server_epoch_ns'),'observer_backend_pid':data['observer_backend_pid'],'activities':activities,'locks':locks,'row_guard':{'activities':len(activities),'locks':len(locks),'cap':1000,'truncated':len(locks)>1000 or len(activities)>1000}}

class LockWatch:
 def __init__(self,container,database,appids):
  assert database.startswith('qadb') and database[4:].isalnum()
  assert 1<=len(appids)<=8 and all(len(x)==36 and all(c in 'abcdef0123456789-' for c in x) for x in appids)
  values=','.join("('%s'::uuid,'app_%d')"%(x,i) for i,x in enumerate(appids))
  self.query="""WITH native_keys AS (SELECT alias,hashtextextended('native-snapshots:'||id::text,0) AS key FROM (VALUES %s) v(id,alias)), a AS MATERIALIZED (SELECT pid,query_id::text,state,wait_event_type,wait_event,extract(epoch from(clock_timestamp()-xact_start))*1000 AS xact_age_ms,extract(epoch from(clock_timestamp()-query_start))*1000 AS query_age_ms,pg_blocking_pids(pid) AS blockers FROM pg_stat_activity WHERE datid=(SELECT oid FROM pg_database WHERE datname=current_database()) AND pid<>pg_backend_pid() AND state IS DISTINCT FROM 'idle' LIMIT 1001), locks AS (SELECT l.pid,l.locktype,l.database,l.relation,l.page,l.tuple,l.virtualxid,l.transactionid::text,l.classid,l.objid,l.objsubid,l.mode,l.granted,(extract(epoch from l.waitstart)*1000000000)::numeric(30,0)::text AS waitstart_epoch_ns,c.relname AS relation_name,n.alias AS native_app_alias FROM pg_locks l JOIN a ON a.pid=l.pid LEFT JOIN pg_class c ON l.relation=c.oid AND (l.database=(SELECT oid FROM pg_database WHERE datname=current_database()) OR l.database=0) LEFT JOIN native_keys n ON l.locktype='advisory' AND l.objsubid=1 AND ((l.classid::bigint<<32)|l.objid::bigint)=n.key WHERE NOT l.granted OR l.locktype IN ('advisory','transactionid','tuple') LIMIT 1001) SELECT json_build_object('server_epoch_ns',(extract(epoch from clock_timestamp())*1000000000)::numeric(30,0)::text,'observer_backend_pid',pg_backend_pid(),'activities',coalesce((SELECT json_agg(a) FROM a),'[]'::json),'locks',coalesce((SELECT json_agg(locks) FROM locks),'[]'::json))::jsonb;\n"""%values
  self.stderr=tempfile.TemporaryFile()
  self.process=subprocess.Popen(['docker','exec','-i',container,'psql','-U','postgres','-d',database,'-X','-qAt','-v','ON_ERROR_STOP=1','-v','VERBOSITY=verbose'],stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=self.stderr,text=True,bufsize=1)
  self.process.stdin.write("SET statement_timeout='250ms'; SET application_name='ci-lock-watch'; SET stats_fetch_consistency='none';\n");self.process.stdin.flush()
  self.salt=os.urandom(32);self.aliases={};self.pending=None;self.next=0;self.enabled=False;self.phase=None;self.bytes=0;self.samples=0;self.errors=0;self.backend_pid=None
 def tick(self):
  out=[];now=time.monotonic_ns()
  if self.process.poll() is not None:
   self.errors+=1;raise RuntimeError('owned psql observer exit='+str(self.process.returncode)+' '+self.stderr_tail().replace('\n',' '))
  if self.pending and select.select([self.process.stdout],[],[],0)[0]:
   line=self.process.stdout.readline();received=time.monotonic_ns();self.bytes+=len(line.encode());self.samples+=1
   if not line:raise RuntimeError('owned psql observer EOF '+self.stderr_tail().replace('\n',' '))
   if len(line)>2*1024*1024 or self.bytes>32*1024*1024:raise RuntimeError('lock observation output guard')
   data=safe_export(json.loads(line),self.salt,self.aliases);self.backend_pid=data['observer_backend_pid'];data.update({'type':'lock_observation','phase':self.pending['phase'],'observer_begin_ns':str(self.pending['start']),'observer_receive_ns':str(received),'observer_cost_ms':(received-self.pending['start'])/1e6,'server_to_monotonic_ns_interval':[str(self.pending['start']-int(data['server_epoch_ns'])),str(received-int(data['server_epoch_ns']))]});out.append(data);self.pending=None
   if data['row_guard']['truncated']:raise RuntimeError('lock observation row guard')
  if self.pending and now-self.pending['start']>2000000000:raise RuntimeError('lock observation missing response2s')
  if self.enabled and not self.pending and now>=self.next:
   self.pending={'start':time.monotonic_ns(),'phase':self.phase};self.process.stdin.write(self.query);self.process.stdin.flush();self.next=now+200000000
  return out
 def stderr_tail(self):
  self.stderr.seek(0,2);size=self.stderr.tell();self.stderr.seek(max(0,size-16384));return self.stderr.read().decode(errors='replace')
 def close(self):
  if self.process.poll() is None:
   self.process.stdin.close()
   try:self.process.wait(timeout=2)
   except subprocess.TimeoutExpired:self.process.terminate();self.process.wait(timeout=2)
  tail=self.stderr_tail();self.stderr.close()
  return {'stderr_tail':tail,'exit':self.process.returncode,'samples':self.samples,'export_bytes':self.bytes,'errors':self.errors,'pending_at_close':self.pending is not None,'lock_interval_target_ms':200,'statement_timeout_ms':250,'scope':'view snapshots may miss brief waits; waitstart null and disappearance remain unknown; no raw bind/key/SQL exported'}
