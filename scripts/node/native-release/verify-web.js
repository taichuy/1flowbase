'use strict';
const fs = require('node:fs');
const path = require('node:path');
const { createHash } = require('node:crypto');

function verifyWeb(root) {
  const dist = path.resolve(root, 'dist');
  const manifest = fs.readFileSync(path.join(root, 'web-dist.sha256'), 'utf8').trim();
  if (!manifest) throw new Error('Empty web distribution checksums');
  let count = 0;
  for (const line of manifest.split(/\r?\n/)) {
    const match = line.match(/^([a-f0-9]{64})  (\.\/.+)$/);
    if (!match) throw new Error('Invalid web distribution checksum entry');
    const [, expected, relative] = match;
    const file = path.resolve(dist, relative);
    if (!file.startsWith(dist + path.sep)) throw new Error('Web checksum path escapes dist');
    const actual = createHash('sha256').update(fs.readFileSync(file)).digest('hex');
    if (actual !== expected) throw new Error(`Web checksum mismatch: ${relative}`);
    count++;
  }
  if (!fs.statSync(path.join(dist, 'index.html')).isFile()) throw new Error('Missing web index.html');
  return count;
}

if (require.main === module) {
  if (!process.argv[2]) throw new Error('Usage: verify-web.js <web-artifact-root>');
  console.log(`Verified ${verifyWeb(process.argv[2])} shared web files`);
}
module.exports = { verifyWeb };
