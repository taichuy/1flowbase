#!/usr/bin/env python3
"""Issue 2818 one-off controlled W. Not a product runner or natural PR latency."""
import argparse
from collections import Counter
import hashlib
import gzip
import json
import os
from pathlib import Path
import re
import subprocess
import sys
import time

SEED_SHA = 'a9e85b53bb385349896a16eeaa32238093248626'
PRIOR_RUN = '42562236751'
PRIOR_NAME = 'issue2818-seed-owner-42562236751-1'
ARCHIVE_SHA = '196b2154d3b95320465cc6b7c33356e9d9863ecc10e5d4174313c8966c1e76a9'
ARCHIVE_BYTES = 648028887
BASELINE = 'e3cdfefb7a5b85d1d7d9c2ef166e3b923c05de89'
HOME_ROOT = Path(__file__).resolve().parents[4]
SELF = Path(__file__).resolve()
EVIDENCE = HOME_ROOT / 'tmp/test-governance/issue2818/ci-evidence'
TEMP = Path(os.environ.get('RUNNER_TEMP', '/tmp')) / 'issue2818'
RUNNER = HOME_ROOT / 'scripts/shell/resource-job-run.py'
HEAVY = HOME_ROOT / 'scripts/shell/resource-heavy-run.py'
END = time.monotonic() + 70 * 60
SERIAL = 0
RESULTS = []
PG_GROUP = None

