#!/usr/bin/env node
'use strict';

const { runCodexBuild } = require('../ai-gateway-concurrency/cli-smoke/codex-build');

const FIELDS = new Map([
  ['--source-root', 'sourceRoot'],
  ['--output-dir', 'outputDir'],
  ['--jobs', 'jobs'],
]);

function usage() {
  return `Usage: node scripts/node/cli/ai-gateway-codex-build.js \\
  --source-root <clean-detached-codex> --output-dir <artifact-dir> [--jobs <n>]`;
}

function parseArgs(argv) {
  const options = { jobs: 2 };
  for (let index = 0; index < argv.length; index += 1) {
    const flag = argv[index];
    if (flag === '--help' || flag === '-h') return { help: true };
    const field = FIELDS.get(flag);
    if (!field) throw new Error(`unknown option ${flag}`);
    const value = argv[index + 1];
    if (!value || value.startsWith('--')) throw new Error(`${flag} requires a value`);
    options[field] = field === 'jobs' ? Number(value) : value;
    index += 1;
  }
  for (const [field, flag] of [['sourceRoot', '--source-root'], ['outputDir', '--output-dir']]) {
    if (!options[field]) throw new Error(`${flag} is required`);
  }
  if (!Number.isInteger(options.jobs) || options.jobs < 1) throw new Error('--jobs must be a positive integer');
  return options;
}

function main() {
  const options = parseArgs(process.argv.slice(2));
  if (options.help) {
    process.stdout.write(`${usage()}\n`);
    return;
  }
  process.stdout.write(`${JSON.stringify(runCodexBuild(options))}\n`);
}

if (require.main === module) {
  try {
    main();
  } catch (error) {
    process.stderr.write(`[ai-gateway-codex-build] ${error.message}\n`);
    process.exitCode = 1;
  }
}

module.exports = { main, parseArgs, usage };
