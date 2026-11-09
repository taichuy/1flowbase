#!/usr/bin/env python3
"""Opt-in, noninteractive long jobs in an owned systemd user scope.

Requires an existing finite task-budget *user slice*, with the controller already
inside that slice. A scope cannot contain a manager-created child scope. This
runner fails closed for that unsupported budget shape; it never relocates the
controller or changes shared limits. Wrap resource-heavy-run.py in argv to use
the existing admission gate. The deadline includes validation and admission;
bounded cleanup is additional (term grace plus bounded management waits). An
optional memory stop watches the verified aggregate task budget and stops only
this job before the shared memory.max is reached; it is not an OOM diagnosis.

Receipts are not guaranteed after controller SIGKILL or whole-parent OOM. Native
RuntimeMaxSec/TimeoutStopSec still bound the owned scope if its controller dies.
This is lifecycle control for trusted local jobs, not a security sandbox against
commands deliberately migrating themselves to other cgroups.
"""
import argparse
import ctypes
from datetime import datetime, timezone
import json
import math
import os
from pathlib import Path
import signal
import subprocess
import sys
import tempfile
import time
import uuid


CGROOT = Path('/sys/fs/cgroup')
HELPER = Path(__file__).resolve().with_name('resource-budget-context.sh')
SYSTEMCTL = '/usr/bin/systemctl'
SYSTEMD_RUN = '/usr/bin/systemd-run'
POLL = .05
MANAGEMENT_SECONDS = 2
PROPERTIES = ('Id', 'ControlGroup', 'InvocationID', 'ActiveState', 'SubState',
              'Result', 'RuntimeMaxUSec', 'TimeoutStopUSec', 'OOMPolicy',
              'KillMode', 'SendSIGKILL', 'CPUUsageNSec', 'MemoryPeak', 'MemoryCurrent')


def utc():
    return datetime.now(timezone.utc).isoformat()


def atomic_json(path, value):
    """Same-directory replace, including fsync: readers never see partial JSON."""
    fd, temporary = tempfile.mkstemp(prefix='.' + path.name + '.', dir=path.parent)
    try:
        with os.fdopen(fd, 'w') as stream:
            json.dump(value, stream, ensure_ascii=True, indent=2, allow_nan=False)
            stream.write('\n')
            stream.flush()
            os.fsync(stream.fileno())
        os.replace(temporary, path)
        directory = os.open(path.parent, os.O_RDONLY | os.O_DIRECTORY)
        try:
            os.fsync(directory)
        finally:
            os.close(directory)
    finally:
        if os.path.exists(temporary):
            os.unlink(temporary)


def read_json(path):
    try:
        return json.loads(path.read_text())
    except FileNotFoundError:
        return None


def current_cgroup(pid='self'):
    for line in Path(f'/proc/{pid}/cgroup').read_text().splitlines():
        if line.startswith('0::'):
            return line[3:]
    raise RuntimeError('Linux unified cgroup v2 is required')


def process_identity(pid):
    fields = Path(f'/proc/{pid}/stat').read_text().rsplit(')', 1)[1].split()
    return {'pid': pid, 'starttime_ticks': int(fields[19])}


def positive_seconds(value):
    number = float(value)
    if not math.isfinite(number) or number <= 0:
        raise argparse.ArgumentTypeError('must be a finite positive number')
    return number


def positive_bytes(value):
    try:
        number = int(value)
    except ValueError:
        raise argparse.ArgumentTypeError('must be a positive integer number of bytes') from None
    if number <= 0:
        raise argparse.ArgumentTypeError('must be a positive integer number of bytes')
    return number


