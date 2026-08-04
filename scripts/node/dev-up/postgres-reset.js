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
