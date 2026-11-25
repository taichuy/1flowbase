const test = require('node:test');
const assert = require('node:assert/strict');
const { getLocalCpuBudget, getLocalMemoryBudget } = require('../budget.js');

const GIB = 2 ** 30;
const MIB = 2 ** 20;

test('local CPU stays within 60 percent except for the single-core minimum', () => {
  for (const [cpus, expected] of [[1, 1], [2, 1], [3, 1], [4, 2], [6, 3], [8, 4], [12, 7], [20, 12]]) {
    assert.equal(getLocalCpuBudget(cpus), expected);
  }
  for (let cpus = 2; cpus <= 64; cpus += 1) {
    assert.ok(getLocalCpuBudget(cpus) * 5 <= cpus * 3);
  }
  for (const cpus of [0, -1, 1.5, NaN]) {
    assert.throws(() => getLocalCpuBudget(cpus), /positive integer/);
  }
});

test('memory targets at most eight GiB, bounded by effective capacity', () => {
  for (const [capacity, reserve, max] of [
    [1, 0.25, 1], [4, 1, 4], [8, 2, 8], [12, 2, 8],
    [16, 2, 8], [24, 2, 8], [34, 2, 8],
  ]) {
    const actual = getLocalMemoryBudget({ totalMemoryBytes: capacity * GIB, constrainedMemoryBytes: 0 });
    assert.equal(actual.capacityBytes, capacity * GIB);
    assert.equal(actual.reserveBytes, reserve * GIB);
    assert.equal(actual.maxBytes, max * GIB);
    assert.ok(actual.highBytes < actual.maxBytes);
    assert.equal(actual.highBytes % MIB, 0);
  }
  for (let gib = 1; gib <= 64; gib += 1) {
    const { maxBytes } = getLocalMemoryBudget({ totalMemoryBytes: gib * GIB, constrainedMemoryBytes: 0 });
    assert.equal(maxBytes, Math.min(gib, 8) * GIB);
  }
  assert.equal(getLocalMemoryBudget({ totalMemoryBytes: 24 * GIB, constrainedMemoryBytes: 0 }).highBytes, 7372 * MIB);
});

test('container memory constraints override larger physical host capacity', () => {
  const actual = getLocalMemoryBudget({ totalMemoryBytes: 64 * GIB, constrainedMemoryBytes: 8 * GIB });
  assert.equal(actual.capacityBytes, 8 * GIB);
  assert.equal(actual.reserveBytes, 2 * GIB);
  assert.equal(actual.maxBytes, 8 * GIB);
  assert.equal(getLocalMemoryBudget({ totalMemoryBytes: 8 * GIB, constrainedMemoryBytes: 64 * GIB }).maxBytes, 8 * GIB);
});

test('invalid memory measurements fail before any limits are installed', () => {
  for (const totalMemoryBytes of [0, -1, NaN, Infinity]) {
    assert.throws(() => getLocalMemoryBudget({ totalMemoryBytes, constrainedMemoryBytes: 0 }), /Memory capacity/);
  }
  assert.throws(() => getLocalMemoryBudget({ totalMemoryBytes: GIB, constrainedMemoryBytes: 100 }), /Effective memory/);
});


test('nested local Turbo/Vitest worker count never exceeds the CPU budget', () => {
  const { buildWorkspaceTestCommand } = require('../../../cli/run-frontend-workspace-test.js');
  const { getLocalCpuBudget } = require('../budget.js');
  const { loadVerifyRuntimeConfig } = require('../../verify-runtime.js');
  const fs = require('node:fs');
  const os = require('node:os');
  const path = require('node:path');
  const repoRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'oneflowbase-nested-budget-'));
  try {
    for (let availableParallelism = 1; availableParallelism <= 64; availableParallelism += 1) {
      const runtimeConfig = loadVerifyRuntimeConfig({ repoRoot, env: {}, availableParallelism });
      const cpuBudget = getLocalCpuBudget(availableParallelism);
      const command = buildWorkspaceTestCommand({ runtimeConfig, cpuBudget });
      assert.ok(runtimeConfig.backend.cargoJobs <= cpuBudget);
      assert.ok(runtimeConfig.backend.cargoTestThreads <= cpuBudget);
      const turbo = Number(command.args.find((arg) => arg.startsWith('--concurrency=')).split('=')[1]);
      const vitest = Number(command.env.VITEST_MAX_WORKERS);
      assert.ok(turbo > 0 && vitest > 0 && turbo * vitest <= cpuBudget);
    }
  } finally {
    fs.rmSync(repoRoot, { recursive: true, force: true });
  }
});
