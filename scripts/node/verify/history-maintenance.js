const { buildCargoCommandEnv } = require('../testing/warning-capture.js');

// The maintenance executable is feature-gated; default workspace targets do not
// compile it. Keep the existing service, adapter and real PostgreSQL evidence.
function buildHistoryMaintenanceCommands({ cargoJobs, cargoTestThreads, incremental = false }) {
  const env = buildCargoCommandEnv({ cargoParallelism: cargoJobs, incremental });
  const command = (label, args) => ({
    label, command: 'cargo', args: [...args, '--jobs', String(cargoJobs)], cwd: 'api', env,
  });
  const tests = [
    ['postgres', 'control-plane-postgres-tests', ['--test', 'responses_history_migration']],
    ['service', 'control-plane', ['--lib', 'application_public_api::compat::openai::history']],
    ['storage', 'storage-durable-postgres', ['--lib', 'responses_history_migration']],
    ['cli', 'api-server', ['--bin', 'responses-history-migrate']],
  ].map(([name, pkg, selector]) => {
    const entry = command(`cargo-test-history-maintenance-${name}`, [
      'test', '--locked', '-p', pkg, '--features', 'responses-history-migration', ...selector,
    ]);
    entry.args.push('--', `--test-threads=${cargoTestThreads}`);
    return entry;
  });
  const clippy = command('cargo-clippy-history-maintenance', [
    'clippy', '--locked', '-p', 'api-server', '-p', 'control-plane',
    '-p', 'control-plane-contracts', '-p', 'storage-durable-postgres',
    '-p', 'control-plane-postgres-tests', '--all-targets', '--features',
    'api-server/responses-history-migration,control-plane-postgres-tests/responses-history-migration',
  ]);
  clippy.args.push('--', '-D', 'warnings');
  return [
    ...tests,
    command('cargo-build-history-maintenance', [
      'build', '--locked', '-p', 'api-server', '--bin', 'responses-history-migrate',
      '--features', 'responses-history-migration',
    ]),
    clippy,
  ];
}

module.exports = { buildHistoryMaintenanceCommands };
