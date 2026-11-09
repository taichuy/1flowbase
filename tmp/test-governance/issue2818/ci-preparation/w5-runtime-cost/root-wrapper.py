#!/usr/bin/env python3
"""Temporary root-only IR test cfg; no compiler flags outside that test unit."""
import json,os,sys
from pathlib import Path
def prepare(args, mode, root, cwd):
    assert mode in ['normal','counts']
    def value(flag):return args[args.index(flag)+1] if flag in args else None
    name=value('--crate-name');sources=[str((Path(cwd)/x).resolve()) for x in args if not x.startswith('-') and x.endswith('.rs')]
    matched=name=='interface_runtime' and '--test' in args
    compile_unit=bool(name and sources)
    if matched:assert sources==[str((Path(root)/'api/crates/interface-runtime/src/lib.rs').resolve())]
    assert not any('w5_heap_counts' in x for x in args),'preexisting probe cfg is forbidden'
    effective=[*args,*(['--cfg','w5_heap_counts'] if matched and mode=='counts' else [])]
    return {'matched_test_root':matched,'compile_unit':compile_unit,'blocked_nonroot_compile':compile_unit and not matched,'crate_name':name,'sources':sources,'original_argv':args,'effective_argv':effective,'mode':mode}
def main():
    real,args=sys.argv[1],sys.argv[2:]
    row=prepare(args,os.environ['W5_WRAPPER_MODE'],os.environ['W5_WRAPPER_SOURCE_ROOT'],Path.cwd())
    row['real_rustc']=real
    with open(os.environ['W5_WRAPPER_LOG'],'a') as f:f.write(json.dumps(row)+'\n')
    if row['blocked_nonroot_compile']:raise SystemExit(86)
    os.execvpe(real,[real,*row['effective_argv']],os.environ)
if __name__=='__main__':main()
