#!/usr/bin/env python3
"""One W5 real-fixture cost qualification, reusing the existing focused IR target."""
import ast,hashlib,importlib.util,json,os,re,subprocess,sys,time
from pathlib import Path
HERE=Path(__file__).resolve().parent;ROOT=HERE.parents[4]
def read(p):return json.loads(p.read_text())
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def save(p,v):Path(p).write_text(json.dumps(v,indent=2,sort_keys=True)+'\n')
def module(name,path):
    spec=importlib.util.spec_from_file_location(name,path);obj=importlib.util.module_from_spec(spec);spec.loader.exec_module(obj);return obj
def check(require_authorized=True):
    b=read(HERE/'binding.json');assert len(b['schedule'])==46
    if require_authorized:
        assert b['execution_authorized'] and isinstance(b['current_sha'],str) and re.fullmatch(r'[0-9a-f]{40}',b['current_sha'])
        assert not b['candidate_source_contract_pending']
        approval=b['candidate_actual_verifier'];assert approval and approval['candidate_sha']==b['current_sha']
        assert sha(ROOT/approval['path'])==approval['sha256']
        actual=read(ROOT/approval['path']);assert actual.get('candidate_sha',actual.get('current_sha',actual.get('source_sha')))==b['current_sha'] and 'PASS' in actual['status'] and not actual.get('blockers')
        assert read(ROOT/b['environment_priming']['source_contract'])['source_revisions']['current']==b['current_sha']
    assert all(sha(ROOT/f)==h for f,h in b['fixture_hashes'].items())
    assert sha(HERE/'root-wrapper.py')==b['wrapper_sha256'] and sha(HERE/'root-compile.py')==b['compile_helper_sha256']
    assert sha(HERE.parent/'experiment.py')==b['shared_budget_helper_sha256']
    assert sha(ROOT/b['prior_probe_fixture_verifier']['path'])==b['prior_probe_fixture_verifier']['sha256'] and len(b['prior_probe_fixture_verifier']['checks'])==14 and all(c['status']=='PASS' for c in b['prior_probe_fixture_verifier']['checks'])
    assert b['cache']['id']==13682384755 and b['old_sha']=='ca19501827fdeac86b9f5d9fcba767715220f3dd'
    assert sha(ROOT/b['environment_priming']['source_contract'])==b['environment_priming']['source_contract_sha256']
    assert sha(ROOT/b['candidate_behavior_gate']['manifest'])==b['candidate_behavior_gate']['manifest_sha256']
    behavior=read(ROOT/b['candidate_behavior_gate']['manifest']);manifest=read(ROOT/b['fixture_manifest'])
    assert sha(ROOT/b['candidate_behavior_gate']['legacy_manifest'])==b['candidate_behavior_gate']['legacy_manifest_sha256']
    legacy=read(ROOT/b['candidate_behavior_gate']['legacy_manifest'])
    assert behavior['legacy_names']==legacy['names'] and behavior['legacy_names_sha256']==legacy['canonical_names_sha256']
    assert behavior['candidate_sha']==manifest['current_sha']==b['current_sha']
    assert behavior['expected_count']==b['candidate_behavior_gate']['expected_count']==88
    assert behavior['expected_compiled_with_cost_probes']==b['candidate_behavior_gate']['expected_compiled_count']==91
    assert len(behavior['legacy_names'])==83 and len(behavior['revision_guard_names'])==5
    assert sorted(manifest['candidate_revision_guards'])==behavior['revision_guard_names']
    return b

