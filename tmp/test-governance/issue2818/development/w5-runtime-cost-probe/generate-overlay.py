#!/usr/bin/env python3
"""Write/check an evidence-only overlay; never edit the source worktree or compile."""
import argparse, difflib, hashlib, json, re, subprocess, tempfile
from pathlib import Path
HERE = Path(__file__).resolve().parent
BASE = "api/crates/interface-runtime/src/_tests/"
OLD = "ca19501827fdeac86b9f5d9fcba767715220f3dd"
CURRENT = "903971d5c3ddd37f93c1f7314e02dae4a2bb120c"
FRAGMENTS = {BASE+"invocation_tests.rs":"empty.append.rs", BASE+"invocation/document_operation_tests.rs":"document.append.rs", BASE+"finalization_tests.rs":"pending.append.rs"}
MOD = BASE+"mod.rs"; HELPER = BASE+"w5_runtime_cost_probe.rs"
def sha(b): return hashlib.sha256(b).hexdigest()
def body(data, line):
    text=data.decode(); masked=re.sub(r'//[^\n]*|/\*.*?\*/|"(?:\\.|[^"\\])*"',lambda m:' '*len(m[0]),text,flags=re.S)
    matches=[m for m in re.finditer(r'\bfn\s+\w+',masked) if text[:m.start()].count('\n')+1==line]
    assert len(matches)==1, line
    start=matches[0].start(); end=masked.index('{',matches[0].end())+1; depth=1
    while depth: depth+=(masked[end]=='{')-(masked[end]=='}'); end+=1
    return text[start:end].encode()
def main():
    parser=argparse.ArgumentParser(); parser.add_argument('--repo-root',required=True,type=Path); args=parser.parse_args(); root=args.repo_root.resolve()
    model=json.loads((HERE.parent.parent/'model/candidates/raw/w5-runtime-cost-probe/source-map.json').read_text())
    git=lambda c,p:subprocess.check_output(['git','show',c+':'+p],cwd=root)
    bindings=[]
    for row in model['source_map']:
        before=body(git(OLD,row['path']),row['old_lines'][0]); after=body(git(CURRENT,row['path']),row['current_lines'][0])
        assert before==after and sha(before)==row['body_signature_sha256'],row
        bindings.append({'path':row['path'],'function':row['function'],'sha256':sha(before),'old_current_equal':True})
    inputs={p:git(CURRENT,p) for p in [MOD,*FRAGMENTS]}
    outputs={MOD:inputs[MOD]+b'\nmod w5_runtime_cost_probe;\n', HELPER:(HERE/'helper.rs').read_bytes()}
    for path,fragment in FRAGMENTS.items(): outputs[path]=inputs[path]+(HERE/fragment).read_bytes()
    patch=''
    for path,data in outputs.items():
        new=path not in inputs
        patch+='diff --git a/'+path+' b/'+path+'\n'+('new file mode 100644\n' if new else '')
        patch+=''.join(difflib.unified_diff(inputs.get(path,b'').decode().splitlines(True),data.decode().splitlines(True),fromfile='/dev/null' if new else 'a/'+path,tofile='b/'+path,n=3))
        target=HERE/'overlay-files'/path; target.parent.mkdir(parents=True,exist_ok=True); target.write_bytes(data)
    target=HERE/'measurement-overlay.patch'; target.write_text(patch)
    apply=[]
    for commit in [OLD,CURRENT]:
        with tempfile.TemporaryDirectory(prefix='apply-check-',dir=HERE) as directory:
            directory=Path(directory)
            for path in inputs:
                dest=directory/path; dest.parent.mkdir(parents=True,exist_ok=True); dest.write_bytes(git(commit,path))
            command=['git','apply','--unsafe-paths','--directory',str(directory),str(target)]
            result=subprocess.run(command,cwd=root,capture_output=True,text=True); assert result.returncode==0,result.stderr
            for path in inputs: assert (directory/path).read_bytes().startswith(git(commit,path)),path
            assert (directory/HELPER).read_bytes()==outputs[HELPER]
            apply.append({'input_sha':commit,'same_patch_applied':True,'original_files_full_prefix_unchanged':True})
    fixture=[]
    for path,fragment in FRAGMENTS.items():
        code=(HERE/fragment).read_text(); s0=code.index('let s0 ='); window=code.index('let window = probe::begin();')
        fixture.append({'path':path,'fragment':fragment,'fragment_sha256':sha(code.encode()),'setup_segment_sha256':sha(code[s0:window].encode()),'entrypoint':re.search(r'async fn (\w+)',code)[1],'timing_start':'probe::begin before actual synchronous kernel.invoke','timing_end':'window.finish after scope drops actual invocation future and outcome/failure','prefix_sha256_old':sha(git(OLD,path)),'prefix_sha256_current':sha(inputs[path])})
    tests=['_tests::invocation_tests::w5_probe_empty_plan','_tests::invocation_tests::event_observation_tests::document_operation_tests::w5_probe_document_before','_tests::review_remediation_tests::finalization_tests::w5_probe_pending_storage']
    manifest={'status':'temporary overlay prepared and apply-checked only; no Rust compile/discovery/execution','old_sha':OLD,'current_sha':CURRENT,'current_IR_tree_equals':'489943b16401b2e4bc472b23699e9bfd2a3e811a','patch':str(target),'patch_sha256':sha(target.read_bytes()),'maximum_write_set':list(outputs),'new_Rust_files':[HELPER],'new_files':{'helper_sha256':sha(outputs[HELPER])},'same_patch_checks':apply,'unchanged_original_function_bindings':bindings,'probe_fixture_sources':fixture,'exact_ignored_test_names':tests,'runtime':'one exact current_thread Tokio ignored test; --exact --ignored --test-threads=1 --nocapture','counter_cfg':'w5_heap_counts applied only by root IR --test RUSTC_WRAPPER; no global RUSTFLAGS/workspace wrapper','allocator':'instrumented-only process-global System; continuous successful Rust requested-live from process start; alloc includes zeroed; realloc success adds full new request/removes old, failure leaves live unchanged; no TLS/pointer map/window zero reset','limits':['snapshots are sequential atomic loads, not an atomic cross-counter transaction; other process threads are not attributable to driver','negative S1-to-S3 live delta and dealloc bytes > interval allocated bytes are valid','negative absolute live or concurrent interference makes live/peak attribution unknown; retain event counters/raw checkpoints','System realloc internals, allocator metadata/usable sizes, foreign malloc and RSS unmeasured','real API enum/MainDurableStore/HTTP/large-inline-I-P/physical-stack/build-resource risks remain unmeasured'],'primary_buffers':'per-invocation fresh original fixtures; actual Vec::new len0/cap0 and fingerprint None; explicit inspection handles are retained equally outside both windows','new_probe_lines':sum(len((HERE/f).read_text().splitlines()) for f in ['helper.rs',*FRAGMENTS.values()]),'no_original_case_or_production_changes':True}
    (HERE/'manifest.json').write_text(json.dumps(manifest,indent=2)+'\n'); print(str(HERE/'manifest.json'))
if __name__=='__main__': main()
