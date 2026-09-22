#!/usr/bin/env python3
"""Serialize heavy jobs per user; wait for memory headroom without killing jobs."""
import fcntl
import os
from pathlib import Path
import signal
import subprocess
import sys
import time


def has_gate_ancestor():
    owner = os.environ.get('DEV_HEAVY_GATE_PID')
    pid = os.getppid()
    while owner and pid > 1:
        if str(pid) == owner:
            return True
        try:
            pid = int(Path(f'/proc/{pid}/stat').read_text().rsplit(')', 1)[1].split()[1])
        except (OSError, ValueError):
            return False
    return False


def ready(meminfo=Path('/proc/meminfo'), group=None):
    memory = {line.split(':')[0]: int(line.split()[1]) for line in meminfo.read_text().splitlines()}
    if memory['MemAvailable'] * 100 < memory['MemTotal'] * 25:
        return False
    group = group or Path(f'/sys/fs/cgroup/user.slice/user-{os.getuid()}.slice/user@{os.getuid()}.service/dev.slice')
    # An inactive slice has no cgroup yet. Active slices must be readable.
    if group.exists():
        high = (group / 'memory.high').read_text().strip()
        if high != 'max' and int((group / 'memory.current').read_text()) >= int(high):
            return False
    return True


def main():
    command = sys.argv[1:]
    if not command:
        raise SystemExit('Usage: dev-heavy-run COMMAND [ARGS...]')
    if has_gate_ancestor():
        os.execvp(command[0], command)
    runtime = Path(os.environ.get('XDG_RUNTIME_DIR', f'/run/user/{os.getuid()}'))
    cancelled = 0

    def cancel(signum, _frame):
        # Never call Popen methods from a handler: wait() may hold its lock.
        nonlocal cancelled
        cancelled = signum

    for sig in (signal.SIGTERM, signal.SIGINT, signal.SIGHUP):
        signal.signal(sig, cancel)
    with (runtime / '1flowbase-heavy.lock').open('a') as lock:
        reported = False
        while True:
            if cancelled:
                return 128 + cancelled
            try:
                fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
                acquired = True
            except BlockingIOError:
                acquired = False
            if acquired and ready():
                break
            if acquired:
                fcntl.flock(lock, fcntl.LOCK_UN)
            if not reported:
                print('Waiting for heavy-job slot and memory headroom (available >=25%, dev below MemoryHigh). Ctrl-C cancels.', file=sys.stderr, flush=True)
                reported = True
            time.sleep(2)
        environment = dict(os.environ, DEV_HEAVY_GATE_PID=str(os.getpid()))
        child = subprocess.Popen(command, env=environment)
        while child.poll() is None:
            if cancelled:
                child.send_signal(cancelled)
                try:
                    child.wait(timeout=5)
                except subprocess.TimeoutExpired:
                    child.kill()
                    child.wait()
                return 128 + cancelled
            time.sleep(.1)
        code = child.returncode
        return code if code >= 0 else 128 - code


if __name__ == '__main__':
    sys.exit(main())
