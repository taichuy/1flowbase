#!/usr/bin/env python3
"""Issue 2818 one-off controlled W. Not a product runner or natural PR latency."""
import argparse
from collections import Counter
import hashlib
import json
import os
from pathlib import Path
import re
import selectors
import subprocess
import sys
import time

BASELINE = 'e3cdfefb7a5b85d1d7d9c2ef166e3b923c05de89'
HOME_ROOT = Path(__file__).resolve().parents[4]
SELF = Path(__file__).resolve()
EVIDENCE = HOME_ROOT / 'tmp/test-governance/issue2818/ci-evidence'
TEMP = Path(os.environ.get('RUNNER_TEMP', '/tmp')) / 'issue2818'
RUNNER = HOME_ROOT / 'scripts/shell/resource-job-run.py'
HEAVY = HOME_ROOT / 'scripts/shell/resource-heavy-run.py'
STOP = 7_200_000_000
END = time.monotonic() + 80 * 60
SERIAL = 0
RESULTS = []

BUILDERS = r"""
const root=process.argv[1], path=require('node:path');
const runtime=require(path.join(root,'scripts/node/testing/verify-runtime.js')).loadVerifyRuntimeConfig({repoRoot:root,env:process.env});
const backend=require(path.join(root,'scripts/node/verify/backend.js'));
const foundation=require(path.join(root,'scripts/node/foundation-contracts/core.js'));
const options={repoRoot:root,env:process.env,cargoJobs:runtime.backend.cargoJobs,cargoTestThreads:runtime.backend.cargoTestThreads,incremental:runtime.backend.incremental};
const seed=backend.buildBackendCommands({...options,target:'official-i18n-seed'});
const fast=['mcp-gateway','application-backend'].flatMap(id=>foundation.buildFoundationPlan({foundation:id}).packs[id].fast);
const checks=backend.buildBackendCommands({...options,target:'check',shard:'apps'});
process.stdout.write(JSON.stringify({runtime,tests:[...seed,...fast],checks}));
"""

def save(path, data):
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(data, indent=2, sort_keys=True) + '\n')

def read(path):
    return json.loads(path.read_text())

def capture(argv, cwd=HOME_ROOT):
    return subprocess.check_output(argv, cwd=cwd, text=True, timeout=30).strip()

