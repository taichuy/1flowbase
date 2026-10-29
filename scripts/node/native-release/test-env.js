'use strict';
// Test support for generated dotenv files; production parsing belongs to the Rust launcher.
const fs = require('node:fs');
const path = require('node:path');
function readEnv(root) {
  const values = {};
  for (const line of fs.readFileSync(path.join(root, '.env'), 'utf8').split(/\r?\n/)) {
    if (!line || line.startsWith('#')) continue;
    const at = line.indexOf('=');
    if (at < 0) continue;
    const raw = line.slice(at + 1);
    values[line.slice(0, at)] = raw.startsWith('"') ? JSON.parse(raw) : raw;
  }
  return values;
}
function writeEnv(root, overrides) {
  const values = { ...readEnv(root), ...overrides };
  fs.writeFileSync(path.join(root, '.env'), Object.entries(values).map(([key,value]) => `${key}=${JSON.stringify(value)}`).join('\n') + '\n');
}
module.exports = { readEnv, writeEnv };
