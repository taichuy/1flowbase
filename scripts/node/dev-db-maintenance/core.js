const fs = require('node:fs');
const path = require('node:path');

const { parseApiEnvironment, parseEnvFile } = require('../dev-up/env.js');
const {
  ensureCommandSuccess,
  getMiddlewarePostgresPort,
  runMiddlewareCompose,
} = require('../dev-up/middleware.js');

const LOCAL_POSTGRES_HOSTS = new Set(['127.0.0.1', 'localhost']);
const TEST_SCHEMA_NAME_PATTERN = /^test_[0-9a-f]{32}$/u;
const BACKUP_DIRECTORY_PATTERN = /^postgres\.(?:empty|backup)-(.+)$/u;
const TEST_SCHEMA_DROP_BATCH_SIZE = 1;

function getRepoRoot() {
  return path.resolve(__dirname, '..', '..', '..');
}

function usage(writeStdout = (text) => process.stdout.write(text)) {
  writeStdout(`用法：node scripts/node/dev-db-maintenance/cli.js [test-schemas|backups] [选项]

默认命令：test-schemas

选项：
  --apply             真正删除；不带该选项时只 dry-run
  --dry-run           显式 dry-run
  --older-than <age>  只清理早于该时间的对象，支持 d/h/m，test-schemas 默认 3d，backups 默认 7d
  --keep <count>      保留最新对象数量，test-schemas 默认 20，backups 默认 1
  --database-url <u>  指定开发数据库 URL；默认读取 API_DATABASE_URL / DATABASE_URL / api-server .env
  -h, --help          查看帮助
`);
}

function parseDurationMs(value) {
  const match = /^(\d+)([dhm])$/u.exec(String(value || '').trim());
  if (!match) {
    throw new Error(`无效时间长度：${value}`);
  }

  const amount = Number.parseInt(match[1], 10);
  if (!Number.isInteger(amount) || amount < 0) {
    throw new Error(`无效时间长度：${value}`);
  }

  const unitMillis = {
    d: 24 * 60 * 60 * 1000,
    h: 60 * 60 * 1000,
    m: 60 * 1000,
  };
  return amount * unitMillis[match[2]];
}

function parseCliArgs(argv = []) {
  const options = {
    apply: false,
    command: 'test-schemas',
    databaseUrl: null,
    help: false,
    keep: null,
    olderThanMs: null,
  };
  let commandSpecified = false;

  for (let index = 0; index < argv.length; index += 1) {
    const arg = argv[index];

    if (arg === '-h' || arg === '--help') {
      options.help = true;
      continue;
    }

    if (arg === '--apply') {
      options.apply = true;
      continue;
    }

    if (arg === '--dry-run') {
      options.apply = false;
      continue;
    }

    if (arg === '--older-than') {
      const value = argv[index + 1];
      if (!value || value.startsWith('-')) {
        throw new Error('--older-than 缺少值');
      }
      options.olderThanMs = parseDurationMs(value);
      index += 1;
      continue;
    }

    if (arg === '--keep') {
      const value = argv[index + 1];
      if (!value || value.startsWith('-')) {
        throw new Error('--keep 缺少值');
      }
      const parsedValue = Number.parseInt(value, 10);
      if (!Number.isInteger(parsedValue) || parsedValue < 0 || String(parsedValue) !== value) {
        throw new Error(`无效保留数量：${value}`);
      }
      options.keep = parsedValue;
      index += 1;
      continue;
    }

    if (arg === '--database-url') {
      const value = argv[index + 1];
      if (!value || value.startsWith('-')) {
        throw new Error('--database-url 缺少值');
      }
      options.databaseUrl = value;
      index += 1;
      continue;
    }

    if (arg.startsWith('-')) {
      throw new Error(`未知选项：${arg}`);
    }

    if (commandSpecified) {
      throw new Error(`只能指定一个命令，收到多余参数：${arg}`);
    }

    if (arg !== 'test-schemas' && arg !== 'backups') {
      throw new Error(`未知命令：${arg}`);
    }

    options.command = arg;
    commandSpecified = true;
  }

  if (options.command === 'backups') {
    options.keep ??= 1;
    options.olderThanMs ??= 7 * 24 * 60 * 60 * 1000;
  } else {
    options.keep ??= 20;
    options.olderThanMs ??= 3 * 24 * 60 * 60 * 1000;
  }

  return options;
}

