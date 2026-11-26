#!/usr/bin/env node

const { spawnSync } = require('node:child_process');

const { buildNodePreferredEnv } = require('../testing/node-runtime.js');
const { getRepoRoot } = require('../testing/warning-capture.js');
const { isCiEnvironment, loadVerifyRuntimeConfig } = require('../testing/verify-runtime.js');

const { getLocalCpuBudget } = require('../testing/resources/budget.js');

function normalizePassThroughArgs(args) {
  if (args[0] === '--') {
    return args.slice(1);
  }

  return args;
}

function buildWorkspaceTestCommand({ runtimeConfig, passThroughArgs = [], cpuBudget }) {
  if (passThroughArgs.some((argument) => /^--(?:concurrency|parallel|maxWorkers)(?:=|$)/u.test(argument))) {
    throw new Error('Workspace test concurrency is managed by the repository runtime configuration');
  }
  const turboConcurrency = Math.min(runtimeConfig.frontend.turboConcurrency, cpuBudget ?? Infinity);
  const vitestMaxWorkers = Math.min(runtimeConfig.frontend.vitestMaxWorkers ?? 1,
    Math.floor((cpuBudget ?? Infinity) / turboConcurrency));
  return {
    command: 'pnpm',
    args: [
      '--dir',
      'web',
      'exec',
      'turbo',
      'run',
      'test',
      `--concurrency=${turboConcurrency}`,
      ...passThroughArgs,
    ],
    cwd: '.',
    ...(cpuBudget === undefined ? {} : { env: { VITEST_MAX_WORKERS: String(vitestMaxWorkers) } }),
  };
}

function main(argv = [], deps = {}) {
  const repoRoot = deps.repoRoot || getRepoRoot();
  const env = deps.env || process.env;
  const runtimeConfig = deps.runtimeConfig || loadVerifyRuntimeConfig({ repoRoot, env });
  const spawnSyncImpl = deps.spawnSyncImpl || spawnSync;

  const command = buildWorkspaceTestCommand({
    runtimeConfig,
    passThroughArgs: normalizePassThroughArgs(argv),
    cpuBudget: isCiEnvironment(env) ? undefined : getLocalCpuBudget(),
  });
  const { env: commandEnv } = buildNodePreferredEnv(env);

  const result = spawnSyncImpl(command.command, command.args, {
    cwd: repoRoot,
    env: { ...commandEnv, ...command.env },
    stdio: 'inherit',
  });

  if (result.error) {
    throw result.error;
  }

  return result.status ?? 1;
}

if (require.main === module) {
  try {
    process.exitCode = main(process.argv.slice(2));
  } catch (error) {
    process.stderr.write(`[1flowbase-run-frontend-workspace-test] ${error.message}\n`);
    process.exitCode = 1;
  }
}

module.exports = {
  buildWorkspaceTestCommand,
  main,
};
