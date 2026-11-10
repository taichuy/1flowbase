#!/usr/bin/env python3
"""One frozen coupled qualification; adapts the existing issue2818 owner, never product source."""
import gzip,hashlib,importlib.util,json,os,re,subprocess,sys,time
from pathlib import Path
from collections import Counter
HERE=Path(__file__).resolve().parent;ROOT=HERE.parents[4]
def read(p):return json.loads(Path(p).read_text())
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def save(p,v):Path(p).parent.mkdir(parents=True,exist_ok=True);Path(p).write_text(json.dumps(v,indent=2,sort_keys=True)+'\n')
def check():
 b=read(HERE/'binding.json');assert b['execution_authorized'] and b['candidate']=='f580c4b45ef3eaf9a86de0f13c2f8346c76adbbf';assert all(sha(ROOT/p)==h for p,h in b['inputs'].items());assert b['net']==-6
 assert read(ROOT/b['net_receipt'])['head']==b['candidate'] and read(ROOT/b['net_receipt'])['net']<0;return b

def main():
 b=check();s=importlib.util.spec_from_file_location('owner',HERE.parent/'experiment.py');g=importlib.util.module_from_spec(s);s.loader.exec_module(g)
 g.SEED_SHA=b['cache']['source_sha'];g.PRIOR_RUN=str(b['cache']['run']);g.PRIOR_NAME=b['cache']['name'];g.PRIOR_ARTIFACT_ID=b['cache']['id'];g.ARCHIVE_SHA=b['cache']['archive_sha256'];g.ARCHIVE_BYTES=b['cache']['archive_bytes'];g.END=time.monotonic()+4200;g.ROOT_WRAPPER=(HERE/'root-wrapper.py').read_text()
 g.EVIDENCE.mkdir(parents=True,exist_ok=False);g.TEMP.mkdir(parents=True,exist_ok=False);e=g.EVIDENCE;source=g.TEMP/'seed-source';target=g.TEMP/'target-seed';state={'source_sha':b['candidate'],'phase':'prepare','completed':[],'not_Root_PASS':True};diagnostic=None;c01=None;c01_original=None
 def cap(argv):return subprocess.check_output(argv,cwd=source,text=True,stderr=subprocess.STDOUT,timeout=60).strip()
 def artifacts(row):
  out=[]
  for line in Path(row['merged_log']).read_text(errors='replace').splitlines():
   try:x=json.loads(line)
   except ValueError:continue
   if x.get('reason')=='compiler-artifact':out.append(x)
  save(e/(row['label']+'-artifacts.json'),{'source_sha':b['candidate'],'artifacts':out,'Fresh':sum(x['fresh'] for x in out),'rebuilt':sum(not x['fresh'] for x in out)});return out
 def mono(phase):
  files=[]
  for crate in ['api_server','model_definitions_http']:
   f=source/'api'/(crate+'.mono_items.json')
   if f.exists():
    dest=e/(phase+'-'+crate+'.mono_items.json.gz');data=f.read_bytes();dest.write_bytes(gzip.compress(data,compresslevel=1));files.append({'crate':crate,'raw_sha256':hashlib.sha256(data).hexdigest(),'bytes':len(data),'gzip':str(dest),'gzip_sha256':sha(dest)})
  save(e/(phase+'-mono-files.json'),{'source_sha':b['candidate'],'files':files,'not_RSS_allocation':True})
 def run(label,argv,env=None,seconds=600,expected=0):
  state['phase']=label;save(e/'qualification-phase-state.json',state)
  try:return g.run(label,argv,cwd=source/'api',env=env,heavy=True,seconds=seconds,expected=expected)
  finally:
   if g.RESULTS and g.RESULTS[-1]['label']==label:artifacts(g.RESULTS[-1])
   if source.exists():mono(label)
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
 try:
  initial=g.budget_snapshot();save(e/'budget-before.json',initial);assert initial['memory.max']=='8000000000' and initial['memory.high']=='max' and initial['memory.swap.max']=='0';q,period=map(int,initial['cpu.max'].split());assert q/period<=os.cpu_count()*.6+.001
  assert g.capture(['rustc','-vV'])==b['cache']['compiler'];assert not any(os.environ.get(k) for k in ['RUSTC_WRAPPER','RUSTC_WORKSPACE_WRAPPER','RUSTC_BOOTSTRAP','RUSTFLAGS','CARGO_ENCODED_RUSTFLAGS'])
  os.environ['ISSUE2818_OBSERVED_SOURCE_SHA']=b['cache']['source_sha'];archive,_=g.download_retained();g.run('checkout-cache-source',['git','worktree','add','--detach',str(source),b['cache']['source_sha']],seconds=120);g.run('restore-source-target',['tar','-I','zstd -T1','-xf',str(archive),'--no-same-owner','-C',str(g.TEMP)],seconds=180)
  assert cap(['git','rev-parse','HEAD'])==b['cache']['source_sha'];cap(['git','diff','--exit-code','HEAD','--']);cap(['git','checkout','--detach',b['candidate']]);assert cap(['git','rev-parse','HEAD'])==b['candidate'];cap(['git','diff','--exit-code','HEAD','--']);assert cap(['git','rev-parse','HEAD:api'])==b['api_tree'];os.environ['ISSUE2818_OBSERVED_SOURCE_SHA']=b['candidate']
  assert all(sha(source/p)==h for p,h in b['source_hashes'].items());save(e/'source-identity.json',{'source_sha':b['candidate'],'api_tree':b['api_tree'],'source_hashes':b['source_hashes'],'tracked_clean':True,'net':b['net'],'cache_source_sha':b['cache']['source_sha']})
  cargos=[str(f.relative_to(source)) for f in (source/'api').rglob('Cargo.toml') if 'target' not in f.parts]+['api/Cargo.lock'];before={p:sha(source/p) for p in cargos};fetch=dict(os.environ)
  for k in ['CARGO_NET_OFFLINE','RUSTC_WRAPPER','RUSTC_WORKSPACE_WRAPPER','RUSTC_BOOTSTRAP','RUSTFLAGS','CARGO_ENCODED_RUSTFLAGS']:fetch.pop(k,None)
  try:g.run('registry-prefetch',['cargo','fetch','--locked','--manifest-path','api/Cargo.toml'],cwd=source,env=fetch,seconds=180)
  finally:after={p:sha(source/p) for p in cargos};save(e/'prefetch-proof.json',{'source_sha':b['candidate'],'before':before,'after':after,'all_unchanged':before==after,'no_offline_wrapper_bootstrap_flags':True})
  assert before==after
  env={**os.environ,'CARGO_TARGET_DIR':str(target),'CARGO_BUILD_JOBS':'1','CARGO_INCREMENTAL':'0','CARGO_NET_OFFLINE':'true','CARGO_TERM_COLOR':'never','TERM':'dumb','ISSUE2818_ALL_RUSTC_LOG':str(e/'all-rustc-invocations.jsonl')}
  for k in ['RUSTC_WRAPPER','RUSTC_WORKSPACE_WRAPPER','RUSTC_BOOTSTRAP','RUSTFLAGS','CARGO_ENCODED_RUSTFLAGS']:env.pop(k,None)
  row=g.run('actual-cargo-metadata',['cargo','metadata','--locked','--format-version','1','--manifest-path','api/Cargo.toml'],cwd=source,env=env,seconds=120);metadata=json.loads(Path(row['stdout_log']).read_text());assert metadata['resolve'] and any(x['name']=='model-definitions-http' for x in metadata['packages']);save(e/'actual-cargo-metadata.json',metadata);roots=g.root_names(source,metadata)
  diagnostic=g.install_stage_wrapper(source,env);assert diagnostic.get('ISSUE2818_MONO_STATS')=='1';guarded={**diagnostic,'ISSUE2818_EXPECT_NO_RUST_COMPILE':'1'};os.environ['ISSUE2818_FROZEN_RUNTIME']=json.dumps(b['frozen_runtime']);data=g.plan(source);save(e/'frozen-real-builder-plan.json',data)
  # New owner is tested first. Its legitimate production/test feature units are charged, never assumed Fresh.
  owner_cmd=['cargo','test','-p','model-definitions-http','--lib','--jobs','1','--no-run','--message-format=json'];owner,owner_bin=binary_compile('owner-guards-codegen',owner_cmd,'model_definitions_http',diagnostic);listed,ignored=list_binary('owner-guards',owner_bin,env);assert sorted(listed)==sorted(b['owner_guard_names']) and not ignored;row=run('owner-guards-actual',[str(owner_bin),'--test-threads=1'],env,120);actual=prove_actual(Path(row['merged_log']).read_text(),listed);save(e/'owner-guards-actual.json',{'source_sha':b['candidate'],'actual':actual,'binary_sha256':sha(owner_bin)});state['completed'].append('owner-guards')
  row=run('seed-normal-lib-prebuild',b['A'],diagnostic);items=artifacts(row);api=[x for x in items if x['target']['name']=='api_server' and x['target']['kind']==['lib'] and not x['profile']['test']];own=[x for x in items if x['target']['name']=='model_definitions_http' and x['target']['kind']==['lib'] and not x['profile']['test']];assert len(api)==len(own)==1 and all(any(str(f).endswith(ext) for f in api[0]['filenames']) for ext in ['.rlib','.rmeta']);assert not api[0]['fresh'];save(e/'A-qualified.json',{'source_sha':b['candidate'],'API_artifact':api[0],'owner_artifact':own[0],'Fresh':sum(x['fresh'] for x in items),'rebuilt':[{'package_id':x['package_id'],'target':x['target'],'features':x['features'],'profile':x['profile']} for x in items if not x['fresh']],'not_6_4GB_or_Runtime_PASS':True})
  owner_mono=read(source/'api/model_definitions_http.mono_items.json');api_mono=read(source/'api/api_server.mono_items.json');maps=b['codec_source_map'];owned=[]
  for c in maps:
   hits=[x for x in owner_mono if c['non_generic_function'] in x['name']];bad=[x for x in api_mono if c['non_generic_function'] in x['name']];owned.append({'dto':c['dto'],'codec':c['non_generic_function'],'owner_rows':hits,'API_codec_rows':bad,'owner_DTO_rows':[x for x in owner_mono if c['dto'] in x['name']],'API_DTO_residual_rows':[x for x in api_mono if c['dto'] in x['name']]})
  ownership={'source_sha':b['candidate'],'rows':owned,'all_nine_concrete_codecs_owned':all(x['owner_rows'] and not x['API_codec_rows'] for x in owned),'limit':'DTO deserialize/Value/docs/test/response residuals are explicit. Mono definition counts are not RSS, physical stack or runtime retention, and generic aggregates alone do not attribute every HTTP specialization.'};save(e/'actual-codec-mono-ownership.json',ownership);assert ownership['all_nine_concrete_codecs_owned'],'actual concrete codec ownership not established';state['completed'].append('ordinary-API-and-codec-mono')
  binaries={}
  for label,name in [('contracts','api_private_contracts'),('routes','api_private_routes')]:
   argv=['cargo','test','-p','api-server','--features','test-support','--test',name,'--no-run','--jobs','1','--message-format=json'];root,binary=binary_compile(label+'-full-codegen-link',argv,name,diagnostic);listed,ignored=list_binary(label+'-full',binary,env);binaries[name]=(root,binary,listed,ignored)
  # Original24 are exact names from the frozen assertion map, not a redefined suite denominator.
  route_root,route_bin,route_names,route_ignored=binaries['api_private_routes'];expected=b['W2_names'];assert set(expected)<=set(route_names) and not(set(expected)&set(route_ignored));actual=[]
  for serial,name in enumerate(expected):
   row=run('original24-'+str(serial),[str(route_bin),name,'--exact','--test-threads=1'],env,120);actual+=prove_actual(Path(row['merged_log']).read_text(),[name])
  save(e/'original24-actual.json',{'source_sha':b['candidate'],'actual':actual,'binary_sha256':sha(route_bin),'full_discovery_retained':True});state['completed'].append('private-roots-and-original24')
  native,native_bin=binary_compile('native7-codegen-link',['cargo','test','-p','api-server','--features','test-support','--test','console_core_registry_tests','--no-run','--jobs','1','--message-format=json'],'console_core_registry_tests',diagnostic);listed,ignored=list_binary('native7-full',native_bin,env);assert sorted(listed)==sorted(b['native7_names']) and not ignored;row=run('native7-actual',[str(native_bin),'--test-threads=1'],env,120);save(e/'native7-actual.json',{'source_sha':b['candidate'],'actual':prove_actual(Path(row['merged_log']).read_text(),listed),'binary_sha256':sha(native_bin)});state['completed'].append('native7')
  # Original builders retain args/env. Cargo JSON is observation only; resolved units, never CLI feature spelling, determine reuse.
  for index,command in enumerate(data['tests']):
   argv=[command['command'],*command['args']];obs=json_argv(argv);ce={**g.command_env(data,command,target),'RUSTC_WRAPPER':diagnostic['RUSTC_WRAPPER'],'ISSUE2818_ROOT_LOG':diagnostic['ISSUE2818_ROOT_LOG'],'ISSUE2818_ALL_RUSTC_LOG':env['ISSUE2818_ALL_RUSTC_LOG']}
   listing=run('consumer-'+str(index)+'-list',[*obs,'--list'],ce);emitted=artifacts(listing);apiunits=[x for x in emitted if x['target']['name']=='api_server' and x['target']['kind']==['lib'] and not x['profile']['test']];assert len(apiunits)==1 and apiunits[0]['fresh'] and apiunits[0]['features']==api[0]['features'] and apiunits[0]['profile']==api[0]['profile'],'actual resolved API unit not reused'
   listed=g.cases(listing['merged_log'],roots,'list');assert listed;warm={**ce,'ISSUE2818_EXPECT_NO_RUST_COMPILE':'1'};ign=run('consumer-'+str(index)+'-ignored',[*obs,'--list','--ignored'],warm,120);ignored=g.cases(ign['merged_log'],roots,'list');actual=run('consumer-'+str(index)+'-actual',obs,warm,120);ran=g.cases(actual['merged_log'],roots,'run');assert Counter((x[0],x[1],'ignored' if x in ignored else 'ok') for x in listed)==Counter(map(tuple,ran)) and any(x[2]=='ok' for x in ran);save(e/('consumer-'+str(index)+'-actual.json'),{'source_sha':b['candidate'],'original_builder':command,'list':listed,'ignored':ignored,'actual':ran,'resolved_API_features':apiunits[0]['features']});state['completed'].append('consumer-'+str(index))
  boundary=['cargo','test','-p','control-plane-postgres-tests','--test','dependency_boundaries','--jobs','1','--no-run','--message-format=json'];root,binpath=binary_compile('dependency-boundaries-codegen',boundary,'dependency_boundaries',diagnostic);listed,ignored=list_binary('dependency-boundaries',binpath,env);assert not ignored;row=run('dependency-boundaries-actual',[str(binpath),'--test-threads=1'],env,120);save(e/'dependency-boundaries-actual.json',{'source_sha':b['candidate'],'actual':prove_actual(Path(row['merged_log']).read_text(),listed)});state['completed'].append('dependency-boundaries')
  for index,argv in enumerate(b['clean_checks']):run('clean-check-'+str(index),[*argv,'--message-format=json'],env);state['completed'].append('clean-check-'+str(index))
  # Frozen literal pair: same actual api_identity marker, no target projection and no negative credit for unrelated failure.
  c01=source/b['C01']['path'];c01_original=c01.read_bytes();assert b['C01']['marker'].encode() not in c01_original;c01.write_bytes(c01_original+b['C01']['append'].encode());save(e/'C01-mutation.json',{'source_sha':b['candidate'],'path':b['C01']['path'],'original_sha256':hashlib.sha256(c01_original).hexdigest(),'injected_sha256':sha(c01),'append':b['C01']['append']})
  for side,argv in [('old-literal',b['C01']['old_literal']),('new-alltargets',b['C01']['new'])]:
   row=run('C01-'+side,argv,{**diagnostic,'ISSUE2818_RUSTC_MODE':'record-only'},600,expected='nonzero');text=Path(row['merged_log']).read_text();hit=b['C01']['marker'] in text and re.search(r'-->\s+(?:.*?/)?apps/api-server/tests/http/api_identity\.rs:\d+:\d+',text);save(e/('C01-'+side+'-detection.json'),{'source_sha':b['candidate'],'argv':argv,'exit':row['exit_code'],'marker_and_primary_span_observed':bool(hit),'literal_not_projection':True});assert row['exit_code']==101 and hit,'C01 literal marker not actually observed; no D credit';state['completed'].append('C01-'+side)
  c01.write_bytes(c01_original);cap(['git','diff','--exit-code','HEAD','--']);state['phase']='complete';save(e/'status.json',{'status':'COUPLED_FINITE_WORKLOAD_COMPLETED','source_sha':b['candidate'],'state':state,'not_Root_runtime_or_resource_PASS':True});return 0
 except Exception as error:
  save(e/'failure.json',{'phase':state['phase'],'type':type(error).__name__,'error':str(error),'no_unchanged_retry':True});print(json.dumps({'coupled_failure':{'source_sha':b['candidate'],'phase':state['phase'],'error':str(error)}}),flush=True);raise
 finally:
  if c01 is not None and c01_original is not None:c01.write_bytes(c01_original);save(e/'C01-restored.json',{'sha256':sha(c01),'equals_original':c01.read_bytes()==c01_original})
  save(e/'qualification-phase-state.json',state)
  if source.exists():mono('final')
  g.stage_summary();g.retain_seed_cache(source,target,state);save(e/'cost-summary.json',{'commands':g.RESULTS,'all_command_wall_seconds':sum(x['wall_seconds'] for x in g.RESULTS),'source_sha':b['candidate']});save(e/'budget-after.json',g.budget_snapshot());print(json.dumps({'coupled_final':state}),flush=True)
if __name__=='__main__':
 if len(sys.argv)>1 and sys.argv[1]=='--self-check':print(json.dumps({'self_check':'PASS','candidate':check()['candidate'],'no_execution':True}))
 else:sys.exit(main())
