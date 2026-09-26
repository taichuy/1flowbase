'use strict';

const assert = require('node:assert/strict');
const test = require('node:test');

const {
  createProcessTreeMemoryProbe,
  parseSmapsRollup,
  readProcessTreeMemory,
} = require('../process-memory');

function rollup(rss, pss, privateDirty) {
  return `Rss: ${rss} kB\nPss: ${pss} kB\nPrivate_Dirty: ${privateDirty} kB\n`;
}

test('process tree memory sums Gateway and provider descendants without including siblings', () => {
  const files = new Map([
    ['/proc/100/smaps_rollup', rollup(100, 80, 60)],
    ['/proc/100/task/100/children', '101\n'],
    ['/proc/100/task/110/children', '102\n'],
    ['/proc/101/smaps_rollup', rollup(40, 30, 20)],
    ['/proc/101/task/101/children', '103\n'],
    ['/proc/102/smaps_rollup', rollup(50, 45, 35)],
    ['/proc/102/task/102/children', '\n'],
    ['/proc/103/smaps_rollup', rollup(20, 15, 10)],
    ['/proc/103/task/103/children', '\n'],
  ]);
  const readFile = (name) => {
    if (!files.has(name)) throw Object.assign(new Error('missing'), { code: 'ENOENT' });
    return files.get(name);
  };
  const readDirectory = (name) => name === '/proc/100/task'
    ? ['100', '110']
    : [name.split('/')[2]];
  assert.deepEqual(readProcessTreeMemory(100, readFile, readDirectory), {
    rss_kib: 210,
    pss_kib: 170,
    private_dirty_kib: 125,
    process_count: 4,
  });
  files.delete('/proc/102/smaps_rollup');
  assert.equal(readProcessTreeMemory(100, readFile, readDirectory).process_count, 3);
  files.delete('/proc/100/smaps_rollup');
  assert.throws(() => readProcessTreeMemory(100, readFile, readDirectory), { code: 'ENOENT' });
});

test('process memory rejects an incomplete smaps sample', () => {
  assert.throws(() => parseSmapsRollup('Rss: 1 kB\nPss: 1 kB\n'), /omitted/u);
});

test('process memory probe records the highest observed PSS without a machine-dependent threshold', () => {
  const snapshots = [
    { rss_kib: 120, pss_kib: 100, private_dirty_kib: 60, process_count: 1 },
    { rss_kib: 210, pss_kib: 180, private_dirty_kib: 130, process_count: 2 },
    { rss_kib: 160, pss_kib: 120, private_dirty_kib: 80, process_count: 1 },
  ];
  let tick;
  let cleared = false;
  const probe = createProcessTreeMemoryProbe(100, {
    readSnapshot: () => snapshots.shift(),
    setIntervalImpl: (callback) => { tick = callback; return { unref() {} }; },
    clearIntervalImpl: () => { cleared = true; },
  });
  probe.begin();
  tick();
  const result = probe.end();
  assert.equal(cleared, true);
  assert.equal(result.sample_count, 3);
  assert.equal(result.peak.pss_kib, 180);
  assert.equal(result.peak.process_count, 2);
  assert.equal(result.peak_pss_delta_kib, 80);
  assert.equal(result.final.pss_kib, 120);
});