def batch(phase,evidence,source):
    b=check();e=Path(evidence);helper=module('root_compile',HERE/'root-compile.py');proofs=read(e/'four-binaries.json');records=[]
    entries=[row for row in b['schedule'] if row['phase']==phase];folder=e/'processes'/phase;folder.mkdir(parents=True,exist_ok=False)
    for serial,row in enumerate(entries):
        proof=proofs[row['binary']];binary=Path(proof['binary']);assert sha(binary)==proof['sha256']
        env=dict(os.environ)
        for key in ['RUSTC_WRAPPER','RUSTC_WORKSPACE_WRAPPER','RUSTC_BOOTSTRAP','RUSTFLAGS','CARGO_ENCODED_RUSTFLAGS']:env.pop(key,None)
        env.update(W5_PROBE_ITERS=str(row['iters']),W5_PROBE_WARMUP=str(row['warmup']))
        name=f"{serial:03d}-{row['case']}-{row['side']}-pair{row['pair']}";raw=folder/(name+'.log');started=time.monotonic();argv=helper.exact_command(binary,row['name'],b)
        with raw.open('wb') as f:result=subprocess.run(argv,cwd=source,env=env,stdout=f,stderr=subprocess.STDOUT,timeout=120)
        record={**row,'argv':argv,'binary_sha256':proof['sha256'],'raw_log':str(raw),'exit':result.returncode,'wall_seconds':time.monotonic()-started};records.append(record);save(folder/'process-receipts.json',records)
        assert result.returncode==0,'exact probe command failed; no unchanged retry'
        text=raw.read_text();helper.prove_actual(text,row)
        if row['mode']=='normal':
            totals=re.findall(r'W5_TOTAL case=\S+ iterations=100 normal_nanos=Some\((\d+)\) counts_timing_not_latency=false',text);assert len(totals)==1;record['normal_total_nanoseconds']=int(totals[0])
        else:
            record['live_nonnegative_all_reported']='live_nonnegative=false' not in text
            record['instrumented_time_is_latency']=False
        save(folder/'process-receipts.json',records)
    save(e/(phase+'-processes.json'),records)
    if phase=='old-old-noise':
        noise={}
        for case in ['empty','document']:
            relative=[]
            for pair in range(5):
                values=[r['normal_total_nanoseconds'] for r in records if r['case']==case and r['pair']==pair];assert len(values)==2;relative.append(abs(values[0]-values[1])/min(values))
            noise[case]={'relative_differences':relative,'band':max(relative),'all_five_retained':True}
        save(e/'precomparison-noise-bands.json',{'cases':noise,'fixed_before_old_current':True,'rule':b['noise']})
    return 0

