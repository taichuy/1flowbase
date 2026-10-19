const { getRepoRoot, runManagedCommandSequence } = require('../testing/warning-capture.js');
const { loadVerifyRuntimeConfig } = require('../testing/verify-runtime.js');
const { resolveNodeBinaryFromPath } = require('../testing/node-runtime.js');
const { resolveScriptsNodeCliEntry } = require("./command-paths.js");

function buildCiCommands({ repoRoot, env = process.env }) {
  const nodeBinary = resolveNodeBinaryFromPath(env);

  return [
    {
      label: 'ci-verify-repo',
      command: nodeBinary,
      args: [resolveScriptsNodeCliEntry(repoRoot, 'verify'), 'repo'],
      cwd: repoRoot,
    },
    {
      label: 'ci-backend-consistency',
      command: nodeBinary,
      args: [resolveScriptsNodeCliEntry(repoRoot, 'verify'), 'backend-consistency'],
      cwd: repoRoot,
    },
    {
      label: 'ci-coverage-all',
      command: nodeBinary,
      args: [resolveScriptsNodeCliEntry(repoRoot, 'verify'), 'coverage', 'all'],
      cwd: repoRoot,
    },
  ];
}

function usageCi(writeStdout = (text) => process.stdout.write(text)) {
  writeStdout(
    'Usage: node scripts/node/verify-ci.js\n'
      + 'Runs: verify-repo + verify-backend-consistency + verify-coverage all\n'
  );
}

async function runCi(argv = [], deps = {}) {
  if (argv.includes('-h') || argv.includes('--help')) {
    usageCi(deps.writeStdout);
    return 0;
  }

  const repoRoot = deps.repoRoot || getRepoRoot();
  const env = deps.env || process.env;
  const runtimeConfig = deps.runtimeConfig || loadVerifyRuntimeConfig({ repoRoot, env });
  const managedRunner = deps.managedRunnerImpl || runManagedCommandSequence;

  return managedRunner({
    repoRoot,
    env,
    scope: 'verify-ci',
    lockMode: 'heavy',
    commandDisplay: 'node scripts/node/verify-ci.js',
    runtimeConfig,
    commands: buildCiCommands({ repoRoot, env }),
    spawnSyncImpl: deps.spawnSyncImpl,
    writeStdout: deps.writeStdout,
    writeStderr: deps.writeStderr,
  });
}

module.exports = { buildCiCommands, runCi };