function parsePostgresDatabaseUrl(databaseUrl) {
  if (!databaseUrl) {
    throw new Error('缺少数据库 URL');
  }

  let parsedUrl;
  try {
    parsedUrl = new URL(databaseUrl);
  } catch (_error) {
    throw new Error(`无效数据库 URL：${databaseUrl}`);
  }

  if (parsedUrl.protocol !== 'postgres:' && parsedUrl.protocol !== 'postgresql:') {
    throw new Error(`无效数据库协议：${parsedUrl.protocol}`);
  }

  const databaseName = decodeURIComponent(parsedUrl.pathname.replace(/^\/+/, ''));
  if (!databaseName) {
    throw new Error('数据库 URL 缺少 database name');
  }

  return {
    databaseName,
    host: parsedUrl.hostname.trim().toLowerCase(),
    port: parsedUrl.port || '5432',
    user: decodeURIComponent(parsedUrl.username || 'postgres'),
  };
}

function readApiServerEnv(repoRoot) {
  return parseEnvFile(path.join(repoRoot, 'api', 'apps', 'api-server', '.env'));
}

function resolveDatabaseUrl({ options, repoRoot, sourceEnv }) {
  const fileEnv = readApiServerEnv(repoRoot);
  return (
    options.databaseUrl ||
    sourceEnv.API_DATABASE_URL ||
    sourceEnv.DATABASE_URL ||
    fileEnv.API_DATABASE_URL ||
    fileEnv.DATABASE_URL ||
    null
  );
}

function assertLocalDevelopmentDatabase({ databaseUrl, repoRoot, sourceEnv }) {
  const fileEnv = readApiServerEnv(repoRoot);
  const apiEnvironment = parseApiEnvironment(sourceEnv.API_ENV || fileEnv.API_ENV || 'development');
  if (apiEnvironment === 'production') {
    throw new Error('拒绝维护 production 数据库');
  }

  const database = parsePostgresDatabaseUrl(databaseUrl);
  if (!LOCAL_POSTGRES_HOSTS.has(database.host)) {
    throw new Error(`只允许维护本地开发数据库，当前 host=${database.host}`);
  }

  const expectedPort = getMiddlewarePostgresPort(repoRoot);
  if (database.port !== expectedPort) {
    throw new Error(`只允许维护 middleware Postgres 端口 ${expectedPort}，当前 port=${database.port}`);
  }

  return database;
}

function parseTestSchemaTimestamp(schemaName) {
  if (!TEST_SCHEMA_NAME_PATTERN.test(schemaName)) {
    return null;
  }

  const millis = Number.parseInt(schemaName.slice(5, 17), 16);
  if (!Number.isSafeInteger(millis)) {
    return null;
  }

  return new Date(millis);
}

function buildTestSchemaPrunePlan({
  now = new Date(),
  schemaNames,
  olderThanMs,
  keep,
}) {
  const cutoffMillis = now.getTime() - olderThanMs;
  const skippedSchemaNames = [];
  const candidates = [];

  for (const schemaName of schemaNames) {
    const createdAt = parseTestSchemaTimestamp(schemaName);
    if (!createdAt) {
      skippedSchemaNames.push(schemaName);
      continue;
    }

    candidates.push({ name: schemaName, createdAt });
  }

  candidates.sort((left, right) => {
    const timeDiff = right.createdAt.getTime() - left.createdAt.getTime();
    return timeDiff === 0 ? right.name.localeCompare(left.name) : timeDiff;
  });

  const keepSchemas = [];
  const dropSchemas = [];
  candidates.forEach((schema, index) => {
    if (index >= keep && schema.createdAt.getTime() < cutoffMillis) {
      dropSchemas.push(schema.name);
    } else {
      keepSchemas.push(schema);
    }
  });

  dropSchemas.sort();
  return { dropSchemas, keepSchemas, skippedSchemaNames: skippedSchemaNames.sort() };
}

