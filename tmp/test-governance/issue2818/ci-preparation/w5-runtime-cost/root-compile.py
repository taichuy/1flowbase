#!/usr/bin/env python3
"""Focused IR entry helpers only; this file has no execution/publication entrypoint."""
import hashlib,json,os,shutil,subprocess
from pathlib import Path
UNIT_FILES=['test-lib-interface_runtime','test-lib-interface_runtime.json','dep-test-lib-interface_runtime']
def sha(path):return hashlib.sha256(Path(path).read_bytes()).hexdigest()
def invalidate_test_root(target):
    """Dirty only the IR lib-test unit; never touch source/production/dependency units."""
    target=Path(target).resolve();fingerprints=target/'debug/.fingerprint';removed=[]
    for folder in sorted(fingerprints.glob('interface-runtime-*')):
        assert folder.parent==fingerprints and not folder.is_symlink()
        for name in UNIT_FILES:
            f=folder/name
            if f.is_file():
                assert not f.is_symlink()
                removed.append({'path':str(f),'bytes':f.stat().st_size,'sha256':sha(f)});f.unlink()
    return {'scope':'IR lib-test fingerprint records only; no Cargo clean/source touch','removed':removed,'source_bytes_mutated':False}
def compile_one(owner,binding,source,target,binary_dir,side,mode):
    assert side in ['old','current'] and mode in ['normal','counts']
    source=Path(source).resolve();target=Path(target).resolve();binary_dir=Path(binary_dir).resolve()
    assert not binary_dir.is_relative_to(target)
    name=side+'-'+mode;root=source/'api/crates/interface-runtime/src/lib.rs';before=sha(root)
    revision=binding['old_sha'] if side=='old' else binding['current_sha']
    assert subprocess.check_output(['git','rev-parse','HEAD'],cwd=source,text=True).strip()==revision
    manifest=json.loads((owner.HOME_ROOT/binding['fixture_manifest']).read_text())
    assert sha(owner.HOME_ROOT/binding['fixture_manifest'])==binding['fixture_hashes'][binding['fixture_manifest']]
    tracked=set(binding['maximum_overlay_write_set'])-set(manifest['new_Rust_files'])
    assert set(subprocess.check_output(['git','diff','--name-only','HEAD','--'],cwd=source,text=True).splitlines())==tracked
    for path in tracked:
        original=subprocess.check_output(['git','show',revision+':'+path],cwd=source)
        assert (source/path).read_bytes().startswith(original),'original source prefix changed'
    assert sha(source/manifest['new_Rust_files'][0])==manifest['new_files']['helper_sha256']
    invalidation=invalidate_test_root(target);owner.save(owner.EVIDENCE/(name+'-root-invalidation.json'),invalidation)
    wrapper=Path(__file__).with_name('root-wrapper.py').resolve();log=owner.EVIDENCE/(name+'-rustc.jsonl');assert not log.exists()
    assert sha(wrapper)==binding['wrapper_sha256']
    env={**os.environ,'CARGO_TARGET_DIR':str(target),'CARGO_BUILD_JOBS':'1','CARGO_INCREMENTAL':'0','CARGO_NET_OFFLINE':'true','CARGO_TERM_COLOR':'never','TERM':'dumb','RUSTC_WRAPPER':str(wrapper),'W5_WRAPPER_MODE':mode,'W5_WRAPPER_SOURCE_ROOT':str(source),'W5_WRAPPER_LOG':str(log)}
    for key in ['RUSTC_WORKSPACE_WRAPPER','RUSTC_BOOTSTRAP','RUSTFLAGS','CARGO_ENCODED_RUSTFLAGS']:env.pop(key,None)
    owner.save(owner.EVIDENCE/(name+'-compile-env.json'),{k:env.get(k) for k in ['CARGO_BUILD_JOBS','CARGO_INCREMENTAL','CARGO_NET_OFFLINE','RUSTC_WRAPPER','RUSTC_WORKSPACE_WRAPPER','RUSTC_BOOTSTRAP','RUSTFLAGS','CARGO_ENCODED_RUSTFLAGS','W5_WRAPPER_MODE']})
    argv=binding['frozen_focused_entry']['command'];assert argv==['cargo','test','-p','interface-runtime','--lib','--jobs','1','--no-run','--message-format=json']
    result=owner.run('compile-'+name,argv,cwd=source/'api',env=env,heavy=True,seconds=120)
    artifacts=[]
    for line in Path(result['merged_log']).open(errors='replace'):
        try:row=json.loads(line)
        except ValueError:continue
        if row.get('reason')=='compiler-artifact':artifacts.append(row)
    roots=[r for r in artifacts if r['target']['name']=='interface_runtime' and r['target']['kind']==['lib'] and r['profile']['test']]
    assert len(roots)==1 and roots[0]['fresh'] is False and roots[0]['executable']
    artifact=roots[0];assert Path(artifact['target']['src_path'])==root
    assert artifact['profile']=={'opt_level':'0','debuginfo':0,'debug_assertions':True,'overflow_checks':True,'test':True}
    assert all(r['fresh'] for r in artifacts if r is not artifact),'unrequested nonroot rebuild'
    events=[json.loads(line) for line in log.read_text().splitlines()]
    matched=[r for r in events if r['matched_test_root']];assert len(matched)==1 and not any(r['blocked_nonroot_compile'] for r in events)
    event=matched[0];assert event['mode']==mode
    assert event['effective_argv']==event['original_argv']+(['--cfg','w5_heap_counts'] if mode=='counts' else [])
    assert not any('w5_heap_counts' in arg for r in events if not r['matched_test_root'] for arg in r['effective_argv'])
    assert sha(root)==before,'source bytes changed during root invalidation/compile'
    binary_dir.mkdir(parents=True,exist_ok=True);output=binary_dir/name;assert not output.exists();shutil.copy2(artifact['executable'],output)
    proof={'binary':str(output),'sha256':sha(output),'bytes':output.stat().st_size,'side':side,'mode':mode,'source_sha':revision,'overlay_original_prefixes_verified':True,'actual_rustc_root':event,'actual_artifact':artifact,'all_other_artifacts_fresh':True,'source_lib_sha256':before,'source_bytes_unchanged':True,'invalidation':invalidation}
    owner.save(owner.EVIDENCE/(name+'-binary-proof.json'),proof);return proof

