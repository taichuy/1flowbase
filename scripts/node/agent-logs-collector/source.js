'use strict';
const fs = require('node:fs');
const fsp = require('node:fs/promises');
const path = require('node:path');
const crypto = require('node:crypto');
const hash = value => crypto.createHash('sha256').update(value).digest('hex');
async function* files(source) {
  const stat = await fsp.lstat(source);
  if (stat.isSymbolicLink()) return; // Do not escape the selected source or recurse through cycles.
  if (stat.isFile()) { if (source.endsWith('.jsonl')) yield source; return; }
  if (!stat.isDirectory()) return;
  const entries = await fsp.readdir(source, { withFileTypes: true });
  entries.sort((a, b) => a.name.localeCompare(b.name));
  for (const entry of entries) if (!entry.isSymbolicLink()) yield* files(path.join(source, entry.name));
}
async function* lines(file) {
  let pending = Buffer.alloc(0), offset = 0;
  for await (const chunk of fs.createReadStream(file)) {
    pending = Buffer.concat([pending, chunk]);
    let end;
    while ((end = pending.indexOf(10)) !== -1) {
      const bytes = pending.subarray(0, end);
      const start = offset;
      offset += end + 1; pending = pending.subarray(end + 1);
      if (!bytes.toString('utf8').trim()) continue;
      let line;
      try { line = JSON.parse(bytes.toString('utf8')); }
      catch { throw new Error(`Invalid JSONL record at byte ${start} in ${file}`); }
      yield { line, start, end: offset, bytes };
    }
  }
  // A trailing unterminated line is uncommitted, even if it is currently valid JSON.
}
module.exports = { files, lines, hash };
