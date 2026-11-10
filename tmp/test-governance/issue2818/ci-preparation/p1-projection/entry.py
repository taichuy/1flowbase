#!/usr/bin/env python3
"""Issue 2818 P1 finite ordinary-build experiment; never full qualification."""
import hashlib, importlib.util, json, os, re, subprocess, sys, time
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[4]
def read(p):
    return json.loads(Path(p).read_text())
def save(p, value):
    p = Path(p); p.parent.mkdir(parents=True, exist_ok=True)
    p.write_text(json.dumps(value, indent=2, sort_keys=True) + '\n')
def sha(p):
    digest = hashlib.sha256()
    with Path(p).open('rb') as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b''):
            digest.update(chunk)
    return digest.hexdigest()
METHODS = {
    'chat': 'application_public_protocol::protocol_mappers::openai_chat::OpenAiChatStreamMapper::runtime_event_to_sse',
    'responses': 'application_public_protocol::protocol_mappers::openai_responses::OpenAiResponseStreamMapper::runtime_event_to_sse',
    'anthropic': 'application_public_protocol::protocol_mappers::anthropic_stream::AnthropicStreamMapper::runtime_event_to_sse',
}
def symbols(path):
    rows = []
    process = subprocess.Popen(['nm', '-A', '-S', '-C', str(path)], stdout=subprocess.PIPE, text=True)
    for line in process.stdout:
        if any(name in line for name in METHODS.values()) or ('protocol_mappers::' in line and '::runtime_event_to_sse' in line):
            rows.append(line.rstrip())
    if process.wait() != 0:
        raise RuntimeError('nm failed')
    print(json.dumps({'path':str(path), 'sha256':sha(path), 'bytes':Path(path).stat().st_size,
                      'selected_symbols':rows, 'not_RSS_attribution':True}))
