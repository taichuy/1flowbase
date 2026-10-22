const path = require('node:path');

function withPostgresMetricsEnv({ repoRoot, scope, env }) {
  return {
    ...env,
    POSTGRES_TEST_METRICS_DIR: path.join(repoRoot, 'tmp/test-governance/postgres-fixtures', scope),
  };
}

function isApiTestCommand({ command, args }) {
  if (command !== 'cargo') return false;
  const cargoArgs = args.slice(0, args.indexOf('--') < 0 ? args.length : args.indexOf('--'));
  if (cargoArgs.includes('--doc') || cargoArgs.includes('--no-run')) return false;
  const runsTests = cargoArgs[0] === 'test'
    || (cargoArgs[0] === 'nextest' && cargoArgs[1] === 'run')
    || (cargoArgs[0] === 'llvm-cov' && (cargoArgs[1]?.startsWith('--')
      || (cargoArgs[1] === 'nextest' && cargoArgs[2] !== 'list')));
  const excludesApi = cargoArgs.some((arg, index) => arg === '--exclude' && cargoArgs[index + 1] === 'api-server');
  return runsTests && ((!excludesApi && cargoArgs.includes('--workspace'))
    || cargoArgs.some((arg, index) => ['-p', '--package'].includes(arg) && cargoArgs[index + 1] === 'api-server'));
}

function buildPostgresTemplateCommands({ repoRoot, command, env }) {
  if (!isApiTestCommand(command)) return [command];
  const commandEnv = { ...env, ...command.env };
  const cwd = path.resolve(repoRoot, command.cwd || '.');
  // Raw cargo build and show-env + nextest use this existing base target.
  // A wrapped cargo llvm-cov retains its own llvm-cov-target below the base.
  const targetDir = path.resolve(cwd, commandEnv.CARGO_TARGET_DIR || 'target');
  const executable = path.join(targetDir, 'debug', `with-postgres-template${process.platform === 'win32' ? '.exe' : ''}`);
  return [
    {
      label: `postgres-template-build-${command.label}`,
      command: 'cargo',
      args: ['build', '--locked', '--package', 'postgres-test-support', '--bin', 'with-postgres-template', '--profile', 'test'],
      cwd: command.cwd,
      env: command.env,
    },
    {
      ...command,
      command: executable,
      args: ['--', command.command, ...command.args],
    },
  ];
}

module.exports = { buildPostgresTemplateCommands, isApiTestCommand, withPostgresMetricsEnv };
