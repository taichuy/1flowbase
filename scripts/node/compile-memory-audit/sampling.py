"""Linux observations. GNU time is an invocation/descendant bound, not PID RSS."""
import json
import os
import re
from pathlib import Path
import signal
import subprocess
import time


def atomic_json(file, value):
    temporary = file.with_suffix(file.suffix + '.tmp')
    temporary.write_text(json.dumps(value, indent=2, allow_nan=False) + '\n')
    temporary.replace(file)


def process(pid):
    directory = Path('/proc') / str(pid)
    try:
        fields = (directory / 'stat').read_text().rsplit(')', 1)[1].split()
        status = dict(line.split(':', 1) for line in (directory / 'status').read_text().splitlines() if ':' in line)
        return {'pid': int(pid), 'ppid': int(fields[1]), 'pgid': int(fields[2]),
                'starttime_ticks': int(fields[19]), 'exe': os.readlink(directory / 'exe'),
                'command': (directory / 'cmdline').read_bytes().replace(b'\0', b' ').decode(errors='replace'),
                'rss_kib': int(status.get('VmRSS', '0 kB').split()[0]),
                'hwm_kib': int(status.get('VmHWM', '0 kB').split()[0]),
                'cgroup': (directory / 'cgroup').read_text().strip()}
    except (OSError, ValueError, IndexError):
        return None  # A disappearing process is not a zero-RSS observation.


def processes():
    return {int(p.name): item for p in Path('/proc').iterdir()
            if p.name.isdigit() and (item := process(p.name))}


def descendants(table, root):
    selected = {root}
    while True:
        expanded = selected | {pid for pid, item in table.items() if item['ppid'] in selected}
        if expanded == selected:
            return {pid: table[pid] for pid in selected if pid in table}
        selected = expanded


def process_role(executable):
    name = Path(executable).name
    if name == 'rustc':
        return 'rustc'
    if name == 'collect2' or re.fullmatch(r'(?:.*-)?(?:cc|gcc|g\+\+|clang|clang\+\+)(?:-\d+)?', name):
        return 'linker-driver'
    if re.fullmatch(r'(?:.*-)?(?:ld|ld\.bfd|ld\.gold|ld\.lld|lld|rust-lld)(?:-\d+)?', name):
        return 'linker'
    return None


def cargo_processes():
    return [item for item in processes().values() if Path(item['exe']).name == 'cargo'
            or item['command'].split(' ', 1)[0].endswith('/cargo')]


def admission(workspace):
    if cargo_processes():
        raise RuntimeError('Another Cargo exists; unrelated processes are never stopped')
    def cpu():
        return list(map(int, Path('/proc/stat').read_text().splitlines()[0].split()[1:9]))
    before = cpu()
    time.sleep(.25)
    delta = [b - a for a, b in zip(before, cpu())]
    total = sum(delta)
    memory = {line.split(':')[0]: int(line.split()[1]) for line in Path('/proc/meminfo').read_text().splitlines()}
    disk = os.statvfs(workspace)
    result = {'cpu_used': 1 - (delta[3] + delta[4]) / total if total else 0,
              'physical_ram_used': 1 - memory['MemAvailable'] / memory['MemTotal'],
              'disk_used': 1 - disk.f_bfree / disk.f_blocks, 'at': time.time()}
    if max(result['cpu_used'], result['physical_ram_used'], result['disk_used']) > .9:
        raise RuntimeError('Resource admission above 90%: ' + json.dumps(result))
    if cargo_processes():
        raise RuntimeError('Cargo appeared during admission')
    return result


def read_pairs(file):
    return {key: int(value) for key, value in (line.split() for line in file.read_text().splitlines())}


