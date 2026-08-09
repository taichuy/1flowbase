const crypto = require('node:crypto');
const fs = require('node:fs');
const path = require('node:path');

const { log } = require('./cli.js');
const {
  buildServiceEnv,
  getServicePrestartCommands,
  parseApiEnvironment,
} = require('./env.js');
const {
  ensureCommandSuccess,
  getMiddlewarePostgresPort,
  runCommand,
  runMiddlewareCompose,
  writeCommandOutput,
} = require('./middleware.js');

const LOCAL_POSTGRES_HOSTS = new Set(['127.0.0.1', 'localhost']);
const ALLOW_DB_RESET_ENV = 'ONEFLOWBASE_DEV_UP_ALLOW_DB_RESET';
const KNOWN_EQUIVALENT_MIGRATION_DRIFTS = new Map([
  [
    '20260808230000',
    {
      relativePath:
        'api/crates/storage-durable/postgres/migrations/20260808230000_add_user_attribution_to_provider_request_logs.sql',
      appliedChecksum:
        '65656b2b49acc6f0c034d3df7440f5113f08d14613279d55aacc7463948c783c1a1aeb3f667500cd18245bf2a493db66',
      resolvedChecksum:
        '5f8137b467d8d6d16aa5416407b64249ba43e17ea452e37e4811e7b4d7cb5502cd53ad975f2df41615f57dc56e5a9811',
    },
  ],
]);

function getCommandOutput(result) {
  return [result?.stdout, result?.stderr, result?.error?.message].filter(Boolean).join('\n');
}

function isRecoverableMigrationDrift(result) {
  const output = getCommandOutput(result);
  return (
    output.includes('was previously applied but has been modified') ||
    output.includes('was previously applied but is missing in the resolved migrations')
  );
}

function isExplicitResetOptInEnabled(env) {
  return ['1', 'true', 'yes'].includes(
    String(env?.[ALLOW_DB_RESET_ENV] || '')
      .trim()
      .toLowerCase()
  );
}

function quotePostgresIdentifier(identifier) {
  return `"${String(identifier).replaceAll('"', '""')}"`;
}

function extractModifiedMigrationVersion(result) {
  return getCommandOutput(result).match(
    /migration (\d+) was previously applied but has been modified/iu
  )?.[1];
}

function parsePostgresDatabaseUrl(databaseUrl) {
  if (!databaseUrl) {
    return null;
  }

  let parsedUrl;
  try {
    parsedUrl = new URL(databaseUrl);
  } catch (_error) {
    return null;
  }

  if (parsedUrl.protocol !== 'postgres:' && parsedUrl.protocol !== 'postgresql:') {
    return null;
  }

  const databaseName = decodeURIComponent(parsedUrl.pathname.replace(/^\/+/, ''));
  if (!databaseName) {
    return null;
  }

  return {
    host: parsedUrl.hostname.trim().toLowerCase(),
    port: parsedUrl.port || '5432',
    user: decodeURIComponent(parsedUrl.username || 'postgres'),
    databaseName,
  };
}

function buildLocalPostgresResetPlan(service, databaseUrl) {
  if (!service?.repoRoot) {
    return null;
  }

  const database = parsePostgresDatabaseUrl(databaseUrl);
  if (!database || !LOCAL_POSTGRES_HOSTS.has(database.host)) {
    return null;
  }

  const expectedPort = getMiddlewarePostgresPort(service.repoRoot);
  if (database.port !== expectedPort) {
    return null;
  }

  const quotedDatabaseName = quotePostgresIdentifier(database.databaseName);
  return {
    databaseName: database.databaseName,
    commands: [
      {
        description: `Rebuild development database ${database.databaseName}`,
        args: [
          'exec',
          '-T',
          'db',
          'psql',
          '-U',
          database.user,
          '-d',
          'postgres',
          '-c',
          `DROP DATABASE IF EXISTS ${quotedDatabaseName} WITH (FORCE);`,
        ],
      },
      {
        description: `Create development database ${database.databaseName}`,
        args: [
          'exec',
          '-T',
          'db',
          'psql',
          '-U',
          database.user,
          '-d',
          'postgres',
          '-c',
          `CREATE DATABASE ${quotedDatabaseName};`,
        ],
      },
    ],
  };
}

