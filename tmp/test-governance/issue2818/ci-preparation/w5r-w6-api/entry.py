#!/usr/bin/env python3
"""One authorized joint API batch, using the existing budget/diagnostic/retention owner."""
import hashlib,importlib.util,json,os,re,subprocess,sys,time
from collections import Counter
from pathlib import Path
HERE=Path(__file__).resolve().parent;ROOT=HERE.parents[4]
def read(p):return json.loads(Path(p).read_text())
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def save(p,v):Path(p).parent.mkdir(parents=True,exist_ok=True);Path(p).write_text(json.dumps(v,indent=2,sort_keys=True)+'\n')
def check():
 b=read(HERE/'binding.json');assert sha(ROOT/b['packet'])==b['packet_sha256'];packet=read(ROOT/b['packet']);assert packet['candidate']==b['candidate']=='9f35e154b6908eb54b51d1ceb504552b0a4e0eba'
 assert sha(HERE.parent/'experiment.py')==b['shared_budget_helper_sha256'];assert sha(HERE/'prior-api-units.json')==b['prior_api_units_sha256'];assert sha(ROOT/b['prefetch_contract'])==b['prefetch_contract_sha256']
 assert packet['steps'][0]['argv']==b['A'] and packet['steps'][1]['argv']==b['B'];assert all(x['seconds']==600 for x in packet['steps'][:2]);return b

