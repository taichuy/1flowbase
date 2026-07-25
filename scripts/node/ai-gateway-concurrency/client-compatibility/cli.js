#!/usr/bin/env node
'use strict';

const path = require('node:path');
const { runClientCompatibility } = require('./runner');

function parseArgs(argv) {
  if (argv[0] !== 'run') throw new Error('command must be run');
  const values = {};
  const fields = new Map([
    ['--ready-manifest', 'readyManifest'], ['--runtime-root', 'runtimeRoot'],
    ['--evidence-root', 'evidenceRoot'], ['--lock', 'lockPath'],
  ]);
  for (let index = 1; index < argv.length; index += 2) {
    const field = fields.get(argv[index]);
    if (!field || !argv[index + 1]) throw new Error(`invalid argument: ${argv[index]}`);
    values[field] = argv[index + 1];
  }
  for (const field of ['readyManifest', 'runtimeRoot', 'evidenceRoot']) {
    if (!values[field]) throw new Error(`${field} is required`);
  }
  if (!values.lockPath) values.lockPath = path.join(__dirname, 'client-compatibility.lock.json');
  return values;
}

async function main(argv = process.argv.slice(2)) {
  const result = await runClientCompatibility(parseArgs(argv));
  process.stdout.write(`${JSON.stringify(result, null, 2)}\n`);
  return result.status === 'pass' ? 0 : 1;
}

if (require.main === module) main().then((code) => { process.exitCode = code; }).catch((error) => {
  process.stderr.write(`${error.stack || error.message}\n`);
  process.exitCode = 1;
});

module.exports = { main, parseArgs };
