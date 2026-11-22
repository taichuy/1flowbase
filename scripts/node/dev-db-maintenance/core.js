const { spawnSync } = require('node:child_process');
const fs = require('node:fs');
const path = require('node:path');

const {
  buildLocalLoopbackEnv,
  parseApiEnvironment,
  parseEnvFile,
  resolveCommandPath,
} = require('../dev-up/env.js');
const {
  ensureCommandSuccess,
  getMiddlewarePostgresPort,
  runMiddlewareCompose,
} = require('../dev-up/middleware.js');

const LOCAL_POSTGRES_HOSTS = new Set(['127.0.0.1', 'localhost']);
const TEST_SCHEMA_NAME_PATTERN = /^test_[0-9a-f]{32}$/u;
const TEST_BACKUP_DATABASE_NAME_PATTERN = /^test_backup_[0-9a-f]{32}$/u;
const TEST_TEMPLATE_DATABASE_NAME_PATTERN = /^test_tpl_[0-9a-f]{16}$/u;
const BACKUP_DIRECTORY_PATTERN = /^postgres\.(?:empty|backup)-(.+)$/u;
const TEST_SCHEMA_DROP_BATCH_SIZE = 1;
const TEST_DATABASE_DROP_BATCH_SIZE = 1;

function getRepoRoot() {
  return path.resolve(__dirname, '..', '..', '..');
}

