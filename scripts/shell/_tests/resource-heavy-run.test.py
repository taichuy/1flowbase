import importlib.util
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import time
import unittest

sys.dont_write_bytecode = True

SOURCE = Path(__file__).resolve().parents[1] / 'resource-heavy-run.py'
spec = importlib.util.spec_from_file_location('heavy', SOURCE)
heavy = importlib.util.module_from_spec(spec)
spec.loader.exec_module(heavy)


class AdmissionTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.root = Path(self.temp.name)
        self.processes = []
        self.flag = self.root / 'ready'
        self.flag.touch()
        self.runner = self.root / 'runner.py'
        self.runner.write_text(
            'import importlib.util,sys\nfrom pathlib import Path\n'
            f's=importlib.util.spec_from_file_location("heavy",{str(SOURCE)!r})\n'
            'm=importlib.util.module_from_spec(s);s.loader.exec_module(m)\n'
            f'm.ready=lambda: Path({str(self.flag)!r}).exists()\n'
            'sys.exit(m.main())\n')

    def tearDown(self):
        for process in self.processes:
            if process.poll() is None:
                process.terminate()
            process.wait(timeout=5)
            process.stderr.close()
        self.temp.cleanup()

    def start(self, code):
        environment = dict(os.environ, XDG_RUNTIME_DIR=str(self.root), PYTHONDONTWRITEBYTECODE='1')
        environment.pop('DEV_HEAVY_GATE_PID', None)
        process = subprocess.Popen([sys.executable, str(self.runner), sys.executable, '-c', code],
                                   env=environment, stderr=subprocess.PIPE, text=True)
        self.processes.append(process)
        return process

    def wait_file(self, path):
        deadline = time.monotonic() + 5
        while not path.exists() and time.monotonic() < deadline:
            time.sleep(.02)
        self.assertTrue(path.exists())

    def test_low_memory_waits_then_runs(self):
        self.flag.unlink()
        marker = self.root / 'started'
        process = self.start(f'from pathlib import Path;Path({str(marker)!r}).touch()')
        self.assertIn('Waiting', process.stderr.readline())
        self.assertFalse(marker.exists())
        self.flag.touch()
        self.assertEqual(process.wait(timeout=5), 0)
        self.assertTrue(marker.exists())

    def test_serializes_and_cancelling_waiter_preserves_running_job(self):
        marker, release = self.root / 'first', self.root / 'release'
        first = self.start(f'from pathlib import Path\nimport time\nPath({str(marker)!r}).touch()\n'
                           f'while not Path({str(release)!r}).exists(): time.sleep(.02)')
        self.wait_file(marker)
        second_marker = self.root / 'second'
        second = self.start(f'from pathlib import Path;Path({str(second_marker)!r}).touch()')
        self.assertIn('Waiting', second.stderr.readline())
        self.assertFalse(second_marker.exists())
        second.terminate()
        self.assertEqual(second.wait(timeout=5), 143)
        self.assertIsNone(first.poll())
        release.touch()
        self.assertEqual(first.wait(timeout=5), 0)
        third = self.start('pass')
        self.assertEqual(third.wait(timeout=5), 0)

    def test_nested_job_does_not_deadlock_and_preserves_exit_status(self):
        process = self.start(f'import subprocess,sys;sys.exit(subprocess.call('
                             f'[{sys.executable!r},{str(SOURCE)!r},{sys.executable!r},"-c","raise SystemExit(7)"]))')
        self.assertEqual(process.wait(timeout=5), 7)

    def test_cancelling_active_job_releases_slot(self):
        marker = self.root / 'started'
        process = self.start(f'from pathlib import Path;import time;Path({str(marker)!r}).touch();time.sleep(30)')
        self.wait_file(marker)
        process.terminate()
        self.assertEqual(process.wait(timeout=5), 143)
        self.assertEqual(self.start('pass').wait(timeout=5), 0)

    def test_real_admission_predicate_ignores_swap_and_checks_parent(self):
        mem = self.root / 'meminfo'
        group = self.root / 'group'
        group.mkdir()
        (group / 'memory.high').write_text('100')
        (group / 'memory.current').write_text('99')
        mem.write_text('MemTotal: 1000 kB\nMemAvailable: 249 kB\nSwapFree: 0 kB\n')
        self.assertFalse(heavy.ready(mem, group))
        mem.write_text('MemTotal: 1000 kB\nMemAvailable: 250 kB\nSwapFree: 0 kB\n')
        self.assertTrue(heavy.ready(mem, group))
        (group / 'memory.current').write_text('100')
        self.assertFalse(heavy.ready(mem, group))


if __name__ == '__main__':
    unittest.main()
