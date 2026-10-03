'use strict';
const fs = require('node:fs');
const { execFileSync } = require('node:child_process');

function parseStat(raw, pid) {
  const fields = raw.slice(raw.lastIndexOf(')') + 2).trim().split(/\s+/);
  const cpu_ticks = Number(fields[11]) + Number(fields[12]);
  const start_ticks = Number(fields[19]);
  if (!Number.isSafeInteger(cpu_ticks) || !Number.isSafeInteger(start_ticks)) throw Error('invalid proc stat');
  return { pid, start_ticks, cpu_ticks, name: raw.slice(raw.indexOf('(') + 1, raw.lastIndexOf(')')) };
}
function clockTicks() {
  const hz = Number(execFileSync('getconf', ['CLK_TCK'], { encoding: 'utf8', timeout: 2000, maxBuffer: 128 }).trim());
  if (!Number.isSafeInteger(hz) || hz <= 0) throw Error('invalid CLK_TCK');
  return hz;
}
function tree(root, read = fs.readFileSync, list = fs.readdirSync) {
  const pids = [root];
  let complete = true;
  for (let i = 0; i < pids.length; i++) {
    try {
      for (const tid of list(`/proc/${pids[i]}/task`)) {
        try {
          const children = read(`/proc/${pids[i]}/task/${tid}/children`, 'utf8').trim();
          for (const child of children ? children.split(/\s+/).map(Number) : []) {
            if (Number.isSafeInteger(child) && child > 0 && !pids.includes(child)) pids.push(child);
          }
        } catch { complete = false; }
      }
    } catch { complete = false; }
  }
  return { pids, complete };
}
function snapshot(root, databasePid = null) {
  const started = process.hrtime.bigint();
  const enumeration = tree(root);
  const rows = [];
  const group_complete = { api: true, plugin: enumeration.complete, database: false };
  const entries = enumeration.pids.map(pid => [pid, pid === root ? 'api' : 'plugin']);
  if (databasePid && !enumeration.pids.includes(databasePid)) {
    const database = tree(databasePid);
    group_complete.database = database.complete;
    entries.push(...database.pids.map(pid => [pid, 'database']));
  }
  for (const [pid, group] of entries) {
    try {
      const row = { ...parseStat(fs.readFileSync(`/proc/${pid}/stat`, 'utf8'), pid), group, pss_bytes: null, rss_bytes: null };
      try {
        const memory = fs.readFileSync(`/proc/${pid}/smaps_rollup`, 'utf8');
        const match = /^Pss:\s+(\d+)\s+kB$/m.exec(memory);
        if (match) row.pss_bytes = Number(match[1]) * 1024;
        const rss = /^Rss:\s+(\d+)\s+kB$/m.exec(memory);
        if (rss) row.rss_bytes = Number(rss[1]) * 1024;
      } catch { /* permission denied is missing data, never zero */ }
      if (row.rss_bytes === null) {
        try {
          const match = /^VmRSS:\s+(\d+)\s+kB$/m.exec(fs.readFileSync(`/proc/${pid}/status`, 'utf8'));
          if (match) row.rss_bytes = Number(match[1]) * 1024;
        } catch { /* RSS can also be unavailable. */ }
      }
      const verify = parseStat(fs.readFileSync(`/proc/${pid}/stat`, 'utf8'), pid);
      if (verify.start_ticks !== row.start_ticks) { group_complete[group] = false; continue; }
      rows.push(row);
    } catch { group_complete[group] = false; }
  }
  return { timestamp_ns: started.toString(), capture_ms: Number(process.hrtime.bigint() - started) / 1e6, complete: group_complete.api && group_complete.plugin, group_complete, rows };
}
function cpuDelta(before, after, hz) {
  const groups = {};
  for (const group of ['api', 'plugin', 'database']) {
    const old = before.rows.filter(r => r.group === group);
    const now = after.rows.filter(r => r.group === group);
    let valid = (before.group_complete?.[group] ?? before.complete) && (after.group_complete?.[group] ?? after.complete) && old.length === now.length;
    let ticks = 0;
    for (const row of now) {
      const previous = old.find(p => p.pid === row.pid && p.start_ticks === row.start_ticks);
      if (!previous || row.cpu_ticks < previous.cpu_ticks) valid = false;
      else ticks += row.cpu_ticks - previous.cpu_ticks;
    }
    if (group === 'database' || group === 'api') valid = valid && old.length > 0 && now.length > 0;
    groups[group] = { cpu_seconds: valid ? ticks / hz : null, observed_cpu_seconds_lower_bound: ticks / hz, complete: valid, processes_before: old.length, processes_after: now.length };
  }
  return groups;
}
function sampledPeaks(samples) {
  const peaks = {};
  for (const group of ['api', 'plugin', 'database']) {
    const values = samples.map(s => {
      const rows = s.rows.filter(r => r.group === group);
      const result = { timestamp_ns: s.timestamp_ns };
      for (const metric of ['pss_bytes', 'rss_bytes']) {
        const readable = rows.length > 0 && rows.every(r => Number.isFinite(r[metric]));
        result[metric] = readable && (s.group_complete?.[group] ?? s.complete) ? rows.reduce((sum, r) => sum + r[metric], 0) : null;
      }
      return result;
    });
    peaks[group] = {};
    for (const metric of ['pss_bytes', 'rss_bytes']) {
      const usable = values.filter(v => v[metric] !== null);
      peaks[group][metric] = usable.length ? Math.max(...usable.map(v => v[metric])) : null;
      peaks[group][`${metric}_valid_samples`] = usable.length;
      peaks[group][`${metric}_missing_samples`] = values.length - usable.length;
    }
  }
  return peaks;
}
module.exports = { parseStat, clockTicks, tree, snapshot, cpuDelta, sampledPeaks };
