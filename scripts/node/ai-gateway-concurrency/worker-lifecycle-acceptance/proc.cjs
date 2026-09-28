'use strict';
const fs = require('node:fs');
const crypto = require('node:crypto');
const { assertKillIdentity } = require('./oracle.cjs');
const digests = new Map();
function executableDigest(file, identity) {
  if (digests.has(identity)) return digests.get(identity);
  const hash = crypto.createHash('sha256');
  const fd = fs.openSync(file, 'r'); const chunk = Buffer.allocUnsafe(65536);
  try { let length; while ((length = fs.readSync(fd, chunk, 0, chunk.length, null)) > 0) hash.update(chunk.subarray(0, length)); }
  finally { fs.closeSync(fd); }
  const value = hash.digest('hex'); digests.set(identity, value); return value;
}
function snapshot(pid) {
  const base = `/proc/${pid}`;
  const stat = fs.readFileSync(`${base}/stat`, 'utf8').split(') ').pop().split(' ');
  const exe = fs.readlinkSync(`${base}/exe`);
  const memory = fs.readFileSync(`${base}/smaps_rollup`, 'utf8');
  return { pid, ppid: Number(stat[1]), startTime: stat[19], exe,
    digest: executableDigest(`${base}/exe`, `${pid}:${stat[19]}`),
    argv: fs.readFileSync(`${base}/cmdline`, 'utf8').split('\0').filter(Boolean),
    rssKb: Number(memory.match(/^Rss:\s+(\d+)/m)?.[1]),
    pssKb: Number(memory.match(/^Pss:\s+(\d+)/m)?.[1]),
    fdCount: fs.readdirSync(`${base}/fd`).length };
}
function discover(expected) {
  // Linux exposes children per thread: Tokio can spawn on any executor thread.
  const tasks = `/proc/${expected.parentPid}/task`;
  const children = [...new Set(fs.readdirSync(tasks).flatMap((tid) => {
    try { return fs.readFileSync(`${tasks}/${tid}/children`, 'utf8').trim().split(/\s+/).filter(Boolean).map(Number); }
    catch (error) { if (error.code === 'ENOENT') return []; throw error; }
  }))];
  const matches = children.flatMap((pid) => {
    try { const s = snapshot(pid); return s.exe === expected.workerExe && s.argv.includes(expected.workerArg) ? [s] : []; }
    catch (error) { if (error.code === 'ENOENT' || error.code === 'ESRCH') return []; throw error; }
  });
  if (matches.length !== 1) throw new Error(`Expected exactly one supplier child, found ${matches.length}`);
  assertKillIdentity(matches[0], { ...expected, startTime: matches[0].startTime }, snapshot(expected.parentPid));
  return matches[0];
}
function killVerified(expected, previous) {
  const current = snapshot(previous.pid);
  assertKillIdentity(current, { ...expected, startTime: previous.startTime }, snapshot(expected.parentPid));
  process.kill(current.pid, 'SIGKILL');
  return current;
}
module.exports = { snapshot, discover, killVerified };
