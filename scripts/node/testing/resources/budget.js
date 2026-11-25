#!/usr/bin/env node

const os = require('node:os');

const MIB = 2 ** 20;
const GIB = 2 ** 30;

function getAvailableParallelism() {
  return typeof os.availableParallelism === 'function'
    ? os.availableParallelism()
    : os.cpus().length;
}

function getLocalCpuBudget(availableParallelism = getAvailableParallelism()) {
  if (!Number.isInteger(availableParallelism) || availableParallelism < 1) {
    throw new Error('availableParallelism must be a positive integer');
  }
  return Math.max(1, Math.floor(availableParallelism * 0.6));
}

function getLocalMemoryBudget({
  totalMemoryBytes = os.totalmem(),
  constrainedMemoryBytes = process.constrainedMemory?.() ?? 0,
} = {}) {
  if (!Number.isFinite(totalMemoryBytes) || totalMemoryBytes < MIB ||
      !Number.isFinite(constrainedMemoryBytes) || constrainedMemoryBytes < 0) {
    throw new Error('Memory capacity must be finite and at least 1 MiB; constraint must be non-negative');
  }
  const capacityBytes = Math.floor(Math.min(
    totalMemoryBytes,
    constrainedMemoryBytes > 0 ? constrainedMemoryBytes : totalMemoryBytes,
  ) / MIB) * MIB;
  if (capacityBytes < MIB) throw new Error('Effective memory capacity must be at least 1 MiB');

  // Session MemoryLow is a reclaim preference, not a deduction from this budget.
  const reserveBytes = Math.floor(Math.min(2 * GIB, capacityBytes / 4) / MIB) * MIB;
  const maxBytes = Math.floor(Math.min(capacityBytes, 8 * GIB) / MIB) * MIB;
  const highBytes = Math.max(MIB, Math.floor(maxBytes * 0.9 / MIB) * MIB);
  return { capacityBytes, reserveBytes, highBytes, maxBytes };
}

function main(argv = process.argv.slice(2), write = (text) => process.stdout.write(text)) {
  if (argv.length === 1 && argv[0] === 'cpu-jobs') {
    write(`${getLocalCpuBudget()}\n`);
  } else if (argv.length === 1 && argv[0] === 'memory-limits') {
    const { reserveBytes, highBytes, maxBytes } = getLocalMemoryBudget();
    write(`${reserveBytes} ${highBytes} ${maxBytes}\n`);
  } else if (argv.length === 0) {
    write(`${JSON.stringify({ cpuJobs: getLocalCpuBudget(), memory: getLocalMemoryBudget() }, null, 2)}\n`);
  } else {
    throw new Error('Usage: node budget.js [cpu-jobs|memory-limits]');
  }
}

if (require.main === module) {
  try { main(); } catch (error) {
    process.stderr.write(`[1flowbase-resource-budget] ${error.message}\n`);
    process.exitCode = 1;
  }
}

module.exports = { getAvailableParallelism, getLocalCpuBudget, getLocalMemoryBudget, main };