def budget_memory_observation(budget, stop_bytes):
    """Read only the already-validated ancestor, retaining its exact identity."""
    group = CGROOT / budget['cgroup'].lstrip('/')
    if group.stat().st_ino != budget['inode']:
        raise RuntimeError('task budget inode changed before memory observation')
    maximum = (group / 'memory.max').read_text().strip()
    if not maximum.isdigit() or int(maximum) <= 0:
        raise RuntimeError('task budget memory.max must remain finite and positive')
    if stop_bytes >= int(maximum):
        raise ValueError('memory-stop-bytes must be smaller than task budget memory.max')
    current = int((group / 'memory.current').read_text().strip())
    if current < 0:
        raise RuntimeError('task budget memory.current must be nonnegative')
    if group.stat().st_ino != budget['inode']:
        raise RuntimeError('task budget inode changed during memory observation')
    return {'source': 'verified task-budget memory.current', 'quality': 'measured',
            'budget': dict(budget), 'current_bytes': current, 'stop_bytes': stop_bytes,
            'memory_max_bytes': int(maximum), 'observed_at': utc(),
            'observed_monotonic': time.monotonic()}


def bounded_call(argv, log, timeout=MANAGEMENT_SECONDS):
    """No pipe lifetime dependency and no unbounded post-kill wait."""
    if timeout <= 0:
        raise TimeoutError('wall deadline before management command')
    with tempfile.TemporaryFile() as output, log.open('ab') as errors:
        errors.write(('\n' + repr(argv) + '\n').encode())
        errors.flush()
        child = subprocess.Popen(argv, stdin=subprocess.DEVNULL, stdout=output,
                                 stderr=errors, start_new_session=True)
        try:
            status = child.wait(timeout=timeout)
        except subprocess.TimeoutExpired:
            child.kill()  # Only the direct management process we created.
            try:
                child.wait(timeout=.5)
            except subprocess.TimeoutExpired:
                pass
            raise TimeoutError('management command exceeded bounded wait: ' + argv[0])
        output.seek(0)
        data = output.read(32769)
        if len(data) > 32768:
            raise RuntimeError('management output exceeded 32768 bytes')
        if status:
            raise RuntimeError(f'management command exited {status}: {argv[0]}')
        return data.decode('utf-8', errors='replace')


def validate_budget(log, timeout=MANAGEMENT_SECONDS):
    # Reuse the repository's real ancestry/inode/finite-limit validator, rather
    # than treating environment declarations or legacy wrapper flags as proof.
    bounded_call(['/bin/bash', '-c', 'source "$1"; oneflowbase_task_budget_context',
                  'resource-job-budget', str(HELPER)], log, timeout)
    budget = os.environ['ONEFLOWBASE_TASK_BUDGET_CGROUP']
    return {'cgroup': budget, 'inode': int(os.environ['ONEFLOWBASE_TASK_BUDGET_ID'])}


