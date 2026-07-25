#!/usr/bin/env node
'use strict';

const path = require('node:path');
const { finalizeEvidence, workflowResultBase } = require('./evidence');
const { requireCharacterizeProfile, requireFullSha } = require('./inputs');
const { runWorkflowContract } = require('./runner');

const RUN_FIELDS = new Map([
  ['--main-source-sha', 'mainSourceSha'],
  ['--official-source-sha', 'officialSourceSha'],
  ['--profile', 'profile'],
  ['--repo-root', 'repoRoot'],
  ['--database-url', 'databaseUrl'],
  ['--api-server-bin', 'apiServerBin'],
  ['--plugin-runner-bin', 'pluginRunnerBin'],
  ['--openai-package-dir', 'openaiPackageDir'],
  ['--anthropic-package-dir', 'anthropicPackageDir'],
  ['--host-target', 'hostTarget'],
]);

function parseFields(argv, fields) {
  const values = {};
  for (let index = 0; index < argv.length; index += 1) {
    const field = fields.get(argv[index]);
    if (!field || !argv[index + 1] || argv[index + 1].startsWith('--')) {
      throw new Error(`invalid argument: ${argv[index]}`);
    }
    if (values[field]) throw new Error(`duplicate argument: ${argv[index]}`);
    values[field] = argv[index + 1];
    index += 1;
  }
  return values;
}

function parseArgs(argv) {
  const command = argv[0];
  if (command === 'run') return { command, options: parseFields(argv.slice(1), RUN_FIELDS) };
  if (command === 'finalize') {
    const fields = new Map([
      ['--main-source-sha', 'mainSourceSha'],
      ['--official-source-sha', 'officialSourceSha'],
      ['--profile', 'profile'],
      ['--repo-root', 'repoRoot'],
      ['--host-target', 'hostTarget'],
      ['--summary-path', 'summaryPath'],
    ]);
    return { command, options: parseFields(argv.slice(1), fields) };
  }
  throw new Error('command must be run or finalize');
}

async function main(argv = process.argv.slice(2)) {
  const parsed = parseArgs(argv);
  if (parsed.command === 'run') {
    const result = await runWorkflowContract(parsed.options);
    process.stdout.write(`[ai-gateway-workflow] ${result.status}\n`);
    return result.status === 'pass' ? 0 : 1;
  }
  const input = {
    mainSourceSha: requireFullSha(parsed.options.mainSourceSha, 'main source SHA'),
    officialSourceSha: requireFullSha(parsed.options.officialSourceSha, 'official source SHA'),
    profile: requireCharacterizeProfile(parsed.options.profile),
    hostTarget: parsed.options.hostTarget || 'unavailable',
  };
  const result = finalizeEvidence({
    repoRoot: path.resolve(parsed.options.repoRoot),
    summaryPath: parsed.options.summaryPath,
    fallback: {
      ...workflowResultBase(input),
      status: 'fail',
      cli_smoke: null,
      characterize: null,
      cleanup: { status: 'not-started', errors: [] },
      error: { type: 'Unverified', message: 'workflow stopped before the integration runner produced evidence' },
    },
  });
  return result.status === 'pass' ? 0 : 1;
}

if (require.main === module) {
  main().then((status) => { process.exitCode = status; }).catch((error) => {
    process.stderr.write(`[ai-gateway-workflow] ${error.message}\n`);
    process.exitCode = 1;
  });
}

module.exports = { main, parseArgs, parseFields };