function tryRepairKnownEquivalentMigrationDrift(
  service,
  prestartCommand,
  result,
  { runMiddlewareComposeImpl = runMiddlewareCompose, logImpl = log } = {}
) {
  const version = extractModifiedMigrationVersion(result);
  const drift = KNOWN_EQUIVALENT_MIGRATION_DRIFTS.get(version);
  if (!drift) {
    return false;
  }

  const database = parsePostgresDatabaseUrl(prestartCommand.env.API_DATABASE_URL);
  const expectedPort = getMiddlewarePostgresPort(service.repoRoot);
  if (
    !database ||
    !LOCAL_POSTGRES_HOSTS.has(database.host) ||
    database.port !== expectedPort
  ) {
    return false;
  }

  const migrationPath = path.join(service.repoRoot, drift.relativePath);
  if (!fs.existsSync(migrationPath)) {
    return false;
  }

  const resolvedChecksum = crypto
    .createHash('sha384')
    .update(fs.readFileSync(migrationPath))
    .digest('hex');
  if (resolvedChecksum !== drift.resolvedChecksum) {
    return false;
  }

  const repairSql = `with repaired as (
    update _sqlx_migrations
    set checksum = decode('${drift.resolvedChecksum}', 'hex')
    where version = ${version}
      and success
      and checksum = decode('${drift.appliedChecksum}', 'hex')
    returning 1
  )
  select count(*) from repaired;`;
  const repairResult = runMiddlewareComposeImpl(
    service.repoRoot,
    [
      'exec',
      '-T',
      'db',
      'psql',
      '-U',
      database.user,
      '-d',
      database.databaseName,
      '-X',
      '-A',
      '-t',
      '-v',
      'ON_ERROR_STOP=1',
      '-c',
      repairSql,
    ],
    { captureOutput: true, allowFailure: true }
  );
  ensureCommandSuccess(`Repair development migration ${version} checksum`, repairResult);
  if (repairResult.stdout.trim() !== '1') {
    return false;
  }

  logImpl(
    `${service.label} repaired known equivalent local migration ${version} checksum without rebuilding database`
  );
  return true;
}

function tryRecoverApiServerPrestartFailure(
  service,
  prestartCommand,
  result,
  { runMiddlewareComposeImpl = runMiddlewareCompose, logImpl = log } = {}
) {
  if (!service || service.key !== 'api-server' || !prestartCommand?.env) {
    return false;
  }

  if (parseApiEnvironment(prestartCommand.env.API_ENV) === 'production') {
    return false;
  }

  if (!isRecoverableMigrationDrift(result)) {
    return false;
  }

  if (
    tryRepairKnownEquivalentMigrationDrift(service, prestartCommand, result, {
      runMiddlewareComposeImpl,
      logImpl,
    })
  ) {
    return true;
  }

  if (!isExplicitResetOptInEnabled(prestartCommand.env)) {
    logImpl(
      `${service.label} detected local development database migration records that do not match the current repository; ` +
        `automatic rebuild was stopped to prevent data loss. Set ${ALLOW_DB_RESET_ENV}=1 and retry only after confirming that the local database may be cleared`
    );
    return false;
  }

  const resetPlan = buildLocalPostgresResetPlan(service, prestartCommand.env.API_DATABASE_URL);
  if (!resetPlan) {
    return false;
  }

  logImpl(
    `${service.label} detected local development database migration records that do not match the current repository; rebuilding database ${resetPlan.databaseName}`
  );

  for (const command of resetPlan.commands) {
    const resetResult = runMiddlewareComposeImpl(service.repoRoot, command.args, {
      captureOutput: true,
      allowFailure: true,
    });
    ensureCommandSuccess(command.description, resetResult);
  }

  logImpl(`${service.label} rebuilt database ${resetPlan.databaseName}; retrying the pre-start step`);
  return true;
}

function runServicePrestartCommands(
  service,
  {
    sourceEnv = process.env,
    runCommandImpl = runCommand,
    runMiddlewareComposeImpl = runMiddlewareCompose,
    logImpl = log,
  } = {}
) {
  for (const prestartCommand of getServicePrestartCommands(service, sourceEnv)) {
    logImpl(`${service.label} running pre-start step: ${prestartCommand.description}`);
    let recovered = false;

    while (true) {
      const result = runCommandImpl(prestartCommand.command, prestartCommand.args, {
        cwd: prestartCommand.cwd,
        env: prestartCommand.env,
        captureOutput: prestartCommand.captureOutput !== false,
      });

      if (!result.error && result.status === 0) {
        writeCommandOutput(result);
        break;
      }

      writeCommandOutput(result);

      if (
        !recovered &&
        tryRecoverApiServerPrestartFailure(service, prestartCommand, result, {
          runMiddlewareComposeImpl,
          logImpl,
        })
      ) {
        recovered = true;
        continue;
      }

      if (result.error) {
        throw result.error;
      }

      throw new Error(`${prestartCommand.description} failed with exit code ${result.status}`);
    }
  }
}

module.exports = {
  runServicePrestartCommands,
};
