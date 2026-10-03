#!/usr/bin/env python3
import os,sys,time,json,select
interval=.5; tick=os.sysconf('SC_CLK_TCK'); start=time.monotonic()
api=pg=driver=mock=None
root_fence=None
denied_paths=set()
def text(p):
 if p in denied_paths:return None
 try:
  with open(p) as f:return f.read()
 except PermissionError:
  denied_paths.add(p)
  return None
 except OSError:return None
class RootIdentityFence:
 def __init__(self,roots,rows):
  if len(roots)!=4 or len(set(roots.values()))!=4 or any(not isinstance(pid,int) or pid<=1 for pid in roots.values()):
   raise ValueError('Exactly four distinct owned roots with PID > 1 required')
  current={row['pid']:row for row in rows}
  if any(pid not in current for pid in roots.values()):raise RuntimeError('Owned root absent at startup')
  self.roots={name:(pid,current[pid]['start_ticks']) for name,pid in roots.items()}
 def assert_current(self,rows):
  current={row['pid']:row for row in rows}
  for name,(pid,start_ticks) in self.roots.items():
   if pid not in current or current[pid]['start_ticks']!=start_ticks:
    raise RuntimeError('Owned root disappeared or changed identity: '+name)

def procs():
 rows=[]
 for name in os.listdir('/proc'):
  if not name.isdigit():continue
  raw=text('/proc/'+name+'/stat')
  if not raw:continue
  fields=raw[raw.rfind(')')+2:].split()
  rows.append({'pid':int(name),'ppid':int(fields[1]),'start_ticks':int(fields[19]),'cpu_ticks':int(fields[11])+int(fields[12]),'cpu_user_ticks':int(fields[11]),'cpu_system_ticks':int(fields[12]),'rss_bytes':int(fields[21])*os.sysconf('SC_PAGE_SIZE')})
 return rows
def descendants(rows,parent):
 found={parent}
 while True:
  extra={p['pid'] for p in rows if p['ppid'] in found}; new=found|extra
  if new==found:return new
  found=new
def snapshot(mark=None):
 rows=procs(); root_fence.assert_current(rows); a=descendants(rows,api); d=descendants(rows,pg); helpers=descendants(rows,driver); selected=[]
 for p in rows:
  pid=p['pid']; group='api' if pid==api else 'plugins' if pid in a else 'postgres' if pid in d else 'driver' if pid==driver else 'mock' if pid==mock else 'sampler' if pid==os.getpid() else 'observer_helpers' if pid in helpers else None
  if not group:continue
  p['comm']=(text('/proc/'+str(pid)+'/comm') or '').strip() or None
  try:p['exe_basename']=os.path.basename(os.readlink('/proc/'+str(pid)+'/exe'))
  except OSError:p['exe_basename']=None
  p['cgroup_membership']=text('/proc/'+str(pid)+'/cgroup')
  p['group']=group;p['identity']=str(pid)+':'+str(p['start_ticks']);p['started_after_window']=p['start_ticks']/tick>=start
  sm=text('/proc/'+str(pid)+'/smaps_rollup');p['pss_bytes']=None
  if sm:
   for line in sm.splitlines():
    if line.startswith('Pss:'):p['pss_bytes']=int(line.split()[1])*1024
  io=text('/proc/'+str(pid)+'/io');p['io']=dict((k,int(v)) for k,v in (line.split(':') for line in io.splitlines())) if io else None
  selected.append(p)
 mem={line.split(':')[0]:int(line.split()[1])*1024 for line in text('/proc/meminfo').splitlines()}
 fs=os.statvfs('/');cpu=list(map(int,text('/proc/stat').splitlines()[0].split()[1:9]))
 cgroups={}
 for p in selected:
  for line in (p.get('cgroup_membership') or '').splitlines():
   if not line.startswith('0::'):continue
   cg=line[3:];base='/sys/fs/cgroup'+(cg if cg!='/' else '')
   if cg not in cgroups:cgroups[cg]={'io_stat':text(base+'/io.stat'),'cpu_stat':text(base+'/cpu.stat'),'memory_current':text(base+'/memory.current'),'attribution':'unverified cgroup ownership; shared API/runner cgroup is environmental only'}
 return {'cgroups':cgroups,'host_diskstats':text('/proc/diskstats'),'monotonic_ns':time.monotonic_ns(),'mark':mark,'processes':selected,'host_cpu_ticks':cpu,'memory_available_bytes':mem['MemAvailable'],'memory_total_bytes':mem['MemTotal'],'disk_available_bytes':fs.f_bavail*fs.f_frsize,'disk_total_bytes':fs.f_blocks*fs.f_frsize}
def main():
 global api,pg,driver,mock,root_fence,start
 if len(sys.argv)!=5:raise ValueError('Four owned root PIDs required')
 api,pg,driver,mock=map(int,sys.argv[1:5])
 root_fence=RootIdentityFence({'api':api,'postgres':pg,'driver':driver,'mock':mock},procs())
 while True:
  ready,_,_=select.select([sys.stdin],[],[],interval)
  if ready:
   line=sys.stdin.readline()
   if not line:break
   value=json.loads(line)
   if value.get('stop'):break
   if value.get('mark')=='whole:begin':start=time.monotonic()
   s=snapshot(value.get('mark'))
  else:s=snapshot()
  print(json.dumps(s,separators=(',',':')),flush=True)

if __name__=='__main__':main()