function usage(writeStdout = (text) => process.stdout.write(text)) {
  writeStdout(`用法：node scripts/node/dev-db-maintenance/cli.js [test-schemas|test-databases|backups] [选项]

默认命令：test-schemas（同时清理泄漏的 test_* schema 与 test_backup_* 临时库）

选项：
  --apply             真正删除；不带该选项时只 dry-run
  --dry-run           显式 dry-run
  --older-than <age>  只清理早于该时间的对象，支持 d/h/m，test-schemas/test-databases 默认 3d，backups 默认 7d
  --keep <count>      保留最新对象数量，test-schemas/test-databases 默认 20，backups 默认 1
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

    if (arg !== 'test-schemas' && arg !== 'test-databases' && arg !== 'backups') {
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
    throw new Error('无效数据库 URL');
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
    password: decodeURIComponent(parsedUrl.password || ''),
    port: parsedUrl.port || '5432',
    sslMode: parsedUrl.searchParams.get('sslmode') || null,
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

function parseUuidV7Millis(hex32) {
  const millis = Number.parseInt(hex32.slice(0, 12), 16);
  if (!Number.isSafeInteger(millis)) {
    return null;
  }

  return new Date(millis);
}

function parseTestSchemaTimestamp(schemaName) {
  if (!TEST_SCHEMA_NAME_PATTERN.test(schemaName)) {
    return null;
  }

  return parseUuidV7Millis(schemaName.slice(5));
}

function parseTestDatabaseTimestamp(databaseName) {
  if (!TEST_BACKUP_DATABASE_NAME_PATTERN.test(databaseName)) {
    return null;
  }

  return parseUuidV7Millis(databaseName.slice('test_backup_'.length));
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

function normalizeTestDatabaseEntry(entry) {
  if (typeof entry === 'string') {
    const isTemplate = TEST_TEMPLATE_DATABASE_NAME_PATTERN.test(entry);
    return {
      activeBackends: 0,
      allowConnections: !isTemplate,
      isTemplate,
      modifiedAtMs: null,
      name: entry,
    };
  }

  return {
    activeBackends: Number(entry.activeBackends || 0),
    allowConnections: entry.allowConnections !== false,
    isTemplate: Boolean(entry.isTemplate),
    modifiedAtMs:
      typeof entry.modifiedAtMs === 'number' && Number.isFinite(entry.modifiedAtMs)
        ? entry.modifiedAtMs
        : null,
    name: String(entry.name || ''),
  };
}

function buildTestDatabasePrunePlan({
  now = new Date(),
  databases = [],
  olderThanMs,
  keep,
}) {
  const cutoffMillis = now.getTime() - olderThanMs;
  const activeDatabaseNames = [];
  const skippedDatabaseNames = [];
  const keepDatabases = [];
  const dropDatabases = [];
  const backupCandidates = [];
  const sealedTemplates = [];

  for (const rawEntry of databases) {
    const entry = normalizeTestDatabaseEntry(rawEntry);
    const { name } = entry;

    if (TEST_BACKUP_DATABASE_NAME_PATTERN.test(name) && !entry.isTemplate) {
      const createdAt = parseTestDatabaseTimestamp(name);
      if (!createdAt) {
        skippedDatabaseNames.push(name);
        continue;
      }

      if (entry.activeBackends > 0) {
        activeDatabaseNames.push(name);
        keepDatabases.push({ ...entry, createdAt });
        continue;
      }

      backupCandidates.push({ ...entry, createdAt });
      continue;
    }

    if (TEST_TEMPLATE_DATABASE_NAME_PATTERN.test(name)) {
      const createdAt = entry.modifiedAtMs !== null ? new Date(entry.modifiedAtMs) : null;
      if (entry.activeBackends > 0) {
        activeDatabaseNames.push(name);
        keepDatabases.push({ ...entry, createdAt });
        continue;
      }

      if (entry.isTemplate && !entry.allowConnections) {
        sealedTemplates.push({ ...entry, createdAt });
      } else if (createdAt && createdAt.getTime() < cutoffMillis) {
        dropDatabases.push({ isTemplate: entry.isTemplate, name });
      } else {
        keepDatabases.push({ ...entry, createdAt });
      }
      continue;
    }

    skippedDatabaseNames.push(name);
  }

  backupCandidates.sort((left, right) => {
    const timeDiff = right.createdAt.getTime() - left.createdAt.getTime();
    return timeDiff === 0 ? right.name.localeCompare(left.name) : timeDiff;
  });

  backupCandidates.forEach((database, index) => {
    if (index >= keep && database.createdAt.getTime() < cutoffMillis) {
      dropDatabases.push({ isTemplate: false, name: database.name });
    } else {
      keepDatabases.push(database);
    }
  });

  sealedTemplates.sort((left, right) => {
    if (left.createdAt === null && right.createdAt === null) {
      return right.name.localeCompare(left.name);
    }
    if (left.createdAt === null) {
      return -1;
    }
    if (right.createdAt === null) {
      return 1;
    }
    const timeDiff = right.createdAt.getTime() - left.createdAt.getTime();
    return timeDiff === 0 ? right.name.localeCompare(left.name) : timeDiff;
  });

  sealedTemplates.forEach((template, index) => {
    // Always preserve at least the newest active migration template database;
    // only prune superseded older templates once past olderThanMs.
    if (index >= 1 && template.createdAt !== null && template.createdAt.getTime() < cutoffMillis) {
      dropDatabases.push({ isTemplate: true, name: template.name });
    } else {
      keepDatabases.push(template);
    }
  });

  dropDatabases.sort((left, right) => left.name.localeCompare(right.name));
  return {
    activeDatabaseNames: activeDatabaseNames.sort(),
    dropDatabaseNames: dropDatabases.map((entry) => entry.name),
    dropDatabases,
    keepDatabases,
    skippedDatabaseNames: skippedDatabaseNames.sort(),
  };
}

function quotePostgresIdentifier(identifier) {
  return `"${String(identifier).replaceAll('"', '""')}"`;
}

function runPsqlQuery({
  repoRoot,
  database,
  targetDatabaseName = database.databaseName,
  sql,
  runMiddlewareComposeCommand,
  runPsqlCommand,
  sourceEnv = process.env,
}) {
  let result;
  const psqlCommandPath =
    runMiddlewareComposeCommand === undefined
      ? runPsqlCommand
        ? 'psql'
        : resolveCommandPath('psql', { sourceEnv })
      : null;

  if (psqlCommandPath) {
    const pgEnv = {
      ...buildLocalLoopbackEnv(sourceEnv),
      PGDATABASE: targetDatabaseName,
      PGHOST: database.host,
      PGPORT: String(database.port),
      PGUSER: database.user,
    };
    if (database.password) {
      pgEnv.PGPASSWORD = database.password;
    }
    if (database.sslMode) {
      pgEnv.PGSSLMODE = database.sslMode;
    }

    const executePsql = runPsqlCommand || spawnSync;
    result = executePsql(
      psqlCommandPath,
      [
        '-X',
        '-h',
        database.host,
        '-p',
        String(database.port),
        '-U',
        database.user,
        '-d',
        targetDatabaseName,
        '-v',
        'ON_ERROR_STOP=1',
        '-Atc',
        sql,
      ],
      {
        cwd: repoRoot,
        encoding: 'utf8',
        env: pgEnv,
        stdio: ['ignore', 'pipe', 'pipe'],
      }
    );
    if (result.error) {
      throw result.error;
    }
    if (result.status !== 0) {
      const detail = String(result.stderr || result.stdout || '').trim();
      throw new Error(`查询开发数据库失败 (exit code ${result.status})${detail ? `: ${detail}` : ''}`);
    }
  } else {
    const composeRunner = runMiddlewareComposeCommand || runMiddlewareCompose;
    result = composeRunner(
      repoRoot,
      [
        'exec',
        '-T',
        'db',
        'psql',
        '-X',
        '-U',
        database.user,
        '-d',
        targetDatabaseName,
        '-v',
        'ON_ERROR_STOP=1',
        '-Atc',
        sql,
      ],
      { captureOutput: true, allowFailure: true }
    );
    ensureCommandSuccess('查询开发数据库', result);
  }

  return String(result.stdout || '')
    .split(/\r?\n/u)
    .map((line) => line.trim())
    .filter(Boolean);
}

function queryTestSchemaNames({
  repoRoot,
  database,
  runMiddlewareComposeCommand,
  runPsqlCommand,
  sourceEnv,
}) {
  return runPsqlQuery({
    repoRoot,
    database,
    runMiddlewareComposeCommand,
    runPsqlCommand,
    sourceEnv,
    sql: "select nspname from pg_namespace where nspname like 'test\\_%' escape '\\' order by nspname",
  });
}

function dropTestSchemas({
  repoRoot,
  database,
  schemaNames,
  runMiddlewareComposeCommand,
  runPsqlCommand,
  sourceEnv,
}) {
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
    runPsqlCommand,
    sourceEnv,
    sql,
  });
}