def main():
    b = read(HERE/'p1-binding.json')
    legacy = read(HERE.parent/'coupled-api/binding.json')
    assert b['kind'] == 'P1_MINIMAL_EXPERIMENT_NOT_FULL_QUALIFICATION'
    assert re.fullmatch('[0-9a-f]{40}', b['candidate']) and b['candidate'] != b['baseline']
    assert b['A'] == legacy['A'] and b['cache'] == legacy['cache']
    assert b['stage_order'] == legacy['stage_order']
    assert b['frozen_runtime'] == legacy['frozen_runtime']
    assert all(sha(ROOT/p) == h for p,h in b['inputs'].items())
    assert os.environ.get('CANDIDATE_SHA') == b['candidate']
    spec = importlib.util.spec_from_file_location('existing_runner', HERE.parent/'experiment.py')
    g = importlib.util.module_from_spec(spec); spec.loader.exec_module(g)
    g.SEED_SHA=b['cache']['source_sha']; g.PRIOR_RUN=str(b['cache']['run'])
    g.PRIOR_NAME=b['cache']['name']; g.PRIOR_ARTIFACT_ID=b['cache']['id']
    g.ARCHIVE_SHA=b['cache']['archive_sha256']; g.ARCHIVE_BYTES=b['cache']['archive_bytes']
    g.ROOT_WRAPPER=(HERE/'root-wrapper.py').read_text()
    g.END=time.monotonic()+4200
    g.EVIDENCE.mkdir(parents=True, exist_ok=False); g.TEMP.mkdir(parents=True, exist_ok=False)
    e=g.EVIDENCE; source=g.TEMP/'seed-source'; target=g.TEMP/'target-seed'
    state={'source_sha':b['candidate'],'baseline_sha':b['baseline'],'kind':b['kind'],
           'phase':'prepare','stages':{x:'NotReached' for x in b['stage_order']},
           'additional_affected_scope':{'api_public_api':'NotReached'},
           'full_F0_AC009_and_workload':'Incomplete','not_Root_PASS':True}
    started=time.monotonic(); initial=None; primary=None; row=None; items=[]; ordinary=None
    def cap(argv):
        return subprocess.check_output(argv,cwd=source,text=True,stderr=subprocess.STDOUT,timeout=60).strip()
    def stage(name,status):
        state['phase']=name; state['stages'][name]=status
        save(e/'qualification-phase-state.json',state)
    def artifacts():
        found=[]
        for log in e.glob('*seed-normal-lib-prebuild.combined.log'):
            for line in log.open(errors='replace'):
                try: value=json.loads(line)
                except ValueError: continue
                if value.get('reason')=='compiler-artifact': found.append(value)
        save(e/'ordinary-cargo-artifacts.json',
             {'source_sha':b['candidate'],'artifacts':found,
              'fresh':sum(x['fresh'] for x in found),'rebuilt':sum(not x['fresh'] for x in found),
              'incomplete_units_have_no_completed_artifact':True})
        return found
    try:
        save(e/'p1-binding.json',b); save(e/'qualification-phase-state.json',state)
        initial=g.budget_snapshot(); save(e/'budget-before.json',initial)
        assert initial['memory.max']=='8000000000' and initial['memory.high']=='max' and initial['memory.swap.max']=='0'
        quota,period=map(int,initial['cpu.max'].split())
        assert quota/period <= os.cpu_count()*.6+.001
        assert g.capture(['rustc','-vV']) == b['cache']['compiler']
        assert not any(os.environ.get(k) for k in ['RUSTC_WRAPPER','RUSTC_WORKSPACE_WRAPPER','RUSTC_BOOTSTRAP','RUSTFLAGS','CARGO_ENCODED_RUSTFLAGS','MALLOC_CONF'])
        save(e/'toolchain-environment.json',{'rustc':g.capture(['rustc','-vV']),
             'cargo':g.capture(['cargo','-vV']),'active_toolchain':g.capture(['rustup','show','active-toolchain']),
             'uname':g.capture(['uname','-a']),'logical_cpus':os.cpu_count(),
             'runner_image':{k:os.environ.get(k) for k in ['ImageOS','ImageVersion','RUNNER_OS','RUNNER_ARCH']},
             'variables':{k:os.environ.get(k) for k in ['RUSTC_WRAPPER','RUSTC_WORKSPACE_WRAPPER','RUSTC_BOOTSTRAP','RUSTFLAGS','CARGO_ENCODED_RUSTFLAGS','MALLOC_CONF']}})
        assert g.PG_GROUP is None and os.access(Path(initial['path'])/'memory.reclaim',os.W_OK)
        stage('resource-runner-regression','Running')
        regression=g.run('resource-runner-read-race-regression',
             [sys.executable,str(ROOT/'scripts/shell/_tests/resource-job-run.test.py'),
              'RealJobTests.test_t1_cgroup_removal_read_race_preserves_owned_identity','-v'],
             cwd=ROOT,env={**os.environ,'RESOURCE_JOB_RUN_OS_TESTS':'1'},seconds=60)
        text=Path(regression['merged_log']).read_text()
        assert re.search(r'^test_t1_cgroup_removal_read_race_preserves_owned_identity .+ \.\.\. ok$',text,re.M)
        assert 'Ran 1 test in ' in text and text.rstrip().endswith('OK')
        stage('resource-runner-regression','Passed')
        state['phase']='prepare'
        os.environ['ISSUE2818_OBSERVED_SOURCE_SHA']=b['cache']['source_sha']
        archive,_=g.download_retained()
        resume=read(e/'cache-resume.json')
        resume.update(cache_state='same fixed partial target; no API/test qualification',
                      scope=b['cache']['qualified_scope'],not_runtime_PASS=True)
        save(e/'cache-resume.json',resume)
        g.run('checkout-cache-source',['git','worktree','add','--detach',str(source),b['cache']['source_sha']],seconds=120)
        g.run('restore-source-target',['tar','-I','zstd -T1','-xf',str(archive),'--no-same-owner','-C',str(g.TEMP)],seconds=180)
        assert cap(['git','rev-parse','HEAD'])==b['cache']['source_sha']
        cap(['git','diff','--exit-code','HEAD','--'])
        cap(['git','checkout','--detach',b['candidate']])
        assert cap(['git','rev-parse','HEAD'])==b['candidate']
        cap(['git','diff','--exit-code','HEAD','--'])
        untracked=cap(['git','ls-files','--others','--exclude-standard']).splitlines()
        save(e/'restored-untracked-paths.json',untracked)
        assert not any(x.startswith(('api/','scripts/')) for x in untracked), 'untracked source under compiler or runner inputs'
        assert cap(['git','rev-parse','HEAD:api'])==b['api_tree']
        assert all(sha(source/p)==h for p,h in b['source_hashes'].items())
        os.environ['ISSUE2818_OBSERVED_SOURCE_SHA']=b['candidate']
        save(e/'source-identity.json',{'source_sha':b['candidate'],'api_tree':b['api_tree'],
              'source_hashes':b['source_hashes'],'tracked_clean':True,'baseline_sha':b['baseline'],
              'historical_reference_not_matched_control':b['historical_reference']})
        restored=g.budget_snapshot();save(e/'budget-after-restore.json',restored)
        assert int(restored['memory.peak'])<=6400000000,'cache preparation exceeded original whole-task target'
        cargos=[str(f.relative_to(source)) for f in (source/'api').rglob('Cargo.toml') if 'target' not in f.parts]+['api/Cargo.lock']
        before={p:sha(source/p) for p in cargos};fetch=dict(os.environ)
        for k in ['CARGO_NET_OFFLINE','RUSTC_WRAPPER','RUSTC_WORKSPACE_WRAPPER','RUSTC_BOOTSTRAP','RUSTFLAGS','CARGO_ENCODED_RUSTFLAGS']:
            fetch.pop(k,None)
        try:
            g.run('registry-prefetch',['cargo','fetch','--locked','--manifest-path','api/Cargo.toml'],
                  cwd=source,env=fetch,seconds=180)
        finally:
            after={p:sha(source/p) for p in cargos}
            save(e/'prefetch-proof.json',{'before':before,'after':after,'all_unchanged':before==after})
        assert before==after
        env={**os.environ,'CARGO_TARGET_DIR':str(target),'CARGO_BUILD_JOBS':'1','CARGO_INCREMENTAL':'0',
             'CARGO_NET_OFFLINE':'true','CARGO_TERM_COLOR':'never','RUST_TEST_THREADS':'1','TERM':'dumb',
             'ISSUE2818_ALL_RUSTC_LOG':str(e/'all-rustc-invocations.jsonl')}
        for k in ['RUSTC_WRAPPER','RUSTC_WORKSPACE_WRAPPER','RUSTC_BOOTSTRAP','RUSTFLAGS','CARGO_ENCODED_RUSTFLAGS']:
            env.pop(k,None)
        metadata_row=g.run('actual-cargo-metadata',['cargo','metadata','--locked','--format-version','1','--manifest-path','api/Cargo.toml'],
                           cwd=source,env=env,seconds=120)
        metadata=json.loads(Path(metadata_row['stdout_log']).read_text())
        assert metadata['resolve'] and all(any(x['name']==n for x in metadata['packages'])
                        for n in ['api-server','application-public-protocol'])
        save(e/'actual-cargo-metadata.json',metadata)
        ordinary=g.install_stage_wrapper(source,env)
        ordinary['ISSUE2818_RUSTC_MODE']='record-only';ordinary.pop('ISSUE2818_MONO_STATS',None)
        save(e/'ordinary-environment.json',{k:ordinary.get(k) for k in [
             'CARGO_BUILD_JOBS','CARGO_INCREMENTAL','CARGO_NET_OFFLINE','CARGO_TARGET_DIR',
             'RUST_TEST_THREADS','RUSTC_WRAPPER','RUSTC_WORKSPACE_WRAPPER','RUSTC_BOOTSTRAP',
             'RUSTFLAGS','CARGO_ENCODED_RUSTFLAGS','MALLOC_CONF','ISSUE2818_RUSTC_MODE']})
        probe=read(e/'mono-stats-probe.json')
        probe.update(capability_probe_only=True,actual_compile_mode='record-only',active_compile_instrumentation=[])
        save(e/'mono-stats-probe.json',probe)
        os.environ['ISSUE2818_FROZEN_RUNTIME']=json.dumps(b['frozen_runtime'])
        save(e/'frozen-real-builder-plan.json',g.plan(source))
        stage('ordinary-API','Running')
        try:
            row=g.run('seed-normal-lib-prebuild',b['A'],cwd=source/'api',env=ordinary,heavy=True,seconds=600)
        finally:
            items=artifacts()
        invoked=[json.loads(line) for line in (e/'all-rustc-invocations.jsonl').read_text().splitlines()]
        for name in ['api_server','application_public_protocol']:
            actual=[x for x in invoked if x['crate']==name and '--test' not in x['original_argv']]
            built=[x for x in items if x['target']['name']==name and x['target']['kind']==['lib'] and not x['profile']['test']]
            assert len(built)==1 and not built[0]['fresh'] and actual
            assert all(x.get('source_sha')==b['candidate'] for x in actual)
            assert all(any(str(f).endswith(ext) for f in built[0]['filenames']) for ext in ['.rlib','.rmeta'])
        root_rows=[json.loads(x) for x in (e/'root-rustc-invocations.jsonl').read_text().splitlines()]
        actual=[x for x in root_rows if not x.get('probe')]
        assert actual and all(x.get('mode')=='record-only' and x['effective_argv']==x['original_argv']
                              and not x.get('bootstrap_for_this_exec') for x in actual)
        receipt=read(row['receipt']);peak=receipt['resources'].get('fields',{}).get('memory.peak',{}).get('value')
        budget=g.budget_snapshot()
        save(e/'A-completion.json',{'source_sha':b['candidate'],'scope_peak_bytes':peak,'task_budget':budget,
                                   'completed':True,'not_Root_PASS':True})
        assert isinstance(peak,int) and peak<=6400000000,'ordinary API scope exceeds original target'
        assert int(budget['memory.peak'])<=6400000000,'whole-task peak exceeds original target after API'
        stage('ordinary-API','Passed')
    except Exception as error:
        primary={'phase':state['phase'],'type':type(error).__name__,'error':str(error)}
        for name,status in state['stages'].items():
            if status=='Running':state['stages'][name]='Failed'
        save(e/'failure.json',primary);print(json.dumps({'P1_failure':primary}),flush=True)
    finally:
        secondary=[]
        try:
            items=artifacts()
            mechanism={}
            for name in ['application_public_protocol','api_server']:
                built=[x for x in items if x['target']['name']==name and x['target']['kind']==['lib'] and not x['profile']['test']]
                if len(built)!=1:
                    mechanism[name]={'status':'NoCompletedArtifact'};continue
                lib=next(Path(p) for p in built[0]['filenames'] if p.endswith('.rlib'))
                inspected=g.run('symbols-'+name,[sys.executable,str(Path(__file__).resolve()),'symbols',str(lib)],seconds=120)
                proof=json.loads(Path(inspected['stdout_log']).read_text())
                proof['cargo_artifact']=built[0]
                proof['defined_core_methods']={k:any(v in line and re.search(r'\s[TtWw]\s',line) for line in proof['selected_symbols'])
                                                for k,v in METHODS.items()}
                mechanism[name]=proof
            save(e/'mechanism-symbols.json',{'source_sha':b['candidate'],'artifacts':mechanism,
                   'interpretation':'Concrete core definitions expected in owner; API may retain thin generic shims and other shared generic bodies. No RSS shares inferred.'})
            if primary is None:
                assert all(mechanism['application_public_protocol']['defined_core_methods'].values())
                assert not any(mechanism['api_server']['defined_core_methods'].values())
        except Exception as error:
            secondary.append({'phase':'mechanism-symbols','error':str(error)})
        try:
            if (source/'api/Cargo.lock').exists() and target.exists():
                g.retain_seed_cache(source,target,{'source_sha':b['candidate'],'kind':b['kind'],
                         'ordinary_complete':state['stages']['ordinary-API']=='Passed','not_Root_PASS':True})
                assert read(e/'seed-retention.json')['status']=='archived','cache retention failed'
        except Exception as error:
            secondary.append({'phase':'retention','error':str(error)})
        final=g.budget_snapshot();save(e/'budget-final.json',final)
        identity_fields=['inode','path','memory.max','memory.high','memory.swap.max','cpu.max']
        identity_ok=bool(initial) and all(final[k]==initial[k] for k in identity_fields)
        event_ok=bool(initial) and final['memory.events']==initial['memory.events']
        resource_ok=identity_ok and event_ok and int(final['memory.peak'])<=6400000000
        save(e/'full-task-resource-qualification.json',{'source_sha':b['candidate'],
             'initial':initial,'final':final,'identity_unchanged':identity_ok,'events_unchanged':event_ok,
             'same_6400000000_peak_gate':resource_ok,'no_peak_reset':True,
             'scope':'Owned task slice including archive download/restore, ordinary compile, symbol reading and cache retention; external Actions setup/upload steps not covered',
             'full_workload':'Incomplete','not_Root_PASS':True})
        if not resource_ok:
            secondary.append({'phase':'full-task-budget','error':'Original identity/event/peak gate not met'})
        ok=primary is None and not secondary
        state.update(status='P1_PROBE_PASSED_FULL_QUALIFICATION_INCOMPLETE' if ok else 'P1_PROBE_FAILED',
                     primary_error=primary,secondary_errors=secondary,task_elapsed_seconds=time.monotonic()-started)
        save(e/'qualification-phase-state.json',state);save(e/'status.json',state)
        save(e/'runner-costs.json',{'commands':g.RESULTS,'command_wall_sum':sum(x['wall_seconds'] for x in g.RESULTS),
                                  'task_wall_seconds':time.monotonic()-started,
                                  'failure_cleanup_archive_and_probe_costs_included':True})
        save(e/'raw-manifest.json',{str(p.relative_to(e)):sha(p) for p in sorted(e.rglob('*'))
              if p.is_file() and p.name not in ['raw-manifest.json','seed-target-and-source.tar.zst']})
        print(json.dumps({'P1_probe_status':state['status'],'full_workload':'Incomplete'}),flush=True)
    return 0 if ok else 1
if __name__=='__main__':
    if len(sys.argv)>1 and sys.argv[1]=='symbols':
        symbols(Path(sys.argv[2]))
    else:
        sys.exit(main())
