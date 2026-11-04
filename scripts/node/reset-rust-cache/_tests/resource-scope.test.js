const test = require('node:test');
const assert = require('node:assert/strict');
const { EventEmitter } = require('node:events');
const { MARKER, budgets, verifyScope, pressureMonitor, drainScope, runInResourceScope } = require('../resource-scope.js');

const parent = '/user.slice/dev.slice/dev-rust.slice';
const unit = 'rust-cache-reset-123-aaaaaaaaaaaaaaaaaaaaaaaa.scope';
const owner = { unit, parent, cpu: 8, memory: 8 * 2 ** 30 };
function fixture({ code = 0, failure, pressure, hold = false } = {}) {
  const proc = new EventEmitter();
  proc.pid = 123; proc.execPath = '/usr/bin/node';
  const calls = [];
  let ticks;
  let closed = false;
  const child = new EventEmitter();
  child.kill = (signal) => { calls.push(['launcher-kill', signal]); if (!closed) { closed = true; child.emit('close', null, signal); } };
  const runtime = {
    process: proc, nonce: () => 'aaaaaaaaaaaaaaaaaaaaaaaa',
    os: { cpus: () => Array(16).fill({}), availableParallelism: () => 12, totalmem: () => 16 * 2 ** 30 },
    read: (file) => {
      if (file === '/proc/self/cgroup') return '0::' + parent;
      if (file === '/proc/meminfo') return 'MemTotal: 1000 kB\nMemAvailable: ' + (pressure === 'low' ? 90 : 500) + ' kB';
      if (file.endsWith('memory.pressure') || file === '/proc/pressure/memory') return 'some avg10=0.00 avg60=0.00\nfull avg10=' + (pressure === 'psi' ? '30.0' : '0.0') + ' avg60=0.0';
      if (failure === 'controller' && file.endsWith('memory.swap.max')) throw new Error('missing swap controller');
      if (file.endsWith('cpu.max')) return 'max 100000';
      if (file.endsWith('memory.max') || file.endsWith('memory.swap.max')) return 'max';
      if (file.endsWith('cgroup.events')) return 'populated 0';
      throw new Error('Unexpected fixture read ' + file);
    },
    systemctl: (args) => {
      calls.push(args);
      if (failure === 'systemd') throw new Error('user systemd unavailable');
      if (args[0] === 'show') return parent;
      return '';
    },
    spawn: (command, args, options) => {
      calls.push(['spawn', command, args, options]);
      queueMicrotask(() => {
        if (failure === 'spawn') child.emit('error', new Error('systemd-run missing'));
        else if (!hold && !pressure) { closed = true; child.emit('close', code, null); }
        else {
          const count = pressure === 'psi' ? 4 : 3;
          for (let i = 0; i < count; i++) ticks();
        }
      });
      return child;
    },
    setInterval: (fn) => { ticks = fn; return 4; },
    clearInterval: (id) => { calls.push(['clear', id]); },
  };
  return { runtime, calls, child, proc };
}
function run(f, extra = {}) {
  return runInResourceScope({
    argv: [], env: {}, platform: 'linux', write: () => {},
    run: () => assert.fail('parent must reexec before cleanup'), runtime: f.runtime, ...extra,
  });
}

test('half host budgets do not double halve available parallelism and honor lower explicit/ancestor caps', () => {
  const { runtime } = fixture();
  assert.deepEqual(budgets({ env: {}, runtime }), { cpu: 8, memory: 8 * 2 ** 30 });
  runtime.os.availableParallelism = () => 8;
  assert.equal(budgets({ env: {}, runtime }).cpu, 8);
  assert.deepEqual(budgets({ env: { ONEFLOWBASE_CARGO_JOB_LIMIT: '2' }, runtime, inherited: { cpu: 4, memory: 2 ** 30 } }),
    { cpu: 2, memory: 2 ** 30 });
  assert.throws(() => budgets({ env: { ONEFLOWBASE_CARGO_JOB_LIMIT: 'wat' }, runtime }), /positive integer/);
});

test('missing user systemd or controllers fails before cleanup or spawning', async () => {
  for (const failure of ['systemd', 'controller']) {
    const f = fixture({ failure });
    await assert.rejects(run(f), /unavailable|controller/);
    assert.equal(f.calls.some(([name]) => name === 'spawn'), false);
  }
});

test('forged internal marker and public cargo flag never authorize cleanup', async () => {
  const f = fixture();
  await assert.rejects(run(f, { env: { [MARKER]: JSON.stringify(owner), CARGO_MEMORY_BUDGET_ACTIVE: '1' } }), /actual cgroup/);
  assert.equal(f.calls.length, 0);
  const f2 = fixture({ failure: 'systemd' });
  await assert.rejects(run(f2, { env: { CARGO_MEMORY_BUDGET_ACTIVE: '1' } }), /unavailable/);
});