BUILDERS = r"""
const root=process.argv[1], path=require('node:path');
const runtime=process.env.ISSUE2818_FROZEN_RUNTIME ? JSON.parse(process.env.ISSUE2818_FROZEN_RUNTIME) : require(path.join(root,'scripts/node/testing/verify-runtime.js')).loadVerifyRuntimeConfig({repoRoot:root,env:process.env});
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
    # One kernel pipe preserves Cargo target headers before their child output.
    child = subprocess.Popen(argv, stdout=subprocess.PIPE, stderr=subprocess.STDOUT)
    with Path(merged).open('wb') as combined:
        while True:
            chunk = os.read(child.stdout.fileno(), 65536)
            if not chunk:
                break
            combined.write(chunk)
            combined.flush()
            sys.stdout.buffer.write(chunk)
            sys.stdout.buffer.flush()
    return child.wait()

def native_sample():
    group = Path('/sys/fs/cgroup') / os.environ['ONEFLOWBASE_TASK_BUDGET_CGROUP'].lstrip('/')
    stats = {k:int(v) for k,v in (line.split() for line in (group/'memory.stat').read_text().splitlines())}
    processes = []
    pids = set()
    for file in group.rglob('cgroup.procs'):
        try:
            pids.update(int(x) for x in file.read_text().split())
        except FileNotFoundError:
            pass
    for pid in pids:
        try:
            proc = Path('/proc') / str(pid)
            status = dict(line.split(':',1) for line in (proc/'status').read_text().splitlines() if ':' in line)
            row = {'pid':pid,'comm':status['Name'].strip(),
                   'rss':int(status.get('VmRSS','0 kB').split()[0])*1024,
                   'vm_hwm':int(status.get('VmHWM','0 kB').split()[0])*1024}
            if row['comm'] in ('rustc','cargo'):
                argv = (proc/'cmdline').read_bytes().split(b'\0')
                def flag(key):
                    key = key.encode()
                    return argv[argv.index(key)+1].decode(errors='replace') if key in argv and argv.index(key)+1<len(argv) else None
                row['crate_name'] = flag('--crate-name')
                row['crate_type'] = flag('--crate-type')
                row['is_test'] = b'--test' in argv
            processes.append(row)
        except (FileNotFoundError,ProcessLookupError):
            pass
    current = int((group/'memory.current').read_text())
    pg = int((PG_GROUP/'memory.current').read_text()) if PG_GROUP else 0
    return {'monotonic':time.monotonic(),'memory_current':current,'pg_memory_current':pg,
            'aggregate_current':current+pg,'memory_stat':stats,'processes':processes,
            'tree_rss_sample':sum(r['rss'] for r in processes),
            'max_process_vm_hwm':max((r['vm_hwm'] for r in processes),default=0),
            'memory_pressure':(group/'memory.pressure').read_text().strip(),
            'cpu_stat':{k:int(v) for k,v in (line.split() for line in (group/'cpu.stat').read_text().splitlines())}}

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
    wrapper = [sys.executable, str(RUNNER), '--deadline-seconds', str(limit), '--output-dir', str(folder), '--', *actual]
    started = time.monotonic()
    next_report = started
    print(json.dumps({'stage':label,'event':'started','deadline_seconds':round(limit,2)}),flush=True)
    child = subprocess.Popen(wrapper, cwd=cwd, env=env)
    peaks = {'tree_rss_sample':0,'max_process_vm_hwm':0,'aggregate_current':0}
    count = 0
    with (EVIDENCE/f'{SERIAL:03d}-{label}.native-samples.jsonl').open('w') as samples:
        while child.poll() is None:
            sample = native_sample()
            samples.write(json.dumps(sample)+'\n')
            count += 1
            for key in peaks:
                peaks[key] = max(peaks[key], sample[key])
            if time.monotonic() >= next_report:
                stat=sample['memory_stat']
                print(json.dumps({'stage':label,'elapsed_seconds':round(time.monotonic()-started,1),
                    'memory_current':sample['memory_current'],'tree_rss_sample':sample['tree_rss_sample'],
                    'anon':stat.get('anon'),'file':stat.get('file'),'pgscan':stat.get('pgscan'),
                    'refault_file':stat.get('workingset_refault_file'),
                    'pressure':sample['memory_pressure'],
                    'rustc':[{'crate':r.get('crate_name'),'type':r.get('crate_type'),
                              'test':r.get('is_test'),'rss':r['rss'],'hwm':r['vm_hwm']}
                             for r in sample['processes'] if r['comm']=='rustc'][:4]}),flush=True)
                next_report=time.monotonic()+30
            time.sleep(.1)
    code = child.wait()
    print(json.dumps({'stage':label,'event':'finished','elapsed_seconds':round(time.monotonic()-started,2),
                      'exit_code':code}),flush=True)
    save(EVIDENCE/f'{SERIAL:03d}-{label}.sample-peaks.json',
         {'quality':'100ms samples; not exact simultaneous kernel peak','samples':count,**peaks})
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

def prepare(name, ref, archive=None):
    source = TEMP/name
    run(f'checkout-{name}', ['git','worktree','add','--detach',str(source),ref], seconds=120)
    if archive is not None:
        run('restore-retained-source-target',['tar','-I','zstd -T1','-xf',str(archive),
            '--no-same-owner','-C',str(TEMP)],seconds=180)
        assert (source/'.git').is_file(), 'fresh worktree .git must survive the mtime-preserving overlay'
        assert (TEMP/'target-seed').is_dir() and any((TEMP/'target-seed').rglob('*.rlib')), 'no compiled target restored; never fall back to empty target'
        capture(['git','diff','--exit-code','HEAD','--'],source)
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

def summarize_prior(path, output):
    count=0
    first=None
    last=None
    peaks={'memory_current':0,'tree_rss_sample':0,'max_process_vm_hwm':0,'anon':0,'file':0}
    targets=Counter()
    with Path(path).open('rb') as source, gzip.open(str(output)+'.samples.jsonl.gz','wb',compresslevel=1) as packed:
        for raw in source:
            packed.write(raw)
            row=json.loads(raw)
            count+=1
            first=first or row
            last=row
            for key in ['memory_current','tree_rss_sample','max_process_vm_hwm']:
                peaks[key]=max(peaks[key],row.get(key,0))
            for key in ['anon','file']:
                peaks[key]=max(peaks[key],row.get('memory_stat',{}).get(key,0))
            targets.update(p.get('crate_name','unknown') for p in row.get('processes',[]) if p.get('comm')=='rustc')
    stat_keys=['anon','file','pgscan','pgsteal','workingset_refault_file','workingset_refault_anon']
    small=lambda row: {key:row.get('memory_stat',{}).get(key) for key in stat_keys}
    save(Path(output),{'source_run':PRIOR_RUN,'sample_count':count,'sampled_peaks':peaks,
         'first':small(first or {}),'last':small(last or {}),'rustc_sample_presence':targets.most_common(8),
         'pressure_available_in_prior':bool(last and 'memory_pressure' in last),
         'not_runtime_PASS':True})
    print(json.dumps({'source_run':PRIOR_RUN,'sample_count':count,'sampled_peaks':peaks}),flush=True)

def cargo_fresh_summary():
    rows=[]
    for log in EVIDENCE.glob('*seed-normal-lib-prebuild.combined.log'):
        for line in log.open(errors='replace'):
            if not line.startswith('{'):
                continue
            try:
                row=json.loads(line)
            except json.JSONDecodeError:
                continue
            if row.get('reason')=='compiler-artifact':
                rows.append({'package_id':row['package_id'],'target':row['target']['name'],
                    'kind':row['target']['kind'],'fresh':row['fresh'],'profile':row['profile']})
    save(EVIDENCE/'restored-cargo-fresh.json',{'cache_start':'restored-partial-target; not cold',
        'fresh_artifacts':sum(r['fresh'] for r in rows),'rebuilt_artifacts':sum(not r['fresh'] for r in rows),
        'artifacts':rows,'limitation':'Interrupted rustc units have no compiler-artifact completion; no claim of complete warm cache'})
    print(json.dumps({'cache':'restored-partial','completed_fresh':sum(r['fresh'] for r in rows),
                      'completed_rebuilt':sum(not r['fresh'] for r in rows)}),flush=True)

def download_retained():
    prior=TEMP/'incoming'
    run('download-prior-artifact',['gh','run','download',PRIOR_RUN,'--name',PRIOR_NAME,
        '--repo','taichuy/1flowbase','--dir',str(prior)],seconds=600)
    archived=list(prior.rglob('seed-target-and-source.tar.zst'))
    assert len(archived)==1, 'exactly one fixed archive required'
    archive=archived[0]
    evidence=archive.parent
    retention=read(evidence/'seed-retention.json')
    assert retention['status']=='archived' and retention['source_sha']==SEED_SHA
    assert retention['sha256']==ARCHIVE_SHA and retention['bytes']==ARCHIVE_BYTES
    assert archive.stat().st_size==ARCHIVE_BYTES
    checksum=run('verify-retained-archive',['sha256sum',str(archive)],seconds=60)
    assert Path(checksum['stdout_log']).read_text().split()[0]==ARCHIVE_SHA
    assert retention['compiler']==capture(['rustc','-vV']), 'archive compiler differs; no cold fallback'
    assert retention['root_paths']==[str(TEMP/'seed-source'),str(TEMP/'target-seed')]
    members=run('validate-retained-members',['tar','-I','zstd -T1','-tf',str(archive)],seconds=90)
    for member in Path(members['stdout_log']).read_text().splitlines():
        parts=Path(member).parts
        assert parts and parts[0] in ('seed-source','target-seed') and '..' not in parts
        assert '.git' not in parts, 'archive must not overwrite the new worktree identity'
    prior_plan=read(evidence/'seed-source-plan.json')
    commands=read(evidence/'commands.json')
    prebuild=next(c for c in commands if c['label']=='seed-normal-lib-prebuild')['argv']
    assert prebuild==['cargo','build','--profile','test','--package','api-server',
        '--features','test-support','--lib','--jobs',str(prior_plan['runtime']['backend']['cargoJobs'])]
    assert prior_plan['runtime']['backend']['cargoJobs']==2, 'preserve observed incoming jobs=2'
    os.environ['ISSUE2818_FROZEN_RUNTIME']=json.dumps(prior_plan['runtime'])
    old_samples=evidence/'006-seed-normal-lib-prebuild.native-samples.jsonl'
    run('summarize-prior-native',[sys.executable,str(SELF),'prior-summary',str(old_samples),
        str(EVIDENCE/'prior-native-summary.json')],seconds=90)
    save(EVIDENCE/'cache-resume.json',{'source_run':PRIOR_RUN,'artifact':PRIOR_NAME,
        'archive_sha256':ARCHIVE_SHA,'archive_bytes':ARCHIVE_BYTES,'source_sha':SEED_SHA,
        'runtime':prior_plan['runtime'],'original_prebuild_argv':prebuild,
        'cache_state':'restored partial target from a deadline; not cold','memory_high':'max',
        'different_condition':'previous high=7200000000; current high=max; no before-after speed claim'})
    return archive,evidence,prior_plan,prebuild

def retain_seed_cache(source, target, metadata):
    # One fixed archive, using the existing budget runner; no persistent cache service.
    global END
    END=max(END,time.monotonic()+360)
    archive=EVIDENCE/'seed-target-and-source.tar.zst'
    out=EVIDENCE/'seed-retention.json'
    if not target.exists():
        save(out,{'status':'no-target-created'})
        return
    import resource
    cap=3_000_000_000
    command=[sys.executable,'-c',
        'import os,resource; resource.setrlimit(resource.RLIMIT_FSIZE,('+str(cap)+','+str(cap)+')); os.execvp("tar",'+repr(['tar','--exclude=seed-source/.git','-I','zstd -T1 -1','-cf',str(archive),'-C',str(TEMP),'seed-source','target-seed'])+')']
    try:
        row=run('retain-seed-target',command,seconds=300)
        digest=hashlib.sha256()
        with archive.open('rb') as stream:
            for chunk in iter(lambda:stream.read(1024*1024),b''):
                digest.update(chunk)
        save(out,{'status':'archived','archive':str(archive),'bytes':archive.stat().st_size,
                  'sha256':digest.hexdigest(),'compiler':capture(['rustc','-vV']),
                  'source_sha':metadata['source_sha'],'root_paths':[str(source),str(target)],
                  'cache_level':metadata,'note':'partial or complete as recorded; restore must revalidate Cargo freshness; source mtimes retained and .git excluded',
                  'archive_cost_seconds':row['wall_seconds']})
    except Exception as failure:
        archive.unlink(missing_ok=True)
        save(out,{'status':'archive-unavailable','reason':str(failure),'bounded_bytes':cap,'bounded_seconds':300})

def main():
    # Lowest-cost next evidence: one real database-free seed owner, not another full W.
    EVIDENCE.mkdir(parents=True,exist_ok=False)
    TEMP.mkdir(parents=True,exist_ok=False)
    metadata={'source_sha':os.environ['CANDIDATE_SHA'],'normal_lib_complete':False,'seed_queries_complete':False,
              'resumed_from_run':PRIOR_RUN,'cache_start':'restored-partial-target'}
    assert metadata['source_sha']==SEED_SHA
    assert TEMP==Path('/home/runner/work/_temp/issue2818'), 'restore paths must stay exact'
    source=TEMP/'seed-source'
    target=TEMP/'target-seed'
    initial=budget_snapshot()
    save(EVIDENCE/'budget-before.json',initial)
    assert int(initial['memory.max'])==8_000_000_000 and initial['memory.swap.max']=='0'
    assert initial['memory.high']=='max', 'no extra MemoryHigh policy in this diagnostic window'
    quota,period=map(int,initial['cpu.max'].split())
    assert quota/period<=os.cpu_count()*.6+.001
    assert capture(['rustc','--version']).startswith('rustc 1.99.1 '), 'exact compiler pin required'
    save(EVIDENCE/'environment.json',{'rustc':capture(['rustc','-vV']),'cargo':capture(['cargo','-vV']),
         'host_cpu_count':os.cpu_count(),'source_sha':metadata['source_sha'],'postgres':False,
         'phase':'seed resume from 425622 artifact; 600-second ordinary-lib diagnostic window',
         'cache_start':'restored-partial, not cold','memory_high':'max','extra_G0_policy':False})
    try:
        archive,prior_evidence,prior_plan,prior_build=download_retained()
        source,data,cargo_meta=prepare('seed-source',metadata['source_sha'],archive)
        assert data==prior_plan, 'actual SHA-owned command argv/env/runtime changed after restore'
        assert frozen(source)==read(prior_evidence/'seed-source-source-hashes.json'), 'restored source/config/fixture hash mismatch'
        save(EVIDENCE/'restored-source-validation.json',{'source_sha':metadata['source_sha'],
            'tracked_git_diff':'empty','source_hashes_match':True,'native_mtime_restore':True,
            'source_path':str(source),'target_path':str(target),'target_nonempty':True})
        seed_data={**data,'tests':data['tests'][:2],'checks':[]}
        env=command_env(data,seed_data['tests'][0],target)
        run('registry-prefetch',['cargo','fetch','--locked','--manifest-path','api/Cargo.toml'],cwd=source,seconds=900)
        graph_env={**env,'RUSTC_BOOTSTRAP':'1'}
        original=seed_data['tests'][0]['args'];stop=original.index('--') if '--' in original else len(original)
        qgraph=run('seed-query-unit-graph',['cargo',*original[:stop],'-Z','unstable-options','--unit-graph','--no-run'],cwd=source/'api',env=graph_env,seconds=60)
        build=prior_build
        bgraph=run('seed-lib-unit-graph',[*build,'-Z','unstable-options','--unit-graph'],cwd=source/'api',env=graph_env,seconds=60)
        q=json.loads(Path(qgraph['stdout_log']).read_text());b=json.loads(Path(bgraph['stdout_log']).read_text())
        def sig(u):
            return json.dumps({k:u[k] for k in ('pkg_id','target','profile','platform','mode','features')},sort_keys=True)
        def closure(g,i):
            seen=set();todo=[i]
            while todo:
                k=todo.pop()
                if k in seen:continue
                seen.add(k);todo.extend(d['index'] for d in g['units'][k]['dependencies'])
            return {sig(g['units'][k]) for k in seen}
        r=b['roots'][0];matches=[i for i,u in enumerate(q['units']) if sig(u)==sig(b['units'][r])]
        assert len(matches)==1 and closure(q,matches[0])==closure(b,r), 'prebuild not exact seed dependency'
        assert len(closure(b,r))==548, 'the frozen ordinary-lib dependency graph must remain 548 units'
        save(EVIDENCE/'seed-prebuild-compatibility.json',{'exact_recursive_unit_equality':True,'units':len(closure(b,r)),
             'compiler':capture(['rustc','--version']),'purpose':'one ordinary-lib heavy root before private-contracts root; unchanged jobs/features/profile'})
        run('seed-normal-lib-prebuild',[*build,'--message-format=json'],cwd=source/'api',env=env,heavy=True,seconds=600)
        cargo_fresh_summary()
        assert read(EVIDENCE/'restored-cargo-fresh.json')['fresh_artifacts']>0, 'no Cargo reuse observed; do not continue a cold fallback'
        metadata['normal_lib_complete']=True
        executions=[]
        for index,command in enumerate(seed_data['tests']):
            executions.append(run('seed-query-'+str(index),[command['command'],*command['args']],
                 cwd=source/command['cwd'],env=command_env(data,command,target),heavy=True,seconds=2700))
        metadata['seed_queries_complete']=True
        discover('candidate-seed',source,seed_data,cargo_meta,target,executions)
        save(EVIDENCE/'status.json',{'status':'SEED_OWNER_EXECUTED','source_sha':metadata['source_sha'],
             'not_final_QA':True,'not_full_W_or_performance_PASS':True,'remaining':'old/new complete sets, MCP/application, known D, additional API check cost, job-boundary A/B'})
        return 0
    finally:
        cargo_fresh_summary()
        retain_seed_cache(source,target,metadata)

if __name__=='__main__':
    if len(sys.argv)>1 and sys.argv[1]=='prior-summary':
        summarize_prior(sys.argv[2],sys.argv[3])
        sys.exit(0)
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
