#!/usr/bin/env python3
"""Shared managed proposal; original workload and C01 plus existing managed lifecycle cases."""
import gzip,hashlib,importlib.util,json,os,re,signal,socket,subprocess,sys,time
from pathlib import Path
from collections import Counter
HERE=Path(__file__).resolve().parent;ROOT=HERE.parents[4]
MANAGED_CASES=['_tests::interface_lifecycle_acceptance::managed_create_pair::root_2007_ac_003_004_managed_create_pair', '_tests::interface_lifecycle_acceptance::managed_interfaces::root_2014_ac_001_complete_compiled_profiles', '_tests::interface_lifecycle_acceptance::managed_interfaces::root_2014_ac_002_installed_two_interface_plugin', '_tests::interface_lifecycle_acceptance::managed_interfaces::root_2014_ac_003_unary_stream_terminals']
def read(p):return json.loads(Path(p).read_text())
def sha(p):
 digest=hashlib.sha256()
 with Path(p).open('rb') as handle:
  for chunk in iter(lambda:handle.read(1024*1024),b''):digest.update(chunk)
 return digest.hexdigest()
def save(p,v):Path(p).parent.mkdir(parents=True,exist_ok=True);Path(p).write_text(json.dumps(v,indent=2,sort_keys=True)+'\n')
def check(execution=False):
 b=read(HERE/'binding.json')
 assert re.fullmatch(r'[0-9a-f]{40}',b['candidate'])
 assert b['managed_cases']==MANAGED_CASES and b['postgres_major']==16
 assert {'sdk-worker','managed-lifecycle','postgres-owned'}<=set(b['stage_order'])
 assert b['tracked_diff_not_AC006']['net'] < 0, 'tracked source reduction is necessary but not AC006 qualification'
 assert not execution or (b['execution_authorized'] and b['proposal_status']=='READY_FOR_ROOT_APPROVED_EXECUTION' and b['candidate'] not in b['rejected_candidate_shas'])
 assert not os.environ.get('CANDIDATE_SHA') or os.environ['CANDIDATE_SHA']==b['candidate']
 assert all(sha(ROOT/p)==h for p,h in b['inputs'].items())
 return b

