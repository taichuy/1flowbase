#!/usr/bin/env python3
"""Temporary Root2691 positive-gap executor. CI-only; stdlib; no product edits/retries."""
import argparse, datetime, gzip, hashlib, io, json, os, pathlib, re, resource
import shutil, signal, subprocess, sys, tarfile, threading, time, traceback, urllib.parse

FROZEN = '45933db316b3824623a669075dbfcd14d0db2e9a'

def sha(path):
    digest = hashlib.sha256()
    with pathlib.Path(path).open('rb') as stream:
        while block := stream.read(1024 * 1024): digest.update(block)
    return digest.hexdigest()

def utc(): return datetime.datetime.now(datetime.timezone.utc).isoformat()
def write(path, value):
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(value, indent=2, ensure_ascii=False) + '\n')
def scrub(value):
    value = re.sub(r'postgres(?:ql)?://[^\s"\x27<>]+', '[REDACTED_DATABASE_URL]', str(value), flags=re.I)
    return re.sub(r'(?i)((?:set-cookie|cookie|csrf[_-]?(?:token)?|authorization)\s*[:=]\s*)[^\r\n]+', r'\1[REDACTED]', value)

def snapshot():
    mem = {line.split(':')[0]: int(line.split()[1]) for line in pathlib.Path('/proc/meminfo').read_text().splitlines()}
    disk = os.statvfs(os.environ['GITHUB_WORKSPACE'])
    relative = pathlib.Path('/proc/self/cgroup').read_text().strip().split('::')[-1]
    path = pathlib.Path('/sys/fs/cgroup') / relative.lstrip('/')
    groups = {}
    while path.is_relative_to('/sys/fs/cgroup'):
        data = {}
        for key in ['memory.current', 'memory.max', 'memory.high', 'memory.events', 'cpu.max', 'cpu.stat']:
            try: data[key] = (path / key).read_text().strip()
            except OSError: pass
        if data: groups[str(path)] = data
        if path == pathlib.Path('/sys/fs/cgroup'): break
        path = path.parent
    return dict(at=utc(), cpu_count=os.cpu_count(), affinity_count=len(os.sched_getaffinity(0)),
                mem_total_kib=mem['MemTotal'], mem_available_kib=mem['MemAvailable'],
                memory_used_fraction=1-mem['MemAvailable']/mem['MemTotal'],
                disk_total_bytes=disk.f_blocks*disk.f_frsize, disk_free_bytes=disk.f_bfree*disk.f_frsize,
                disk_used_fraction=1-disk.f_bfree/disk.f_blocks, cgroups=groups,
                cpu_ticks=[int(x) for x in pathlib.Path('/proc/stat').read_text().splitlines()[0].split()[1:9]])

def proc_table():
    rows = {}
    for directory in pathlib.Path('/proc').iterdir():
        if not directory.name.isdigit(): continue
        try:
            st = (directory/'stat').read_text().rsplit(')', 1)[1].split()
            status = (directory/'status').read_text()
            row = dict(pid=int(directory.name), ppid=int(st[1]), start_ticks=int(st[19]), name=(directory/'comm').read_text().strip())
            for key in ['VmRSS', 'VmHWM']:
                match = re.search(r'^'+key+r':\s*(\d+)', status, re.M)
                row[key+'_kib'] = int(match[1]) if match else None
            rows[row['pid']] = row
        except (OSError, ValueError, IndexError): pass
    return rows

def family(root_pid, known):
    table = proc_table(); owned = {root_pid}
    while True:
        added = {pid for pid, row in table.items() if row['ppid'] in owned} - owned
        if not added: break
        owned |= added
    for pid in owned:
        if pid in table: known.setdefault(pid, table[pid])
    return [row for pid, row in table.items() if pid in known and row['start_ticks']==known[pid]['start_ticks']]

