const test = require('node:test');
const assert = require('node:assert/strict');
const { getLocalCpuBudget, getLocalMemoryBudget } = require('../budget.js');

const GIB = 2 ** 30;
const MIB = 2 ** 20;

test('Rust targets four CPUs on small machines and half on larger machines', () => {
  for (const [cpus, expected] of [[1, 1], [2, 2], [3, 3], [4, 4], [6, 4], [8, 4], [12, 6], [20, 10]]) {
    assert.equal(getLocalCpuBudget(cpus), expected);
  }
  for (const cpus of [0, -1, 1.5, NaN]) {
    assert.throws(() => getLocalCpuBudget(cpus), /positive integer/);
  }
});

test('memory targets eight GiB or half capacity, bounded by effective capacity', () => {
  for (const [capacity, reserve, max] of [
    [1, 0.25, 1], [4, 1, 4], [8, 2, 8], [12, 2, 8],
    [16, 2, 8], [24, 2, 12], [32, 2, 16],
  ]) {
    const actual = getLocalMemoryBudget({ totalMemoryBytes: capacity * GIB, constrainedMemoryBytes: 0 });
    assert.equal(actual.capacityBytes, capacity * GIB);
    assert.equal(actual.reserveBytes, reserve * GIB);
    assert.equal(actual.maxBytes, max * GIB);
    assert.ok(actual.highBytes < actual.maxBytes);
    assert.equal(actual.highBytes % MIB, 0);
  }
  assert.equal(getLocalMemoryBudget({ totalMemoryBytes: 24 * GIB, constrainedMemoryBytes: 0 }).highBytes, 11059 * MIB);
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