def main():
 task_started=time.monotonic();pg={'process':None};pg_cleanup_error=None;primary_error=None;initial=None
 b=check(execution=True);s=importlib.util.spec_from_file_location('owner',HERE.parent/'experiment.py');g=importlib.util.module_from_spec(s);s.loader.exec_module(g)
 g.SEED_SHA=b['cache']['source_sha'];g.PRIOR_RUN=str(b['cache']['run']);g.PRIOR_NAME=b['cache']['name'];g.PRIOR_ARTIFACT_ID=b['cache']['id'];g.ARCHIVE_SHA=b['cache']['archive_sha256'];g.ARCHIVE_BYTES=b['cache']['archive_bytes'];g.END=time.monotonic()+4200;g.ROOT_WRAPPER=(HERE/'root-wrapper.py').read_text()
 g.EVIDENCE.mkdir(parents=True,exist_ok=False);g.TEMP.mkdir(parents=True,exist_ok=False);e=g.EVIDENCE;source=g.TEMP/'seed-source';target=g.TEMP/'target-seed';state={'source_sha':b['candidate'],'active_source_sha':b['candidate'],'phase':'prepare','completed':[],'stages':{name:'NotReached' for name in b['stage_order']},'old_new_four_filter_retention':'Incomplete','C01_pair':'Incomplete','not_Root_PASS':True};diagnostic=None;c01=None;c01_original=None
 def cap(argv):return subprocess.check_output(argv,cwd=source,text=True,stderr=subprocess.STDOUT,timeout=60).strip()
 def artifacts(row):
  out=[]
  for line in Path(row['merged_log']).read_text(errors='replace').splitlines():
   try:x=json.loads(line)
   except ValueError:continue
   if x.get('reason')=='compiler-artifact':out.append(x)
  save(e/(row['label']+'-artifacts.json'),{'source_sha':state['active_source_sha'],'artifacts':out,'Fresh':sum(x['fresh'] for x in out),'rebuilt':sum(not x['fresh'] for x in out)});return out
 def run(label,argv,env=None,seconds=600,expected=0,cwd=None):
  state['phase']=label;save(e/'qualification-phase-state.json',state)
  try:return g.run(label,argv,cwd=source/'api' if cwd is None else cwd,env=env,heavy=True,seconds=seconds,expected=expected)
  finally:
   if g.RESULTS and g.RESULTS[-1]['label']==label:artifacts(g.RESULTS[-1])
 def json_argv(argv):
  a=list(argv);a.insert(a.index('--') if '--' in a else len(a),'--message-format=json');return a
 def names(text):return re.findall(r'^(.+): (?:test|benchmark)$',text,re.M)
 def prove_actual(text,expected):
  actual=re.findall(r'^test (.+?) \.\.\. (ok|FAILED|ignored)(?:[, ].*)?$',text,re.M);assert Counter(actual)==Counter((n,'ok') for n in expected) and expected,'actual missing/zero/failed/ignored cases';return actual
 def binary_compile(label,argv,root_name,env):
  row=run(label,argv,env);items=artifacts(row);roots=[x for x in items if x['target']['name']==root_name and x.get('executable')];assert len(roots)==1;root=roots[0];binary=Path(root['executable']);assert binary.is_file();proof={'artifact':root,'binary_sha256':sha(binary),'source_sha':b['candidate']};save(e/(label+'-binary.json'),proof);return root,binary
 def list_binary(label,binary,env):
  rows=[]
  for suffix,args in [('list',['--list']),('ignored',['--list','--ignored'])]:
   row=run(label+'-'+suffix,[str(binary),*args],env,120);rows.append(names(Path(row['merged_log']).read_text()))
  assert rows[0] and set(rows[1])<=set(rows[0]);save(e/(label+'-discovery.json'),{'source_sha':b['candidate'],'binary_sha256':sha(binary),'listed':rows[0],'ignored':rows[1]});return rows
 def stage(name,status):
  state['stages'][name]=status;save(e/'qualification-phase-state.json',state)
 def rustc_rows(start=0):
  log=e/'all-rustc-invocations.jsonl'
  return [json.loads(line) for line in log.read_text().splitlines()[start:]] if log.exists() else []
 def crate_name(row):
  args=row.get('original_argv',[])
  return args[args.index('--crate-name')+1] if '--crate-name' in args else None
 def pg_identity():
  started=read(pg['folder']/'started.json');worker=read(pg['folder']/'worker-ready.json');command=read(pg['folder']/'command-started.json');pid=command['pid'];lines=(pg['data']/'postmaster.pid').read_text().splitlines();budget=os.environ['ONEFLOWBASE_TASK_BUDGET_CGROUP'];inode=int(os.environ['ONEFLOWBASE_TASK_BUDGET_ID']);expected=budget+'/'+started['unit'];group=Path('/sys/fs/cgroup')/expected.lstrip('/')
  assert started['controller']['pid']==pg['process'].pid and started['declared_budget']['cgroup']==budget and int(started['declared_budget']['inode'])==inode
  assert started['argv']==pg['argv'] and worker['cgroup']==command['cgroup']==expected and group.stat().st_ino==worker['inode']
  assert Path('/sys/fs/cgroup',budget.lstrip('/')).stat().st_ino==inode and pg['process'].poll() is None
  assert int(lines[0])==pid and Path(lines[1]).resolve()==pg['data'].resolve() and int(lines[3])==pg['port'] and lines[-1].strip()=='ready'
  assert int(Path(f'/proc/{pid}/stat').read_text().rsplit(')',1)[1].split()[19])==command['starttime_ticks']
  assert Path(f'/proc/{pid}/exe').resolve()==pg['bin']['postgres'].resolve() and Path(f'/proc/{pid}/cgroup').read_text().strip()=='0::'+expected
  return {'started':started,'worker':worker,'command':command,'postmaster_pid':lines,'job':{'unit':started['unit'],'cgroup':expected,'inode':worker['inode']},'budget':{'cgroup':budget,'inode':inode}}
 def finish_postgres(cancel=False):
  process=pg.get('process')
  if process is None:return
  if cancel and process.poll() is None:process.send_signal(signal.SIGTERM)
  try:
   try:code=process.wait(timeout=20)
   except subprocess.TimeoutExpired:
    process.send_signal(signal.SIGTERM);code=process.wait(timeout=20)
  finally:
   receipt=read(pg['folder']/'finished.json') if (pg['folder']/'finished.json').exists() else None
   save(e/'postgres-owned-lifetime.json',{'source_sha':b['candidate'],'argv':pg['argv'],'receipt':str(pg['folder']/'finished.json'),'driver_pid':process.pid,'driver_exit':process.poll(),'elapsed_wall_seconds':time.monotonic()-pg['started'],'normal_shutdown_requested':not cancel,'driver_finished':receipt,'overlaps_test_commands':True,'included_in_command_wall_sum':False,'included_in_whole_task_budget':True})
  assert receipt and receipt['cleanup']['complete'] and receipt['cleanup']['cgroup_empty'] and receipt['cleanup']['launcher_reaped'],'PostgreSQL owned driver cleanup is incomplete'
  if pg.get('identity'):
   assert receipt['budget']==pg['identity']['budget'] and all(receipt['job'].get(k)==v for k,v in pg['identity']['job'].items())
  if not cancel:assert code==0 and receipt['classification']=='exit' and receipt['command']['returncode']==0 and not receipt['oom_evidence'],'PostgreSQL did not stop normally'
  pg['process']=None
 def start_postgres():
  stage('postgres-owned','Running');assert os.geteuid()!=0,'initdb requires the unprivileged CI runner owner'
  row=run('postgres-toolchain',['pg_config','--version','--bindir','--sharedir','--pkglibdir'],env,60);config=Path(row['stdout_log']).read_text().strip().splitlines();assert len(config)==4 and re.match(r'^PostgreSQL '+str(b['postgres_major'])+r'(?:\.|$)',config[0])
  bindir,sharedir,pkglibdir=map(Path,config[1:]);pg['bin']={name:bindir/name for name in ['postgres','initdb','pg_ctl','psql']};assert all(p.is_absolute() and p.is_file() and os.access(p,os.X_OK) for p in pg['bin'].values())
  extension=sharedir/'extension/ltree.control';library=pkglibdir/'ltree.so';assert extension.is_file() and library.is_file(),'native runner PostgreSQL ltree is unavailable'
  row=run('postgres-version',[str(pg['bin']['postgres']),'--version'],env,60);version=Path(row['stdout_log']).read_text().strip();assert re.match(r'^postgres \(PostgreSQL\) '+str(b['postgres_major'])+r'(?:\.|$)',version)
  save(e/'postgres-toolchain.json',{'source_sha':b['candidate'],'pg_config_output':config,'postgres_version':version,'binaries':{k:str(v) for k,v in pg['bin'].items()},'ltree_control':{'path':str(extension),'sha256':sha(extension)},'ltree_library':{'path':str(library),'sha256':sha(library)},'system_service_used':False})
  pgroot=g.TEMP/'postgres-owned';pgroot.mkdir(mode=0o700,exist_ok=False);pg['data']=pgroot/'data';pg['socket']=pgroot/'socket';pg['socket'].mkdir(mode=0o700,exist_ok=False)
  with socket.socket(socket.AF_INET,socket.SOCK_STREAM) as sock:sock.bind(('127.0.0.1',0));pg['port']=sock.getsockname()[1]
  run('postgres-initdb',[str(pg['bin']['initdb']),'-D',str(pg['data']),'--username=issue2818','--auth-local=trust','--auth-host=trust'],env,120)
  pg['folder']=e/'postgres-owned-driver';pg['argv']=[str(pg['bin']['postgres']),'-D',str(pg['data']),'-h','127.0.0.1','-p',str(pg['port']),'-k',str(pg['socket'])];limit=min(1800,g.END-time.monotonic());assert limit>0
  state['phase']='postgres-owned-start';save(e/'qualification-phase-state.json',state);pg['started']=time.monotonic()
  with (e/'postgres-driver.stdout.log').open('xb') as out,(e/'postgres-driver.stderr.log').open('xb') as err:pg['process']=subprocess.Popen([sys.executable,str(g.RUNNER),'--deadline-seconds',str(limit),'--output-dir',str(pg['folder']),'--',*pg['argv']],cwd=source/'api',env=env,stdin=subprocess.DEVNULL,stdout=out,stderr=err)
  deadline=min(g.END,time.monotonic()+30)
  while time.monotonic()<deadline:
   assert pg['process'].poll() is None,'PostgreSQL owned driver exited during startup'
   pidfile=pg['data']/'postmaster.pid'
   if (pg['folder']/'command-started.json').exists() and pidfile.exists():
    lines=pidfile.read_text().splitlines()
    if len(lines)>=8 and lines[-1].strip()=='ready':break
   time.sleep(.1)
  else:raise RuntimeError('PostgreSQL readiness deadline exceeded')
  pg['identity']=pg_identity()
  query="SELECT json_build_object('data_directory',current_setting('data_directory'),'server_version_num',current_setting('server_version_num'),'port',current_setting('port'),'listen_addresses',current_setting('listen_addresses'),'unix_socket_directories',current_setting('unix_socket_directories'),'fsync',current_setting('fsync'),'synchronous_commit',current_setting('synchronous_commit'),'full_page_writes',current_setting('full_page_writes'),'ltree',(SELECT default_version FROM pg_available_extensions WHERE name='ltree'),'createdb',(SELECT rolcreatedb FROM pg_roles WHERE rolname=current_user))"
  row=run('postgres-server-identity',[str(pg['bin']['psql']),'-X','-h','127.0.0.1','-p',str(pg['port']),'-U','issue2818','-d','postgres','-v','ON_ERROR_STOP=1','-Atqc',query],env,60);server=json.loads(Path(row['stdout_log']).read_text())
  assert Path(server['data_directory']).resolve()==pg['data'].resolve() and int(server['server_version_num'])//10000==b['postgres_major'] and int(server['port'])==pg['port']
  assert server['listen_addresses']=='127.0.0.1' and Path(server['unix_socket_directories']).resolve()==pg['socket'].resolve() and server['ltree'] and server['createdb']
  assert all(server[k]=='on' for k in ['fsync','synchronous_commit','full_page_writes']),'PostgreSQL durability differs from default'
  save(e/'postgres-owned-started.json',{'source_sha':b['candidate'],'identity':pg['identity'],'server':server,'task_budget':g.budget_snapshot(),'temporary_data':str(pgroot),'deadline_seconds':limit})
  return {'DATABASE_URL':f"postgresql://issue2818@127.0.0.1:{pg['port']}/postgres",'API_DATABASE_URL':f"postgresql://issue2818@127.0.0.1:{pg['port']}/postgres"}
 try:
  initial=g.budget_snapshot();save(e/'budget-before.json',initial);assert initial['memory.max']=='8000000000' and initial['memory.high']=='max' and initial['memory.swap.max']=='0';q,period=map(int,initial['cpu.max'].split());assert q/period<=os.cpu_count()*.6+.001
  assert g.capture(['rustc','-vV'])==b['cache']['compiler'];assert not any(os.environ.get(k) for k in ['RUSTC_WRAPPER','RUSTC_WORKSPACE_WRAPPER','RUSTC_BOOTSTRAP','RUSTFLAGS','CARGO_ENCODED_RUSTFLAGS'])
  save(e/'compiler-allocator-policy.json',{'source_sha':b['candidate'],'MALLOC_CONF':os.environ.get('MALLOC_CONF'),'override_applied':False,'compiler_identity':b['cache']['compiler']});assert not os.environ.get('MALLOC_CONF'),'source trial must keep the default compiler allocator configuration'
  assert g.PG_GROUP is None and os.access(Path(initial['path'])/'memory.reclaim',os.W_OK),'owned same-budget archive reclaim is unavailable'
  stage('resource-runner-regression','Running');state['phase']='resource-runner-read-race-regression'
  regression=g.run('resource-runner-read-race-regression',[sys.executable,str(ROOT/'scripts/shell/_tests/resource-job-run.test.py'),'RealJobTests.test_t1_cgroup_removal_read_race_preserves_owned_identity','-v'],cwd=ROOT,env={**os.environ,'RESOURCE_JOB_RUN_OS_TESTS':'1'},seconds=60)
  regression_text=Path(regression['merged_log']).read_text();assert re.search(r'^test_t1_cgroup_removal_read_race_preserves_owned_identity .+ \.\.\. ok$',regression_text,re.M) and 'Ran 1 test in ' in regression_text and regression_text.rstrip().endswith('OK'),'real cgroup regression did not execute and pass'
  save(e/'resource-runner-regression.json',{'source_sha':b['candidate'],'runner_sha256':sha(g.RUNNER),'test_sha256':sha(ROOT/'scripts/shell/_tests/resource-job-run.test.py'),'actual_methods_passed':1,'race_subcases':7,'receipt':regression['receipt']});state['completed'].append('resource-runner-regression');stage('resource-runner-regression','Passed');state['phase']='prepare'
  os.environ['ISSUE2818_OBSERVED_SOURCE_SHA']=b['cache']['source_sha'];archive,_=g.download_retained();cache_resume=read(e/'cache-resume.json');cache_resume['inherited_helper_note']={k:cache_resume.get(k) for k in ['cache_state','scope']};cache_resume.update(cache_state='retained '+b['cache']['source_sha']+' partial target; revalidate actual roots',scope=b['cache']['qualified_scope'],not_runtime_PASS=True);save(e/'cache-resume.json',cache_resume);g.run('checkout-cache-source',['git','worktree','add','--detach',str(source),b['cache']['source_sha']],seconds=120);g.run('restore-source-target',['tar','-I','zstd -T1','-xf',str(archive),'--no-same-owner','-C',str(g.TEMP)],seconds=180)
  assert cap(['git','rev-parse','HEAD'])==b['cache']['source_sha'];cap(['git','diff','--exit-code','HEAD','--']);cap(['git','checkout','--detach',b['candidate']]);assert cap(['git','rev-parse','HEAD'])==b['candidate'];cap(['git','diff','--exit-code','HEAD','--']);assert cap(['git','rev-parse','HEAD:api'])==b['api_tree'];os.environ['ISSUE2818_OBSERVED_SOURCE_SHA']=b['candidate']
  assert all(sha(source/p)==h for p,h in b['source_hashes'].items());save(e/'source-identity.json',{'source_sha':b['candidate'],'api_tree':b['api_tree'],'source_hashes':b['source_hashes'],'tracked_clean':True,'tracked_diff_not_AC006':b['tracked_diff_not_AC006'],'cache_source_sha':b['cache']['source_sha']})
  restored=g.budget_snapshot();save(e/'budget-after-restore.json',restored);assert int(restored['memory.peak'])<=6400000000,'cache preparation exceeded the original full-task 6.4GB target; no heavy stage started'
  cargos=[str(f.relative_to(source)) for f in (source/'api').rglob('Cargo.toml') if 'target' not in f.parts]+['api/Cargo.lock'];before={p:sha(source/p) for p in cargos};fetch=dict(os.environ)
  for k in ['CARGO_NET_OFFLINE','RUSTC_WRAPPER','RUSTC_WORKSPACE_WRAPPER','RUSTC_BOOTSTRAP','RUSTFLAGS','CARGO_ENCODED_RUSTFLAGS']:fetch.pop(k,None)
  try:g.run('registry-prefetch',['cargo','fetch','--locked','--manifest-path','api/Cargo.toml'],cwd=source,env=fetch,seconds=180)
  finally:after={p:sha(source/p) for p in cargos};save(e/'prefetch-proof.json',{'source_sha':b['candidate'],'before':before,'after':after,'all_unchanged':before==after,'no_offline_wrapper_bootstrap_flags':True})
  assert before==after
  env={**os.environ,'CARGO_TARGET_DIR':str(target),'CARGO_BUILD_JOBS':'1','CARGO_INCREMENTAL':'0','CARGO_NET_OFFLINE':'true','CARGO_TERM_COLOR':'never','RUST_TEST_THREADS':'1','TERM':'dumb','ISSUE2818_ALL_RUSTC_LOG':str(e/'all-rustc-invocations.jsonl')}
  for k in ['RUSTC_WRAPPER','RUSTC_WORKSPACE_WRAPPER','RUSTC_BOOTSTRAP','RUSTFLAGS','CARGO_ENCODED_RUSTFLAGS']:env.pop(k,None)
  row=g.run('actual-cargo-metadata',['cargo','metadata','--locked','--format-version','1','--manifest-path','api/Cargo.toml'],cwd=source,env=env,seconds=120);metadata=json.loads(Path(row['stdout_log']).read_text());assert metadata['resolve'] and all(any(x['name']==name for x in metadata['packages']) for name in ['api-server','interface-runtime']);save(e/'actual-cargo-metadata.json',metadata);roots=g.root_names(source,metadata)
  diagnostic=g.install_stage_wrapper(source,env);diagnostic['ISSUE2818_RUSTC_MODE']='record-only';diagnostic.pop('ISSUE2818_MONO_STATS',None);probe=read(e/'mono-stats-probe.json');probe.update(capability_probe_only=True,actual_compile_mode='record-only',active_compile_instrumentation=[]);save(e/'mono-stats-probe.json',probe);os.environ['ISSUE2818_FROZEN_RUNTIME']=json.dumps(b['frozen_runtime']);data=g.plan(source);save(e/'frozen-real-builder-plan.json',data)
  # Ordinary API first: unchanged rustc argv, actual source/codegen, no -Z instrumentation.
  stage('ordinary-API','Running');before=len(rustc_rows());row=run('seed-normal-lib-prebuild',b['A'],diagnostic);items=artifacts(row)
  api=[x for x in items if x['target']['name']=='api_server' and x['target']['kind']==['lib'] and not x['profile']['test']]
  roots_actual=[x for x in rustc_rows(before) if crate_name(x)=='api_server' and '--test' not in x['original_argv']]
  assert len(api)==1 and not api[0]['fresh'] and roots_actual and all(x.get('source_sha')==b['candidate'] for x in roots_actual)
  assert all(any(str(f).endswith(ext) for f in api[0]['filenames']) for ext in ['.rlib','.rmeta'])
  invoked=[json.loads(line) for line in (e/'root-rustc-invocations.jsonl').read_text().splitlines()]
  actual=[x for x in invoked if not x.get('probe') and x.get('mode')=='record-only']
  assert actual and all(x['effective_argv']==x['original_argv'] and not x.get('bootstrap_for_this_exec') for x in actual)
  receipt=read(row['receipt']);peak=receipt['resources'].get('fields',{}).get('memory.peak',{}).get('value');budget=g.budget_snapshot()
  save(e/'A-qualified.json',{'source_sha':b['candidate'],'API_artifact':api[0],'actual_root_rustc':roots_actual,'scope_peak_bytes':peak,'scope_6_4GB_pass':isinstance(peak,int) and peak<=6400000000,'full_task_budget':budget,'full_task_6_4GB_so_far':int(budget['memory.peak'])<=6400000000,'not_Root_PASS':True})
  assert isinstance(peak,int) and peak<=6400000000,'ordinary API scope exceeds original 6.4GB target; later heavy stages NotReached'
  assert int(budget['memory.peak'])<=6400000000,'full task exceeded original 6.4GB target after ordinary API; later stages NotReached'
  state['completed'].append('ordinary-API');stage('ordinary-API','Passed')
  stage('interface-runtime-74','Running')
  root,binary=binary_compile('interface-runtime-codegen',['cargo','test','-p','interface-runtime','--lib','--no-run','--jobs','1','--message-format=json'],'interface_runtime',diagnostic)
  listed,ignored=list_binary('interface-runtime',binary,env);assert sorted(listed)==b['ir_names'] and not ignored
  row=run('interface-runtime-actual',[str(binary),'--test-threads=1'],env,180);save(e/'interface-runtime-actual.json',{'source_sha':b['candidate'],'actual':prove_actual(Path(row['merged_log']).read_text(),listed),'binary_sha256':sha(binary)})
  state['completed'].append('interface-runtime-74');stage('interface-runtime-74','Passed')
  stage('native31','Running')
  root,binary=binary_compile('native31-codegen-link',['cargo','test','-p','api-server','--features','test-support','--test','console_core_registry_tests','--no-run','--jobs','1','--message-format=json'],'console_core_registry_tests',diagnostic)
  listed,ignored=list_binary('native31',binary,env);assert sorted(listed)==b['native31_names'] and not ignored
  row=run('native31-actual',[str(binary),'--test-threads=1'],env,180);save(e/'native31-actual.json',{'source_sha':b['candidate'],'actual':prove_actual(Path(row['merged_log']).read_text(),listed),'binary_sha256':sha(binary)})
  state['completed'].append('native31');stage('native31','Passed')
  stage('contracts-root','Running')
  root,binary=binary_compile('contracts-full-codegen-link',['cargo','test','-p','api-server','--features','test-support','--test','api_private_contracts','--no-run','--jobs','1','--message-format=json'],'api_private_contracts',diagnostic)
  contracts_root,contracts_binary,contracts_sha=root,binary,sha(binary);contracts_list,contracts_ignored=list_binary('contracts-full',contracts_binary,env)
  assert all(Counter(contracts_list)[name]==1 and name not in contracts_ignored for name in b['managed_cases']),'managed case absent, duplicated or ignored in the real contracts binary'
  save(e/'managed-cases-discovery.json',{'source_sha':b['candidate'],'artifact':contracts_root,'binary_sha256':contracts_sha,'full_list_count':len(contracts_list),'cases':b['managed_cases'],'case_list_counts':{name:Counter(contracts_list)[name] for name in b['managed_cases']},'case_ignored':[]})
  state['completed'].append('contracts-root');stage('contracts-root','Passed')
  # Keep original builders, resolved API Fresh proof and denominators; prepare all four before starting PostgreSQL.
  prepared=[]
  for index,command in enumerate(data['tests']):
   stage('consumer-'+str(index),'Running')
   argv=[command['command'],*command['args']];obs=json_argv(argv);ce={**g.command_env(data,command,target),'RUST_TEST_THREADS':'1','ISSUE2818_RUSTC_MODE':'record-only','RUSTC_WRAPPER':diagnostic['RUSTC_WRAPPER'],'ISSUE2818_ROOT_LOG':diagnostic['ISSUE2818_ROOT_LOG'],'ISSUE2818_ALL_RUSTC_LOG':env['ISSUE2818_ALL_RUSTC_LOG']}
   listing=run('consumer-'+str(index)+'-list',[*obs,'--list'],ce);emitted=artifacts(listing);apiunits=[x for x in emitted if x['target']['name']=='api_server' and x['target']['kind']==['lib'] and not x['profile']['test']];assert len(apiunits)==1 and apiunits[0]['fresh'] and apiunits[0]['features']==api[0]['features'] and apiunits[0]['profile']==api[0]['profile'],'actual resolved API unit not reused'
   listed=g.cases(listing['merged_log'],roots,'list');assert listed;warm={**ce,'ISSUE2818_EXPECT_NO_RUST_COMPILE':'1'};ign=run('consumer-'+str(index)+'-ignored',[*obs,'--list','--ignored'],warm,120);ignored=g.cases(ign['merged_log'],roots,'list')
   prepared.append((index,command,obs,warm,listed,ignored,apiunits));save(e/('consumer-'+str(index)+'-prepared.json'),{'source_sha':b['candidate'],'original_builder':command,'list':listed,'ignored':ignored,'resolved_API_unit':apiunits[0],'actual':'NotReached'});stage('consumer-'+str(index),'NotReached')
  stage('sdk-worker','Running')
  worker_root,worker_binary=binary_compile('sdk-worker-codegen',['cargo','build','--profile','test','--locked','-p','runtime-extension-sdk','--example','managed_hook_worker','--jobs','1','--message-format=json'],'managed_hook_worker',diagnostic)
  assert worker_root['target']['kind']==['example'] and not worker_root['profile']['test'] and Path(worker_root['target']['src_path']).resolve()==(source/'api/crates/runtime-extension-sdk/src/_tests/managed_hook_worker.rs').resolve() and os.access(worker_binary,os.X_OK)
  worker_before=sha(worker_binary);run('sdk-worker-strip',['strip','--strip-debug',str(worker_binary)],env,60);worker_sha=sha(worker_binary)
  save(e/'sdk-worker-identity.json',{'source_sha':b['candidate'],'artifact':worker_root,'source_sha256':sha(worker_root['target']['src_path']),'binary_path':str(worker_binary),'before_strip_sha256':worker_before,'binary_sha256':worker_sha,'profile':'test','extra_profile_overrides':[]})
  state['completed'].append('sdk-worker');stage('sdk-worker','Passed')
  runtime=start_postgres();runtime['MANAGED_HOOK_WORKER_FIXTURE']=str(worker_binary);package=source/'api/apps/api-server';managed_env={**env,**runtime,'CARGO_MANIFEST_DIR':str(package),'LD_LIBRARY_PATH':os.pathsep.join(x for x in [str(contracts_binary.parent),str(target/'debug'),env.get('LD_LIBRARY_PATH')] if x)}
  stage('managed-lifecycle','Running');managed_results=[]
  for index,name in enumerate(b['managed_cases']):
   assert sha(contracts_binary)==contracts_sha and sha(worker_binary)==worker_sha,'prepared real binary changed before actual execution'
   row=run('managed-lifecycle-'+str(index),[str(contracts_binary),name,'--exact','--format=pretty','--color=never','--test-threads=1'],managed_env,300,cwd=package);text=Path(row['merged_log']).read_text();actual=prove_actual(text,[name]);counts=re.findall(r'^test result: ok\. (\d+) passed; (\d+) failed; (\d+) ignored; (\d+) measured; (\d+) filtered out;',text,re.M)
   assert len(counts)==1 and tuple(map(int,counts[0][:4]))==(1,0,0,0) and int(counts[0][4])==len(contracts_list)-1,'exact managed case must run once; retain and report other binary tests as filtered'
   proof={'source_sha':b['candidate'],'name':name,'binary_sha256':contracts_sha,'worker_sha256':worker_sha,'actual':actual,'passed':1,'failed':0,'ignored':0,'measured':0,'filtered_out':int(counts[0][4]),'full_binary_list_count':len(contracts_list),'cwd':str(package),'CARGO_MANIFEST_DIR':managed_env['CARGO_MANIFEST_DIR'],'receipt':row['receipt']};save(e/('managed-lifecycle-'+str(index)+'-actual.json'),proof);managed_results.append(proof)
  save(e/'managed-lifecycle-actual.json',{'source_sha':b['candidate'],'cases':managed_results,'expected_names':b['managed_cases'],'passed':len(managed_results)});state['completed'].append('managed-lifecycle');stage('managed-lifecycle','Passed')
  for index,command,obs,warm,listed,ignored,apiunits in prepared:
   stage('consumer-'+str(index),'Running');actual=run('consumer-'+str(index)+'-actual',obs,{**warm,**runtime},120);ran=g.cases(actual['merged_log'],roots,'run');assert Counter((x[0],x[1],'ignored' if x in ignored else 'ok') for x in listed)==Counter(map(tuple,ran)) and any(x[2]=='ok' for x in ran);save(e/('consumer-'+str(index)+'-actual.json'),{'source_sha':b['candidate'],'original_builder':command,'list':listed,'ignored':ignored,'actual':ran,'resolved_API_features':apiunits[0]['features']});state['completed'].append('consumer-'+str(index));stage('consumer-'+str(index),'Passed')
  pg_identity();run('postgres-fast-stop',[str(pg['bin']['pg_ctl']),'-D',str(pg['data']),'-m','fast','-w','-t','30','stop'],env,45);finish_postgres();state['completed'].append('postgres-owned');stage('postgres-owned','Passed')
  stage('dependency-boundaries','Running')
  boundary=['cargo','test','-p','control-plane-postgres-tests','--test','dependency_boundaries','--jobs','1','--no-run','--message-format=json'];root,binpath=binary_compile('dependency-boundaries-codegen',boundary,'dependency_boundaries',diagnostic);listed,ignored=list_binary('dependency-boundaries',binpath,env);assert not ignored;row=run('dependency-boundaries-actual',[str(binpath),'--test-threads=1'],env,120);save(e/'dependency-boundaries-actual.json',{'source_sha':b['candidate'],'actual':prove_actual(Path(row['merged_log']).read_text(),listed)});state['completed'].append('dependency-boundaries');stage('dependency-boundaries','Passed')
  for index,argv in enumerate(b['clean_checks']):
   name='clean-check-'+str(index);stage(name,'Running');run(name,[*argv,'--message-format=json'],diagnostic);state['completed'].append(name);stage(name,'Passed')
  # True-source C01: e3 product's old literal, candidate product's API all-targets.
  for side,source_sha,argv in [('old-literal',b['C01']['baseline_sha'],b['C01']['old_literal']),('new-alltargets',b['candidate'],b['C01']['new'])]:
   name='C01-'+side;stage(name,'Running');cap(['git','diff','--exit-code','HEAD','--']);cap(['git','checkout','--detach',source_sha])
   assert cap(['git','rev-parse','HEAD'])==source_sha
   expected_tree=b['C01']['baseline_api_tree'] if side=='old-literal' else b['api_tree'];assert cap(['git','rev-parse','HEAD:api'])==expected_tree
   state['active_source_sha']=source_sha;os.environ['ISSUE2818_OBSERVED_SOURCE_SHA']=source_sha
   side_env={**diagnostic,'ISSUE2818_OBSERVED_SOURCE_SHA':source_sha,'ISSUE2818_RUSTC_MODE':'record-only'}
   if side=='old-literal':
    old_plan=g.plan(source);assert [old_plan['tests'][0]['command'],*old_plan['tests'][0]['args']]==argv
    save(e/'C01-actual-old-builder.json',{'source_sha':source_sha,'plan':old_plan})
   c01=source/b['C01']['path'];c01_original=c01.read_bytes();assert b['C01']['marker'].encode() not in c01_original
   if side=='old-literal':assert hashlib.sha256(c01_original).hexdigest()==b['C01']['baseline_marker_file_sha256']
   c01.write_bytes(c01_original+b['C01']['append'].encode());before=len(rustc_rows())
   save(e/(name+'-mutation.json'),{'source_sha':source_sha,'api_tree':expected_tree,'path':b['C01']['path'],'original_sha256':hashlib.sha256(c01_original).hexdigest(),'injected_sha256':sha(c01),'append':b['C01']['append']})
   row=run(name,argv,side_env,600,expected='nonzero');text=Path(row['merged_log']).read_text();observed=rustc_rows(before)
   identity=[x for x in observed if crate_name(x)=='api_identity' and x.get('source_sha')==source_sha]
   old_api=[x for x in observed if crate_name(x)=='api_server' and '--test' not in x['original_argv'] and x.get('source_sha')==source_sha]
   hit=b['C01']['marker'] in text and re.search(r'-->\s+(?:.*?/)?apps/api-server/tests/http/api_identity\.rs:\d+:\d+',text)
   errors=re.findall(r'^error(?:\[E\d+\])?: (.+)$',text,re.M);unrelated=[x for x in errors if b['C01']['marker'] not in x and not x.startswith('could not compile')]
   run_receipt=read(row['receipt']);resource_clean=run_receipt['classification']=='exit' and not run_receipt['oom_evidence'] and run_receipt['cleanup']['complete']
   qualified=row['exit_code']==101 and resource_clean and bool(hit) and bool(identity) and not unrelated and (side!='old-literal' or bool(old_api))
   save(e/(name+'-detection.json'),{'source_sha':source_sha,'api_tree':expected_tree,'argv':argv,'exit':row['exit_code'],'marker_and_primary_span_observed':bool(hit),'actual_identity_rustc':identity,'actual_old_API_rustc':old_api,'resource_clean':resource_clean,'unrelated_errors':unrelated,'status':'Detected' if qualified else 'NoEvidence','not_four_filter_behavior_PASS':True})
   assert qualified,'C01 source/root/marker evidence incomplete; unrelated or resource failure gives no D credit'
   c01.write_bytes(c01_original);c01=None;c01_original=None;cap(['git','diff','--exit-code','HEAD','--']);state['completed'].append(name);stage(name,'Detected')
  state['C01_pair']='Detected';state['phase']='complete';save(e/'status.json',{'status':'W9_W10_WORKLOAD_COMPLETED_FINAL_RESOURCE_CHECK_PENDING','source_sha':b['candidate'],'state':state,'not_Root_runtime_or_resource_PASS':True});return 0
 except Exception as error:
  primary_error={'phase':state['phase'],'type':type(error).__name__,'error':str(error)}
  for name,status in state['stages'].items():
   if status=='Running':state['stages'][name]='NoEvidence' if name.startswith('C01-') else 'Failed'
  save(e/'failure.json',{'phase':state['phase'],'type':type(error).__name__,'error':str(error),'no_unchanged_retry':True});print(json.dumps({'coupled_failure':{'source_sha':b['candidate'],'phase':state['phase'],'error':str(error)}}),flush=True);raise
 finally:
  if pg.get('process') is not None:
   try:finish_postgres(cancel=True)
   except Exception as cleanup_error:pg_cleanup_error=cleanup_error;state['stages']['postgres-owned']='Failed';save(e/'postgres-cleanup-failure.json',{'error':str(cleanup_error),'type':type(cleanup_error).__name__})
  if c01 is not None and c01_original is not None:c01.write_bytes(c01_original);save(e/'C01-restored.json',{'sha256':sha(c01),'equals_original':c01.read_bytes()==c01_original})
  if source.exists() and cap(['git','rev-parse','HEAD'])!=b['candidate']:
   cap(['git','diff','--exit-code','HEAD','--']);cap(['git','checkout','--detach',b['candidate']])
  state['active_source_sha']=b['candidate'];os.environ['ISSUE2818_OBSERVED_SOURCE_SHA']=b['candidate']
  save(e/'qualification-phase-state.json',state)
  g.stage_summary();retention_error=None
  try:
   if pg_cleanup_error is not None:save(e/'seed-retention.json',{'status':'skipped-pg-cleanup-incomplete','reason':str(pg_cleanup_error)})
   else:g.retain_seed_cache(source,target,state)
  except Exception as error:retention_error={'type':type(error).__name__,'error':str(error)}
  retention=read(e/'seed-retention.json') if (e/'seed-retention.json').exists() else {'status':'missing','error':retention_error}
  final_budget=g.budget_snapshot();save(e/'budget-after.json',final_budget)
  counts=lambda v:{k:int(n) for k,n in (line.split() for line in v['memory.events'].splitlines())}
  oom_delta={k:counts(final_budget).get(k,0)-counts(initial).get(k,0) for k in ['oom','oom_kill','oom_group_kill']} if initial else None
  limits_unchanged=bool(initial) and all(final_budget[k]==initial[k] for k in ['inode','path','memory.max','memory.high','memory.swap.max','cpu.max'])
  resource_pass=limits_unchanged and int(final_budget['memory.peak'])<=6400000000 and oom_delta is not None and not any(oom_delta.values()) and retention['status']=='archived' and retention_error is None and pg_cleanup_error is None
  qualification={'source_sha':b['candidate'],'full_task_budget':final_budget,'full_task_peak_bytes':int(final_budget['memory.peak']),'target_bytes':6400000000,'peak_reset':False,'cache_windows_included':True,'limits_unchanged':limits_unchanged,'oom_delta':oom_delta,'retention_status':retention['status'],'retention_error':retention_error,'postgres_cleanup_error':str(pg_cleanup_error) if pg_cleanup_error else None,'prior_failure':primary_error,'resource_conditions_passed':resource_pass,'not_Root_acceptance':True}
  save(e/'full-task-resource-qualification.json',qualification);save(e/'cost-summary.json',{'commands':g.RESULTS,'all_command_wall_seconds':sum(x['wall_seconds'] for x in g.RESULTS),'elapsed_task_wall_seconds':time.monotonic()-task_started,'postgres_lifetime':read(e/'postgres-owned-lifetime.json') if (e/'postgres-owned-lifetime.json').exists() else None,'postgres_lifetime_overlaps_commands_and_is_not_added_to_the_sum':True,'source_sha':b['candidate']})
  save(e/'status.json',{'status':'FINITE_WORKLOAD_COMPLETED' if resource_pass and primary_error is None else 'FAILED_OR_INCOMPLETE','source_sha':b['candidate'],'state':state,'resource_qualification':qualification,'not_Root_runtime_or_resource_PASS':True});print(json.dumps({'coupled_final':state,'full_task_resource_qualification':qualification}),flush=True)
  if pg_cleanup_error is not None:raise pg_cleanup_error
  if primary_error is None:assert resource_pass,'full-task resource or cache-retention qualification failed'
if __name__=='__main__':
 if len(sys.argv)>1 and sys.argv[1]=='--self-check':print(json.dumps({'self_check':'PASS','candidate':check()['candidate'],'no_execution':True}))
 else:sys.exit(main())