function quotePostgresIdentifier(identifier) {
  return `"${String(identifier).replaceAll('"', '""')}"`;
}

function runPsqlQuery({ repoRoot, database, sql, runMiddlewareComposeCommand = runMiddlewareCompose }) {
  const result = runMiddlewareComposeCommand(
    repoRoot,
    [
      'exec',
      '-T',
      'db',
      'psql',
      '-U',
      database.user,
      '-d',
      database.databaseName,
      '-Atc',
      sql,
    ],
    { captureOutput: true, allowFailure: true }
  );
  ensureCommandSuccess('查询开发数据库', result);
  return String(result.stdout || '')
    .split(/\r?\n/u)
    .map((line) => line.trim())
    .filter(Boolean);
}

function queryTestSchemaNames({ repoRoot, database, runMiddlewareComposeCommand }) {
  return runPsqlQuery({
    repoRoot,
    database,
    runMiddlewareComposeCommand,
    sql: "select nspname from pg_namespace where nspname like 'test\\_%' escape '\\' order by nspname",
  });
}

function dropTestSchemas({ repoRoot, database, schemaNames, runMiddlewareComposeCommand }) {
  if (schemaNames.length === 0) {
    return;
  }

  const sql = schemaNames
    .map((schemaName) => `drop schema if exists ${quotePostgresIdentifier(schemaName)} cascade;`)
    .join('\n');
  runPsqlQuery({
    repoRoot,
    database,
    runMiddlewareComposeCommand,
    sql,
  });
}

function parseBackupTimestamp(directoryName, stats) {
  const match = BACKUP_DIRECTORY_PATTERN.exec(directoryName);
  if (!match) {
    return null;
  }

  const compactMatch = /^(\d{4})(\d{2})(\d{2})-(\d{2})(\d{2})(\d{2})$/u.exec(match[1]);
  if (compactMatch) {
    return new Date(
      Date.UTC(
        Number.parseInt(compactMatch[1], 10),
        Number.parseInt(compactMatch[2], 10) - 1,
        Number.parseInt(compactMatch[3], 10),
        Number.parseInt(compactMatch[4], 10),
        Number.parseInt(compactMatch[5], 10),
        Number.parseInt(compactMatch[6], 10)
      )
    );
  }

  return stats.mtime;
}

function buildBackupPrunePlan({
  now = new Date(),
  repoRoot = getRepoRoot(),
  olderThanMs,
  keep,
}) {
  const volumesDir = path.join(repoRoot, 'docker', 'volumes');
  if (!fs.existsSync(volumesDir)) {
    return { removeDirectories: [], keepDirectories: [] };
  }

  const candidates = fs.readdirSync(volumesDir, { withFileTypes: true })
    .filter((entry) => entry.isDirectory() && BACKUP_DIRECTORY_PATTERN.test(entry.name))
    .map((entry) => {
      const absolutePath = path.join(volumesDir, entry.name);
      const stats = fs.statSync(absolutePath);
      return {
        absolutePath,
        createdAt: parseBackupTimestamp(entry.name, stats),
        relativePath: path.join('docker', 'volumes', entry.name),
      };
    });

  candidates.sort((left, right) => {
    const timeDiff = right.createdAt.getTime() - left.createdAt.getTime();
    return timeDiff === 0 ? right.relativePath.localeCompare(left.relativePath) : timeDiff;
  });

  const cutoffMillis = now.getTime() - olderThanMs;
  const keepDirectories = [];
  const removeDirectories = [];
  candidates.forEach((directory, index) => {
    if (index >= keep && directory.createdAt.getTime() < cutoffMillis) {
      removeDirectories.push(directory);
    } else {
      keepDirectories.push(directory);
    }
  });

  removeDirectories.sort((left, right) => left.relativePath.localeCompare(right.relativePath));
  return { removeDirectories, keepDirectories };
}

