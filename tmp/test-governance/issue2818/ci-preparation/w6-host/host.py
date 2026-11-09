#!/usr/bin/env python3
"""W6 small-host draft. No Rust runs unless invoked with --execute after publication."""
import ast, hashlib, importlib.util, json, os, shutil, sys, time
from pathlib import Path
HERE=Path(__file__).resolve().parent
ROOT=HERE.parents[4]
BINDING=HERE/'binding.json'
QUAL=ROOT/'tmp/test-governance/issue2818/development/static-runtime-reads-schema/qualification'
def read(p): return json.loads(p.read_text())
def sha(p): return hashlib.sha256(p.read_bytes()).hexdigest()
def self_check():
    b=read(BINDING)
    assert b['source_sha']=='903971d5c3ddd37f93c1f7314e02dae4a2bb120c'
    assert b['old_source_sha']==b['cache']['source_sha']=='7bb249e47d29dce87a291677884bd42962aa2046'
    assert b['parameters']['diagnostic_flags']==[]
    assert sha(HERE.parent/'experiment.py')==b['shared_budget_helper_sha256']
    assert all(sha(ROOT/p)==h for p,h in b['fixture_hashes'].items())
    assert b['serde_json']['corresponding_rlib'] in b['serde_json']['compiler_artifact']['filenames']
    assert b['serde_json']['compiler_artifact']['features']==['alloc','default','float_roundtrip','indexmap','preserve_order','raw_value','std']
    for p in [HERE.parent/'experiment.py',QUAL/'render.py',Path(__file__)]: ast.parse(p.read_text())
    return b
def samples(path, tag, names):
    text=path.read_text()
    assert 'validation=pass full_value=equal original_bytes=equal per_call_freshness=pass' in text
    assert 'source=903971d5c3ddd37f93c1f7314e02dae4a2bb120c' in text
    rows=[]
    for line in text.splitlines():
        if not line.startswith(tag+'\t'): continue
        row=dict(v.split('=',1) for v in line.split('\t')[1:]);rows.append(row)
    assert len(rows)==10
    assert {(int(r['pair']),r['implementation']) for r in rows}=={(i,n) for i in range(5) for n in names}
    return rows
