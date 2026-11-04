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
  const writes = [];
  let nowMs = 0;
  let ticks;
  let closed = false;
  const child = new EventEmitter();
  child.kill = (signal) => { calls.push(['launcher-kill', signal]); if (!closed) { closed = true; child.emit('close', null, signal); } };
  const runtime = {
    process: proc, nonce: () => 'aaaaaaaaaaaaaaaaaaaaaaaa',
    now: () => nowMs,
    os: { cpus: () => Array(16).fill({}), availableParallelism: () => 12, totalmem: () => 16 * 2 ** 30 },
    read: (file) => {
      if (file === '/proc/self/cgroup') return '0::' + parent;
      if (file === '/proc/meminfo') return 'MemTotal: 1000 kB\nMemAvailable: ' + (pressure === 'low' ? 90 : 500) + ' kB';
      if (file.endsWith('memory.pressure') || file === '/proc/pressure/memory') {
        const host = file === '/proc/pressure/memory';
        const high = pressure === (host ? 'psi' : 'scope-psi');
        const total = pressure === (host ? 'total-host' : 'total-scope') ? nowMs * 200 : 0;
        return 'some avg10=0.00 avg60=0.00 total=0\nfull avg10=' + (high ? '30.0' : '0.0') + ' avg60=0.0 total=' + total;
      }
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
          const count = pressure === 'low' || !pressure ? 3 : 4;
          for (let i = 0; i < count; i++) { nowMs += 1000; ticks(); }
          // Low availability no longer closes the child through an abort.
          if (pressure === 'low' && !closed) { closed = true; child.emit('close', code, null); }
        }
      });
      return child;
    },
    setInterval: (fn) => { ticks = fn; return 4; },
    clearInterval: (id) => { calls.push(['clear', id]); },
  };
  return { runtime, calls, writes, child, proc };
}
function run(f, extra = {}) {
  return runInResourceScope({
    argv: [], env: {}, platform: 'linux', write: (text) => f.writes.push(text),
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

test('low headroom warns and preserves natural status while host/scope PSI still abort only owned scope', async () => {
  // Retire only the former low-only kill expectation; kernel caps and severe PSI remain enforced.
  for (const code of [0, 17]) {
    const f = fixture({ pressure: 'low', code });
    assert.equal(await run(f), code);
    assert.equal(f.calls.some(([name]) => name === 'launcher-kill'), false);
    assert.equal(f.writes.filter((line) => /warning:.*below 10%/.test(line)).length, 1);
    assert.ok(f.calls.filter(([name]) => name === 'kill' || name === 'stop').every((args) => args.at(-1) === unit));
  }
  for (const pressure of ['psi', 'scope-psi', 'total-host', 'total-scope']) {
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

test('low availability warns once on entry and recovery without aborting or log flooding', () => {
  const f = pressureFixture();
  f.data.available = 90;
  for (let i = 0; i < 10; i++) { f.data.time = i * 1000; assert.equal(f.sample(), null); }
  assert.equal(f.warnings.length, 1);
  assert.match(f.warnings[0], /below 10%/);
  f.data.available = 500; f.data.time = 10000; assert.equal(f.sample(), null);
  f.data.time = 11000; assert.equal(f.sample(), null);
  assert.equal(f.warnings.length, 2); assert.match(f.warnings[1], /recover/i);
  f.data.available = 90; f.data.time = 12000; assert.equal(f.sample(), null);
  assert.equal(f.warnings.length, 3);
  assert.throws(() => pressureMonitor(() => '')(), /availability/);
});

function pressureFixture() {
  const data = { time: 0, available: 500, host: { avg10: 0, total: 10 ** 12 }, scope: { avg10: 0, total: 10 ** 12 } };
  const warnings = [];
  const read = (file) => {
    if (file === '/proc/meminfo') {
      if (data.memError) throw data.memError;
      return data.memText ?? 'MemTotal: 1000 kB\nMemAvailable: ' + data.available + ' kB';
    }
    const host = file === '/proc/pressure/memory';
    if (data[host ? 'hostError' : 'scopeError']) throw data[host ? 'hostError' : 'scopeError'];
    const value = data[host ? 'host' : 'scope'];
    return value.raw ?? 'full avg10=' + value.avg10 + ' avg60=0.00 avg300=0.00 total=' + value.total;
  };
  return { data, warnings, sample: pressureMonitor(read, parent + '/' + unit, {
    now: () => data.time, warn: (message) => warnings.push(message),
  }) };
}

test('avg10 retains the original shared host/scope maximum and four consecutive samples', () => {
  for (const source of ['host', 'scope']) {
    const f = pressureFixture();
    f.data[source].avg10 = 19.99; assert.equal(f.sample(), null);
    f.data[source].avg10 = 20;
    for (let i = 1; i <= 3; i++) { f.data.time = i * 100; assert.equal(f.sample(), null); }
    f.data[source].avg10 = 19.99; f.data.time = 400; assert.equal(f.sample(), null);
    f.data[source].avg10 = 20;
    for (let i = 5; i <= 7; i++) { f.data.time = i * 100; assert.equal(f.sample(), null); }
    f.data.time = 800; assert.match(f.sample(), /PSI/);
  }
  const alternating = pressureFixture();
  for (let i = 0; i < 4; i++) {
    alternating.data.time = i * 100;
    alternating.data.host.avg10 = i % 2 ? 0 : 20;
    alternating.data.scope.avg10 = i % 2 ? 20 : 0;
    if (i < 3) assert.equal(alternating.sample(), null);
    else assert.match(alternating.sample(), /PSI/);
  }
});

test('low avg10 total pressure uses actual complete window duration and the unchanged severe threshold', () => {
  for (const source of ['host', 'scope']) {
    const f = pressureFixture(); assert.equal(f.sample(), null);
    // 249999us / 1.25s is below 20%; dividing by a fixed one second would falsely abort.
    for (let i = 1; i <= 5; i++) {
      f.data.time += 1250; f.data[source].total += 249999; assert.equal(f.sample(), null);
    }
    for (let i = 1; i <= 3; i++) {
      f.data.time += 1250; f.data[source].total += 250000; assert.equal(f.sample(), null);
    }
    f.data.time += 1250; f.data[source].total += 250000; assert.match(f.sample(), /PSI/);
  }
});

test('999ms observations accumulate into 1998ms windows without clearing severe window counts', () => {
  const f = pressureFixture(); assert.equal(f.sample(), null);
  for (let i = 1; i <= 7; i++) {
    f.data.time = i * 999; f.data.host.total += 300000; assert.equal(f.sample(), null);
  }
  f.data.time = 8 * 999; f.data.host.total += 300000; assert.match(f.sample(), /PSI/);
});

test('late scope pressure windows survive host low windows and short observations', () => {
  const f = pressureFixture();
  f.data.scopeError = Object.assign(new Error('prestart'), { code: 'ENOENT' });
  assert.equal(f.sample(), null);
  delete f.data.scopeError; f.data.time = 999; assert.equal(f.sample(), null);
  for (let i = 1; i <= 4; i++) {
    f.data.time = 999 + i * 1000; f.data.scope.total = 10 ** 12 + i * 300000;
    if (i === 4) assert.match(f.sample(), /PSI/);
    else {
      assert.equal(f.sample(), null);
      f.data.time = 1000 + i * 1000; f.data.scope.total += 300; assert.equal(f.sample(), null);
    }
  }
});

test('a normal complete window resets its source count and avg10 recovery is independent', () => {
  const f = pressureFixture(); assert.equal(f.sample(), null);
  for (let i = 1; i <= 3; i++) {
    f.data.time += 1000; f.data.host.total += 200000; assert.equal(f.sample(), null);
  }
  f.data.time += 1000; assert.equal(f.sample(), null);
  for (let i = 1; i <= 3; i++) {
    f.data.time += 1000; f.data.host.total += 200000; assert.equal(f.sample(), null);
  }
  f.data.time += 1000; f.data.host.total += 200000; assert.match(f.sample(), /PSI/);
  const mixed = pressureFixture();
  mixed.data.host.avg10 = 20; assert.equal(mixed.sample(), null);
  mixed.data.time = 1000; mixed.data.host.total += 200000; assert.equal(mixed.sample(), null);
  mixed.data.host.avg10 = 0; // avg10 recovers after two samples; total windows have their own count.
  for (let i = 2; i <= 3; i++) {
    mixed.data.time = i * 1000; mixed.data.host.total += 200000; assert.equal(mixed.sample(), null);
  }
  mixed.data.time = 4000; mixed.data.host.total += 200000; assert.match(mixed.sample(), /PSI/);
});

test('malformed memory and host/scope PSI data fail closed instead of becoming zero or NaN', () => {
  for (const raw of ['', 'some avg10=0 total=0', 'full avg10=... total=0', 'full avg10=NaN total=0',
    'full avg10=Infinity total=0', 'full avg10=-1 total=0', 'full avg10=100.01 total=0',
    'full avg10=0', 'full avg10=0 total=-1', 'full avg10=0 total=1.5',
    'full avg10=0 total=NaN', 'full avg10=0 total=9007199254740992', 'full avg10=0 avg10=1 total=0']) {
    for (const source of ['host', 'scope']) {
      const f = pressureFixture(); f.data[source].raw = raw; assert.throws(f.sample, /PSI/);
    }
  }
  for (const raw of ['', 'MemTotal: 0 kB\nMemAvailable: 10 kB', 'MemTotal: 1000 kB\nMemAvailable: -1 kB',
    'MemTotal: Infinity kB\nMemAvailable: 10 kB', 'MemTotal: 1000 kB\nMemAvailable: 10garbage kB',
    'MemTotal: 1000 kB\nMemAvailable: 10 kB\nMemAvailable: 20 kB',
    'MemTotal: 9007199254740992 kB\nMemAvailable: 10 kB']) {
    const f = pressureFixture(); f.data.memText = raw; assert.throws(f.sample, /availability/);
  }
});

test('clock invalidity or regression and any source total regression including a short window fail closed', () => {
  for (const time of [NaN, Infinity, -1, '1000']) {
    const f = pressureFixture(); f.data.time = time; assert.throws(f.sample, /clock/);
  }
  for (const time of [0, -1]) {
    const f = pressureFixture(); f.sample(); f.data.time = time; assert.throws(f.sample, /clock/);
  }
  const backwards = pressureFixture(); backwards.data.time = 1000; backwards.sample();
  backwards.data.time = 999; assert.throws(backwards.sample, /clock/);
  for (const source of ['host', 'scope']) {
    const f = pressureFixture(); f.sample();
    f.data.time = 999; f.data[source].total += 500; assert.equal(f.sample(), null);
    f.data.time = 1000; f.data[source].total -= 100; assert.throws(f.sample, /total.*regress/i);
  }
});

test('only scope ENOENT and ENODEV races remain optional and never hide host monitoring errors', () => {
  for (const code of ['ENOENT', 'ENODEV']) {
    const f = pressureFixture(); f.sample();
    f.data.time = 1000; f.data.scopeError = Object.assign(new Error('scope race'), { code });
    assert.equal(f.sample(), null);
    f.data.time = 2000; delete f.data.scopeError; assert.equal(f.sample(), null);
    f.data.time = 3000; f.data.hostError = Object.assign(new Error('host failed'), { code });
    assert.throws(f.sample, /host failed/);
    const reappeared = pressureFixture(); reappeared.sample();
    reappeared.data.time = 999; reappeared.data.scope.total += 500; reappeared.sample();
    reappeared.data.time = 1000; reappeared.data.scopeError = Object.assign(new Error('scope race'), { code });
    assert.equal(reappeared.sample(), null);
    reappeared.data.time = 1999; delete reappeared.data.scopeError; reappeared.data.scope.total -= 100;
    assert.throws(reappeared.sample, /total.*regress/i);
  }
  const f = pressureFixture(); f.data.scopeError = Object.assign(new Error('scope denied'), { code: 'EACCES' });
  assert.throws(f.sample, /scope denied/);
});

test('monitor read failure prevents launch in preflight and drains only owned scope after launch', async () => {
  const before = fixture(); const read = before.runtime.read;
  before.runtime.read = (file) => file === '/proc/pressure/memory' ? 'full avg10=NaN total=0' : read(file);
  await assert.rejects(run(before), /PSI/);
  assert.equal(before.calls.some(([name]) => name === 'spawn'), false);
  const after = fixture({ hold: true }); const valid = after.runtime.read; let reads = 0;
  after.runtime.read = (file) => {
    if (file === '/proc/pressure/memory' && ++reads > 1) throw new Error('host failed');
    return valid(file);
  };
  assert.equal(await run(after), 1);
  assert.ok(after.writes.some((line) => /resource monitor failed/.test(line)));
  assert.ok(after.calls.filter(([name]) => name === 'kill' || name === 'stop').every((args) => args.at(-1) === unit));
});

test('natural terminal close does not perform an extra pressure decision', async () => {
  const f = fixture(); let decisions = 0;
  f.runtime.now = () => { decisions += 1; return decisions * 1000; };
  assert.equal(await run(f), 0);
  assert.equal(decisions, 1); // Preflight only: no terminal sample enters the monitor.
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