def discovery_command(binary):return [str(binary),'--list','--ignored']
def exact_command(binary,name,binding):
    assert name in binding['exact_ignored_names']
    return [str(binary),name,'--exact','--ignored','--test-threads=1','--nocapture']
def prove_discovery(text,binding):
    names=sorted(line[:-6] for line in text.splitlines() if line.endswith(': test'))
    assert names==sorted(binding['exact_ignored_names']) and len(names)==3;return names

def prove_actual(text,entry):
    assert 'running 1 test' in text and '1 passed; 0 failed; 0 ignored;' in text
    mode='mode='+entry['mode'];assert mode in text
    samples=[line for line in text.splitlines() if line.startswith('W5_SAMPLE ')]
    assert len(samples)==entry['iters'] if entry['case']!='pending_storage_only' else len(samples)==1
    assert all('normal_nanos=None' in line and 'instrumented_window_nanos=Some(' in line for line in samples) if entry['mode']=='counts' else all('normal_nanos=Some(' in line and 'instrumented_window_nanos=None' in line for line in samples)
    if entry['case']=='pending_storage_only':
        assert 'W5_TOTAL ' not in text and 'normal_nanos=None' in text
    else:
        totals=[line for line in text.splitlines() if line.startswith('W5_TOTAL ')];assert len(totals)==1
        assert ('iterations='+str(entry['iters'])) in totals[0]
        assert ('normal_nanos=None' in totals[0] and 'counts_timing_not_latency=true' in totals[0]) if entry['mode']=='counts' else ('normal_nanos=Some(' in totals[0] and 'counts_timing_not_latency=false' in totals[0])
    # Do not reject negative S1-to-S3 deltas or S4/S5 live!=0: process-global totals are not driver ownership.
    return {'sample_lines':samples,'requested_live_attribution':'unknown if negative absolute live or other-thread interference; retain raw checkpoints','instrumented_time_is_latency':False}


def candidate_discovery_command(binary):return [str(binary),'--list']
def prove_candidate_compiled_behavior(text,binding,behavior):
    names=sorted(line[:-6] for line in text.splitlines() if line.endswith(': test'))
    probes=set(binding['exact_ignored_names']);legacy=set(behavior['legacy_names']);guards=set(behavior['revision_guard_names']);expected=sorted(behavior['expected_names'])
    assert len(legacy)==83 and len(guards)==5 and not legacy&guards and set(expected)==legacy|guards
    assert len(names)==len(set(names))==behavior['expected_compiled_with_cost_probes']==91 and probes.issubset(names)
    actual=[name for name in names if name not in probes]
    assert actual==expected and len(actual)==behavior['expected_count']==88,'candidate names must be frozen83 plus exact5 guards; never relax set'
    return {'compiled_all_names':names,'behavior_names':actual,'legacy83_names':sorted(legacy),'revision_guard_names':sorted(guards),'excluded_probe_names':sorted(probes),'old83_evidence_reused':True}
def candidate_behavior_command(binary,binding):
    assert len(binding['exact_ignored_names'])==3
    return [str(binary),'--exact',*[arg for name in binding['exact_ignored_names'] for arg in ['--skip',name]],'--test-threads=1']
def prove_candidate_actual_behavior(text,behavior):
    import re
    rows=re.findall(r'^test (.+?) \.\.\. (ok|FAILED|ignored)(?:$|[, ])',text,re.M)
    expected=behavior['expected_count'];legacy=set(behavior['legacy_names']);guards=set(behavior['revision_guard_names'])
    assert expected==88 and len(legacy)==83 and len(guards)==5 and not legacy&guards
    assert len(rows)==expected and sorted(name for name,status in rows)==sorted(behavior['expected_names'])
    assert all(status=='ok' for name,status in rows)
    assert re.search(r'test result: ok\. '+str(expected)+r' passed; 0 failed; 0 ignored; 0 measured; 3 filtered out;',text)
    assert 'W5_SAMPLE ' not in text and 'W5_TOTAL ' not in text,'cost probe executed in behavior gate'
    actual=set(name for name,status in rows);assert actual&legacy==legacy and actual&guards==guards
    return {'passed':expected,'legacy_passed':83,'revision_guards_passed':5,'failed':0,'ignored':0,'filtered_probe_count':3,'actual_names':sorted(actual),'legacy83_actual_names':sorted(actual&legacy),'revision_guard_actual_names':sorted(actual&guards),'reused_same_candidate_normal_binary':True,'extra_Rust_compiles':0,'old83_rerun':False}