def worker(output):
    """Small scope-local rendezvous: command never runs before ownership proof."""
    config = read_json(output / 'started.json')
    deadline = config['deadline_monotonic']
    stopping = False

    def stop(_signum, _frame):
        nonlocal stopping
        stopping = True  # Manager signals the whole scope, including the command.

    for sig in (signal.SIGTERM, signal.SIGINT, signal.SIGHUP):
        signal.signal(sig, stop)
    budget = validate_budget(output / 'manager.stderr.log',
                             min(MANAGEMENT_SECONDS, deadline - time.monotonic()))
    actual = current_cgroup()
    expected = budget['cgroup'] + '/' + config['unit']
    if actual != expected:
        raise RuntimeError('worker is not in the exact requested child scope')
    group = CGROOT / actual.lstrip('/')
    identity = dict(process_identity(os.getpid()), cgroup=actual, inode=group.stat().st_ino)
    atomic_json(output / 'worker-ready.json', identity)
    while time.monotonic() < deadline and not stopping:
        if (output / 'abort.json').exists():
            return 0
        go = read_json(output / 'go.json')
        if go:
            if go != identity:
                raise RuntimeError('controller/worker ownership handshake mismatch')
            break
        time.sleep(POLL)
    else:
        return 0
    if stopping or time.monotonic() >= deadline or (output / 'abort.json').exists():
        return 0
    os.chdir(config['cwd'])
    start = time.monotonic()
    with (output / 'stdout.log').open('ab', buffering=0) as stdout, \
            (output / 'stderr.log').open('ab', buffering=0) as stderr:
        try:
            child = subprocess.Popen(config['argv'], stdin=subprocess.DEVNULL,
                                     stdout=stdout, stderr=stderr)
        except OSError as error:
            atomic_json(output / 'command-finished.json', {
                'kind': 'spawn_failure', 'errno': error.errno, 'error': str(error),
                'started_monotonic': start, 'finished_monotonic': time.monotonic(),
                'finished_at': utc(), 'returncode': None, 'wait_status': None})
        else:
            atomic_json(output / 'command-started.json', {
                **process_identity(child.pid), 'started_at': utc(),
                'started_monotonic': start, 'cgroup': actual})
            while True:
                waited, status, usage = os.wait4(child.pid, os.WNOHANG)
                if waited:
                    code = os.waitstatus_to_exitcode(status)
                    child.returncode = code
                    atomic_json(output / 'command-finished.json', {
                        'kind': 'exit' if code >= 0 else 'signal',
                        'returncode': code, 'wait_status': status,
                        'exit_code': code if code >= 0 else None,
                        'signal': -code if code < 0 else None,
                        'started_monotonic': start, 'finished_monotonic': time.monotonic(),
                        'finished_at': utc(),
                        'wait4_usage': {'quality': 'measured',
                                        'scope': 'principal and its reaped children; Linux wait4',
                                        'user_seconds': usage.ru_utime,
                                        'system_seconds': usage.ru_stime,
                                        'maxrss_kib': usage.ru_maxrss}})
                    break
                time.sleep(POLL)
    # Hold the cgroup/counters for the controller. Native runtime/stop timers
    # bound this rendezvous even if the controller vanishes. No process walk.
    while not stopping:
        time.sleep(POLL)
    return 0


