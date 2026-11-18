const test = require('node:test');
const assert = require('node:assert/strict');
const { stopOwnedBackend, readLinuxIdentity, canonicalPath } = require('../owned-backend.js');

const service = { cwd: '/repo/api', pidFile: '/repo/tmp/api.json', port: 7900 };
const owned = { pid: 31, startTicks: '123', exe: '/repo/api/target/debug/api-server', cwd: '/repo/api' };
const foreign = { pid: 42, startTicks: '456', exe: '/other/api/target/debug/api-server', cwd: '/other/api' };

function fixture({ record = { pid: 31, ownerPid: 999 }, identities = [owned], listeners = [31], force = false } = {}) {
  const live = new Map(identities.map((identity) => [identity.pid, { ...identity }]));
  const signals = [];
  let time = 0;
  let removed = false;
  const deps = {
    platform: 'linux',
    canonicalPathImpl: (value) => value,
    readPidRecordImpl: () => record,
    removePidRecordImpl: () => { removed = true; },
    readIdentityImpl: (pid) => live.get(pid) || null,
    listPortOccupantPidsImpl: () => listeners.filter((pid) => live.has(pid)),
    isPortOpenImpl: async () => listeners.some((pid) => live.has(pid)),
    signalPidImpl: (pid, signal) => {
      signals.push({ pid, signal });
      assert.ok(pid > 0, 'must never signal a process group');
      if (!force || signal === 'SIGKILL') live.delete(pid);
    },
    nowImpl: () => time,
    sleepImpl: async (ms) => { time += ms; },
    timeoutMs: 2, pollMs: 1,
  };
  return { deps, live, signals, removed: () => removed };
}

test('wrong configured port or live foreign PID aborts without any signal', async () => {
  for (const record of [null, { pid: 123 }, { pid: 42 }, { pid: 31 }]) {
    const f = fixture({ record, identities: [owned, foreign], listeners: [42] });
    await assert.rejects(stopOwnedBackend(service, f.deps), /ownership/u);
    assert.deepEqual(f.signals, []);
    assert.ok(f.live.has(42));
    assert.ok(f.live.has(31), 'validate all candidates before stopping any');
    assert.equal(f.removed(), false);
  }
});

test('owned API stops gracefully or forcibly and never signals ownerPid/group', async () => {
  for (const force of [false, true]) {
    const f = fixture({ force });
    await stopOwnedBackend(service, f.deps);
    assert.equal(f.live.has(31), false);
    assert.deepEqual(f.signals, [
      { pid: 31, signal: 'SIGTERM' },
      ...(force ? [{ pid: 31, signal: 'SIGKILL' }] : []),
    ]);
    assert.equal(f.removed(), true);
  }
});

test('no PID record can stop an independently proven worktree listener', async () => {
  const f = fixture({ record: null });
  await stopOwnedBackend(service, f.deps);
  assert.equal(f.live.size, 0);
  assert.deepEqual(f.signals, [{ pid: 31, signal: 'SIGTERM' }]);
});

test('absent and stale PID records with no listener need no signal', async () => {
  for (const record of [null, { pid: 123 }]) {
    const f = fixture({ record, identities: [], listeners: [] });
    await stopOwnedBackend(service, f.deps);
    assert.deepEqual(f.signals, []);
    assert.equal(f.removed(), true);
  }
});

test('PID reuse before TERM or KILL never signals replacement process', async () => {
  for (const changeBeforeKill of [false, true]) {
    const f = fixture({ force: true });
    let reads = 0;
    const original = f.deps.readIdentityImpl;
    f.deps.readIdentityImpl = (pid) => {
      reads += 1;
      if ((!changeBeforeKill && reads === 2) || (changeBeforeKill && f.signals.length > 0)) {
        f.live.set(pid, { ...foreign, pid, startTicks: '999' });
      }
      return original(pid);
    };
    await assert.rejects(stopOwnedBackend(service, f.deps), /identity changed/u);
    assert.deepEqual(f.signals, changeBeforeKill ? [{ pid: 31, signal: 'SIGTERM' }] : []);
    assert.equal(f.live.get(31).startTicks, '999');
    assert.equal(f.removed(), false);
  }
});

