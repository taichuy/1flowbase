const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const { spawn } = require('node:child_process');
const { stopService } = require('../process.js');

const fixture = path.join(__dirname, 'fixtures', 'shutdown-service.js');
const pause = (ms) => new Promise((resolve) => setTimeout(resolve, ms));

function running(pid) {
  if (!pid) return false;
  try {
    process.kill(pid, 0);
    if (process.platform === 'linux') {
      const stat = fs.readFileSync(`/proc/${pid}/stat`, 'utf8');
      return !/\) Z /u.test(stat);
    }
    return true;
  } catch (error) {
    if (error.code === 'ESRCH' || error.code === 'ENOENT') return false;
    throw error;
  }
}

async function waitUntil(predicate, timeoutMs = 3000) {
  const end = Date.now() + timeoutMs;
  while (Date.now() < end) {
    if (predicate()) return true;
    await pause(20);
  }
  return predicate();
}

async function runServiceFixture(t, mode) {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'dev-up-shutdown-'));
  const leader = spawn(process.execPath, [fixture, 'parent', mode, root], {
    detached: true,
    stdio: 'ignore',
  });
  let childPid;
  t.after(() => {
    // The fixture owns an isolated group; this is a final safety net if an assertion fails.
    try { process.kill(-leader.pid, 'SIGKILL'); } catch (error) {
      if (error.code !== 'ESRCH') throw error;
    }
    if (childPid && running(childPid)) {
      try { process.kill(childPid, 'SIGKILL'); } catch (error) {
        if (error.code !== 'ESRCH') throw error;
      }
    }
    fs.rmSync(root, { recursive: true, force: true });
  });
  assert.equal(await waitUntil(() => fs.existsSync(`${root}/ready`)), true);
  assert.equal(await waitUntil(() => fs.existsSync(`${root}/child-ready`)), true);
  const pids = JSON.parse(fs.readFileSync(`${root}/ready`, 'utf8'));
  childPid = pids.child;
  assert.equal(pids.parent, leader.pid);
  assert.equal(running(childPid), true);
  const service = {
    label: 'fixture', pidFile: path.join(root, 'pid.json'),
    probeHost: '127.0.0.1', port: 0,
  };
  fs.writeFileSync(service.pidFile, JSON.stringify({ pid: leader.pid, ownerPid: leader.pid }));
  return { root, leader, childPid, service };
}

if (process.platform !== 'win32') {
  test('graceful stop gives the detached parent sole first TERM and lets it drain its child', async (t) => {
    const { root, leader, childPid, service } = await runServiceFixture(t, 'graceful');
    await stopService(service, { isPortOpenImpl: async () => false, logImpl() {} });
    assert.equal(running(leader.pid), false);
    assert.equal(running(childPid), false);
    assert.deepEqual(fs.readFileSync(`${root}/events`, 'utf8').trim().split('\n'), [
      'parent-term', 'child-graceful', 'parent-exit',
    ]);
    assert.equal(fs.existsSync(service.pidFile), false);
  });

  test('timeout kills the parent before cleaning residual owned children', async (t) => {
    const { root, leader, childPid, service } = await runServiceFixture(t, 'stubborn');
    const signals = [];
    await stopService(service, {
      isPortOpenImpl: async () => false, logImpl() {},
      signalPidImpl(pid, signal) {
        signals.push([pid, signal]);
        process.kill(pid, signal);
      },
      async waitForProcessExitImpl(pid, timeoutMs) {
        return waitUntil(() => !running(pid), timeoutMs === undefined ? 300 : timeoutMs);
      },
    });
    assert.deepEqual(signals, [[leader.pid, 'SIGTERM'], [leader.pid, 'SIGKILL']]);
    assert.equal(running(leader.pid), false);
    assert.equal(running(childPid), false);
    assert.deepEqual(fs.readFileSync(`${root}/events`, 'utf8').trim().split('\n'), [
      'parent-term', 'child-term',
    ]);
  });

  test('parent exit with a live child cleans the captured owned group', async (t) => {
    const { root, leader, childPid, service } = await runServiceFixture(t, 'orphan');
    await stopService(service, { isPortOpenImpl: async () => false, logImpl() {} });
    assert.equal(running(leader.pid), false);
    assert.equal(running(childPid), false);
    assert.deepEqual(fs.readFileSync(`${root}/events`, 'utf8').trim().split('\n'), [
      'parent-term', 'child-term',
    ]);
  });

  test('never signals the caller group and retains ownership if parent cannot be stopped', async () => {
    const root = fs.mkdtempSync(path.join(os.tmpdir(), 'dev-up-shutdown-safe-'));
    try {
      const service = { label: 'fixture', pidFile: path.join(root, 'pid.json'), probeHost: '127.0.0.1', port: 0 };
      fs.writeFileSync(service.pidFile, JSON.stringify({ pid: process.pid }));
      for (const groupId of [process.pid, process.pid + 100]) {
        const signals = [];
        await assert.rejects(stopService(service, {
          readProcessGroupIdImpl: () => groupId,
          signalPidImpl: (pid, signal) => signals.push([pid, signal]),
          waitForProcessExitImpl: async () => false,
          clearOwnedProcessGroupImpl: () => assert.fail('unowned group must not be cleaned'),
          isPortOpenImpl: async () => false, logImpl() {},
        }), /remained alive after SIGKILL/);
        assert.deepEqual(signals, [[process.pid, 'SIGTERM'], [process.pid, 'SIGKILL']]);
        assert.equal(fs.existsSync(service.pidFile), true);
      }
    } finally {
      fs.rmSync(root, { recursive: true, force: true });
    }
  });
}
