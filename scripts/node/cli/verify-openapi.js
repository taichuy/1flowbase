#!/usr/bin/env node

const {
  buildCargoCommandEnv,
  getRepoRoot,
  runManagedCommandSequence,
} = require('../testing/warning-capture.js');
const { loadVerifyRuntimeConfig } = require('../testing/verify-runtime.js');

function buildCommands({ cargoJobs, cargoTestThreads, incremental = false }) {
  return [
    {
      label: 'openapi-alignment',
      command: 'cargo',
      args: [
        'test',
        '-p',
        'api-server',
        '--jobs',
        String(cargoJobs),
        'openapi',
        '--',
        `--test-threads=${cargoTestThreads}`,
      ],
      cwd: 'api',
      env: buildCargoCommandEnv({
        cargoParallelism: cargoJobs,
        incremental,
      }),
    },
  ];
}

async function main(_argv = [], deps = {}) {
  const repoRoot = deps.repoRoot || getRepoRoot();
  const env = deps.env || process.env;
  const runtimeConfig = deps.runtimeConfig || loadVerifyRuntimeConfig({ repoRoot, env });
  const managedRunner = deps.managedRunnerImpl || runManagedCommandSequence;

  return managedRunner({
    repoRoot,
    env,
    scope: 'verify-openapi',
    lockMode: 'heavy',
    commandDisplay: 'node scripts/node/cli/verify-openapi.js',
    runtimeConfig,
    commands: buildCommands({
      cargoJobs: runtimeConfig.backend.cargoJobs,
      cargoTestThreads: runtimeConfig.backend.cargoTestThreads,
      incremental: runtimeConfig.backend.incremental,
    }),
    spawnSyncImpl: deps.spawnSyncImpl,
    writeStdout: deps.writeStdout,
    writeStderr: deps.writeStderr,
  });
}

if (require.main === module) {
  Promise.resolve()
    .then(() => main(process.argv.slice(2)))
    .then((status) => {
      process.exitCode = status;
    })
    .catch((error) => {
      process.stderr.write(`[1flowbase-verify-openapi] ${error.message}\n`);
      process.exitCode = 1;
    });
}

module.exports = {
  buildCommands,
  main,
};
