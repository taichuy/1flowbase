#!/usr/bin/env node
'use strict';

const { runLocalAcceptance } = require('./runner');
const { runCountTokensUpgrade } = require('./count-tokens-upgrade-runner');

const USAGE = 'usage: local-acceptance/cli.js run [--manifest <path>] | count-tokens-upgrade --manifest <path>';

function parseArgs(argv) {
  if (argv[0] === 'count-tokens-upgrade') {
    if (argv.length === 3 && argv[1] === '--manifest' && argv[2]) {
      return { command: 'count-tokens-upgrade', options: { manifest: argv[2] } };
    }
    throw new Error(USAGE);
  }
  if (argv[0] !== 'run') {
    throw new Error(USAGE);
  }
  if (argv.length === 1) return { command: 'run', options: {} };
  if (argv.length === 3 && argv[1] === '--manifest' && argv[2]) {
    return { command: 'run', options: { manifest: argv[2] } };
  }
  throw new Error(USAGE);
}

async function main(argv, dependencies = {}) {
  const parsed = parseArgs(argv);
  const result = parsed.command === 'count-tokens-upgrade'
    ? await (dependencies.runCountTokensUpgrade || runCountTokensUpgrade)(parsed.options, {
      sourceEnv: dependencies.sourceEnv || process.env,
    })
    : await (dependencies.runLocalAcceptance || runLocalAcceptance)(parsed.options);
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