function queryTestDatabases({
  repoRoot,
  database,
  runMiddlewareComposeCommand,
  runPsqlCommand,
  sourceEnv,
}) {
  const lines = runPsqlQuery({
    repoRoot,
    database,
    targetDatabaseName: 'postgres',
    runMiddlewareComposeCommand,
    runPsqlCommand,
    sourceEnv,
    sql: [
      'with role_privs as (',
      "  select coalesce(rolsuper or pg_has_role(current_user, 'pg_read_server_files', 'USAGE'), false) as can_stat_files",
      '  from pg_roles',
      '  where rolname = current_user',
      ')',
      'select d.datname',
      "  || '|' || case when d.datistemplate then '1' else '0' end",
      "  || '|' || case when d.datallowconn then '1' else '0' end",
      "  || '|' || (select count(*) from pg_stat_activity a where a.datname = d.datname and a.pid <> pg_backend_pid())",
      "  || '|' || coalesce(",
      '    case',
      '      when (select can_stat_files from role_privs)',
      "        then ((extract(epoch from (pg_stat_file('base/' || d.oid || '/PG_VERSION')).modification) * 1000)::bigint)::text",
      "      else ''",
      '    end,',
      "    ''",
      '  )',
      'from pg_database d',
      "where d.datname like 'test\\_%' escape '\\'",
      'order by d.datname',
    ].join(' '),
  });

  return lines.map((line) => {
    const [name = '', isTemplate = '0', allowConnections = '1', activeCount = '0', mtimeStr = ''] =
      line.split('|');
    const parsedMtime = mtimeStr ? Number.parseInt(mtimeStr, 10) : null;
    return {
      activeBackends: Number.parseInt(activeCount, 10) || 0,
      allowConnections: allowConnections === '1',
      isTemplate: isTemplate === '1',
      modifiedAtMs: Number.isFinite(parsedMtime) ? parsedMtime : null,
      name,
    };
  });
}

