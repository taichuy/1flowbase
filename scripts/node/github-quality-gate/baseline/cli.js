#!/usr/bin/env node
const { resolveBaseline, writeBaseline } = require('./core.js');

function main({ env = process.env, resolve = resolveBaseline, write = writeBaseline, stderr = process.stderr } = {}) {
  try {
    const result = resolve({ env, onRejected: (message) => stderr.write(`[quality-baseline] ${message}\n`) });
    write(result, { env });
    return 0;
  } catch (error) {
    stderr.write(`[quality-baseline] ${error.message}\n`);
    return 1;
  }
}

if (require.main === module) process.exitCode = main();
module.exports = { main };
