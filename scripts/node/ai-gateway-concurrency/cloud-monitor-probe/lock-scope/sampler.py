#!/usr/bin/env python3
import os,sys,time,json,select
from lock_watch import LockWatch
watch=LockWatch(sys.argv[5],sys.argv[6],json.loads(sys.argv[7]))
api,pg,driver,mock=map(int,sys.argv[1:5]); interval=.5; tick=os.sysconf('SC_CLK_TCK'); start=time.monotonic()
def text(p):
 try:
  with open(p) as f:return f.read()
 except OSError:return None
def procs():
 rows=[]
 for name in os.listdir('/proc'):
  if not name.isdigit():continue
  raw=text('/proc/'+name+'/stat')
  if not raw:continue
  fields=raw[raw.rfind(')')+2:].split()
  rows.append({'pid':int(name),'ppid':int(fields[1]),'start_ticks':int(fields[19]),'cpu_ticks':int(fields[11])+int(fields[12]),'rss_bytes':int(fields[21])*os.sysconf('SC_PAGE_SIZE')})
 return rows
def descendants(rows,parent):
 found={parent}
 while True:
  extra={p['pid'] for p in rows if p['ppid'] in found}; new=found|extra
  if new==found:return new
  found=new
def snapshot(mark=None):
 rows=procs(); a=descendants(rows,api); d=descendants(rows,pg); selected=[]
 for p in rows:
  pid=p['pid']; group='api' if pid==api else 'plugins' if pid in a else 'postgres' if pid in d else 'driver' if pid==driver else 'mock' if pid==mock else 'sampler' if pid==os.getpid() else None
  if not group:continue
  p['group']=group;p['postgres_observer_backend']=pid==watch.backend_pid;p['identity']=str(pid)+':'+str(p['start_ticks']);p['started_after_window']=p['start_ticks']/tick>=start
  sm=text('/proc/'+str(pid)+'/smaps_rollup');p['pss_bytes']=None
  if sm:
   for line in sm.splitlines():
    if line.startswith('Pss:'):p['pss_bytes']=int(line.split()[1])*1024
  io=text('/proc/'+str(pid)+'/io');p['io']=dict((k,int(v)) for k,v in (line.split(':') for line in io.splitlines())) if io else None
  selected.append(p)
 mem={line.split(':')[0]:int(line.split()[1])*1024 for line in text('/proc/meminfo').splitlines()}
 fs=os.statvfs('/');cpu=list(map(int,text('/proc/stat').splitlines()[0].split()[1:9]))
 return {'monotonic_ns':time.monotonic_ns(),'mark':mark,'processes':selected,'host_cpu_ticks':cpu,'memory_available_bytes':mem['MemAvailable'],'memory_total_bytes':mem['MemTotal'],'disk_available_bytes':fs.f_bavail*fs.f_frsize,'disk_total_bytes':fs.f_blocks*fs.f_frsize}
try:
 next_resource=0
 while True:
  for row in watch.tick():print(json.dumps(row,separators=(',',':')),flush=True)
  ready,_,_=select.select([sys.stdin],[],[],.02)
  if ready:
   line=sys.stdin.readline()
   if not line:break
   value=json.loads(line)
   if value.get('stop'):break
   mark=value.get('mark')
   if mark=='whole:begin':start=time.monotonic()
   if mark and mark.endswith(':begin'):
    watch.phase=mark[:-6];watch.enabled=watch.phase=='fixedstream_h524288_c8'
   if mark and mark.endswith(':end') and mark[:-4]==watch.phase:watch.enabled=False
   s=snapshot(mark);next_resource=time.monotonic()+interval
  elif time.monotonic()>=next_resource:
   s=snapshot();next_resource=time.monotonic()+interval
  else:continue
  print(json.dumps(s,separators=(',',':')),flush=True)
finally:
 print(json.dumps({'type':'lock_observer_summary',**watch.close()},separators=(',',':')),flush=True)