class Job:
    def __init__(self, args):
        self.args = args
        self.output = Path(args.output_dir).absolute()
        self.output.mkdir(mode=0o700)  # Exclusive: never append to an old run.
        self.start = time.monotonic()
        self.deadline = self.start + args.deadline_seconds
        self.unit = f'oneflowbase-job-{os.getpid()}-{uuid.uuid4().hex}.scope'
        self.log = self.output / 'manager.stderr.log'
        self.cancel = None
        for sig in (signal.SIGTERM, signal.SIGINT, signal.SIGHUP):
            signal.signal(sig, self.cancelled)
        self.launcher = None
        self.owner = None
        self.budget = None
        self.manager = None
        self.manager_observed_at = None
        self.resources = {'quality': 'unavailable', 'reason': 'owned cgroup not observed'}
        self.baseline_events = None
        self.oom_evidence = None
        self.memory_stop_evidence = None
        self.launcher_files = []
        self.started = {'schema_version': 1, 'started_at': utc(),
                        'started_monotonic': self.start,
                        'deadline_monotonic': self.deadline,
                        'deadline_seconds': args.deadline_seconds,
                        'term_grace_seconds': args.term_grace_seconds,
                        'memory_stop_bytes': args.memory_stop_bytes,
                        'argv': args.command, 'cwd': os.getcwd(),
                        'controller': process_identity(os.getpid()),
                        'controller_cgroup': current_cgroup(), 'unit': self.unit,
                        'declared_budget': {
                            'cgroup': os.environ.get('ONEFLOWBASE_TASK_BUDGET_CGROUP'),
                            'inode': os.environ.get('ONEFLOWBASE_TASK_BUDGET_ID')},
                        'limitations': [
                            'receipt requires surviving controller; whole-parent OOM/SIGKILL excluded',
                            'trusted commands must not migrate out of the owned cgroup',
                            'deadline bounds work; bounded cleanup time is additional',
                            'stdin is /dev/null; stdout/stderr are raw files']}
        atomic_json(self.output / 'started.json', self.started)
        for name in ('stdout.log', 'stderr.log', 'manager.stdout.log', 'manager.stderr.log'):
            (self.output / name).touch(exist_ok=False)

    def cancelled(self, signum, _frame):
        if self.cancel is None:
            self.cancel = {'signal': signum, 'observed_monotonic': time.monotonic()}

    def remaining(self):
        return min(MANAGEMENT_SECONDS, self.deadline - time.monotonic())

    def show(self, timeout=MANAGEMENT_SECONDS):
        data = bounded_call([SYSTEMCTL, '--user', 'show', self.unit,
                             '--property=' + ','.join(PROPERTIES)], self.log, timeout)
        return dict(line.split('=', 1) for line in data.splitlines() if '=' in line)

    def acquire_owner(self, timeout=MANAGEMENT_SECONDS):
        state = self.show(timeout)
        relative = state.get('ControlGroup')
        expected = self.budget['cgroup'] + '/' + self.unit
        if state.get('Id') != self.unit or relative != expected or not state.get('InvocationID'):
            raise RuntimeError('manager did not return the exact owned child ControlGroup')
        group = CGROOT / relative.lstrip('/')
        if str(group.resolve(strict=True)) != str(group):
            raise RuntimeError('noncanonical owned cgroup')
        inode = group.stat().st_ino
        if current_cgroup() == relative or current_cgroup().startswith(relative + '/'):
            raise RuntimeError('controller must remain outside owned child cgroup')
        owner = {'unit': self.unit, 'cgroup': relative, 'inode': inode,
                 'invocation_id': state['InvocationID']}
        if self.owner is not None and owner != self.owner:
            raise RuntimeError('owned cgroup identity changed; refusing cleanup')
        self.owner, self.manager = owner, state
        self.manager_observed_at = utc()
        return owner

    def sample(self):
        if not self.owner:
            return
        group = CGROOT / self.owner['cgroup'].lstrip('/')
        try:
            if group.stat().st_ino != self.owner['inode']:
                raise RuntimeError('owned cgroup inode changed')
            fields = {}
            for name in ('memory.events', 'memory.events.local', 'cpu.stat', 'memory.peak',
                         'memory.current', 'memory.swap.peak', 'cgroup.events'):
                try:
                    content = (group / name).read_text().strip()
                    fields[name] = {'quality': 'measured', 'value':
                                    dict((key, int(value)) for key, value in
                                         (line.split() for line in content.splitlines()))
                                    if '\n' in content or ' ' in content else int(content)}
                except OSError as error:
                    fields[name] = {'quality': 'unavailable', 'reason': str(error)}
            # Snapshot is exact at observation, never claimed as final totals or
            # inferred RSS. In particular, memory.peak is the kernel cgroup peak.
            if group.stat().st_ino != self.owner['inode']:
                raise RuntimeError('owned cgroup inode changed during sampling')
            self.resources = {'quality': 'last_observed', 'observed_at': utc(),
                              'observed_monotonic': time.monotonic(), 'cgroup': self.owner,
                              'fields': fields}
            events = fields['memory.events'].get('value', {})
            if self.baseline_events is None:
                self.baseline_events = events
            kills = events.get('oom_kill', 0) - self.baseline_events.get('oom_kill', 0)
            if kills > 0:
                self.oom_evidence = {'source': 'owned cgroup memory.events oom_kill delta',
                                     'quality': 'measured', 'value': kills}
        except FileNotFoundError:
            pass  # Preserve last observation, explicitly not a final counter.

    def sample_memory_stop(self):
        if self.args.memory_stop_bytes is None or self.memory_stop_evidence is not None:
            return
        observation = budget_memory_observation(self.budget, self.args.memory_stop_bytes)
        if observation['current_bytes'] >= observation['stop_bytes']:
            self.memory_stop_evidence = observation
            atomic_json(self.output / 'memory-stop.json', observation)

    def empty(self):
        if not self.owner:
            return False
        group = CGROOT / self.owner['cgroup'].lstrip('/')
        try:
            if group.stat().st_ino != self.owner['inode']:
                raise RuntimeError('refusing changed cgroup inode')
            events = dict(line.split() for line in (group / 'cgroup.events').read_text().splitlines())
            return events.get('populated') == '0'
        except FileNotFoundError:
            return True

    def start_scope(self):
        self.budget = validate_budget(self.log, self.remaining())
        self.sample_memory_stop()  # Validate the threshold before creating any scope.
        if self.memory_stop_evidence:
            return
        budget_path = CGROOT / self.budget['cgroup'].lstrip('/')
        slice_name = budget_path.name
        if not slice_name.endswith('.slice'):
            raise RuntimeError('native child scope requires a systemd user slice task budget')
        # Ensure Slice= resolves to the declared path, rather than a similarly
        # named unit elsewhere. Do not activate or edit any shared unit.
        text = bounded_call([SYSTEMCTL, '--user', 'show', slice_name,
                             '--property=Id,ControlGroup,ActiveState'], self.log, self.remaining())
        state = dict(line.split('=', 1) for line in text.splitlines() if '=' in line)
        if state.get('Id') != slice_name or state.get('ControlGroup') != self.budget['cgroup'] \
                or state.get('ActiveState') != 'active' or budget_path.stat().st_ino != self.budget['inode']:
            raise RuntimeError('budget must be the exact active user-manager slice')
        if self.cancel or time.monotonic() >= self.deadline:
            return
        # Reap orphaned descendants via the kernel, without discovering or
        # signalling PIDs. This prevents zombie residue when grandchildren setsid.
        if ctypes.CDLL(None, use_errno=True).prctl(36, 1, 0, 0, 0) != 0:
            raise OSError(ctypes.get_errno(), 'PR_SET_CHILD_SUBREAPER failed')
        runtime = self.deadline - time.monotonic()
        argv = [SYSTEMD_RUN, '--user', '--scope', '--quiet', '--expand-environment=no',
                '--unit=' + self.unit, '--slice=' + slice_name,
                '--property=RuntimeMaxSec=' + f'{runtime:.6f}s',
                '--property=TimeoutStopSec=' + f'{self.args.term_grace_seconds:.6f}s',
                '--property=KillMode=control-group', '--property=SendSIGKILL=yes',
                '--property=OOMPolicy=stop', '--', sys.executable,
                str(Path(__file__).resolve()), '--_worker', str(self.output)]
        self.launcher_files = [(self.output / name).open('ab', buffering=0)
                               for name in ('manager.stdout.log', 'manager.stderr.log')]
        self.launcher = subprocess.Popen(argv, stdin=subprocess.DEVNULL,
                                         stdout=self.launcher_files[0], stderr=self.launcher_files[1],
                                         cwd=self.started['cwd'], start_new_session=True)
        while not self.cancel and time.monotonic() < self.deadline:
            self.sample_memory_stop()
            if self.memory_stop_evidence:
                return
            ready = read_json(self.output / 'worker-ready.json')
            if ready:
                self.acquire_owner(self.remaining())
                if ready['cgroup'] != self.owner['cgroup'] or ready['inode'] != self.owner['inode'] \
                        or process_identity(ready['pid']) != {
                            'pid': ready['pid'], 'starttime_ticks': ready['starttime_ticks']} \
                        or current_cgroup(ready['pid']) != self.owner['cgroup']:
                    raise RuntimeError('worker readiness identity mismatch')
                self.sample()
                atomic_json(self.output / 'job.json', self.owner)
                self.sample_memory_stop()
                if not self.cancel and not self.memory_stop_evidence and time.monotonic() < self.deadline:
                    atomic_json(self.output / 'go.json', ready)
                return
            if self.launcher.poll() is not None:
                raise RuntimeError('systemd launcher/worker exited before readiness')
            time.sleep(POLL)

    def observe(self):
        while True:
            self.sample()
            self.sample_memory_stop()
            outcome = read_json(self.output / 'command-finished.json')
            now = time.monotonic()
            # Event ordering is based on monotonic evidence, not numeric exits.
            finish = outcome['finished_monotonic'] if outcome else float('inf')
            cancel_time = self.cancel['observed_monotonic'] if self.cancel else float('inf')
            stop_time = (self.memory_stop_evidence['observed_monotonic']
                         if self.memory_stop_evidence else float('inf'))
            if cancel_time < min(finish, self.deadline, stop_time):
                return 'cancel', 128 + self.cancel['signal']
            if self.deadline <= min(finish, now, stop_time):
                return 'deadline', 124
            if stop_time < finish and stop_time <= now:
                return 'resource_failure', 125
            if outcome:
                if outcome['kind'] == 'spawn_failure':
                    return 'spawn_failure', 125
                if self.oom_evidence:
                    return 'resource_failure', 125
                code = outcome['returncode']
                return ('exit', code) if code >= 0 else ('signal', 128 - code)
            if self.launcher and self.launcher.poll() is not None:
                return ('resource_failure', 125) if self.oom_evidence else ('runner_failure', 125)
            if not self.launcher:
                raise RuntimeError('scope not started')
            time.sleep(POLL)

    def cleanup(self):
        result = {'complete': False, 'errors': [], 'cgroup_empty': None,
                  'launcher_reaped': self.launcher is None}
        atomic_json(self.output / 'abort.json', {'at': utc()})
        if self.launcher and not self.owner:
            try:
                self.acquire_owner()
            except (OSError, RuntimeError, TimeoutError) as error:
                result['errors'].append(str(error))
        if self.owner:
            try:
                self.sample()
                if not self.empty():
                    self.acquire_owner()  # Revalidate manager path + inode before signalling.
                    bounded_call([SYSTEMCTL, '--user', 'stop', '--no-block', self.unit], self.log)
                end = time.monotonic() + self.args.term_grace_seconds + .5
                while not self.empty() and time.monotonic() < end:
                    self.sample()
                    time.sleep(POLL)
                if not self.empty():
                    self.acquire_owner()
                    bounded_call([SYSTEMCTL, '--user', 'kill', '--kill-whom=all',
                                  '--signal=KILL', self.unit], self.log)
                end = time.monotonic() + 2
                while not self.empty() and time.monotonic() < end:
                    self.sample()
                    time.sleep(POLL)
                result['cgroup_empty'] = self.empty()
            except (OSError, RuntimeError, TimeoutError) as error:
                result['errors'].append(str(error))
            try:
                # No --collect: capture status before any failed-unit reset/GC.
                state = self.show()
                if state.get('Id') != self.unit or state.get('InvocationID') != self.owner['invocation_id']:
                    raise RuntimeError('manager terminal evidence identity changed/unavailable')
                self.manager = state
                self.manager_observed_at = utc()
                if self.manager.get('Result') == 'oom-kill':
                    self.oom_evidence = {'source': 'systemd owned scope Result=oom-kill',
                                         'quality': 'measured', 'value': True}
            except (OSError, RuntimeError, TimeoutError) as error:
                result['status_evidence_error'] = str(error)
        if self.launcher:
            try:
                self.launcher.wait(timeout=.5)
            except subprocess.TimeoutExpired:
                self.launcher.kill()  # Exact Popen handle, never a discovered/shared PID.
                try:
                    self.launcher.wait(timeout=.5)
                except subprocess.TimeoutExpired:
                    result['errors'].append('launcher did not exit after direct kill')
            result['launcher_reaped'] = self.launcher.returncode is not None
            result['launcher_returncode'] = self.launcher.returncode
        # Only reap our adopted children after all direct Popen waits are done.
        end = time.monotonic() + 1
        while time.monotonic() < end:
            try:
                pid, _status = os.waitpid(-1, os.WNOHANG)
                if pid == 0:
                    time.sleep(POLL)
            except ChildProcessError:
                break
        for stream in self.launcher_files:
            stream.close()
        if self.launcher is None:
            result['complete'] = True  # No manager launch was attempted.
        else:
            result['complete'] = result['cgroup_empty'] is True and result['launcher_reaped']
        return result

    def manager_receipt(self):
        if self.manager is None:
            return {'quality': 'unavailable', 'reason': 'no verified manager observation'}
        accounting = {}
        for name in ('CPUUsageNSec', 'MemoryPeak', 'MemoryCurrent'):
            value = self.manager.get(name, '')
            if not value.isdigit() or int(value) == (1 << 64) - 1:
                accounting[name] = {'quality': 'unavailable', 'raw': value}
            else:
                accounting[name] = {'quality': 'measured', 'value': int(value)}
        return {'quality': 'last_observed', 'observed_at': self.manager_observed_at,
                'properties': self.manager, 'resource_fields': accounting}

    def run(self):
        classification, status, error = 'runner_failure', 125, None
        try:
            self.start_scope()
            classification, status = self.observe()
        except (OSError, KeyError, ValueError, RuntimeError, TimeoutError) as failure:
            error = str(failure)
            if self.cancel and self.cancel['observed_monotonic'] < self.deadline:
                classification, status = 'cancel', 128 + self.cancel['signal']
            elif time.monotonic() >= self.deadline:
                classification, status = 'deadline', 124
            else:
                classification, status = 'setup_failure', 125
        try:
            cleanup = self.cleanup()
        except (OSError, ValueError, RuntimeError, TimeoutError) as failure:
            cleanup = {'complete': False, 'errors': [str(failure)]}
        outcome = read_json(self.output / 'command-finished.json')
        if classification in ('exit', 'signal', 'runner_failure') and self.oom_evidence:
            classification, status = 'resource_failure', 125
        trigger = classification
        if not cleanup['complete']:
            classification, status = 'cleanup_failure', 125
        receipt = {'schema_version': 1, 'started_at': self.started['started_at'],
                   'finished_at': utc(), 'elapsed_seconds': time.monotonic() - self.start,
                   'classification': classification, 'trigger': trigger, 'runner_exit_code': status,
                   'error': error, 'cancel': self.cancel, 'job': self.owner, 'budget': self.budget,
                   'command': outcome or {'kind': 'unavailable',
                                          'reason': 'worker did not publish command completion'},
                   'resources': self.resources, 'oom_evidence': self.oom_evidence,
                   'memory_stop_evidence': self.memory_stop_evidence,
                   'manager_last_observed': self.manager_receipt(), 'cleanup': cleanup}
        atomic_json(self.output / 'finished.json', receipt)
        print(f"{classification}: {self.output / 'finished.json'}", file=sys.stderr)
        return status


def main():
    if len(sys.argv) == 3 and sys.argv[1] == '--_worker':
        return worker(Path(sys.argv[2]))
    parser = argparse.ArgumentParser(description=__doc__,
                                     formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument('--deadline-seconds', required=True, type=positive_seconds)
    parser.add_argument('--output-dir', required=True, help='exclusive NEW directory; parent must exist')
    parser.add_argument('--term-grace-seconds', default=2., type=positive_seconds)
    parser.add_argument('--memory-stop-bytes', type=positive_bytes,
                        help='stop this job when verified aggregate task-budget memory.current '
                             'reaches this value; must be below its finite memory.max')
    parser.add_argument('command', nargs=argparse.REMAINDER,
                        help='-- COMMAND [ARGS...]; optional existing heavy-run wrapper goes here')
    args = parser.parse_args()
    if args.command[:1] == ['--']:
        args.command.pop(0)
    if not args.command:
        parser.error('a command is required after --')
    try:
        job = Job(args)
    except (OSError, RuntimeError) as error:
        parser.exit(125, str(error) + '\n')
    return job.run()


if __name__ == '__main__':
    sys.exit(main())