def main():
    b=check();g=module('budget_owner',HERE.parent/'experiment.py');helper=module('root_compile',HERE/'root-compile.py')
    g.PRIOR_RUN=b['cache']['run'];g.PRIOR_NAME=b['cache']['name'];g.PRIOR_ARTIFACT_ID=b['cache']['id'];g.ARCHIVE_BYTES=b['cache']['archive_bytes'];g.ARCHIVE_SHA=b['cache']['archive_sha256'];g.END=time.monotonic()+20*60
    g.EVIDENCE.mkdir(parents=True,exist_ok=False);g.TEMP.mkdir(parents=True,exist_ok=False);e=g.EVIDENCE;source=g.TEMP/'seed-source';target=g.TEMP/'target-seed';binary_dir=e/'test-binaries';state={'old_sha':b['old_sha'],'source_sha':b['current_sha'],'phase':'prepare','completed':False,'no_old83_or_API_PG':True,'candidate_behavior_once':True,'cache_source_sha':b['cache']['source_sha']};proofs={}
    def capture(argv):return subprocess.check_output(argv,cwd=source,stderr=subprocess.STDOUT,timeout=60)
    manifest=read(ROOT/b['fixture_manifest']);patch=ROOT/b['overlay_patch']
    try:
        initial=g.budget_snapshot();save(e/'budget-before.json',initial);assert initial['memory.max']=='8000000000' and initial['memory.high']=='max' and initial['memory.swap.max']=='0';q,p=map(int,initial['cpu.max'].split());assert q/p<=os.cpu_count()*.6+.001
        assert g.capture(['rustc','--version']).startswith('rustc 1.99.1 ')
        assert not any(os.environ.get(k) for k in ['RUSTC_WRAPPER','RUSTC_WORKSPACE_WRAPPER','RUSTC_BOOTSTRAP','RUSTFLAGS','CARGO_ENCODED_RUSTFLAGS'])
        save(e/'environment.json',{'binding':b,'rustc':g.capture(['rustc','-vV']),'cargo':g.capture(['cargo','-vV']),'host_cpu_count':os.cpu_count()})
        archive,incoming=g.download_retained();g.run('checkout-cache-source',['git','worktree','add','--detach',str(source),b['cache']['source_sha']],seconds=120);g.run('restore-source-target',['tar','-I','zstd -T1','-xf',str(archive),'--no-same-owner','-C',str(g.TEMP)],seconds=180)
        save(e/'cache-resume.json',{'cache':b['cache'],'scope':'cloud restored source7bb/dependencies; W5 probe uses isolated old/current checkout, no API or new cache archive'})
        capture(['git','diff','--exit-code','HEAD','--'])
        # Authorized source-only environment priming, before old/current overlays.
        state['phase']='registry-prefetch'
        assert capture(['git','rev-parse','HEAD']).decode().strip()==b['cache']['source_sha']
        source_contract=read(ROOT/b['environment_priming']['source_contract'])
        cargo_before={path:sha(source/path) for path in source_contract['file_hashes']}
        assert all(digest==source_contract['file_hashes'][path]['cache'] for path,digest in cargo_before.items())
        prefetch_env=dict(os.environ)
        clear=['CARGO_NET_OFFLINE','RUSTC_WRAPPER','RUSTC_WORKSPACE_WRAPPER','RUSTC_BOOTSTRAP','RUSTFLAGS','CARGO_ENCODED_RUSTFLAGS']
        for key in clear:prefetch_env.pop(key,None)
        os.environ['ISSUE2818_OBSERVED_SOURCE_SHA']=b['cache']['source_sha']
        prefetch_result=None
        try:
            prefetch_result=g.run('registry-prefetch',b['environment_priming']['command'],cwd=source,env=prefetch_env,seconds=180)
        finally:
            cargo_after={path:sha(source/path) for path in source_contract['file_hashes']}
            save(e/'prefetch-lock-source-proof.json',{'source_sha':b['cache']['source_sha'],'source_path':str(source),'before_overlay':True,'argv':b['environment_priming']['command'],'deadline_seconds':180,'effective_env':{key:prefetch_env.get(key) for key in clear},'Cargo_lock_before':cargo_before['api/Cargo.lock'],'Cargo_lock_after':cargo_after['api/Cargo.lock'],'all_manifest_lock_before':cargo_before,'all_manifest_lock_after':cargo_after,'all_files_unchanged':cargo_before==cargo_after,'prefetch_result':prefetch_result,'prior_failed_run':42665929614,'prior_failure_preserved':True})
        assert cargo_before==cargo_after,'Cargo source or lock mutated during fetch --locked'
        capture(['git','diff','--exit-code','HEAD','--'])
        cargo_home=Path(prefetch_env.get('CARGO_HOME',str(Path.home()/'.cargo')))
        checkout_proof=[]
        for checkout in sorted((cargo_home/'git/checkouts').glob('tokio-tungstenite-*/*')):
            if not (checkout/'Cargo.toml').is_file():continue
            actual_rev=subprocess.check_output(['git','rev-parse','HEAD'],cwd=checkout,text=True,timeout=10).strip()
            checkout_proof.append({'path':str(checkout),'git_rev':actual_rev,'Cargo_toml_exists':True,'Cargo_toml_sha256':sha(checkout/'Cargo.toml')})
        save(e/'prefetched-Git-checkout-proof.json',{'required_repo':'https://github.com/openai-oss-forks/tokio-tungstenite','required_rev':b['environment_priming']['required_tokio_git_rev'],'checkout_evidence':checkout_proof,'no_compilation_in_priming':True})
        assert any(row['git_rev']==b['environment_priming']['required_tokio_git_rev'] for row in checkout_proof),'required pinned Git checkout absent after fetch'
        for side in ['old','current']:
            revision=b['old_sha'] if side=='old' else b['current_sha'];state['phase']='source-'+side;capture(['git','checkout','--detach',revision]);assert capture(['git','rev-parse','HEAD']).decode().strip()==revision
            originals={f:capture(['git','show',revision+':'+f]) for f in manifest['maximum_write_set'] if f not in manifest['new_Rust_files']}
            capture(['git','apply','--check',str(patch)]);capture(['git','apply',str(patch)])
            assert all((source/f).read_bytes().startswith(data) for f,data in originals.items());assert sha(source/manifest['new_Rust_files'][0])==manifest['new_files']['helper_sha256']
            save(e/(side+'-overlay-proof.json'),{'actual_source_sha':revision,'same_patch_sha256':sha(patch),'original_four_full_prefixes_preserved':True,'overlay_after_hashes':{f:sha(source/f) for f in manifest['maximum_write_set']}})
            os.environ['ISSUE2818_OBSERVED_SOURCE_SHA']=revision
            for mode in ['normal','counts']:
                state['phase']='compile-'+side+'-'+mode;proofs[side+'-'+mode]=helper.compile_one(g,b,source,target,binary_dir,side,mode)
            assert proofs[side+'-normal']['sha256']!=proofs[side+'-counts']['sha256']
            capture(['git','restore','--worktree','--source',revision,'--',*originals]);(source/manifest['new_Rust_files'][0]).unlink();capture(['git','diff','--exit-code','HEAD','--'])
        assert len(proofs)==4;save(e/'four-binaries.json',proofs)
        assert len({json.dumps(r['actual_artifact']['features']) for r in proofs.values()})==1
        behavior=read(ROOT/b['candidate_behavior_gate']['manifest'])
        for name,proof in proofs.items():
            state['phase']='discover-'+name;assert sha(proof['binary'])==proof['sha256']
            argv=helper.candidate_discovery_command(proof['binary']) if name=='current-normal' else helper.discovery_command(proof['binary'])
            row=g.run(state['phase'],argv,cwd=source/'api',heavy=True,seconds=120);text=Path(row['merged_log']).read_text()
            if name=='current-normal':
                compiled=helper.prove_candidate_compiled_behavior(text,b,behavior);save(e/'candidate-behavior-compiled-proof.json',{**compiled,'source_sha':b['current_sha'],'binary':proof['binary'],'binary_sha256':proof['sha256'],'legacy_names_sha256':behavior['legacy_names_sha256'],'no_actual_tests':True})
            else:
                names=helper.prove_discovery(text,b);save(e/(name+'-ignored-discovery.json'),{'names':names,'binary_sha256':proof['sha256'],'no_actual_tests':True})
        os.environ['ISSUE2818_OBSERVED_SOURCE_SHA']=b['current_sha']
        state['phase']='candidate-behavior';normal=proofs['current-normal'];assert sha(normal['binary'])==normal['sha256']
        behavior_env=dict(os.environ)
        for key in ['W5_PROBE_ITERS','W5_PROBE_WARMUP','RUSTC_WRAPPER','RUSTC_WORKSPACE_WRAPPER','RUSTC_BOOTSTRAP','RUSTFLAGS','CARGO_ENCODED_RUSTFLAGS']:behavior_env.pop(key,None)
        row=g.run('candidate-behavior',helper.candidate_behavior_command(normal['binary'],b),cwd=source/'api',env=behavior_env,heavy=True,seconds=120)
        actual_behavior=helper.prove_candidate_actual_behavior(Path(row['merged_log']).read_text(),behavior);assert sha(normal['binary'])==normal['sha256']
        save(e/'candidate-behavior-actual-proof.json',{**actual_behavior,'source_sha':b['current_sha'],'binary':normal['binary'],'binary_sha256':normal['sha256'],'legacy_names_sha256':behavior['legacy_names_sha256'],'exit':0,'cleanup_complete':True,'raw_log':row['merged_log']})
        for phase in ['old-old-noise','old-current-timing','counts']:
            state['phase']=phase
            if phase=='old-current-timing':assert (e/'precomparison-noise-bands.json').is_file()
            g.run(phase,[sys.executable,str(Path(__file__).resolve()),'--batch',phase,str(e),str(source/'api')],cwd=source/'api',heavy=True,seconds=120)
        for proof in proofs.values():assert sha(proof['binary'])==proof['sha256']
        state.update(completed=True,phase='complete');save(e/'status.json',{'status':'W5_EXACT_REAL_FIXTURE_COST_PHASES_COMPLETED','scope':'local System small-type actual IR fixtures only; not API/global resource/Root PASS','no_C01_or_D_change':True,'candidate_behavior_passed':True});return 0
    except Exception as error:
        save(e/'failure.json',{'phase':state['phase'],'type':type(error).__name__,'error':str(error),'no_unchanged_retry':True});raise
    finally:
        save(e/'qualification-phase-state.json',state);save(e/'budget-after.json',g.budget_snapshot());save(e/'cost-summary.json',{'commands':g.RESULTS,'all_command_wall_seconds':sum(r['wall_seconds'] for r in g.RESULTS),'extra_per_process_receipts':'processes/*/process-receipts.json','no_retry_or_old83_rerun':True,'candidate_behavior_once':True});save(e/'cache-retention.json',{'cache':b['cache'],'new_cache_archive_created':False,'status':'prior_cloud_cache_remains_retained; temporary probe target not archived'})
if __name__=='__main__':
    if sys.argv[1:]==['--self-check']:b=check(require_authorized=False);print(json.dumps({'status':'PREPARED_SOURCE_PENDING_NO_RUST_RUN','old':b['old_sha'],'current':b['current_sha'],'compile_roots':4,'normal_processes':40,'counts_processes':6,'discovery_processes':4}))
    elif sys.argv[1:]==['--execute']:sys.exit(main())
    elif len(sys.argv)==5 and sys.argv[1]=='--batch':sys.exit(batch(*sys.argv[2:]))
    else:raise SystemExit('Expected --self-check or Root-authorized --execute')
