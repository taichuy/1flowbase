#!/usr/bin/env node
'use strict';

const { runLocalAcceptance } = require('./runner');

function parseArgs(argv) {
  if (argv[0] !== 'run') {
    throw new Error('usage: local-acceptance/cli.js run [--manifest <path>]');
  }
  if (argv.length === 1) return {};
  if (argv.length === 3 && argv[1] === '--manifest' && argv[2]) {
    return { manifest: argv[2] };
  }
  throw new Error('usage: local-acceptance/cli.js run [--manifest <path>]');
}

async function main(argv) {
  const result = await runLocalAcceptance(parseArgs(argv));
  process.stdout.write(`${JSON.stringify(result, null, 2)}\n`);
  if (result.status !== 'pass') process.exitCode = 1;
}

if (require.main === module) {
  main(process.argv.slice(2)).catch((error) => {
    process.stderr.write(`${error.stack || error.message}\n`);
    process.exitCode = 1;
  });
}

module.exports = { main, parseArgs };
