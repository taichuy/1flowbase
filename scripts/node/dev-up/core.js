const { log, parseCliArgs, selectServiceKeys, shouldManageDocker, usage } = require('./cli.js');
const {
  buildServiceEnv,
  ensureServiceEnvFile,
  getServicePrestartCommands,
  resolveCommandPath,
} = require('./env.js');
const { manageDocker, resolveComposeCommand } = require('./middleware.js');
const {
  listPortOccupantPids,
  manageServices,
  parseWindowsNetstatPortOccupants,
  startService,
  stopService,
  waitForPortToClose,
  waitForServicePort,
} = require('./process.js');
const { runServicePrestartCommands } = require('./postgres-reset.js');
const {
  CARGO_COLD_STARTUP_TIMEOUT_MS,
  DEFAULT_STARTUP_TIMEOUT_MS,
  ensureRuntimeDirs,
  getRepoRoot,
  getRuntimePaths,
  getServiceDefinitions,
} = require('./services.js');

const DEV_DATABASE_MAINTENANCE_HINT_ACTIONS = new Set(['start', 'ensure', 'restart']);

function shouldShowDevDatabaseMaintenanceHint(options) {
  return DEV_DATABASE_MAINTENANCE_HINT_ACTIONS.has(options.action) && options.scope !== 'frontend';
}

function buildDevDatabaseMaintenanceHintLines() {
  return [
    'Development databases are not cleaned automatically by dev-up. When test schemas or backups accumulate, run a dry run first, then replace --dry-run with --apply after review.',
    'test schema: node scripts/node/dev-db-maintenance/cli.js test-schemas --dry-run --older-than 3d --keep 20',
    'Keep only 1 PGDATA backup: node scripts/node/dev-db-maintenance/cli.js backups --dry-run --keep 1 --older-than 7d',
    'Backup cleanup only affects docker/volumes/postgres.empty-* / postgres.backup-* and never deletes the active docker/volumes/postgres directory.',
  ];
}

function writeDevDatabaseMaintenanceHint(writeLog = log) {
  for (const line of buildDevDatabaseMaintenanceHintLines()) {
    writeLog(line);
  }
}

async function main(argv = process.argv.slice(2)) {
  const options = parseCliArgs(argv);
  if (options.help) {
    usage();
    return 0;
  }

  const repoRoot = getRepoRoot();
  const runtimePaths = getRuntimePaths(repoRoot);
  ensureRuntimeDirs(runtimePaths);

  const serviceDefinitions = getServiceDefinitions(repoRoot);
  const services = selectServiceKeys(options.scope).map((key) => serviceDefinitions[key]);

  if (shouldManageDocker(options)) {
    await manageDocker(repoRoot, options.action);
  } else if (options.skipDocker) {
    log('Skipped Docker middleware management');
  }

  if (shouldShowDevDatabaseMaintenanceHint(options)) {
    writeDevDatabaseMaintenanceHint();
  }

  await manageServices(options.action, services);
  return 0;
}

module.exports = {
  CARGO_COLD_STARTUP_TIMEOUT_MS,
  DEFAULT_STARTUP_TIMEOUT_MS,
  buildDevDatabaseMaintenanceHintLines,
  buildServiceEnv,
  ensureServiceEnvFile,
  getRepoRoot,
  getRuntimePaths,
  getServiceDefinitions,
  getServicePrestartCommands,
  listPortOccupantPids,
  main,
  manageDocker,
  manageServices,
  parseWindowsNetstatPortOccupants,
  parseCliArgs,
  resolveComposeCommand,
  resolveCommandPath,
  runServicePrestartCommands,
  selectServiceKeys,
  shouldManageDocker,
  shouldShowDevDatabaseMaintenanceHint,
  startService,
  stopService,
  waitForPortToClose,
  waitForServicePort,
};