test('child verifies leaf and ancestor constraints before exposing the cargo containment flag', async () => {
  const f = fixture();
  const original = f.runtime.read;
  f.runtime.read = (file) => {
    if (file === '/proc/self/cgroup') return '0::' + parent + '/' + unit;
    if (file.includes('/' + unit + '/')) {
      if (file.endsWith('cpu.max')) return '800000 100000';
      if (file.endsWith('memory.max')) return String(owner.memory);
      if (file.endsWith('memory.swap.max')) return '0';
    }
    return original(file);
  };
  const env = { [MARKER]: JSON.stringify(owner) };
  assert.equal(await run(f, { env, run: () => { assert.equal(env.CARGO_MEMORY_BUDGET_ACTIVE, '1'); return 7; } }), 7);
  for (const suffix of ['cpu.max', 'memory.max', 'memory.swap.max']) {
    const valid = f.runtime.read;
    f.runtime.read = (file) => file.includes('/' + unit + '/') && file.endsWith(suffix)
      ? (suffix === 'cpu.max' ? 'max 100000' : 'max') : valid(file);
    assert.throws(() => verifyScope(env[MARKER], env, f.runtime), /verification/);
    f.runtime.read = valid;
  }
});

test('scope reexec preserves argv and lower cap, removes unverified public flag, drains success and genuine nonzero', async () => {
  for (const code of [0, 17]) {
    const f = fixture({ code });
    assert.equal(await run(f, { argv: ['--warmup-only'], env: { ONEFLOWBASE_CARGO_JOB_LIMIT: '2', CARGO_MEMORY_BUDGET_ACTIVE: '1' } }), code);
    const [, command, args, options] = f.calls.find(([name]) => name === 'spawn');
    assert.equal(command, '/usr/bin/systemd-run');
    assert.ok(args.includes('--property=CPUQuota=200%'));
    assert.ok(args.includes('--property=MemorySwapMax=0'));
    assert.equal(args.at(-1), '--warmup-only');
    assert.match(args.at(-2), /reset-rust-cache\/cli.js$/);
    assert.equal(options.env.CARGO_MEMORY_BUDGET_ACTIVE, undefined);
    assert.equal(options.env.ONEFLOWBASE_CARGO_JOB_LIMIT, '2');
    const stops = f.calls.filter(([name]) => name === 'kill' || name === 'stop');
    assert.ok(stops.length > 0);
    assert.ok(stops.every((args) => args.at(-1) === unit));
    assert.equal(f.proc.listenerCount('SIGINT'), 0);
    assert.equal(f.proc.listenerCount('SIGTERM'), 0);
  }
});

test('sustained host memory or PSI pressure cancels only owned scope and returns failure', async () => {
  for (const pressure of ['low', 'psi']) {
    const f = fixture({ pressure });
    assert.equal(await run(f), 1);
    assert.ok(f.calls.some(([name]) => name === 'launcher-kill'));
    assert.ok(f.calls.filter(([name]) => name === 'kill' || name === 'stop').every((args) => args.at(-1) === unit));
  }
});

test('launcher creation error still drains its unique scope', async () => {
  const f = fixture({ failure: 'spawn' });
  await assert.rejects(run(f), /systemd-run missing/);
  assert.ok(f.calls.some(([name]) => name === 'stop'));
});

test('SIGINT and SIGTERM preserve conventional exit codes and drain only the owned scope', async () => {
  for (const [signal, expected] of [['SIGINT', 130], ['SIGTERM', 143]]) {
    const f = fixture();
    f.runtime.spawn = () => { queueMicrotask(() => f.proc.emit(signal)); return f.child; };
    assert.equal(await run(f), expected);
    assert.ok(f.calls.filter(([name]) => name === 'kill' || name === 'stop').every((args) => args.at(-1) === unit));
  }
});

test('non-Linux and CI retain normal behavior without creating user scope', async () => {
  for (const extra of [{ platform: 'darwin' }, { env: { CI: 'true' } }]) {
    const f = fixture();
    assert.equal(await run(f, { ...extra, run: () => 9 }), 9);
    assert.equal(f.calls.length, 0);
  }
});

test('pressure requires consecutive samples and refuses unsupported monitoring', () => {
  let available = 500;
  const read = (file) => file === '/proc/meminfo'
    ? 'MemTotal: 1000 kB\nMemAvailable: ' + available + ' kB'
    : 'full avg10=0.0 avg60=0.0';
  const sample = pressureMonitor(read);
  available = 90; assert.equal(sample(), null); assert.equal(sample(), null);
  available = 500; assert.equal(sample(), null);
  available = 90; assert.equal(sample(), null); assert.equal(sample(), null);
  assert.match(sample(), /3 consecutive/);
  assert.throws(() => pressureMonitor(() => '')(), /availability/);
});

test('drain rejects unrelated scopes before issuing any signal', () => {
  const f = fixture();
  assert.throws(() => drainScope('dev-rust.slice', parent, f.runtime), /unowned/);
  assert.throws(() => drainScope(unit, parent + '/other.scope', f.runtime), /unowned/);
  assert.equal(f.calls.length, 0);
});

test('kernel memory limit rounded down below a non-aligned request passes actual-bound verification', () => {
  const f = fixture();
  const read = f.runtime.read;
  const requested = { ...owner, memory: owner.memory - 1 };
  f.runtime.read = (file) => {
    if (file === '/proc/self/cgroup') return '0::' + parent + '/' + unit;
    if (file.includes('/' + unit + '/')) {
      if (file.endsWith('cpu.max')) return '800000 100000';
      if (file.endsWith('memory.max')) return String(owner.memory - 4096);
      if (file.endsWith('memory.swap.max')) return '0';
    }
    return read(file);
  };
  assert.equal(verifyScope(JSON.stringify(requested), {}, f.runtime).memory, owner.memory - 4096);
});