def main():
 b=check();s=importlib.util.spec_from_file_location('budget_owner',HERE.parent/'experiment.py');g=importlib.util.module_from_spec(s);s.loader.exec_module(g)
 for name,value in [('PRIOR_RUN','42633851815'),('PRIOR_NAME','issue2818-clean-checks-cache-42633851815-1'),('PRIOR_ARTIFACT_ID',13682384755),('ARCHIVE_BYTES',1505695992),('ARCHIVE_SHA','0acde5468f55e6b1939011ebd98802ccc1807f623710e1182fc30dab598341d6')]:setattr(g,name,value)
 g.END=time.monotonic()+1800;g.EVIDENCE.mkdir(parents=True,exist_ok=False);g.TEMP.mkdir(parents=True,exist_ok=False);e=g.EVIDENCE;source=g.TEMP/'seed-source';target=g.TEMP/'target-seed';state={'source_sha':b['candidate'],'phase':'prepare','A_complete':False,'B_complete':False,'C_complete':False,'no_PG':True,'not_Root_or_performance_PASS':True}
 def capture(argv):return subprocess.check_output(argv,cwd=source,text=True,stderr=subprocess.STDOUT,timeout=60).strip()
 def artifacts(row):
  items=[]
  for line in Path(row['merged_log']).read_text(errors='replace').splitlines():
   try:x=json.loads(line)
   except ValueError:continue
   if x.get('reason')=='compiler-artifact':items.append(x)
  return items
 def unit(x):return json.dumps({k:x[k] for k in ['package_id','features','profile']}|{'name':x['target']['name'],'kind':x['target']['kind'],'src_path':x['target']['src_path']},sort_keys=True)
 def collect(label,argv,env):
  try:return g.run(label,argv,cwd=source/'api',env=env,heavy=True,seconds=600)
  finally:
   g.stage_summary();g.collect_mono_files(source,target)
 try:
  initial=g.budget_snapshot();save(e/'budget-before.json',initial);assert initial['memory.max']=='8000000000' and initial['memory.high']=='max' and initial['memory.swap.max']=='0';q,p=map(int,initial['cpu.max'].split());assert q/p<=os.cpu_count()*.6+.001
  assert g.capture(['rustc','--version']).startswith('rustc 1.99.1 ');assert not any(os.environ.get(k) for k in ['RUSTC_WRAPPER','RUSTC_WORKSPACE_WRAPPER','RUSTC_BOOTSTRAP','RUSTFLAGS','CARGO_ENCODED_RUSTFLAGS'])
  os.environ['ISSUE2818_OBSERVED_SOURCE_SHA']=b['cache_source'];archive,_=g.download_retained();g.run('checkout-cache-source',['git','worktree','add','--detach',str(source),b['cache_source']],seconds=120);g.run('restore-source-target',['tar','-I','zstd -T1','-xf',str(archive),'--no-same-owner','-C',str(g.TEMP)],seconds=180)
  assert capture(['git','rev-parse','HEAD'])==b['cache_source'];capture(['git','diff','--exit-code','HEAD','--']);contract=read(ROOT/b['prefetch_contract']);before={p:sha(source/p) for p in contract['file_hashes']};assert all(v==contract['file_hashes'][p]['cache'] for p,v in before.items())
  clear=['CARGO_NET_OFFLINE','RUSTC_WRAPPER','RUSTC_WORKSPACE_WRAPPER','RUSTC_BOOTSTRAP','RUSTFLAGS','CARGO_ENCODED_RUSTFLAGS'];fetch_env=dict(os.environ)
  for key in clear:fetch_env.pop(key,None)
  state['phase']='registry-prefetch'
  try:g.run('registry-prefetch',['cargo','fetch','--locked','--manifest-path','api/Cargo.toml'],cwd=source,env=fetch_env,seconds=180)
  finally:
   after={p:sha(source/p) for p in before};save(e/'prefetch-lock-source-proof.json',{'before':before,'after':after,'all_unchanged':before==after,'source_sha':b['cache_source'],'effective_env':{k:fetch_env.get(k) for k in clear}})
  assert before==after;capture(['git','checkout','--detach',b['candidate']]);assert capture(['git','rev-parse','HEAD'])==b['candidate'];capture(['git','diff','--exit-code','HEAD','--']);assert capture(['git','rev-parse','HEAD:api'])==b['api_tree']
  assert all(sha(source/p)==v for p,v in b['source_hashes'].items());assert all(sha(source/p)==before[p] for p in before)
  os.environ['ISSUE2818_OBSERVED_SOURCE_SHA']=b['candidate'];save(e/'source-identity.json',{'source_sha':b['candidate'],'api_tree':b['api_tree'],'source_hashes':b['source_hashes'],'clean_tracked':True,'no_overlay':True,'cache_source_sha':b['cache_source']})
  meta=g.run('cargo-metadata',['cargo','metadata','--no-deps','--format-version','1','--manifest-path','api/Cargo.toml'],cwd=source,seconds=60);metadata=json.loads(Path(meta['stdout_log']).read_text());roots=g.root_names(source,metadata)
  env={**os.environ,'CARGO_TARGET_DIR':str(target),'CARGO_BUILD_JOBS':'1','CARGO_INCREMENTAL':'0','CARGO_NET_OFFLINE':'true','CARGO_TERM_COLOR':'never','TERM':'dumb'}
  for key in clear[1:]:env.pop(key,None)
  diagnostic=g.install_stage_wrapper(source,env);assert diagnostic.get('ISSUE2818_MONO_STATS')=='1';assert hashlib.sha256(g.ROOT_WRAPPER.encode()).hexdigest()==b['root_wrapper_sha256']
  removed=[]
  for crate,name in [('interface-runtime','interface_runtime'),('api-server','api_server')]:
   for folder in (target/'debug/.fingerprint').glob(crate+'-*'):
    for file in ['lib-'+name,'lib-'+name+'.json','dep-lib-'+name]:
     f=folder/file
     if f.is_file():removed.append({'path':str(f),'sha256':sha(f)});f.unlink()
  save(e/'legitimate-root-invalidation.json',{'removed':removed,'scope':'Only changed IR/API production lib fingerprint records; source/dependencies untouched'})
  state['phase']='A-ordinary-API';row=collect('seed-normal-lib-prebuild',b['A'],diagnostic);items=artifacts(row);save(e/'A-artifacts.json',{'artifacts':items});prior=read(HERE/'prior-api-units.json')['units'];assert Counter(map(unit,items))==Counter(map(unit,prior)),'A dependency identity/profile/features graph changed'
  changed=[x for x in items if not x['fresh']];assert sorted(x['target']['name'] for x in changed)==['api_server','interface_runtime'],'A unexpected recompilation or changed root not actually rebuilt';api=next(x for x in changed if x['target']['name']=='api_server');assert api['profile']['test']==False and api['target']['kind']==['lib'] and all(any(str(f).endswith(ext) for f in api['filenames']) for ext in ['.rlib','.rmeta'])
  state['A_complete']=True;save(e/'A-qualified.json',{'source_sha':b['candidate'],'graph_matches_prior':True,'total_artifacts':len(items),'rebuilt':['api_server','interface_runtime'],'other_Fresh':len(items)-2,'actual_API_features':api['features'],'actual_profile':api['profile'],'not_6_4GB_or_Root_PASS':True})
  state['phase']='B-existing-private-contracts';row=collect('contracts-full-codegen-link',b['B'],diagnostic);items=artifacts(row);save(e/'B-artifacts.json',{'artifacts':items});libs=[x for x in items if x['target']['name']=='api_server' and not x['profile']['test']];assert len(libs)==1 and libs[0]['fresh'];tests=[x for x in items if x['target']['name']=='api_private_contracts'];assert len(tests)==1 and tests[0]['executable'] and Path(tests[0]['executable']).is_file();binary=Path(tests[0]['executable']);proof={'source_sha':b['candidate'],'binary_sha256':sha(binary),'artifact':tests[0],'API_lib_Fresh':True,'rebuilt_units':[x for x in items if not x['fresh']]};save(e/'B-qualified.json',proof);state['B_complete']=True
  for label,args in [('contracts-full-list',['--list']),('contracts-full-ignored',['--list','--ignored'])]:
   result=g.run(label,[str(binary),*args],cwd=source/'api',env=env,heavy=True,seconds=120);names=re.findall(r'^(.+): (?:test|benchmark)$',Path(result['merged_log']).read_text(),re.M);save(e/(label+'.json'),{'binary_sha256':proof['binary_sha256'],'names':names,'count':len(names),'actual_execution':False});assert names or label.endswith('ignored')
  state['phase']='C-builder-identity';os.environ['ISSUE2818_FROZEN_RUNTIME']=json.dumps(b['frozen_runtime']);data=g.plan(source);commands=data['tests'][:2];save(e/'C-real-builder-commands.json',{'commands':commands,'runtime':data['runtime'],'B_features':tests[0]['features']})
  for index,command in enumerate(commands):
   args=command['args'];requested=[]
   if '--features' in args:requested=args[args.index('--features')+1].replace(',',' ').split()
   assert sorted(requested)==sorted(tests[0]['features']),'C real builder feature selection differs from B; stop before unexpected Rust rebuild'
   argv=[command['command'],*args];ce=g.command_env(data,command,target)
   listing=g.run('seed-query-'+str(index)+'-list',[*argv,'--list'],cwd=source/command['cwd'],env=ce,heavy=True,seconds=120);ignored=g.run('seed-query-'+str(index)+'-ignored',[*argv,'--list','--ignored'],cwd=source/command['cwd'],env=ce,heavy=True,seconds=120);listed=g.cases(listing['merged_log'],roots,'list');ignored_rows=g.cases(ignored['merged_log'],roots,'list');assert listed
   actual=g.run('seed-query-'+str(index)+'-execute',argv,cwd=source/command['cwd'],env=ce,heavy=True,seconds=120);ran=g.cases(actual['merged_log'],roots,'run');assert Counter((x[0],x[1],'ignored' if x in ignored_rows else 'ok') for x in listed)==Counter(map(tuple,ran)) and any(x[2]=='ok' for x in ran);save(e/('seed-query-'+str(index)+'-cases.json'),{'listed':listed,'ignored':ignored_rows,'actual':ran})
  state['C_complete']=True;state['phase']='complete';save(e/'status.json',{'status':'JOINT_API_FINITE_QUALIFICATION_COMPLETE','state':state,'not_HTTP_or_Root_PASS':True});return 0
 except Exception as error:
  save(e/'failure.json',{'phase':state['phase'],'type':type(error).__name__,'error':str(error),'no_unchanged_retry':True});print(json.dumps({'joint_api_failure':{'source_sha':b['candidate'],'phase':state['phase'],'error':str(error)}}),flush=True);raise
 finally:
  save(e/'qualification-phase-state.json',state);g.stage_summary();g.collect_mono_files(source,target);g.retain_seed_cache(source,target,state);save(e/'cost-summary.json',{'commands':g.RESULTS,'all_command_wall_seconds':sum(x['wall_seconds'] for x in g.RESULTS)});save(e/'budget-after.json',g.budget_snapshot());print(json.dumps({'joint_api_final':state}),flush=True)
if __name__=='__main__':
 if len(sys.argv)>1 and sys.argv[1]=='--self-check':print(json.dumps({'self_check':'PASS','candidate':check()['candidate'],'no_execution':True}))
 else:sys.exit(main())
