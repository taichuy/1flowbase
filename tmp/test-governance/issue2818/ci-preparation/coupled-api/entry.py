#!/usr/bin/env python3
"""W8 proposal; original frozen workload and true-product-source C01."""
import gzip,hashlib,importlib.util,json,os,re,subprocess,sys,time
from pathlib import Path
from collections import Counter
HERE=Path(__file__).resolve().parent;ROOT=HERE.parents[4]
def read(p):return json.loads(Path(p).read_text())
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def save(p,v):Path(p).parent.mkdir(parents=True,exist_ok=True);Path(p).write_text(json.dumps(v,indent=2,sort_keys=True)+'\n')
def check(execution=False):
 b=read(HERE/'binding.json')
 assert re.fullmatch(r'[0-9a-f]{40}',b['candidate'])
 assert b['tracked_diff_not_AC006']['net'] < 0, 'tracked source reduction is necessary but not AC006 qualification'
 assert not execution or (b['execution_authorized'] and b['proposal_status']=='READY_FOR_ROOT_APPROVED_EXECUTION' and b['candidate'] not in b['rejected_candidate_shas'])
 assert not os.environ.get('CANDIDATE_SHA') or os.environ['CANDIDATE_SHA']==b['candidate']
 assert all(sha(ROOT/p)==h for p,h in b['inputs'].items())
 return b

