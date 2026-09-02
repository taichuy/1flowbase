const crypto = require('node:crypto');
const fs = require('node:fs');
const path = require('node:path');

const { runPsql } = require('./core.js');

const MIGRATIONS_TABLE_MISSING_MARKER = '__1FLOWBASE_MIGRATIONS_TABLE_MISSING__';
const READ_APPLIED_MIGRATIONS_SQL = String.raw`\set QUIET 1
select to_regclass('public._sqlx_migrations') is not null as migrations_table_exists \gset
\if :migrations_table_exists
select version::text || '|' || encode(checksum, 'hex')
from public._sqlx_migrations
where success
order by version;
\else
\echo ${MIGRATIONS_TABLE_MISSING_MARKER}
\endif
`;

function loadResolvedMigrations(repoRoot) {
  const migrationsDir = path.join(
    repoRoot,
    'api',
    'crates',
    'storage',
    'durable',
    'postgres',
    'migrations'
  );
  const resolved = new Map();
  for (const fileName of fs.readdirSync(migrationsDir)) {
    const version = fileName.match(/^(\d+)_.*\.sql$/u)?.[1];
    if (!version) {
      continue;
    }
    const checksum = crypto
      .createHash('sha384')
      .update(fs.readFileSync(path.join(migrationsDir, fileName)))
      .digest('hex');
    resolved.set(version, checksum);
  }
  return resolved;
}

function readAppliedMigrations(databaseUrl, runPsqlImpl) {
  const result = runPsqlImpl({
    args: ['-X', '-A', '-t', '-v', 'ON_ERROR_STOP=1'],
    input: READ_APPLIED_MIGRATIONS_SQL,
    databaseUrl,
  });
  if (result.error) {
    throw result.error;
  }
  if (result.status !== 0) {
    const detail = String(result.stderr || result.stdout || '').trim();
    throw new Error(`psql failed with exit code ${result.status}${detail ? `: ${detail}` : ''}`);
  }
  const output = String(result.stdout || '').trim();
  if (!output || output.includes(MIGRATIONS_TABLE_MISSING_MARKER)) {
    return [];
  }
  return output.split(/\r?\n/u).filter(Boolean).map((line) => {
    const [version, checksum] = line.trim().split('|');
    if (!version || !checksum) {
      throw new Error(`invalid applied migration row: ${line}`);
    }
    return { version, checksum };
  });
}

function validateAppliedMigrations({
  repoRoot,
  databaseUrl,
  runPsqlImpl = runPsql,
}) {
  const resolved = loadResolvedMigrations(repoRoot);
  const applied = readAppliedMigrations(databaseUrl, runPsqlImpl);
  for (const migration of applied) {
    const checksum = resolved.get(migration.version);
    if (!checksum) {
      throw new Error(
        `migration ${migration.version} was previously applied but is missing in the resolved migrations`
      );
    }
    if (checksum !== migration.checksum.toLowerCase()) {
      throw new Error(
        `migration ${migration.version} was previously applied but has been modified`
      );
    }
  }
  return { status: 'valid', appliedCount: applied.length };
}

module.exports = { validateAppliedMigrations };