function logTestSchemaPlan({ action, plan, writeStdout }) {
  writeStdout(`[1flowbase-dev-db-maintenance] ${action}: ${plan.dropSchemas.length} test schemas\n`);
  for (const schemaName of plan.dropSchemas.slice(0, 50)) {
    writeStdout(`- ${schemaName}\n`);
  }
  if (plan.dropSchemas.length > 50) {
    writeStdout(`- ... ${plan.dropSchemas.length - 50} more\n`);
  }
  if (plan.skippedSchemaNames.length > 0) {
    writeStdout(`[1flowbase-dev-db-maintenance] skipped non-v7 test-like schemas: ${plan.skippedSchemaNames.length}\n`);
  }
}

function logBackupPlan({ action, plan, writeStdout }) {
  writeStdout(`[1flowbase-dev-db-maintenance] ${action}: ${plan.removeDirectories.length} backup directories\n`);
  for (const directory of plan.removeDirectories) {
    writeStdout(`- ${directory.relativePath}\n`);
  }
}

function runTestSchemaMaintenance({
  now,
  options,
  repoRoot,
  sourceEnv,
  loadTestSchemaNames = queryTestSchemaNames,
  removeTestSchemas = dropTestSchemas,
  runMiddlewareComposeCommand = runMiddlewareCompose,
  writeStdout,
}) {
  const databaseUrl = resolveDatabaseUrl({ options, repoRoot, sourceEnv });
  const database = assertLocalDevelopmentDatabase({ databaseUrl, repoRoot, sourceEnv });
  const schemaNames = loadTestSchemaNames({ repoRoot, database, runMiddlewareComposeCommand });
  const plan = buildTestSchemaPrunePlan({
    now,
    schemaNames,
    olderThanMs: options.olderThanMs,
    keep: options.keep,
  });

  if (!options.apply) {
    logTestSchemaPlan({ action: 'dry-run would drop', plan, writeStdout });
    return 0;
  }

  for (let index = 0; index < plan.dropSchemas.length; index += TEST_SCHEMA_DROP_BATCH_SIZE) {
    removeTestSchemas({
      repoRoot,
      database,
      schemaNames: plan.dropSchemas.slice(index, index + TEST_SCHEMA_DROP_BATCH_SIZE),
      runMiddlewareComposeCommand,
    });
  }
  logTestSchemaPlan({ action: 'dropped', plan, writeStdout });
  return 0;
}

function runBackupMaintenance({ now, options, repoRoot, writeStdout }) {
  const plan = buildBackupPrunePlan({
    now,
    repoRoot,
    olderThanMs: options.olderThanMs,
    keep: options.keep,
  });

  if (!options.apply) {
    logBackupPlan({ action: 'dry-run would remove', plan, writeStdout });
    return 0;
  }

  for (const directory of plan.removeDirectories) {
    fs.rmSync(directory.absolutePath, { recursive: true, force: true });
  }
  logBackupPlan({ action: 'removed', plan, writeStdout });
  return 0;
}

function runDevDatabaseMaintenance({
  now = new Date(),
  options = parseCliArgs([]),
  repoRoot = getRepoRoot(),
  sourceEnv = process.env,
  loadTestSchemaNames,
  removeTestSchemas,
  runMiddlewareComposeCommand,
  writeStdout = (text) => process.stdout.write(text),
} = {}) {
  if (options.help) {
    usage(writeStdout);
    return 0;
  }

  if (options.command === 'backups') {
    return runBackupMaintenance({ now, options, repoRoot, writeStdout });
  }

  return runTestSchemaMaintenance({
    now,
    options,
    repoRoot,
    sourceEnv,
    loadTestSchemaNames,
    removeTestSchemas,
    runMiddlewareComposeCommand,
    writeStdout,
  });
}

module.exports = {
  buildBackupPrunePlan,
  buildTestSchemaPrunePlan,
  getRepoRoot,
  parseCliArgs,
  runDevDatabaseMaintenance,
  usage,
};