def run_command(argv, cwd, env, output, cgroup, interval=.05, monitor_resources=True):
    """Own one process group; return one wait4 CPU scope, never sum nested times."""
    output.mkdir(exist_ok=False)
    before = read_pairs(cgroup / 'memory.events')
    start = time.monotonic()
    stopped = []
    old_handlers = {}
    def cancel(signum, _frame):
        stopped.append(signum)
    for sig in (signal.SIGINT, signal.SIGTERM, signal.SIGHUP):
        old_handlers[sig] = signal.signal(sig, cancel)
    observed = {}
    anon_peak = 0
    next_anon = 0
    previous_cpu = list(map(int, Path('/proc/stat').read_text().splitlines()[0].split()[1:9]))
    resource_samples = []
    error = None
    child = None
    usage = None
    status = None
    cleanup = {'process_group_empty': False, 'principal_reaped': False}
    try:
        with (output / 'command.log').open('wb', buffering=0) as log, \
                (output / 'anon-samples.tsv').open('w') as anon_log, \
                (output / 'process-samples.jsonl').open('w') as process_log:
            child = subprocess.Popen(argv, cwd=cwd, env=env, stdout=log, stderr=subprocess.STDOUT,
                                     stdin=subprocess.DEVNULL, start_new_session=True)
            identity = process(child.pid)
            atomic_json(output / 'started.json', {'argv': argv, 'cwd': str(cwd), 'identity': identity,
                                                  'at': time.time()})
            term_at = None
            while True:
                now = time.monotonic()
                if now >= next_anon:
                    anon = read_pairs(cgroup / 'memory.stat')['anon']
                    anon_peak = max(anon_peak, anon)
                    anon_log.write(f'{time.time()}\t{anon}\n')
                    anon_log.flush()
                    if monitor_resources:
                        current_cpu = list(map(int, Path('/proc/stat').read_text().splitlines()[0].split()[1:9]))
                        delta = [b - a for a, b in zip(previous_cpu, current_cpu)]
                        previous_cpu = current_cpu
                        memory = {line.split(':')[0]: int(line.split()[1]) for line in Path('/proc/meminfo').read_text().splitlines()}
                        disks = [os.statvfs(p) for p in {str(cwd), env.get('CARGO_TARGET_DIR', str(cwd))}]
                        sample = {'at': time.time(), 'cpu_used': 1 - (delta[3] + delta[4]) / sum(delta) if sum(delta) else 0,
                                  'physical_ram_used': 1 - memory['MemAvailable'] / memory['MemTotal'],
                                  'disk_used': max(1 - d.f_bfree / d.f_blocks for d in disks)}
                        resource_samples.append(sample)
                        if max(sample['cpu_used'], sample['physical_ram_used'], sample['disk_used']) > .9:
                            error = 'Machine resource usage exceeded 90%; owned workload cancelled'
                            if not stopped:
                                stopped.append(signal.SIGTERM)
                    next_anon = now + 1  # Original one-second cgroup anonymous-memory method.
                table = processes()
                owned = descendants(table, child.pid)
                cargos = [p for p in table.values() if Path(p['exe']).name == 'cargo']
                if len(cargos) > 1 or any(p['pid'] not in owned for p in cargos):
                    error = 'Global Cargo uniqueness lost; only owned workload is cancelled'
                    if not stopped:
                        stopped.append(signal.SIGTERM)
                for item in owned.values():
                    name = Path(item['exe']).name
                    role = process_role(item['exe'])
                    if role:
                        key = f"{item['pid']}:{item['starttime_ticks']}"
                        prior = observed.get(key)
                        ancestor = table.get(item['ppid'])
                        while ancestor and Path(ancestor['exe']).name != 'rustc':
                            ancestor = table.get(ancestor['ppid'])
                        record = dict(item, role=role, sampled_at=time.time(), compiler_ancestor=ancestor)
                        if not prior or item['rss_kib'] > prior['rss_kib'] or item['hwm_kib'] > prior['hwm_kib']:
                            observed[key] = record
                            process_log.write(json.dumps(record) + '\n')
                            process_log.flush()
                if stopped:
                    if term_at is None:
                        try:
                            os.killpg(child.pid, signal.SIGTERM)
                        except ProcessLookupError:
                            pass
                        term_at = now
                    elif now - term_at > 2:
                        try:
                            os.killpg(child.pid, signal.SIGKILL)
                        except ProcessLookupError:
                            pass
                waited, status, usage = os.wait4(child.pid, os.WNOHANG)
                if waited:
                    child.returncode = os.waitstatus_to_exitcode(status)
                    cleanup['principal_reaped'] = True
                    break
                time.sleep(interval)
        # A successful principal must not leave background work in its process group.
        remaining = [p for p in processes().values() if p['pgid'] == child.pid]
        if remaining:
            error = error or 'Workload left background processes; measurement is incomplete'
            os.killpg(child.pid, signal.SIGTERM)
            deadline = time.monotonic() + 2
            while time.monotonic() < deadline and any(p['pgid'] == child.pid for p in processes().values()):
                time.sleep(.05)
            if any(p['pgid'] == child.pid for p in processes().values()):
                os.killpg(child.pid, signal.SIGKILL)
        cleanup['process_group_empty'] = not any(p['pgid'] == child.pid for p in processes().values())
    finally:
        if child and child.returncode is None:
            try:
                os.killpg(child.pid, signal.SIGKILL)
            except ProcessLookupError:
                pass
            _, status, usage = os.wait4(child.pid, 0)
            child.returncode = os.waitstatus_to_exitcode(status)
            cleanup['principal_reaped'] = True
        for sig, handler in old_handlers.items():
            signal.signal(sig, handler)
    after = read_pairs(cgroup / 'memory.events')
    result = {'argv': argv, 'returncode': child.returncode, 'wall_seconds': time.monotonic() - start,
              'cpu': {'user_seconds': usage.ru_utime, 'system_seconds': usage.ru_stime,
                      'scope': 'one wait4 principal including its reaped descendants; not nested sums'},
              'invocation_descendant_maxrss_kib': usage.ru_maxrss,
              'anon_peak_bytes': anon_peak, 'anon_interval_seconds': 1,
              'process_interval_seconds': interval, 'sampled_processes': list(observed.values()),
              'memory_events_delta': {key: after[key] - before.get(key, 0) for key in after},
              'cancel_signals': stopped, 'error': error, 'cleanup': cleanup, 'resource_samples': resource_samples,
              'cgroup': str(cgroup), 'n': 1}
    atomic_json(output / 'result.json', result)
    return result