def main():
    b=self_check();spec=importlib.util.spec_from_file_location('budget_owner',HERE.parent/'experiment.py');g=importlib.util.module_from_spec(spec);spec.loader.exec_module(g)
    g.PRIOR_RUN=b['cache']['run'];g.PRIOR_NAME=b['cache']['name'];g.PRIOR_ARTIFACT_ID=b['cache']['id'];g.ARCHIVE_SHA=b['cache']['archive_sha256'];g.ARCHIVE_BYTES=b['cache']['archive_bytes']
    g.END=time.monotonic()+20*60
    g.EVIDENCE.mkdir(parents=True,exist_ok=False);g.TEMP.mkdir(parents=True,exist_ok=False)
    e=g.EVIDENCE;seed=g.TEMP/'seed-source';target=g.TEMP/'target-seed';source=g.TEMP/'w6-source';bins=e/'host-binaries';bins.mkdir()
    state={'source_sha':b['source_sha'],'cache_source_sha':b['old_source_sha'],'no_Cargo_API_PG':True,'phase':'prepare','completed':False,'no_retry_or_sample_tuning':True}
    env=dict(os.environ)
    for k in ['RUSTC_WRAPPER','RUSTC_WORKSPACE_WRAPPER','RUSTC_BOOTSTRAP','RUSTFLAGS','CARGO_ENCODED_RUSTFLAGS','W6_OLD_CONTROL']:env.pop(k,None)
    os.environ['ISSUE2818_OBSERVED_SOURCE_SHA']=b['source_sha']
    try:
        before=g.budget_snapshot();g.save(e/'budget-before.json',before)
        assert before['memory.max']=='8000000000' and before['memory.high']=='max' and before['memory.swap.max']=='0'
        q,p=map(int,before['cpu.max'].split());assert q/p<=os.cpu_count()*.6+.001
        assert g.capture(['rustc','--version']).startswith('rustc 1.99.1 ')
        g.save(e/'environment.json',{'rustc':g.capture(['rustc','-vV']),'host_cpu_count':os.cpu_count(),'binding':b,'effective_env':{k:env.get(k) for k in ['RUSTC_WRAPPER','RUSTC_WORKSPACE_WRAPPER','RUSTC_BOOTSTRAP','RUSTFLAGS','CARGO_ENCODED_RUSTFLAGS','W6_OLD_CONTROL']}})
        archive,incoming=g.download_retained()
        g.save(e/'cache-resume.json',{'source_run':b['cache']['run'],'artifact':b['cache']['name'],'archive_sha256':b['cache']['archive_sha256'],'archive_bytes':b['cache']['archive_bytes'],'archive_source_sha':b['old_source_sha'],'cache_state':'successful two-clean-check source7bb retained source/target; direct-link serde rlib independently bound','scope':'W6 host source903 uses this cache only as prebuilt dependencies; no Cargo invocation'})
        g.run('checkout-cache-source',['git','worktree','add','--detach',str(seed),b['old_source_sha']],seconds=120)
        g.run('restore-source-target',['tar','-I','zstd -T1','-xf',str(archive),'--no-same-owner','-C',str(g.TEMP)],seconds=180)
        assert g.capture(['git','rev-parse','HEAD'],seed)==b['old_source_sha'];g.capture(['git','diff','--exit-code','HEAD','--'],seed)
        g.run('checkout-W6-source',['git','worktree','add','--detach',str(source),b['source_sha']],seconds=120)
        assert g.capture(['git','rev-parse','HEAD'],source)==b['source_sha'];g.capture(['git','diff','--exit-code','HEAD','--'],source)
        rlib=Path(b['serde_json']['corresponding_rlib']);assert rlib.is_file() and rlib.parent==target/'debug/deps';rlib_hash=sha(rlib)
        g.save(e/'serde-link-binding.json',{'path':str(rlib),'sha256':rlib_hash,'prior_API_extern':b['serde_json']['actual_API_extern'],'compiler_artifact':b['serde_json']['compiler_artifact']})
        host=e/'host.rs';g.run('render-source-bound-host',[sys.executable,str(QUAL/'render.py'),'--repo-root',str(source),'--output',str(host),'--serde-rlib',str(rlib)],seconds=60)
        manifest=read(host.with_suffix('.manifest.json'));assert manifest['source_sha']==b['source_sha'] and manifest['original_input_sha']==b['old_source_sha']
        rustc=g.capture(['rustup','which','rustc']);flags=['--edition=2021','-C','opt-level=0','-C','debuginfo=0','-C','debug-assertions=yes','-C','overflow-checks=yes','-C','codegen-units=256','-C','embed-bitcode=no','-C','strip=debuginfo','--extern','serde_json='+str(rlib),'-L','dependency='+str(rlib.parent)]
        normal=bins/'w6-normal';instrumented=bins/'w6-heap-counts'
        for name,binary,cfg in [('normal',normal,[]),('heap-counts',instrumented,['--cfg','heap_counts'])]:
            state['phase']='compile-'+name;g.run(state['phase'],[rustc,'--crate-name','issue2818_w6_host',str(host),*flags,*cfg,'-o',str(binary)],env=env,heavy=True,seconds=120)
        # Fixed run order and counts are declared in the committed binding before any outcomes.
        rows={}
        def host_run(label,binary,iters,control=False):
            state['phase']=label
            row=g.run(label,[str(binary),str(iters),'100'],env={**env,**({'W6_OLD_CONTROL':'1'} if control else {})},heavy=True,seconds=120)
            rows[label]=Path(row['merged_log']);return rows[label]
        control=samples(host_run('old-old-noise',normal,1000,True),'sample',['old_control_a','old_control_b'])
        pair=lambda rows,i,n:int(next(r['nanoseconds'] for r in rows if int(r['pair'])==i and r['implementation']==n))
        noise=[abs(pair(control,i,'old_control_a')-pair(control,i,'old_control_b'))/min(pair(control,i,'old_control_a'),pair(control,i,'old_control_b')) for i in range(5)];band=max(noise)
        assert all(r['iterations']=='1000' for r in control)
        g.save(e/'precomparison-noise-band.json',{'raw_pairs':control,'relative_differences':noise,'band':band,'rule':b['parameters']['noise_band'],'fixed_before_old_new_run':True})
        timing=samples(host_run('old-new-timing',normal,1000),'sample',['old','new'])
        heap=samples(host_run('instrumented-requested-bytes',instrumented,1),'heap',['old','new'])
        assert all(r['iterations']=='1000' for r in timing) and all(r['iterations']=='1' and r['after_drop_live_bytes']=='0' for r in heap)
        differences=[(pair(timing,i,'new')-pair(timing,i,'old'))/pair(timing,i,'old') for i in range(5)]
        g.save(e/'host-measurements.json',{'source_sha':b['source_sha'],'old_source_sha':b['old_source_sha'],'normal_control_raw':control,'normal_comparison_raw':timing,'heap_raw':heap,'old_old_relative_differences':noise,'predeclared_noise_band':band,'old_new_relative_differences':differences,'timing_disposition':['unresolved_within_noise' if abs(v)<=band else ('slower_above_noise' if v>0 else 'faster_above_noise') for v in differences],'instrumented_timings_used':False,'stack_peak':'unknown','RSS':'not measured by allocator','System_realloc_internal_peak':'unknown','rodata_delta':'unknown; total dual-body binary size is not delta','allocator_scope':b['allocator_scope'],'no_API_behavior_or_performance_or_Root_PASS':True})
        assert sha(rlib)==rlib_hash;g.capture(['git','diff','--exit-code','HEAD','--'],seed);g.capture(['git','diff','--exit-code','HEAD','--'],source)
        g.save(e/'binary-manifest.json',{x.name:{'bytes':x.stat().st_size,'sha256':sha(x)} for x in [normal,instrumented]})
        codegen={'status':'unknown','scope':'only attributable host old/new body text symbols; shared/helper/dependency text excluded','rodata_delta':'unknown'}
        if shutil.which('nm'):
            row=g.run('host-normal-symbols',['nm','--print-size','--size-sort','--demangle',str(normal)],seconds=30,expected='optional')
            if row['exit_code']==0:
                selected=[line for line in Path(row['merged_log']).read_text().splitlines() if any(key in line for key in ['issue2818_w6_host::old::managed_projection_schema','issue2818_w6_host::new::managed_projection_schema','issue2818_w6_host::new::schema::build'])]
                codegen.update(status='raw_attributable_text_symbols' if selected else 'unknown',raw_selected_lines=selected,total_dual_body_size_is_not_delta=True)
        g.save(e/'host-codegen-symbols.json',codegen)
        state['completed']=True;state['phase']='complete';return 0
    except Exception as error:
        g.save(e/'failure.json',{'phase':state['phase'],'type':type(error).__name__,'error':str(error),'no_unchanged_retry':True});raise
    finally:
        g.save(e/'qualification-phase-state.json',state);g.save(e/'budget-after.json',g.budget_snapshot())
        g.save(e/'cache-retention.json',{'status':'prior_cloud_cache_reused','cache':b['cache'],'source_paths_kept_separate':True,'new_target_compilation':False,'new_cache_archive_created':False,'reason':'host outputs live outside target; original cloud cache remains retained'})
        g.save(e/'cost-summary.json',{'commands':g.RESULTS,'total_measured_command_seconds':sum(r['wall_seconds'] for r in g.RESULTS),'no_additional_compile_or_runtime_retry':True})
if __name__=='__main__':
    if sys.argv[1:]==['--self-check']:
        b=self_check();print(json.dumps({'status':'PREPARED_ONLY','source_sha':b['source_sha'],'cache_source_sha':b['cache']['source_sha'],'no_Rust_execution':True}))
    elif sys.argv[1:]==['--execute']:sys.exit(main())
    else:raise SystemExit('Use --self-check (read only) or --execute (only after Root publication authorization)')