function dropTestDatabases({
  repoRoot,
  database,
  databases,
  runMiddlewareComposeCommand,
  runPsqlCommand,
  sourceEnv,
}) {
  for (const rawEntry of databases) {
    const entry = typeof rawEntry === 'string' ? { isTemplate: false, name: rawEntry } : rawEntry;
    const quotedName = quotePostgresIdentifier(entry.name);
    if (entry.isTemplate) {
      runPsqlQuery({
        repoRoot,
        database,
        targetDatabaseName: 'postgres',
        runMiddlewareComposeCommand,
        runPsqlCommand,
        sourceEnv,
        sql: `alter database ${quotedName} with is_template = false`,
      });
    }
    // Single statement per invocation: PostgreSQL forbids DROP DATABASE inside a transaction block.
    runPsqlQuery({
      repoRoot,
      database,
      targetDatabaseName: 'postgres',
      runMiddlewareComposeCommand,
      runPsqlCommand,
      sourceEnv,
      sql: `drop database if exists ${quotedName}`,
    });
  }
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

function logTestDatabasePlan({ action, plan, writeStdout }) {
  writeStdout(
    `[1flowbase-dev-db-maintenance] ${action}: ${plan.dropDatabases.length} test databases\n`
  );
  for (const entry of plan.dropDatabases.slice(0, 50)) {
    writeStdout(`- ${entry.name}\n`);
  }
  if (plan.dropDatabases.length > 50) {
    writeStdout(`- ... ${plan.dropDatabases.length - 50} more\n`);
  }
  if (plan.activeDatabaseNames.length > 0) {
    writeStdout(
      `[1flowbase-dev-db-maintenance] skipped active test databases: ${plan.activeDatabaseNames.length}\n`
    );
  }
  if (plan.skippedDatabaseNames.length > 0) {
    writeStdout(
      `[1flowbase-dev-db-maintenance] skipped non-v7 test-like databases: ${plan.skippedDatabaseNames.length}\n`
    );
  }
}

function logBackupPlan({ action, plan, writeStdout }) {
  writeStdout(`[1flowbase-dev-db-maintenance] ${action}: ${plan.removeDirectories.length} backup directories\n`);
  for (const directory of plan.removeDirectories) {
    writeStdout(`- ${directory.relativePath}\n`);
  }
}

function executeTestDatabasePrune({
  database,
  loadTestDatabases,
  now,
  options,
  removeTestDatabases,
  repoRoot,
  runMiddlewareComposeCommand,
  runPsqlCommand,
  sourceEnv,
  writeStdout,
}) {
  const databases = loadTestDatabases({
    repoRoot,
    database,
    runMiddlewareComposeCommand,
    runPsqlCommand,
    sourceEnv,
  });
  const plan = buildTestDatabasePrunePlan({
    now,
    databases,
    olderThanMs: options.olderThanMs,
    keep: options.keep,
  });

  if (!options.apply) {
    logTestDatabasePlan({ action: 'dry-run would drop', plan, writeStdout });
    return;
  }

  for (let index = 0; index < plan.dropDatabases.length; index += TEST_DATABASE_DROP_BATCH_SIZE) {
    const batch = plan.dropDatabases.slice(index, index + TEST_DATABASE_DROP_BATCH_SIZE);
    removeTestDatabases({
      repoRoot,
      database,
      databases: batch,
      databaseNames: batch.map((entry) => entry.name),
      runMiddlewareComposeCommand,
      runPsqlCommand,
      sourceEnv,
    });
  }
  logTestDatabasePlan({ action: 'dropped', plan, writeStdout });
}

function runTestSchemaMaintenance({
  now,
  options,
  repoRoot,
  sourceEnv,
  loadTestSchemaNames,
  removeTestSchemas,
  loadTestDatabases,
  removeTestDatabases,
  runMiddlewareComposeCommand,
  runPsqlCommand,
  writeStdout,
}) {
  const databaseUrl = resolveDatabaseUrl({ options, repoRoot, sourceEnv });
  const database = assertLocalDevelopmentDatabase({ databaseUrl, repoRoot, sourceEnv });

  if (options.command !== 'test-databases') {
    const schemaLoader = loadTestSchemaNames || queryTestSchemaNames;
    const schemaRemover = removeTestSchemas || dropTestSchemas;
    const schemaNames = schemaLoader({
      repoRoot,
      database,
      runMiddlewareComposeCommand,
      runPsqlCommand,
      sourceEnv,
    });
    const plan = buildTestSchemaPrunePlan({
      now,
      schemaNames,
      olderThanMs: options.olderThanMs,
      keep: options.keep,
    });

    if (!options.apply) {
      logTestSchemaPlan({ action: 'dry-run would drop', plan, writeStdout });
    } else {
      for (let index = 0; index < plan.dropSchemas.length; index += TEST_SCHEMA_DROP_BATCH_SIZE) {
        schemaRemover({
          repoRoot,
          database,
          schemaNames: plan.dropSchemas.slice(index, index + TEST_SCHEMA_DROP_BATCH_SIZE),
          runMiddlewareComposeCommand,
          runPsqlCommand,
          sourceEnv,
        });
      }
      logTestSchemaPlan({ action: 'dropped', plan, writeStdout });
    }
  }

  const hasInjectedSchemaOnlyMocks =
    options.command === 'test-schemas' &&
    loadTestSchemaNames !== undefined &&
    loadTestDatabases === undefined &&
    runMiddlewareComposeCommand === undefined &&
    runPsqlCommand === undefined;

  if (!hasInjectedSchemaOnlyMocks) {
    executeTestDatabasePrune({
      database,
      loadTestDatabases: loadTestDatabases || queryTestDatabases,
      now,
      options,
      removeTestDatabases: removeTestDatabases || dropTestDatabases,
      repoRoot,
      runMiddlewareComposeCommand,
      runPsqlCommand,
      sourceEnv,
      writeStdout,
    });
  }

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
  loadTestDatabases,
  removeTestDatabases,
  runMiddlewareComposeCommand,
  runPsqlCommand,
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
    loadTestDatabases,
    removeTestDatabases,
    runMiddlewareComposeCommand,
    runPsqlCommand,
    writeStdout,
  });
}

module.exports = {
  buildBackupPrunePlan,
  buildTestDatabasePrunePlan,
  buildTestSchemaPrunePlan,
  getRepoRoot,
  parseCliArgs,
  parseTestDatabaseTimestamp,
  runDevDatabaseMaintenance,
  usage,
};