def volume(directory):
    """lstat all entries, including directories; hardlinks once, no symlink traversal."""
    apparent = allocated = 0
    seen = set()
    counts = {'entries': 0, 'unique_inodes': 0, 'symlinks': 0}
    def visit(file):
        nonlocal apparent, allocated
        stat = file.lstat()
        counts['entries'] += 1
        key = (stat.st_dev, stat.st_ino)
        if key in seen:
            return
        seen.add(key)
        counts['unique_inodes'] += 1
        apparent += stat.st_size
        allocated += stat.st_blocks * 512
        if file.is_symlink():
            counts['symlinks'] += 1
        elif file.is_dir():
            for child in file.iterdir():
                visit(child)
    if directory.exists():
        visit(directory)
    return dict(apparent_bytes=apparent, allocated_bytes=allocated, **counts,
                accounting='lstat includes directory metadata; unique dev/inode; symlinks not followed')


def rustc_rows(file):
    rows = []
    if not file.exists():
        return rows
    for line in file.read_text().splitlines():
        fields = line.split('\t')
        if len(fields) >= 7:
            try:
                rows.append(dict(maxrss_kib=int(fields[0]), wall_seconds=float(fields[1]),
                                 user_seconds=float(fields[2]), system_seconds=float(fields[3]),
                                 crate=fields[4], kind=fields[5], source=fields[6],
                                 features=fields[7] if len(fields) > 7 else '',
                                 scope='GNU time rustc invocation and descendants; not direct rustc/linker PID'))
            except ValueError:
                pass  # Keep GNU time's nonzero-exit diagnostics in the original raw file.
    return rows
