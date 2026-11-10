#!/usr/bin/env python3
import json,os,re,sys,time
from pathlib import Path
real,args=sys.argv[1],sys.argv[2:]
def value(flag):
    if flag in args:
        return args[args.index(flag)+1]
    return next((x.split('=',1)[1] for x in args if x.startswith(flag+'=')),None)
crate=value('--crate-name')
match=(value('--crate-type')=='lib' and '--test' not in args and ((crate=='api_server' and any(Path(x).name=='lib_root.rs' for x in args)) or (crate=='model_definitions_http' and any(Path(x).name=='lib.rs' for x in args))))
match = match or (os.environ.get('ISSUE2818_RUSTC_MODE')=='observe-only' and value('--crate-name')=='console_core_registry_tests' and '--test' in args)
env=dict(os.environ)
effective=list(args)
compile_unit=any(x.endswith('.rs') for x in args) and '-Zhelp' not in args and any(x=='--emit' or x.startswith('--emit=') for x in args)
if compile_unit and env.get('ISSUE2818_EXPECT_NO_RUST_COMPILE')=='1':
    with open(env['ISSUE2818_ROOT_LOG'],'a') as log:log.write(json.dumps({'unexpected_compile_blocked':True,'crate':crate,'original_argv':[real,*args]})+'\n')
    os._exit(86)
allocator_policy=env.pop('ISSUE2818_RUSTC_MALLOC_CONF',None)
if allocator_policy not in (None,'dirty_decay_ms:0,muzzy_decay_ms:0'):
    raise RuntimeError('unexpected compiler allocator policy')
if compile_unit and allocator_policy:
    assert not env.get('MALLOC_CONF'), 'parent allocator configuration must remain unset'
    env['MALLOC_CONF']=allocator_policy
if compile_unit:
    with open(env['ISSUE2818_ALL_RUSTC_LOG'],'a') as log:log.write(json.dumps({'crate':crate,'original_argv':[real,*args],'source_sha':env.get('ISSUE2818_OBSERVED_SOURCE_SHA'),'allocator_conf':env.get('MALLOC_CONF')})+'\n')
if match:
    mode=env.get('ISSUE2818_RUSTC_MODE','diagnostic')
    if mode=='diagnostic':
        effective+=['-Ztime-passes','-Ztime-passes-format=json']
        if env.get('ISSUE2818_MONO_STATS')=='1':
            effective+=['-Zdump-mono-stats','-Zdump-mono-stats-format=json']
        env['RUSTC_BOOTSTRAP']=crate
    auth=re.search(r'--jobserver-(?:auth|fds)=(\d+),(\d+)',env.get('CARGO_MAKEFLAGS',''))
    fds={}
    for fd in (map(int,auth.groups()) if auth else []):
        try:
            os.fstat(fd)
            fds[str(fd)]='open'
        except OSError:
            fds[str(fd)]='closed'
    row={'pid':os.getpid(),'monotonic':time.monotonic(),'mode':mode,'probe':'-Zhelp' in args,
         'original_argv':[real,*args],'effective_argv':[real,*effective],
         'bootstrap_for_this_exec':env.get('RUSTC_BOOTSTRAP'),'jobserver_fds':fds,'allocator_conf':env.get('MALLOC_CONF')}
    with open(env['ISSUE2818_ROOT_LOG'],'a') as log:
        log.write(json.dumps(row)+'\n')
        log.flush()
    if mode=='observe-only':
        os._exit(86)
os.execve(real,[real,*effective],env)
