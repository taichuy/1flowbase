#!/usr/bin/env node
'use strict';

const { runLocalAcceptance } = require('./runner');

async function main(argv) {
  if (argv.length !== 1 || argv[0] !== 'run') {
    throw new Error('usage: node scripts/node/ai-gateway-concurrency/local-acceptance/cli.js run');
  }
  const result = await runLocalAcceptance();
  process.stdout.write(`${JSON.stringify(result, null, 2)}\n`);
  if (result.status !== 'pass') process.exitCode = 1;
}

main(process.argv.slice(2)).catch((error) => {
  process.stderr.write(`${error.stack || error.message}\n`);
  process.exitCode = 1;
});