def stop_owned(child, known):
    events = []
    for sig in [signal.SIGTERM, signal.SIGKILL]:
        for row in family(child.pid, known):
            if row['pid'] == child.pid: continue  # Let GNU time reap and write rusage.
            try: os.kill(row['pid'], sig); events.append(dict(pid=row['pid'], start_ticks=row['start_ticks'], signal=int(sig)))
            except ProcessLookupError: pass
        try: child.wait(timeout=10 if sig==signal.SIGTERM else 5); break
        except subprocess.TimeoutExpired: pass
    return events

def event_growth(before, after):
    changes = {}
    for path, row in after['cgroups'].items():
        if 'memory.events' not in row or path not in before['cgroups']: continue
        old = dict(line.split() for line in before['cgroups'][path].get('memory.events', '').splitlines())
        now = dict(line.split() for line in row['memory.events'].splitlines())
        changes[path] = {key: int(now[key])-int(old.get(key, 0)) for key in ['high', 'max', 'oom', 'oom_kill'] if key in now}
    return changes

class Runner:
    def __init__(self, args):
        # Both execution and finalization refuse local use before starting ANY subprocess.
        assert os.environ.get('GITHUB_ACTIONS') == 'true' and os.environ.get('RUNNER_OS') == 'Linux', 'GitHub Linux runner only'
        assert os.environ.get('GITHUB_EVENT_NAME') == 'workflow_dispatch', 'Manual CI only'
        assert os.environ.get('GITHUB_REF', '').startswith('refs/heads/')
        assert os.environ['GITHUB_REF'] not in ['refs/heads/dev', 'refs/heads/main', 'refs/heads/master'], 'CI-only branch required'
        workspace = pathlib.Path(os.environ['GITHUB_WORKSPACE']).resolve()
        self.root = pathlib.Path(args.candidate).resolve(); self.executor = pathlib.Path(args.executor).resolve()
        self.out = pathlib.Path(args.output).resolve()
        assert self.root == workspace/'candidate' and self.executor == workspace/'executor'
        assert self.out == workspace/'qa-ci-artifacts'
        self.inputs = pathlib.Path(args.manifest).resolve().parent
        assert self.inputs.is_relative_to(self.executor)
        self.plan = json.loads(pathlib.Path(args.manifest).read_text()); assert self.plan['candidate_sha'] == FROZEN
        self.out.mkdir(parents=True, exist_ok=True)
        provenance=self.out/'provenance';provenance.mkdir(exist_ok=True)
        for name in ['manifest.json','reuse.json','runner.py']:
            shutil.copy2(self.inputs/name,provenance/name)
        workflow=self.executor/'.github/workflows/gateway-target-handoff.yml'
        if workflow.exists(): shutil.copy2(workflow,provenance/'workflow.yml')
        state = self.out/'state.json'
        self.state = json.loads(state.read_text()) if state.exists() else dict(started=utc(), builds=[], tests=[], binaries={}, commands=[], stop_reason=None)
        self.cancelled = False; self.observer_failed = False; self.started = time.monotonic()
        self.env = dict(os.environ)
        for key in list(self.env):
            if key.startswith(('API_DATABASE_', 'RB_', 'ONEFLOWBASE_CARGO_', 'BOOTSTRAP_ROOT_')) or key in ['DATABASE_URL', 'CARGO_BUILD_JOBS', 'RUSTFLAGS', 'CARGO_ENCODED_RUSTFLAGS']:
                self.env.pop(key, None)
        self.env.update({key:value for key,value in self.plan['profile_policy'].items() if key.startswith('CARGO_')})
        self.env['CARGO_TARGET_DIR'] = str(self.root/'api/target')
        self.db_password = 'gateway2691-ci-fixture'
        self.env.update(PGPASSWORD=self.db_password, PGAPPNAME='gateway2691-positive-ci', RUST_BACKTRACE='1')
        parts = urllib.parse.SplitResult('postgres', 'postgres:'+self.db_password+'@127.0.0.1:5432', '/qadb_gateway2691_positive', 'application_name=gateway2691-positive-ci', '')
        self.env['DATABASE_URL'] = self.env['API_DATABASE_URL'] = urllib.parse.urlunsplit(parts)
        self.bin_dir = pathlib.Path(os.environ['RUNNER_TEMP'])/'gateway2691-positive-binaries'
        self.bin_dir.mkdir(parents=True, exist_ok=True)
        for sig in [signal.SIGTERM, signal.SIGINT]: signal.signal(sig, lambda *_: setattr(self, 'cancelled', True))

    def save(self): write(self.out/'state.json', self.state)

    def command(self, label, argv, *, heavy=False, timeout=3600, pg=False):
        assert re.fullmatch(r'[a-z0-9-]+', label)
        folder = self.out/'commands'/label; folder.mkdir(parents=True, exist_ok=False)
        start = snapshot(); begin = time.monotonic(); samples=[]; known={}; stop=None; killed=[]
        record = dict(label=label, argv=argv, cwd=str(self.root), started=utc(), before=start, heavy=heavy)
        logpath = folder/'output.log'; timepath = folder/'gnu.time'
        if heavy and (self.state.get('stop_reason') or max(start['memory_used_fraction'], start['disk_used_fraction']) >= .9):
            self.state['stop_reason'] = self.state.get('stop_reason') or 'memory/disk precondition >=90%'
            record.update(exit_code=125,wall_seconds=0,user_cpu_seconds=None,system_cpu_seconds=None,peak_rss_kib=None,stop_reason=self.state['stop_reason'],after=start,log=str(logpath.relative_to(self.out)))
            logpath.write_text('Not started: '+self.state['stop_reason']+'\n')
            write(folder/'receipt.json',record)
            with (self.out/'commands.jsonl').open('a') as stream: stream.write(json.dumps(record)+'\n')
            self.state['commands'].append(dict(label=label,exit_code=125,wall_seconds=0,receipt=str((folder/'receipt.json').relative_to(self.out))));self.save()
            return record
        child = subprocess.Popen(['/usr/bin/time', '-v', '-o', str(timepath), '--', *argv], cwd=self.root, env=self.env,
                                 stdout=subprocess.PIPE, stderr=subprocess.STDOUT, start_new_session=True)
        record['time_pid'] = child.pid
        def consume():
            with logpath.open('w') as log:
                for line in iter(child.stdout.readline, b''):
                    log.write(scrub(line.decode(errors='replace'))); log.flush()
        reader = threading.Thread(target=consume, daemon=True); reader.start()
        done = threading.Event(); observer = None
        if pg:
            def observe():
                index=0
                while not done.is_set():
                    try: self.pg_snapshot(label+'-'+str(index), journal=True)
                    except Exception as error:
                        with (self.out/'pg-observer-errors.jsonl').open('a') as stream: stream.write(json.dumps(dict(at=utc(), error=scrub(error)))+'\n')
                        self.observer_failed=True
                        return
                    index += 1; done.wait(2.5)
            observer = threading.Thread(target=observe, daemon=True); observer.start()
        try:
            while child.poll() is None:
                try: child.wait(timeout=2)
                except subprocess.TimeoutExpired:
                    now=snapshot(); owned=family(child.pid, known)
                    samples.append(dict(host=now, processes=owned, rss_sum_kib=sum(row['VmRSS_kib'] or 0 for row in owned), individual_hwm_kib=max([row['VmHWM_kib'] or 0 for row in owned] or [0])))
                    if heavy and max(now['memory_used_fraction'], now['disk_used_fraction']) >= .9: stop='memory/disk >=90%'
                    if heavy and any(value>0 for row in event_growth(start, now).values() for value in row.values()): stop='new memory high/max/OOM event'
                    if self.cancelled: stop='runner cancelled'
                    if self.observer_failed: stop='PG observer failed'
                    if time.monotonic()-begin > timeout: stop='command timeout'
                    if time.monotonic()-self.started > self.plan['resource_policy']['runner_work_budget_seconds']: stop='runner work budget'
                    if stop:
                        killed=stop_owned(child, known); break
            code=child.wait()
        finally:
            done.set()
            if observer: observer.join(timeout=20)
            reader.join(timeout=20)
        timing=timepath.read_text() if timepath.exists() else ''
        def metric(key):
            match=re.search(re.escape(key)+r':\s*([0-9.]+)', timing)
            return float(match[1]) if match else None
        end=snapshot()
        if heavy and not stop and (max(end['memory_used_fraction'],end['disk_used_fraction'])>=.9 or any(value>0 for row in event_growth(start,end).values() for value in row.values())):
            stop='post-command memory/disk/event stop'
        record.update(exit_code=code, wall_seconds=time.monotonic()-begin, user_cpu_seconds=metric('User time (seconds)'),
                      system_cpu_seconds=metric('System time (seconds)'), peak_rss_kib=metric('Maximum resident set size (kbytes)'),
                      sampled_rss_sum_peak_kib=max([row['rss_sum_kib'] for row in samples] or [0]) if samples else None,
                      sampled_individual_hwm_peak_kib=max([row['individual_hwm_kib'] for row in samples] or [0]) if samples else None,
                      owned_identity_receipts=list(known.values()), after=end, memory_event_delta=event_growth(start,end),
                      stop_reason=stop, cleanup=killed, log=str(logpath.relative_to(self.out)), log_sha256=sha(logpath))
        with (folder/'samples.jsonl').open('w') as stream:
            for row in samples: stream.write(json.dumps(row)+'\n')
        write(folder/'receipt.json', record)
        with (self.out/'commands.jsonl').open('a') as stream: stream.write(json.dumps(record)+'\n')
        self.state['commands'].append(dict(label=label, exit_code=code, wall_seconds=record['wall_seconds'], receipt=str((folder/'receipt.json').relative_to(self.out))))
        if stop: self.state['stop_reason']=stop
        self.save()
        print(json.dumps(dict(label=label,exit_code=code,wall_seconds=record['wall_seconds'],stop_reason=stop)), flush=True)
        return record

    def checked(self, label, argv):
        result=self.command(label, argv, timeout=120)
        assert result['exit_code']==0 and result['stop_reason'] is None, label
        return (self.out/result['log']).read_text().strip()

    def pg_snapshot(self, label, journal=False, sql=None):
        container=os.environ.get('PG_SERVICE_ID', ''); assert re.fullmatch(r'[a-f0-9]{12,64}', container), 'Owned service id required'
        sql=sql or """SELECT json_build_object('at',clock_timestamp(),'databases',(SELECT json_agg(datname) FROM pg_database WHERE datname LIKE 'test_backup_%' OR datname='qadb_gateway2691_positive'),'activity',(SELECT coalesce(json_agg(json_build_object('database',datname,'pid',pid,'application',application_name,'state',state,'wait_type',wait_event_type,'wait',wait_event)), '[]'::json) FROM pg_stat_activity WHERE datname LIKE 'test_backup_%' OR datname='qadb_gateway2691_positive'),'advisory_locks',(SELECT coalesce(json_agg(json_build_object('database',a.datname,'pid',l.pid,'mode',l.mode,'granted',l.granted,'classid',l.classid::text,'objid',l.objid::text,'objsubid',l.objsubid)),'[]'::json) FROM pg_locks l JOIN pg_stat_activity a ON a.pid=l.pid WHERE l.locktype='advisory' AND (a.datname LIKE 'test_backup_%' OR a.datname='qadb_gateway2691_positive')))"""
        argv=['docker','exec','--env','PGPASSWORD',container,'psql','-X','-U','postgres','-d','postgres','-v','ON_ERROR_STOP=1','-At','-c',sql]
        if not journal:
            return json.loads(self.checked(label, argv))
        directory=self.out/'pg-samples'; directory.mkdir(exist_ok=True)
        timing=directory/(label+'.time'); begin=time.monotonic()
        process=subprocess.Popen(['/usr/bin/time','-f','%U %S %M %x','-o',str(timing),'--',*argv],env=self.env,stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True,start_new_session=True)
        cleanup=[];timed_out=False
        try: stdout,stderr=process.communicate(timeout=15)
        except subprocess.TimeoutExpired:
            timed_out=True;known={};family(process.pid,known);cleanup=stop_owned(process,known)
            stdout,stderr=process.communicate(timeout=5)
        values=timing.read_text().strip().splitlines()[-1].split() if timing.exists() else []
        valid=len(values)==4 and all(re.fullmatch(r'[0-9.]+',value) for value in values[:3])
        record=dict(label=label,at=utc(),argv=argv,exit_code=process.returncode,wall_seconds=time.monotonic()-begin,
                    user_cpu_seconds=float(values[0]) if valid else None,system_cpu_seconds=float(values[1]) if valid else None,peak_rss_kib=int(values[2]) if valid else None,
                    sample=json.loads(stdout) if process.returncode==0 and not timed_out else None,error=scrub(stderr) if process.returncode else None,timed_out=timed_out,cleanup=cleanup)
        with (self.out/'pg-samples.jsonl').open('a') as stream: stream.write(json.dumps(record)+'\n')
        timing.unlink(missing_ok=True)
        if record['exit_code'] or timed_out: raise RuntimeError('Owned PG observer failed; receipt retained')
        return record

    def preflight(self):
        actual=self.checked('candidate-sha',['git','rev-parse','HEAD']); assert actual==FROZEN
        assert self.checked('candidate-clean',['git','status','--porcelain'])==''
        executor_sha=self.command('executor-sha',['git','-C',str(self.executor),'rev-parse','HEAD'])
        executor_value=(self.out/executor_sha['log']).read_text().strip(); assert executor_sha['exit_code']==0 and executor_value==os.environ['GITHUB_SHA']
        self.state.update(candidate_sha=actual,executor_sha=executor_value)
        delta=self.checked('reuse-dependency-delta',['git','diff','--name-only',self.plan['previous_candidate_sha'],FROZEN]).splitlines()
        reuse=json.loads((self.inputs/'reuse.json').read_text()); assert sorted(delta)==sorted(reuse['invalidated_paths'])
        for path,digest in self.plan['source_sha256'].items(): assert sha(self.root/path)==digest, path
        self.state['reuse_verified']=True;self.save()
        source=self.out/'provenance'; source.mkdir(exist_ok=True)
        for name in ['manifest.json','reuse.json','runner.py']:
            shutil.copy2(self.inputs/name,source/name)
        shutil.copy2(self.executor/'.github/workflows/gateway-target-handoff.yml',source/'workflow.yml')
        for name in ['Cargo.toml','Cargo.lock']: shutil.copy2(self.root/'api'/name,source/name)
        jobs_text=self.checked('resolve-cargo-jobs',['node','scripts/node/testing/verify-runtime.js','cargo-jobs'])
        assert re.fullmatch(r'[1-9][0-9]*',jobs_text); self.env['CARGO_BUILD_JOBS']=jobs_text
        versions={label:self.checked(label,argv) for label,argv in [('rustc-version',['rustc','--version','--verbose']),('cargo-version',['cargo','--version']),('node-version',['node','--version']),('python-version',['python3','--version']),('uname',['uname','-a'])]}
        image=self.checked('pg-owned-image',['docker','inspect','--format','{{.Id}} {{.Config.Image}} {{.Image}}',os.environ['PG_SERVICE_ID']])
        assert ' postgres:18.4 ' in image
        db=self.pg_snapshot('pg-preflight',sql="SELECT json_build_object('version',version(),'role',current_user,'createdb',(SELECT rolcreatedb FROM pg_roles WHERE rolname=current_user),'databases',(SELECT json_agg(datname ORDER BY datname) FROM pg_database))")
        assert db['createdb'] and db['role']=='postgres' and db['version'].startswith('PostgreSQL 18.4 ')
        assert sorted(db['databases'])==['postgres','qadb_gateway2691_positive','template0','template1'], 'Fresh independent service required'
        write(source/'runtime.json',dict(observed_at=utc(),candidate_sha=actual,executor_sha=executor_value,runner={key:os.environ.get(key) for key in ['RUNNER_OS','RUNNER_ARCH','RUNNER_NAME','ImageOS','ImageVersion','GITHUB_RUN_ID','GITHUB_RUN_ATTEMPT','GITHUB_REF','GITHUB_REPOSITORY']},actual_resources=snapshot(),versions=versions,cargo_jobs=int(jobs_text),profile_policy=self.plan['profile_policy'],cargo_config_overrides=self.plan['cargo_config_overrides'],lockfile_sha256=sha(self.root/'api/Cargo.lock'),pg_image=image,pg_preflight=db,cache_policy='Existing pinned rust-cache action; target cache disabled to avoid stale workspace artifacts',reuse_counts=reuse['counts']))
        self.save()

    def build(self, index):
        entry=self.plan['build_entries'][index]; assert len(self.state['builds'])<self.plan['build_entry_limit']
        argv=['cargo',*entry['arguments'],'--jobs',self.env['CARGO_BUILD_JOBS'],'--message-format=json-render-diagnostics']
        for value in self.plan['cargo_config_overrides']: argv += ['--config',value]
        self.state['builds'].append(dict(id=entry['id'],status='RUNNING',arguments=argv)); self.save()
        result=self.command(entry['id'],argv,heavy=True,timeout=self.plan['resource_policy']['command_timeout_seconds'])
        self.state['builds'][-1].update(status='PASS' if result['exit_code']==0 and not result['stop_reason'] else 'FAIL',receipt=result)
        self.bind(entry['id']); self.save()
        return result['exit_code']==0 and result['stop_reason'] is None

    def bind(self, build_id):
        log=self.out/'commands'/build_id/'output.log'
        if not log.exists(): return
        artifacts=[]
        for line in log.read_text(errors='replace').splitlines():
            try: row=json.loads(line)
            except ValueError: continue
            if row.get('reason')!='compiler-artifact': continue
            if not pathlib.Path(row['manifest_path']).is_relative_to(self.root):
                if row['target']['name']=='tikv_jemallocator' and 'stats' in row['features']: self.state['allocator_stats_seen']=True
                continue
            artifacts.append(row)
            if not row.get('executable'): continue
            target=row['target']['name']; path=pathlib.Path(row['executable']).resolve(); assert path.is_relative_to(self.root/'api/target')
            assert row['profile']['debug_assertions'] and row['profile']['overflow_checks']
            assert row['profile']['debuginfo'] in [0,'0','none'], target
            saved=self.bin_dir/target
            if saved.exists(): assert sha(saved)==sha(path), 'Different executable under same target name'
            else: shutil.copy2(path,saved)
            self.state['binaries'][target]=dict(target=target,candidate_sha=FROZEN,executor_sha=self.state.get('executor_sha'),original_path=str(path),saved_path=str(saved),sha256=sha(saved),size_bytes=saved.stat().st_size,profile=row['profile'],features=row['features'],fresh=row['fresh'],build_id=build_id,kind=row['target']['kind'])
        write(self.out/'compiler'/ (build_id+'.json'),artifacts)
        write(self.out/'binary-manifest.json',dict(candidate_sha=FROZEN,executor_sha=self.state.get('executor_sha'),binaries=self.state['binaries']))

    def positives(self):
        available={}
        for target in self.plan['required_test_targets']:
            binary=self.state['binaries'].get(target); assert binary and binary['profile']['test'], target
            listed=self.checked('list-'+target.replace('_','-'),[binary['saved_path'],'--list','--format','terse'])
            available[target]=[line[:-6] for line in listed.splitlines() if line.endswith(': test')]
            assert available[target]
        for group in self.plan['groups']:
            assert group['expected_n']>0 and len(group['selectors'])==len(set(group['selectors']))==group['expected_n']
            missing=sorted(set(group['selectors'])-set(available[group['target']]))
            if missing:
                self.state['tests'].append(dict(id=group['id'],status='MISSING_SELECTORS',missing=missing,executed=0)); self.save()
        write(self.out/'selector-manifest.json',dict(available_counts={key:len(value) for key,value in available.items()},groups=self.plan['groups'],all_selected_present=not self.state['tests']))
        if self.state['tests']: return False
        for name in self.plan['required_examples']:
            binary=self.state['binaries'].get(name); assert binary and not binary['profile']['test'], name
            self.env['MANAGED_'+('HOOK' if name=='managed_hook_worker' else 'EVENT')+'_WORKER_FIXTURE']=binary['saved_path']
        good=True
        for group in self.plan['groups']:
            if self.state.get('stop_reason'): break
            binary=self.state['binaries'][group['target']]; assert sha(binary['saved_path'])==binary['sha256']
            result=self.command(group['id'],[binary['saved_path'],*group['selectors'],'--exact','--test-threads=1'],heavy=True,timeout=self.plan['resource_policy']['test_group_timeout_seconds'],pg=group['target']!='runtime_extension_host')
            text=(self.out/result['log']).read_text(errors='replace')
            summary=re.findall(r'test result: (\w+)\. (\d+) passed; (\d+) failed; (\d+) ignored; (\d+) measured; (\d+) filtered out',text)
            cases=re.findall(r'^test ([\w:]+) \.\.\. (ok|FAILED|ignored)$',text,re.M)
            passed=result['exit_code']==0 and not result['stop_reason'] and len(summary)==1 and summary[0][0]=='ok' and int(summary[0][1])==group['expected_n'] and all(int(summary[0][i])==0 for i in [2,3,4])
            passed=passed and re.findall(r'^running (\d+) tests?$',text,re.M)==[str(group['expected_n'])]
            passed=passed and not any(status in ['FAILED','ignored'] for _,status in cases)
            good &= passed
            self.state['tests'].append(dict(id=group['id'],target=group['target'],expected=group['expected_n'],status='PASS' if passed else 'FAIL',actual_summary=summary,cases=cases,binary_sha256=binary['sha256'],receipt=result)); self.save()
        return good and len(self.state['tests'])==len(self.plan['groups']) and not (self.out/'pg-observer-errors.jsonl').exists()

    def execute(self):
        self.preflight()
        if not self.build(0): return False
        if not self.build(1): return False
        assert self.state.get('allocator_stats_seen'), 'stats feature not observed in actual artifacts'
        if not self.positives(): return False
        self.state['positive_green']=True; self.save()
        if not self.build(2): return False
        assert 'api-server' in self.state['binaries'] and not self.state['binaries']['api-server']['profile']['test']
        return True

    def finalize(self):
        for entry in self.plan['build_entries']: self.bind(entry['id'])
        self.save()
        binaries=self.state['binaries']; directory=self.out/'bundles'; directory.mkdir(exist_ok=True)
        archive=directory/'candidate-executables.tar.gz'; start=time.monotonic(); cpu=time.process_time()
        with archive.open('wb') as raw:
            with gzip.GzipFile(fileobj=raw,mode='wb',compresslevel=1,mtime=0) as zipped:
                with tarfile.open(fileobj=zipped,mode='w|') as bundle:
                    for name,row in sorted(binaries.items()):
                        path=pathlib.Path(row['saved_path']); assert path.parent==self.bin_dir and sha(path)==row['sha256']
                        info=bundle.gettarinfo(str(path),arcname='executables/'+name); info.uid=info.gid=0;info.uname=info.gname='';info.mtime=0
                        with path.open('rb') as stream: bundle.addfile(info,stream)
        write(directory/'manifest.json',dict(format='tar.gz',compression='gzip level 1; mtime=0',path=archive.name,compressed_size_bytes=archive.stat().st_size,uncompressed_executable_bytes=sum(row['size_bytes'] for row in binaries.values()),sha256=sha(archive),entries=[{key:row[key] for key in ['target','sha256','size_bytes','candidate_sha','profile','build_id']} for row in binaries.values()],wall_seconds=time.monotonic()-start,python_cpu_seconds=time.process_time()-cpu,python_process_lifetime_hwm_kib=resource.getrusage(resource.RUSAGE_SELF).ru_maxrss,rss_boundary='Python process lifetime HWM; not phase RSS delta'))
        if self.root.exists():
            result=self.command('final-candidate-clean',['git','status','--porcelain'],timeout=60)
            self.state['candidate_clean_after']=result['exit_code']==0 and not (self.out/result['log']).read_text().strip()
            if not self.state['candidate_clean_after']: self.state['success']=False
            self.pg_snapshot('pg-final')
        self.save(); self.summary()
        hashes={str(path.relative_to(self.out)):dict(size_bytes=path.stat().st_size,sha256=sha(path)) for path in self.out.rglob('*') if path.is_file() and path.name!='artifact-hashes.json'}
        write(self.out/'artifact-hashes.json',hashes)

    def summary(self):
        planned={group['id'] for group in self.plan['groups']}; ran={row['id'] for row in self.state['tests']}
        commands=[]
        for path in sorted((self.out/'commands').glob('*/receipt.json')): commands.append(json.loads(path.read_text()))
        pg=[json.loads(line) for line in (self.out/'pg-samples.jsonl').read_text().splitlines()] if (self.out/'pg-samples.jsonl').exists() else []
        write(self.out/'summary.json',dict(verdict='CI_POSITIVE_PASS' if self.state.get('success') else 'CI_POSITIVE_FAIL',root_verdict='NOT_DECIDED: source negatives and ABBA deferred to Root',candidate_sha=FROZEN,executor_sha=self.state.get('executor_sha'),build_entries_used=len(self.state['builds']),build_entry_limit=3,expected_tests=64,groups=self.state['tests'],blocked_groups=sorted(planned-ran),positive_green=self.state.get('positive_green',False),stop_reason=self.state.get('stop_reason'),error=self.state.get('error'),counts=dict(passed=sum(int(row['actual_summary'][0][1]) for row in self.state['tests'] if row.get('actual_summary')),reused_rust=274 if self.state.get('reuse_verified') else None,reused_node=118 if self.state.get('reuse_verified') else None,old147_reused=147 if self.state.get('reuse_verified') else None,reused_CI_executions=0,mutants_executed=0,ABBA_runs=0,real_PR_samples=0,real_PR_p50=None,real_PR_p95=None),command_count=len(commands),command_wall_seconds=sum(row['wall_seconds'] for row in commands),command_user_cpu_seconds=sum(row['user_cpu_seconds'] or 0 for row in commands),command_system_cpu_seconds=sum(row['system_cpu_seconds'] or 0 for row in commands),peak_gnu_rss_kib=max([row['peak_rss_kib'] or 0 for row in commands] or [0]),pg_observer_invocations=len(pg),pg_observer_wall_seconds=sum(row['wall_seconds'] for row in pg),cost_boundary='GNU child CPU/RSS per command; process RSS/HWM sampled separately; observer commands and archive CPU separated; action bootstrap/cache/upload overhead not included; no CI latency improvement inferred',state='COMPLETE'))

def main():
    parser=argparse.ArgumentParser(); parser.add_argument('mode',choices=['run','finalize']);parser.add_argument('--candidate',required=True);parser.add_argument('--executor',required=True);parser.add_argument('--output',required=True);parser.add_argument('--manifest',required=True);args=parser.parse_args()
    runner=Runner(args)
    try:
        if args.mode=='run': runner.state['success']=runner.execute()
        else: runner.finalize()
    except Exception:
        runner.state['success']=False;runner.state['error']=scrub(traceback.format_exc())
    finally:
        runner.save();runner.summary()
        hashes={str(path.relative_to(runner.out)):dict(size_bytes=path.stat().st_size,sha256=sha(path)) for path in runner.out.rglob('*') if path.is_file() and path.name!='artifact-hashes.json'}
        write(runner.out/'artifact-hashes.json',hashes)
        print(json.dumps(dict(success=runner.state.get('success',False),stop_reason=runner.state.get('stop_reason'),output=str(runner.out))),flush=True)
    return 0 if runner.state.get('success') else 1
if __name__=='__main__': sys.exit(main())
