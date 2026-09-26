'use strict';

const fs = require('node:fs');

function parseSmapsRollup(source) {
  const fields = Object.fromEntries(
    [...source.matchAll(/^(Rss|Pss|Private_Dirty):\s+(\d+) kB$/gmu)]
      .map((match) => [match[1], Number(match[2])]),
  );
  if (Object.keys(fields).length !== 3) {
    throw new Error('process smaps_rollup omitted Rss, Pss, or Private_Dirty');
  }
  return {
    rss_kib: fields.Rss,
    pss_kib: fields.Pss,
    private_dirty_kib: fields.Private_Dirty,
  };
}

function readProcessTreeMemory(rootPid, readFile = fs.readFileSync, readDirectory = fs.readdirSync) {
  if (!Number.isSafeInteger(rootPid) || rootPid <= 0) {
    throw new Error('gateway process PID must be a positive integer');
  }
  const pending = [rootPid];
  const seen = new Set();
  const total = { rss_kib: 0, pss_kib: 0, private_dirty_kib: 0, process_count: 0 };
  while (pending.length) {
    const pid = pending.pop();
    if (seen.has(pid)) continue;
    seen.add(pid);
    let memory;
    let threadIds;
    try {
      memory = parseSmapsRollup(readFile(`/proc/${pid}/smaps_rollup`, 'utf8'));
      threadIds = readDirectory(`/proc/${pid}/task`);
    } catch (error) {
      // A provider can exit between the parent children snapshot and its own read.
      if (pid !== rootPid && ['ENOENT', 'ESRCH'].includes(error.code)) continue;
      throw error;
    }
    total.rss_kib += memory.rss_kib;
    total.pss_kib += memory.pss_kib;
    total.private_dirty_kib += memory.private_dirty_kib;
    total.process_count += 1;
    for (const threadId of threadIds) {
      let children;
      try {
        children = readFile(`/proc/${pid}/task/${threadId}/children`, 'utf8');
      } catch (error) {
        if (['ENOENT', 'ESRCH'].includes(error.code)) continue;
        throw error;
      }
      for (const child of children.trim().split(/\s+/u).filter(Boolean)) {
        const childPid = Number(child);
        if (Number.isSafeInteger(childPid) && childPid > 0) pending.push(childPid);
      }
    }
  }
  return total;
}

function createProcessTreeMemoryProbe(rootPid, {
  intervalMs = 50,
  readSnapshot = readProcessTreeMemory,
  setIntervalImpl = setInterval,
  clearIntervalImpl = clearInterval,
} = {}) {
  let timer = null;
  let baseline = null;
  let peak = null;
  let final = null;
  let sampleCount = 0;
  let sampleError = null;
  const sample = () => {
    const current = readSnapshot(rootPid);
    sampleCount += 1;
    if (!peak || current.pss_kib > peak.pss_kib) peak = current;
    return current;
  };
  return {
    begin() {
      if (timer) throw new Error('process memory probe already started');
      baseline = sample();
      timer = setIntervalImpl(() => {
        try { sample(); }
        catch (error) { sampleError ??= error; }
      }, intervalMs);
      timer.unref?.();
    },
    end() {
      if (!timer) throw new Error('process memory probe was not started');
      clearIntervalImpl(timer);
      timer = null;
      final = sample();
      if (sampleError) throw sampleError;
      return {
        source: 'linux-proc-process-tree-smaps-rollup',
        interval_ms: intervalMs,
        sample_count: sampleCount,
        baseline,
        peak,
        final,
        peak_pss_delta_kib: peak.pss_kib - baseline.pss_kib,
      };
    },
  };
}

module.exports = { createProcessTreeMemoryProbe, parseSmapsRollup, readProcessTreeMemory };