def main():
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
 def run(label,argv,env=None,seconds=600,expected=0):
  state['phase']=label;save(e/'qualification-phase-state.json',state)
  try:return g.run(label,argv,cwd=source/'api',env=env,heavy=True,seconds=seconds,expected=expected)
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
 try:
  initial=g.budget_snapshot();save(e/'budget-before.json',initial);assert initial['memory.max']=='8000000000' and initial['memory.high']=='max' and initial['memory.swap.max']=='0';q,period=map(int,initial['cpu.max'].split());assert q/period<=os.cpu_count()*.6+.001
  assert g.capture(['rustc','-vV'])==b['cache']['compiler'];assert not any(os.environ.get(k) for k in ['RUSTC_WRAPPER','RUSTC_WORKSPACE_WRAPPER','RUSTC_BOOTSTRAP','RUSTFLAGS','CARGO_ENCODED_RUSTFLAGS'])
  os.environ['ISSUE2818_OBSERVED_SOURCE_SHA']=b['cache']['source_sha'];archive,_=g.download_retained();cache_resume=read(e/'cache-resume.json');cache_resume['inherited_helper_note']={k:cache_resume.get(k) for k in ['cache_state','scope']};cache_resume.update(cache_state='retained '+b['cache']['source_sha']+' partial target; revalidate actual roots',scope=b['cache']['qualified_scope'],not_runtime_PASS=True);save(e/'cache-resume.json',cache_resume);g.run('checkout-cache-source',['git','worktree','add','--detach',str(source),b['cache']['source_sha']],seconds=120);g.run('restore-source-target',['tar','-I','zstd -T1','-xf',str(archive),'--no-same-owner','-C',str(g.TEMP)],seconds=180)
  assert cap(['git','rev-parse','HEAD'])==b['cache']['source_sha'];cap(['git','diff','--exit-code','HEAD','--']);cap(['git','checkout','--detach',b['candidate']]);assert cap(['git','rev-parse','HEAD'])==b['candidate'];cap(['git','diff','--exit-code','HEAD','--']);assert cap(['git','rev-parse','HEAD:api'])==b['api_tree'];os.environ['ISSUE2818_OBSERVED_SOURCE_SHA']=b['candidate']
  assert all(sha(source/p)==h for p,h in b['source_hashes'].items());save(e/'source-identity.json',{'source_sha':b['candidate'],'api_tree':b['api_tree'],'source_hashes':b['source_hashes'],'tracked_clean':True,'tracked_diff_not_AC006':b['tracked_diff_not_AC006'],'cache_source_sha':b['cache']['source_sha']})
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
  list_binary('contracts-full',binary,env);state['completed'].append('contracts-root');stage('contracts-root','Passed')
  # Original builders retain args/env. Cargo JSON is observation only; resolved units, never CLI feature spelling, determine reuse.
  for index,command in enumerate(data['tests']):
   stage('consumer-'+str(index),'Running')
   argv=[command['command'],*command['args']];obs=json_argv(argv);ce={**g.command_env(data,command,target),'RUST_TEST_THREADS':'1','ISSUE2818_RUSTC_MODE':'record-only','RUSTC_WRAPPER':diagnostic['RUSTC_WRAPPER'],'ISSUE2818_ROOT_LOG':diagnostic['ISSUE2818_ROOT_LOG'],'ISSUE2818_ALL_RUSTC_LOG':env['ISSUE2818_ALL_RUSTC_LOG']}
   listing=run('consumer-'+str(index)+'-list',[*obs,'--list'],ce);emitted=artifacts(listing);apiunits=[x for x in emitted if x['target']['name']=='api_server' and x['target']['kind']==['lib'] and not x['profile']['test']];assert len(apiunits)==1 and apiunits[0]['fresh'] and apiunits[0]['features']==api[0]['features'] and apiunits[0]['profile']==api[0]['profile'],'actual resolved API unit not reused'
   listed=g.cases(listing['merged_log'],roots,'list');assert listed;warm={**ce,'ISSUE2818_EXPECT_NO_RUST_COMPILE':'1'};ign=run('consumer-'+str(index)+'-ignored',[*obs,'--list','--ignored'],warm,120);ignored=g.cases(ign['merged_log'],roots,'list');actual=run('consumer-'+str(index)+'-actual',obs,warm,120);ran=g.cases(actual['merged_log'],roots,'run');assert Counter((x[0],x[1],'ignored' if x in ignored else 'ok') for x in listed)==Counter(map(tuple,ran)) and any(x[2]=='ok' for x in ran);save(e/('consumer-'+str(index)+'-actual.json'),{'source_sha':b['candidate'],'original_builder':command,'list':listed,'ignored':ignored,'actual':ran,'resolved_API_features':apiunits[0]['features']});state['completed'].append('consumer-'+str(index));stage('consumer-'+str(index),'Passed')
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
  state['C01_pair']='Detected';state['phase']='complete';save(e/'status.json',{'status':'W8_FINITE_CANDIDATE_WORKLOAD_COMPLETED','source_sha':b['candidate'],'state':state,'not_Root_runtime_or_resource_PASS':True});return 0
 except Exception as error:
  for name,status in state['stages'].items():
   if status=='Running':state['stages'][name]='NoEvidence' if name.startswith('C01-') else 'Failed'
  save(e/'failure.json',{'phase':state['phase'],'type':type(error).__name__,'error':str(error),'no_unchanged_retry':True});print(json.dumps({'coupled_failure':{'source_sha':b['candidate'],'phase':state['phase'],'error':str(error)}}),flush=True);raise
 finally:
  if c01 is not None and c01_original is not None:c01.write_bytes(c01_original);save(e/'C01-restored.json',{'sha256':sha(c01),'equals_original':c01.read_bytes()==c01_original})
  if source.exists() and cap(['git','rev-parse','HEAD'])!=b['candidate']:
   cap(['git','diff','--exit-code','HEAD','--']);cap(['git','checkout','--detach',b['candidate']])
  state['active_source_sha']=b['candidate'];os.environ['ISSUE2818_OBSERVED_SOURCE_SHA']=b['candidate']
  save(e/'qualification-phase-state.json',state)
  g.stage_summary();g.retain_seed_cache(source,target,state);save(e/'cost-summary.json',{'commands':g.RESULTS,'all_command_wall_seconds':sum(x['wall_seconds'] for x in g.RESULTS),'source_sha':b['candidate']});save(e/'budget-after.json',g.budget_snapshot());print(json.dumps({'coupled_final':state}),flush=True)
if __name__=='__main__':
 if len(sys.argv)>1 and sys.argv[1]=='--self-check':print(json.dumps({'self_check':'PASS','candidate':check()['candidate'],'no_execution':True}))
 else:sys.exit(main())