def stream_exec(argv, merged):
    """Preserve separate raw stdout/stderr and a chronological merged test log."""
    child = subprocess.Popen(argv, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
    selector = selectors.DefaultSelector()
    selector.register(child.stdout, selectors.EVENT_READ, sys.stdout.buffer)
    selector.register(child.stderr, selectors.EVENT_READ, sys.stderr.buffer)
    with Path(merged).open('wb') as combined:
        while selector.get_map():
            for key, _ in selector.select(timeout=1):
                chunk = os.read(key.fileobj.fileno(), 65536)
                if not chunk:
                    selector.unregister(key.fileobj)
                    key.fileobj.close()
                else:
                    combined.write(chunk)
                    combined.flush()
                    key.data.write(chunk)
                    key.data.flush()
    return child.wait()

def run(label, argv, cwd=HOME_ROOT, env=None, heavy=False, seconds=1200, expected=0):
    global SERIAL
    SERIAL += 1
    folder = EVIDENCE / f'{SERIAL:03d}-{label}'
    merged = EVIDENCE / f'{SERIAL:03d}-{label}.combined.log'
    limit = min(seconds, END-time.monotonic())
    if limit <= 0:
        raise RuntimeError('global controlled-experiment deadline exhausted')
    actual = ([sys.executable, str(HEAVY)] if heavy else []) + [
        sys.executable, str(SELF), 'exec', str(merged), *argv]
    wrapper = [sys.executable, str(RUNNER), '--memory-stop-bytes', str(STOP),
               '--deadline-seconds', str(limit), '--output-dir', str(folder), '--', *actual]
    started = time.monotonic()
    code = subprocess.call(wrapper, cwd=cwd, env=env)
    receipt = read(folder/'finished.json')
    row = {'label':label,'argv':argv,'cwd':str(cwd),'exit_code':code,
           'wall_seconds':time.monotonic()-started,'receipt':str(folder/'finished.json'),
           'merged_log':str(merged),'stdout_log':str(folder/'stdout.log'),
           'stderr_log':str(folder/'stderr.log'),'classification':receipt['classification']}
    RESULTS.append(row)
    save(EVIDENCE/'commands.json', RESULTS)
    if not receipt['cleanup']['complete']:
        raise RuntimeError(f'{label}: incomplete owned-process cleanup')
    if receipt['classification'] != 'exit':
        raise RuntimeError(f'{label}: {receipt["classification"]}; no unchanged retry')
    if expected == 'nonzero':
        if code == 0:
            raise RuntimeError(f'{label}: injected invalid source was accepted')
    elif code != expected:
        raise RuntimeError(f'{label}: command exit {code}; see raw logs')
    return row

def budget_snapshot():
    group = Path('/sys/fs/cgroup') / os.environ['ONEFLOWBASE_TASK_BUDGET_CGROUP'].lstrip('/')
    names = ['memory.max','memory.high','memory.swap.max','memory.current',
             'memory.peak','memory.events','cpu.max','cpu.stat','cgroup.events']
    data = {name:(group/name).read_text().strip() for name in names}
    data.update(inode=group.stat().st_ino, path=str(group))
    return data

def plan(root):
    data = json.loads(capture(['node','-e',BUILDERS,str(root)],root))
    assert len(data['tests']) == 4, 'frozen W requires exactly the original four filter commands'
    expected = ['ac_002_decodes_digest_verified_build_time_official_seed',
                '_tests::dynamic_backend_consumer_inventory::',
                'mcp_protocol_routes','model_definition_routes_']
    assert all(q in command['args'] for q,command in zip(expected,data['tests']))
    assert all(command['command']=='cargo' and command['args'][0]=='test'
               for command in data['tests'])
    return data

def frozen(root):
    paths = capture(['git','ls-files','-z'],root).split('\0')
    # Source, fixtures and all configuration stay exact; only intended JS selectors may differ.
    allowed = {'scripts/node/verify/backend.js','scripts/node/verify/backend-targets.js',
               'scripts/node/foundation-contracts/core.js',
               'scripts/node/foundation-contracts/_tests/core.test.js',
               'scripts/node/verify-backend/_tests/cli.test.js',
               'scripts/node/verify/_tests/history-maintenance.test.js',
               'scripts/shell/resource-job-run.py','scripts/shell/_tests/resource-job-run.test.py'}
    selected = [p for p in paths if p not in allowed]
    hashes = {p:hashlib.sha256((root/p).read_bytes()).hexdigest()
              for p in selected if (root/p).is_file()}
    return hashes

def prepare(name, ref):
    source = TEMP/name
    run(f'checkout-{name}', ['git','worktree','add','--detach',str(source),ref], seconds=120)
    data = plan(source)
    save(EVIDENCE/f'{name}-plan.json',data)
    metadata = run(f'{name}-metadata',['cargo','metadata','--no-deps','--format-version','1',
                                     '--manifest-path','api/Cargo.toml'],cwd=source,seconds=120)
    parsed = json.loads(Path(metadata['stdout_log']).read_text())
    save(EVIDENCE/f'{name}-metadata.json',parsed)
    save(EVIDENCE/f'{name}-source-hashes.json', frozen(source))
    save(EVIDENCE/f'{name}-identity.json',{'sha':capture(['git','rev-parse','HEAD'],source),
         'status':capture(['git','status','--porcelain'],source),'plan':data})
    return source,data,parsed

def command_env(data, command, target):
    env = dict(os.environ, CARGO_TARGET_DIR=str(target),
               CARGO_BUILD_JOBS=str(data['runtime']['backend']['cargoJobs']),
               RUST_TEST_THREADS=str(data['runtime']['backend']['cargoTestThreads']),
               CARGO_INCREMENTAL='0',CARGO_NET_OFFLINE='true',
               CARGO_TERM_COLOR='never',TERM='dumb')
    for key,value in command.get('env',{}).items():
        if value is None:
            env.pop(key,None)
        else:
            env[key]=str(value)
    return env

def root_names(root,metadata):
    api=next(p for p in metadata['packages'] if p['name']=='api-server')
    aliases={}
    package=Path(api['manifest_path']).parent
    for target in api['targets']:
        source=Path(target['src_path'])
        identity=f'api-server/{",".join(target["kind"])}/{target["name"]}/{source.relative_to(root)}'
        # Cargo versions print package-relative or workspace-relative source headers.
        for base in [root/'api',package]:
            aliases[str(source.relative_to(base))]=identity
    return aliases

def cases(log, roots, mode):
    active=None
    rows=[]
    for line in Path(log).read_text(errors='replace').splitlines():
        header=re.search(r'\bRunning (?:unittests )?(.+?) \(',line)
        if header:
            active=roots.get(header.group(1))
            if active is None:
                raise RuntimeError(f'unknown Cargo root header: {line}')
        doc=re.search(r'\bDoc-tests (\S+)',line)
        if doc:
            active='api-server/doctest/'+doc.group(1)
        if mode=='list':
            match=re.match(r'(.+): (test|benchmark)$',line)
            if match:
                assert active, f'case has no target identity: {line}'
                rows.append([active,match.group(1),match.group(2)])
        else:
            match=re.match(r'test (.+?) \.\.\. (ok|FAILED|ignored)(?:[, ].*)?$',line)
            if match:
                assert active, f'execution has no target identity: {line}'
                rows.append([active,match.group(1),match.group(2)])
    return rows

def work(name,root,data):
    target=TEMP/f'target-{name}'
    assert not target.exists(), 'A/B must start with independent empty target dirs'
    rows=[]
    for index,command in enumerate(data['tests']+data['checks']):
        rows.append(run(f'{name}-W-{index}',[command['command'],*command['args']],
                        cwd=root/command['cwd'],env=command_env(data,command,target),heavy=True))
    save(EVIDENCE/f'{name}-W.json',rows)
    return target,rows

def discover(name,root,data,metadata,target,executions):
    roots=root_names(root,metadata)
    out=[]
    for index,command in enumerate(data['tests']):
        env=command_env(data,command,target)
        argv=[command['command'],*command['args']]
        if '--' not in argv:
            argv+=['--']
        listing=run(f'{name}-list-{index}',[*argv,'--list'],cwd=root/command['cwd'],env=env,heavy=True)
        ignored=run(f'{name}-ignored-{index}',[*argv,'--list','--ignored'],
                    cwd=root/command['cwd'],env=env,heavy=True)
        listed=cases(listing['merged_log'],roots,'list')
        ignored_cases=Counter(tuple(x) for x in cases(ignored['merged_log'],roots,'list'))
        assert not (ignored_cases-Counter(tuple(x) for x in listed)), 'ignored listing outside original list'
        detailed=[]
        for case in listed:
            key=tuple(case)
            flag=ignored_cases[key]>0
            detailed.append([*case,flag])
            if flag:
                ignored_cases[key]-=1
        actual=cases(executions[index]['merged_log'],roots,'run')
        expected=Counter((x[0],x[1],'ignored' if x[3] else 'ok') for x in detailed)
        assert expected==Counter(tuple(x) for x in actual), f'{name} filter {index}: execution/list mismatch'
        assert any(x[2]=='ok' for x in actual), f'{name} filter {index}: no executed passing tests'
        out.append({'filter_index':index,'list':detailed,'actual':actual,
                    'original_argv':[command['command'],*command['args']]})
    save(EVIDENCE/f'{name}-target-qualified-cases.json',out)
    return out

def wait_candidate(args):
    assert re.fullmatch(r'[A-Za-z0-9_./-]+',args.candidate_ref), 'invalid ref'
    stop=min(END,time.monotonic()+args.wait_seconds)
    attempt=0
    while time.monotonic()<stop:
        attempt+=1
        run(f'candidate-fetch-{attempt}',['git','fetch','--no-tags','origin',
             '+'+args.candidate_ref+':refs/issue2818/candidate'],seconds=120)
        sha=capture(['git','rev-parse','refs/issue2818/candidate'])
        # Read the exact fetched revision without accepting an incomplete branch state.
        backend=capture(['git','show',sha+':scripts/node/verify/backend.js'])
        foundation=capture(['git','show',sha+':scripts/node/foundation-contracts/core.js'])
        targets=capture(['git','show',sha+':scripts/node/verify/backend-targets.js'])
        ready=('--all-targets' in backend and 'api-server-all-targets' in backend
               and 'api_private_contracts' in foundation and 'api_application' in foundation
               and 'api_private_contracts' in targets)
        if args.candidate_sha and sha!=args.candidate_sha:
            ready=False
        if ready:
            return sha
        if stop-time.monotonic()<30:
            break
        time.sleep(30)  # Bounded remote availability, no busy polling or heavy retry.
    raise RuntimeError('candidate source not ready within bounded wait; baseline remains evidence')

def compile_negative(sources):
    marker='ISSUE2818_UNSELECTED_ROOT_TYPE_NEGATIVE'
    leaf=Path('api/apps/api-server/tests/http/api_identity.rs')
    for name,root,data,target in sources:
        original=(root/leaf).read_bytes()
        # A fixed invalid type witness in an existing omitted root; no product/fixture changes.
        (root/leaf).write_bytes(original+f'\nconst _: () = {{ compile_error!("{marker}"); }};\n'.encode())
        try:
            command=data['tests'][0] if name=='baseline' else next(
                c for c in data['checks'] if '--all-targets' in c['args'])
            evidence=run(f'{name}-compile-negative',[command['command'],*command['args']],
                         cwd=root/command['cwd'],env=command_env(data,command,target),
                         heavy=True,expected='nonzero')
            assert marker in Path(evidence['merged_log']).read_text(), 'unrelated failure is not defect detection'
            save(EVIDENCE/f'{name}-compile-negative.json',{'leaf':str(leaf),'marker':marker,
                 'injected_sha256':hashlib.sha256((root/leaf).read_bytes()).hexdigest(),
                 'owner_argv':evidence['argv'],'detected':True})
        finally:
            (root/leaf).write_bytes(original)

def main():
    parser=argparse.ArgumentParser()
    parser.add_argument('--candidate-ref',default='refs/heads/opt/2818-global-20261109')
    parser.add_argument('--candidate-sha',default='')
    parser.add_argument('--wait-seconds',type=int,default=1200)
    args=parser.parse_args()
    EVIDENCE.mkdir(parents=True,exist_ok=False)
    TEMP.mkdir(parents=True,exist_ok=False)
    save(EVIDENCE/'environment.json',{'baseline':BASELINE,'host_cpu_count':os.cpu_count(),
         'rustc':capture(['rustc','-vV']),'cargo':capture(['cargo','-vV']),
         'node':capture(['node','--version']),'uname':capture(['uname','-a']),
         'profile_overrides':{k:v for k,v in os.environ.items() if k.startswith(('CARGO_','RUST_'))},
         'measurement_limit':'Controlled serial W, not natural PR job P50/P95. wait4 maxrss is not tree RSS.'})
    initial=budget_snapshot()
    save(EVIDENCE/'budget-before.json',initial)
    assert int(initial['inode'])==int(os.environ['ONEFLOWBASE_TASK_BUDGET_ID'])
    assert int(initial['memory.max'])==7_600_000_000  # PG service separately capped at 400 MB; sum <=8 GB
    assert int(initial['memory.high'])<=STOP and initial['memory.swap.max']=='0'
    quota,period=map(int,initial['cpu.max'].split())
    assert quota/period+.5<=os.cpu_count()*.60+.001  # Include PG's half-CPU cap
    smoke=run('smoke', [sys.executable,'-c',
         'import os,subprocess,sys; p=subprocess.Popen([sys.executable,"-c","import os,time; os.setsid(); time.sleep(60)"],stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL); print(p.pid,flush=True)'],
         seconds=20)
    child=int(Path(smoke['merged_log']).read_text().strip())
    assert not Path(f'/proc/{child}').exists(), 'smoke descendant survived owned scope cleanup'
    root,b,meta=prepare('baseline',BASELINE)
    # Fetch registry/git inputs once, without a target cache; both W sides are offline.
    run('registry-prefetch',['cargo','fetch','--locked','--manifest-path','api/Cargo.toml'],
        cwd=root,heavy=True,seconds=900)
    bt,bw=work('baseline',root,b)
    bc=discover('baseline',root,b,meta,bt,bw)
    candidate_sha=wait_candidate(args)
    aroot,a,ameta=prepare('candidate',candidate_sha)
    assert frozen(root)==frozen(aroot), 'source/config/fixture changed outside frozen W1 selector scope'
    assert b['runtime']==a['runtime'], 'runtime concurrency/config changed'
    assert len(b['checks'])==1 and len(a['checks'])==2
    assert b['checks'][0]==a['checks'][0], 'normal apps three-package check changed'
    extra=a['checks'][1]
    assert extra['args']==['check','--package','api-server','--all-targets','--jobs',
                            str(a['runtime']['backend']['cargoJobs'])]
    # Check API root mounting and labels from the concrete SHA-owned builders, never rewrite argv.
    owners=['api_private_contracts']*3+['api_application']
    for command,owner in zip(a['tests'],owners):
        pos=command['args'].index('--test')
        assert command['args'][pos+1]==owner
    at,aw=work('candidate',aroot,a)
    ac=discover('candidate',aroot,a,ameta,at,aw)
    for old,new in zip(bc,ac):
        for key in ['list','actual']:
            assert Counter(tuple(x) for x in old[key])==Counter(tuple(x) for x in new[key]), key+' multiplicity mismatch'
    before=sum(x['wall_seconds'] for x in bw)
    after=sum(x['wall_seconds'] for x in aw)
    save(EVIDENCE/'paired-feasibility.json',{'baseline_sha':BASELINE,'candidate_sha':candidate_sha,
         'pairs':1,'W_definition':'official seed 2 original filters, MCP mcp_protocol_routes, application model_definition_routes_, apps normal three-package check; candidate adds API-only all-targets check',
         'baseline_seconds':before,'candidate_seconds':after,'saved_seconds':before-after,
         'saving_fraction':(before-after)/before,'extra_experiment_seconds':sum(x['wall_seconds'] for x in RESULTS)-before-after,
         'sampling':'first pair feasibility only; no P95 claim; Root decides three pairs only after benefit',
         'all_known_D':'Root-frozen complete D replay is separate and required before acceptance',
         'link_limit':'API all-targets check preserves type checking, not universal omitted-root final native link checks'})
    compile_negative([('baseline',root,b,bt),('candidate',aroot,a,at)])
    summary=read(EVIDENCE/'paired-feasibility.json')
    summary['extra_experiment_seconds']=sum(x['wall_seconds'] for x in RESULTS)-before-after
    save(EVIDENCE/'paired-feasibility.json',summary)
    save(EVIDENCE/'status.json',{'status':'FIRST_PAIR_EVIDENCE_COLLECTED',
         'not_final_QA':True,'requires':'Root complete D matrix and threshold decision; independent centralized QA'})
    return 0

if __name__=='__main__':
    if len(sys.argv)>1 and sys.argv[1]=='exec':
        sys.exit(stream_exec(sys.argv[3:],sys.argv[2]))
    try:
        sys.exit(main())
    except Exception as error:
        if EVIDENCE.exists():
            save(EVIDENCE/'failure.json',{'kind':type(error).__name__,'message':str(error),
                 'classification':'invalid baseline / source proof / environment / resource; inspect first failing raw receipt; never optimization PASS'})
        raise
    finally:
        if EVIDENCE.exists():
            try:
                save(EVIDENCE/'budget-after.json',budget_snapshot())
            except (OSError,KeyError):
                pass