test('ambiguous listener inspection and still-running owned API fail closed', async () => {
  const unknown = fixture({ record: null, identities: [], listeners: [] });
  unknown.deps.isPortOpenImpl = async () => true;
  await assert.rejects(stopOwnedBackend(service, unknown.deps), /listener/u);
  assert.deepEqual(unknown.signals, []);
  const stuck = fixture({ force: true });
  stuck.deps.signalPidImpl = (pid, signal) => stuck.signals.push({ pid, signal });
  await assert.rejects(stopOwnedBackend(service, stuck.deps), /still running/u);
  assert.ok(stuck.live.has(31));
  assert.equal(stuck.removed(), false);
});

test('exact executable and cwd are both required, not similar path prefixes', async () => {
  for (const identity of [
    { ...owned, exe: '/repo/api/target/debug/api-server-foreign' },
    { ...owned, exe: '/repo-foreign/api/target/debug/api-server' },
    { ...owned, cwd: '/repo/api-foreign' },
  ]) {
    const f = fixture({ identities: [identity] });
    await assert.rejects(stopOwnedBackend(service, f.deps), /ownership/u);
    assert.deepEqual(f.signals, []);
    assert.ok(f.live.has(31));
  }
});

test('Linux reader normalizes deleted executable and parses start ticks after comm parentheses', () => {
  const stat = (ticks) => '31 (api ) server) S ' + Array(18).fill('0').join(' ') + ' ' + ticks;
  const files = { readFileSync: () => stat('123'), readlinkSync: (value) => value.endsWith('/exe') ? '/repo/api/target/debug/api-server (deleted)' : '/repo/api' };
  assert.deepEqual(readLinuxIdentity(31, { fsImpl: files, canonicalPathImpl: (value) => value }), owned);
  let reads = 0;
  files.readFileSync = () => stat(String(++reads));
  assert.throws(() => readLinuxIdentity(31, { fsImpl: files, canonicalPathImpl: (value) => value }), /identity changed/u);
});

test('canonical path supports missing/deleted executable via existing canonical parent', () => {
  const fsImpl = { realpathSync(value) {
    if (value === '/alias/api/target/debug/api-server') throw Object.assign(new Error('missing'), { code: 'ENOENT' });
    if (value === '/alias/api/target/debug') return '/repo/api/target/debug';
    throw new Error('unexpected path');
  } };
  assert.equal(canonicalPath('/alias/api/target/debug/api-server', fsImpl), owned.exe);
});

test('real cleanup entry preserves caches and foreign process on ownership rejection', async () => {
  const fs = require('node:fs');
  const os = require('node:os');
  const path = require('node:path');
  const { runBuildCacheCleanup, parseCliArgs } = require('../core.js');
  const repoRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'oneflowbase-foreign-fixture-'));
  const target = path.join(repoRoot, 'api', 'target');
  fs.mkdirSync(target, { recursive: true });
  fs.writeFileSync(path.join(target, 'sentinel'), 'keep');
  const f = fixture({ record: { pid: 123 }, identities: [foreign], listeners: [42] });
  const removed = [];
  try {
    await assert.rejects(runBuildCacheCleanup({
      repoRoot,
      options: parseCliArgs(['backend']),
      stopBackendServicesImpl: () => stopOwnedBackend({ ...service, cwd: path.join(repoRoot, 'api') }, f.deps),
      removePathImpl: (value) => { removed.push(value); fs.rmSync(value, { recursive: true, force: true }); },
      writeStdout: () => {},
    }), /ownership/u);
    assert.deepEqual(f.signals, []);
    assert.deepEqual(removed, []);
    assert.deepEqual(f.live.get(42), foreign);
    assert.equal(fs.readFileSync(path.join(target, 'sentinel'), 'utf8'), 'keep');
  } finally {
    fs.rmSync(repoRoot, { recursive: true, force: true });
  }
});

test('non-Linux compatibility delegates existing lifecycle policy', async () => {
  let observed;
  await stopOwnedBackend(service, {
    platform: 'darwin',
    stopServiceImpl: async (value, options) => { observed = { value, platform: options.platform }; },
    readIdentityImpl: () => { throw new Error('Linux policy must not run'); },
  });
  assert.deepEqual(observed, { value: service, platform: 'darwin' });
});
