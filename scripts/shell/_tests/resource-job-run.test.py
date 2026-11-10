#!/usr/bin/env python3
"""Finite T1-T5 real-OS fixtures; explicitly opt in inside a fresh task budget.

Discovery (does not launch jobs): python3 scripts/shell/_tests/resource-job-run.test.py --list
Batch (Root/fresh QA only):
  RESOURCE_JOB_RUN_OS_TESTS=1 timeout --signal=TERM --kill-after=10s 240s \
    python3 scripts/shell/_tests/resource-job-run.test.py -v

Inherit real ONEFLOWBASE_TASK_BUDGET_CGROUP/ID for an active finite user slice.
Artifacts persist in task-owned tmp/test-governance/resource-job-run/<uuid>/.
No Cargo, services, production admission lock, or host/parent memory pressure.
"""
import errno
import fcntl
import hashlib
import json
import os
from pathlib import Path
import runpy
import signal
import subprocess
import sys
import time
import unittest
import uuid
from unittest.mock import patch


SOURCE = Path(__file__).resolve().parents[1] / 'resource-job-run.py'
HEAVY = SOURCE.with_name('resource-heavy-run.py')
SELF = Path(__file__).resolve()
CGROOT = Path('/sys/fs/cgroup')
PYTHON = sys.executable
TIMEOUT = 25
GRACE = .3


def load(path):
    return json.loads(path.read_text())


def proc_identity(pid=None):
    pid = pid or os.getpid()
    fields = Path(f'/proc/{pid}/stat').read_text().rsplit(')', 1)[1].split()
    group = next(line[3:] for line in Path(f'/proc/{pid}/cgroup').read_text().splitlines()
                 if line.startswith('0::'))
    return {'pid': pid, 'starttime_ticks': int(fields[19]), 'cgroup': group,
            'session': os.getsid(pid)}


def publish(path):
    value = proc_identity()
    temporary = path.with_suffix('.partial')
    temporary.write_text(json.dumps(value))
    temporary.replace(path)


def fixture_wait(predicate, seconds=18):
    end = time.monotonic() + seconds
    while time.monotonic() < end:
        if predicate():
            return
        time.sleep(.02)
    raise RuntimeError('fixture exceeded its finite wait')


def fixture(mode, root):
    """Real processes, signals, setsid and kernel OOM; never mocked proof."""
    if mode == 'tree-leaf':
        signal.signal(signal.SIGTERM, signal.SIG_IGN)
        publish(root / 'setsid-ready.json')
        time.sleep(20)
    elif mode == 'tree-child':
        signal.signal(signal.SIGTERM, signal.SIG_IGN)
        subprocess.Popen([PYTHON, str(SELF), '--_fixture', 'tree-leaf', str(root)],
                         start_new_session=True)
        fixture_wait(lambda: (root / 'setsid-ready.json').exists())
        publish(root / 'child-ready.json')
        time.sleep(20)
    elif mode in ('forest', 'forest-exit', 'cooperative'):
        if mode == 'cooperative':
            signal.signal(signal.SIGTERM, lambda _sig, _frame: sys.exit(0))
        subprocess.Popen([PYTHON, str(SELF), '--_fixture', 'tree-child', str(root)])
        fixture_wait(lambda: (root / 'child-ready.json').exists())
        publish(root / 'principal-ready.json')
        if mode == 'forest-exit':
            fixture_wait(lambda: (root / 'release').exists())
            return
        time.sleep(20)
    elif mode == 'sentinel':
        publish(root / 'sentinel-ready.json')
        time.sleep(220)
    elif mode == 'lock-holder':
        with (root / 'runtime' / '1flowbase-heavy.lock').open('a') as lock:
            fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
            publish(root / 'holder-ready.json')
            fixture_wait(lambda: (root / 'release-holder').exists())
    elif mode == 'hold':
        publish(root / 'principal-ready.json')
        fixture_wait(lambda: (root / 'release').exists())
    elif mode == 'oom':
        os.write(1, b'before-oom-out\n')
        os.write(2, b'before-oom-err\n')
        publish(root / 'principal-ready.json')
        fixture_wait(lambda: (root / 'release').exists())
        group = CGROOT / proc_identity()['cgroup'].lstrip('/')
        # Fail before allocation unless QA set only this owned leaf's tiny cap.
        if (group / 'memory.max').read_text().strip() != '67108864' \
                or (group / 'memory.swap.max').read_text().strip() != '0':
            raise RuntimeError('OOM fixture requires verified own 64MiB/no-swap limit')
        blocks = []
        for _ in range(32):
            blocks.append(bytearray(4 * 1024 * 1024))
        raise RuntimeError('kernel OOM fixture unexpectedly allocated 128MiB')
    else:
        raise RuntimeError('unknown fixture: ' + mode)


class RealJobTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        if os.environ.get('RESOURCE_JOB_RUN_OS_TESTS') != '1':
            raise unittest.SkipTest('explicit RESOURCE_JOB_RUN_OS_TESTS=1 required')
        if sys.platform != 'linux' or not (CGROOT / 'cgroup.controllers').exists():
            raise unittest.SkipTest('real Linux cgroup v2 unavailable')
        if not all(os.environ.get(key) for key in
                   ('ONEFLOWBASE_TASK_BUDGET_CGROUP', 'ONEFLOWBASE_TASK_BUDGET_ID')):
            raise unittest.SkipTest('fresh finite task-budget declaration required')
        # Do not turn an invalid supplied budget into a capability skip.
        if not os.environ['ONEFLOWBASE_TASK_BUDGET_CGROUP'].endswith('.slice'):
            raise unittest.SkipTest('native job scope needs an active user-slice budget')
        cls.suite_root = SOURCE.parents[2] / 'tmp/test-governance/resource-job-run' / uuid.uuid4().hex
        cls.suite_root.mkdir(parents=True, mode=0o700)
        print('real OS artifacts: ' + str(cls.suite_root), flush=True)
        try:
            result = subprocess.run(['/usr/bin/systemctl', '--user', 'show',
                                     '--property=ControlGroup'], capture_output=True,
                                    timeout=3, text=True, check=True)
        except (OSError, subprocess.SubprocessError) as error:
            raise unittest.SkipTest('user systemd unavailable: ' + str(error))
        (cls.suite_root / 'manager-discovery.log').write_text(result.stdout + result.stderr)

    def setUp(self):
        self.root = self.suite_root / self._testMethodName
        self.root.mkdir(mode=0o700)
        self.processes = []
        self.logs = []
        self.jobs = []
        self.addCleanup(self.cleanup)
        self.sentinel = self.start_process(self.fixture_argv('sentinel'), 'sentinel')
        self.sentinel_identity = self.wait_json(self.root / 'sentinel-ready.json')
        budget = os.environ['ONEFLOWBASE_TASK_BUDGET_CGROUP']
        self.assertTrue(self.sentinel_identity['cgroup'] == budget or
                        self.sentinel_identity['cgroup'].startswith(budget + '/'))

    def cleanup(self):
        failures = []
        # Signal only controller handles we created. Their owned scope cleanup
        # must work; exact cgroup/inode-validated fallback below is fixture hygiene.
        for process in reversed(self.processes):
            if process.poll() is None:
                process.terminate()
            try:
                process.wait(timeout=12)
            except subprocess.TimeoutExpired:
                process.kill()
                try:
                    process.wait(timeout=1)
                except subprocess.TimeoutExpired:
                    failures.append('owned process did not reap after kill')
        for output in self.jobs:
            owner_file = output / 'job.json'
            if not owner_file.exists():
                continue
            owner = load(owner_file)
            group = CGROOT / owner['cgroup'].lstrip('/')
            if not group.exists():
                continue
            try:
                events = dict(line.split() for line in (group / 'cgroup.events').read_text().splitlines())
                if events.get('populated') == '1':
                    # Own unique job only, never a budget/shared parent.
                    self.assertEqual(group.stat().st_ino, owner['inode'])
                    self.assertEqual(group.name, owner['unit'])
                    self.assertTrue(group.name.startswith('oneflowbase-job-'))
                    self.assertEqual(str(group.parent), str(CGROOT) +
                                     os.environ['ONEFLOWBASE_TASK_BUDGET_CGROUP'])
                    self.manager(['kill', '--kill-whom=all', '--signal=KILL', owner['unit']])
                    self.manager(['stop', '--no-block', owner['unit']])
                    failures.append('fixture fallback cleanup needed: ' + owner['unit'])
            except (OSError, subprocess.SubprocessError, AssertionError) as error:
                failures.append(str(error))
        for stream in self.logs:
            stream.close()
        if failures:
            self.fail('; '.join(failures))

    def fixture_argv(self, mode):
        return [PYTHON, str(SELF), '--_fixture', mode, str(self.root)]

    def start_process(self, argv, name, env=None):
        stream = (self.root / (name + '.log')).open('wb')
        self.logs.append(stream)
        process = subprocess.Popen(argv, stdin=subprocess.DEVNULL, stdout=stream, stderr=stream,
                                   env=env, start_new_session=True)
        self.processes.append(process)
        return process

    def start_job(self, command, name='job', deadline=10, env=None):
        output = self.root / name
        self.jobs.append(output)
        started = time.monotonic()
        process = self.start_process(
            [PYTHON, str(SOURCE), '--deadline-seconds', str(deadline), '--output-dir', str(output),
             '--term-grace-seconds', str(GRACE), '--', *command], name + '-controller', env)
        return process, output, started

    def wait_json(self, path, seconds=5):
        end = time.monotonic() + seconds
        while time.monotonic() < end:
            if path.exists():
                return load(path)
            time.sleep(.02)
        self.fail('ready/evidence marker missing within bound: ' + str(path))

    def finish(self, job, expected, code):
        process, output, start = job
        actual = process.wait(timeout=TIMEOUT)
        receipt = self.wait_json(output / 'finished.json', seconds=.5)
        self.assertEqual(actual, code, receipt)
        self.assertEqual(receipt['runner_exit_code'], code, receipt)
        self.assertEqual(receipt['classification'], expected, receipt)
        self.assertLess(time.monotonic() - start, TIMEOUT)
        self.assertTrue(receipt['cleanup']['complete'], receipt)
        self.assertTrue(receipt['started_at'])
        self.assertTrue(receipt['finished_at'])
        self.assert_sentinel()
        if receipt['job']:
            self.assert_empty(receipt['job'])
            started = load(output / 'started.json')
            self.assertNotEqual(started['controller_cgroup'], receipt['job']['cgroup'])
        return receipt

    def assert_empty(self, owner):
        group = CGROOT / owner['cgroup'].lstrip('/')
        if group.exists():
            self.assertEqual(group.stat().st_ino, owner['inode'])
            events = dict(line.split() for line in (group / 'cgroup.events').read_text().splitlines())
            self.assertEqual(events.get('populated'), '0')

    def assert_sentinel(self):
        self.assertIsNone(self.sentinel.poll(), 'unrelated same-budget sentinel died')
        self.assertEqual(proc_identity(self.sentinel.pid), self.sentinel_identity)

    def assert_gone(self, identity):
        end = time.monotonic() + 2
        while time.monotonic() < end:
            try:
                current = proc_identity(identity['pid'])
                if current['starttime_ticks'] != identity['starttime_ticks']:
                    return
            except FileNotFoundError:
                return
            time.sleep(.02)
        self.fail('owned PID+starttime still exists: ' + repr(identity))

    def forest_ready(self):
        principal = self.wait_json(self.root / 'principal-ready.json')
        child = self.wait_json(self.root / 'child-ready.json')
        leaf = self.wait_json(self.root / 'setsid-ready.json')
        self.assertEqual(leaf['session'], leaf['pid'], 'setsid grandchild was not created')
        self.assertEqual({item['cgroup'] for item in (principal, child, leaf)}, {principal['cgroup']})
        self.assertNotEqual(principal['cgroup'], self.sentinel_identity['cgroup'])
        return principal, child, leaf

    def manager(self, argv):
        return subprocess.run(['/usr/bin/systemctl', '--user', *argv], capture_output=True,
                              timeout=3, text=True, check=True)

    def test_t1_cgroup_removal_read_race_preserves_owned_identity(self):
        job_type = runpy.run_path(str(SOURCE))['Job']
        job = object.__new__(job_type)
        group = CGROOT / proc_identity()['cgroup'].lstrip('/') / ('empty-race-' + uuid.uuid4().hex)
        original_read = Path.read_text
        for outcome in ('removed', 'replaced', 'read-replaced', errno.ENODEV, errno.ENOENT, errno.EACCES, errno.EIO):
            with self.subTest(outcome=outcome):
                group.mkdir()
                job.owner = {'cgroup': '/' + str(group.relative_to(CGROOT)), 'inode': group.stat().st_ino}
                def raced_read(path, *args, **kwargs):
                    if path != group / 'cgroup.events':
                        return original_read(path, *args, **kwargs)
                    if isinstance(outcome, int):
                        raise OSError(outcome, 'injected while the owned directory still exists')
                    with path.open() as stream:
                        before = stream.read() if outcome == 'read-replaced' else None
                        group.rmdir()
                        if 'replaced' in outcome:
                            group.mkdir()
                        return before if before is not None else stream.read()
                try:
                    with patch.object(Path, 'read_text', raced_read):
                        if isinstance(outcome, int):
                            with self.assertRaises(OSError):
                                job.empty()
                        elif 'replaced' in outcome:
                            with self.assertRaisesRegex(RuntimeError, 'inode'):
                                job.empty()
                        else:
                            self.assertTrue(job.empty())
                finally:
                    if group.exists():
                        group.rmdir()

    def test_t1_natural_exit_raw_logs_and_exclusive_receipts(self):
        for code in (0, 7, 124):
            with self.subTest(code=code):
                env = dict(os.environ, RESOURCE_JOB_FIXTURE_VALUE='literal $HOME ${not_expanded}')
                script = ('import os,sys;os.write(1,b"raw-out\\x00\\n");'
                          'os.write(2,b"raw-err\\xff\\n");'
                          'assert os.environ["RESOURCE_JOB_FIXTURE_VALUE"] == '
                          '"literal $HOME ${not_expanded}";'
                          f'assert os.getcwd()=={os.getcwd()!r};sys.exit({code})')
                command = [PYTHON, '-c', script]
                job = self.start_job(command, name='exit-' + str(code), env=env)
                receipt = self.finish(job, 'exit', code)
                self.assertEqual(receipt['command']['returncode'], code)
                self.assertEqual(receipt['command']['exit_code'], code)
                self.assertEqual(receipt['command']['wait_status'], code << 8)
                self.assertIsNone(receipt['oom_evidence'])
                output = job[1]
                self.assertEqual((output / 'stdout.log').read_bytes(), b'raw-out\x00\n')
                self.assertEqual((output / 'stderr.log').read_bytes(), b'raw-err\xff\n')
                self.assertEqual(load(output / 'started.json')['argv'], command)
                before = hashlib.sha256((output / 'finished.json').read_bytes()).hexdigest()
                second = self.start_process([PYTHON, str(SOURCE), '--deadline-seconds', '2',
                                             '--output-dir', str(output), '--', PYTHON, '-c',
                                             'raise RuntimeError("must not start")'],
                                            'exclusive-' + str(code))
                self.assertEqual(second.wait(timeout=3), 125)
                self.assertEqual(hashlib.sha256((output / 'finished.json').read_bytes()).hexdigest(), before)

    def test_t1_spawn_and_stale_or_foreign_budget(self):
        missing = self.start_job([str(self.root / 'missing-executable')], name='missing')
        receipt = self.finish(missing, 'spawn_failure', 125)
        self.assertEqual(receipt['command']['kind'], 'spawn_failure')
        self.assertEqual(receipt['command']['errno'], 2)
        self.assertIsNone(receipt['command']['returncode'])
        holder = self.start_job(self.fixture_argv('hold'), name='foreign-holder', deadline=16)
        foreign = self.wait_json(holder[1] / 'job.json')
        self.wait_json(self.root / 'principal-ready.json')
        for label, changes in (
                ('stale', {'ONEFLOWBASE_TASK_BUDGET_ID':
                           str(int(os.environ['ONEFLOWBASE_TASK_BUDGET_ID']) + 1)}),
                ('foreign', {'ONEFLOWBASE_TASK_BUDGET_CGROUP': foreign['cgroup'],
                             'ONEFLOWBASE_TASK_BUDGET_ID': str(foreign['inode'])})):
            with self.subTest(context=label):
                marker = self.root / ('must-not-start-' + label)
                command = [PYTHON, '-c', f'from pathlib import Path;Path({str(marker)!r}).touch()']
                job = self.start_job(command, name=label, env=dict(os.environ, **changes))
                receipt = self.finish(job, 'setup_failure', 125)
                self.assertFalse(marker.exists())
                self.assertFalse((job[1] / 'command-started.json').exists())
                self.assertFalse((job[1] / 'worker-ready.json').exists())
                self.assertIsNone(receipt['job'])
                self.assertIsNone(holder[0].poll(), 'foreign holder was affected')
        (self.root / 'release').touch()
        self.finish(holder, 'exit', 0)

    def test_t1_rejects_nonpositive_or_nonfinite_deadline(self):
        for value in ('0', '-1', 'nan', 'inf'):
            with self.subTest(deadline=value):
                marker = self.root / ('invalid-' + value)
                process = self.start_process([PYTHON, str(SOURCE), '--deadline-seconds', value,
                                              '--output-dir', str(marker), '--', PYTHON, '-c', 'pass'],
                                             'deadline-' + value)
                self.assertEqual(process.wait(timeout=3), 2)
                self.assertFalse(marker.exists())

    def test_t2_deadline_drains_term_ignoring_setsid_descendants(self):
        job = self.start_job(self.fixture_argv('forest'), deadline=3)
        identities = self.forest_ready()
        receipt = self.finish(job, 'deadline', 124)
        self.assertGreaterEqual(receipt['elapsed_seconds'], 2.9)
        self.assertLess(receipt['elapsed_seconds'], 15)
        for identity in identities:
            self.assert_gone(identity)

    def test_t2_cooperative_zero_after_deadline_is_still_deadline(self):
        job = self.start_job(self.fixture_argv('cooperative'), deadline=3)
        identities = self.forest_ready()
        receipt = self.finish(job, 'deadline', 124)
        self.assertEqual(receipt['command']['returncode'], 0, receipt)
        for identity in identities:
            self.assert_gone(identity)

    def test_t3_cancel_term_and_int_drains_only_owned_scope(self):
        for sig in (signal.SIGTERM, signal.SIGINT):
            with self.subTest(signal=sig):
                for marker in ('principal-ready.json', 'child-ready.json', 'setsid-ready.json'):
                    (self.root / marker).unlink(missing_ok=True)
                job = self.start_job(self.fixture_argv('forest'), name='cancel-' + str(sig), deadline=12)
                identities = self.forest_ready()
                cancelled = time.monotonic()
                job[0].send_signal(sig)
                receipt = self.finish(job, 'cancel', 128 + sig)
                self.assertEqual(receipt['cancel']['signal'], sig)
                self.assertLess(time.monotonic() - cancelled, 10)
                for identity in identities:
                    self.assert_gone(identity)

    def test_t3_principal_success_drains_background_children(self):
        job = self.start_job(self.fixture_argv('forest-exit'))
        identities = self.forest_ready()
        (self.root / 'release').touch()
        receipt = self.finish(job, 'exit', 0)
        self.assertEqual(receipt['command']['returncode'], 0)
        for identity in identities:
            self.assert_gone(identity)

    def test_t3_controller_sigkill_native_deadline_still_drains(self):
        job = self.start_job(self.fixture_argv('forest'), deadline=3)
        self.forest_ready()
        owner = self.wait_json(job[1] / 'job.json')
        job[0].kill()  # Our exact controller handle, outside the job cgroup.
        self.assertEqual(job[0].wait(timeout=2), -signal.SIGKILL)
        group = CGROOT / owner['cgroup'].lstrip('/')

        def empty():
            if not group.exists():
                return True
            self.assertEqual(group.stat().st_ino, owner['inode'])
            events = dict(line.split() for line in (group / 'cgroup.events').read_text().splitlines())
            return events.get('populated') == '0'

        fixture_wait(empty, seconds=10)
        self.assertLess(time.monotonic() - job[2], 14)
        self.assertFalse((job[1] / 'finished.json').exists(),
                         'SIGKILL has no terminal receipt guarantee')
        self.assert_sentinel()
        # The dead controller cannot reap adopted orphans. Kernel cgroup
        # emptiness proves cleanup here; PID reaping is asserted on surviving-
        # controller paths above, not promised after controller SIGKILL.

    def test_t4_existing_admission_wait_is_inside_deadline(self):
        runtime = self.root / 'runtime'
        runtime.mkdir()
        holder = self.start_process(self.fixture_argv('lock-holder'), 'holder')
        identity = self.wait_json(self.root / 'holder-ready.json')
        marker = self.root / 'must-not-start'
        environment = dict(os.environ)
        environment.pop('DEV_HEAVY_GATE_PID', None)
        # Isolate only the wrapper's lock; systemctl must keep its real runtime bus.
        job = self.start_job(['/usr/bin/env', f'XDG_RUNTIME_DIR={runtime}',
                              PYTHON, str(HEAVY), PYTHON, '-c',
                              f'from pathlib import Path;Path({str(marker)!r}).touch()'],
                             deadline=3, env=environment)
        # The actual wrapper must enter its real flock wait, not a mocked gate.
        fixture_wait(lambda: (job[1] / 'stderr.log').exists() and
                     b'Waiting for heavy-job slot' in (job[1] / 'stderr.log').read_bytes(), seconds=4)
        receipt = self.finish(job, 'deadline', 124)
        self.assertLess(receipt['elapsed_seconds'], 15)
        self.assertFalse(marker.exists())
        self.assertIsNone(holder.poll())
        self.assertEqual(proc_identity(holder.pid), identity)
        (self.root / 'release-holder').touch()
        self.assertEqual(holder.wait(timeout=3), 0)

    def test_t5_manual_sigkill_is_not_oom(self):
        job = self.start_job([PYTHON, '-c',
                              'import os,signal;os.write(1,b"before-kill\\n");'
                              'os.kill(os.getpid(),signal.SIGKILL)'])
        receipt = self.finish(job, 'signal', 137)
        self.assertEqual(receipt['command']['returncode'], -signal.SIGKILL)
        self.assertEqual(receipt['command']['signal'], signal.SIGKILL)
        self.assertIsNone(receipt['oom_evidence'])
        self.assertEqual((job[1] / 'stdout.log').read_bytes(), b'before-kill\n')

    def test_t5_own_64mib_child_oom_keeps_controller_and_receipt(self):
        job = self.start_job(self.fixture_argv('oom'), deadline=12)
        owner = self.wait_json(job[1] / 'job.json')
        principal = self.wait_json(self.root / 'principal-ready.json')
        group = CGROOT / owner['cgroup'].lstrip('/')
        self.assertEqual(group.stat().st_ino, owner['inode'])
        self.assertEqual(group.name, owner['unit'])
        self.assertEqual(principal['cgroup'], owner['cgroup'])
        self.assertEqual(group.parent, CGROOT / os.environ['ONEFLOWBASE_TASK_BUDGET_CGROUP'].lstrip('/'))
        budget = group.parent
        before = {name: (budget / name).read_text() for name in ('cpu.max', 'memory.max', 'memory.swap.max')}
        try:
            self.manager(['set-property', '--runtime', owner['unit'],
                          'MemoryMax=67108864', 'MemorySwapMax=0'])
        except (OSError, subprocess.SubprocessError) as error:
            self.skipTest('own scope tiny memory cap unsupported; no allocation started: ' + str(error))
        self.assertEqual((group / 'memory.max').read_text().strip(), '67108864')
        self.assertEqual((group / 'memory.swap.max').read_text().strip(), '0')
        self.assertEqual((group / 'memory.oom.group').read_text().strip(), '0')
        (self.root / 'release').touch()
        receipt = self.finish(job, 'resource_failure', 125)
        self.assertEqual(receipt['command']['returncode'], -signal.SIGKILL, receipt)
        self.assertIsNotNone(receipt['oom_evidence'], receipt)
        self.assertEqual(receipt['resources']['quality'], 'last_observed')
        # Kernel counters can disappear with the emptied leaf. The failed native
        # scope's OOM result is independent proof; never invent terminal counters.
        events = receipt['resources']['fields']['memory.events'].get('value', {})
        self.assertTrue(events.get('oom_kill', 0) >= 1 or
                        receipt['manager_last_observed'].get('properties', {}).get('Result') == 'oom-kill', receipt)
        self.assertEqual((job[1] / 'stdout.log').read_bytes(), b'before-oom-out\n')
        self.assertEqual((job[1] / 'stderr.log').read_bytes(), b'before-oom-err\n')
        self.assertEqual({name: (budget / name).read_text() for name in before}, before)
        self.assert_gone(principal)


if __name__ == '__main__':
    if len(sys.argv) == 4 and sys.argv[1] == '--_fixture':
        fixture(sys.argv[2], Path(sys.argv[3]))
    elif sys.argv[1:] == ['--list']:
        for name in unittest.defaultTestLoader.getTestCaseNames(RealJobTests):
            print(name)
    else:
        unittest.main()
