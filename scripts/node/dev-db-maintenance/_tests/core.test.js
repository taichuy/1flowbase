const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');

const {
  buildBackupPrunePlan,
  buildTestSchemaPrunePlan,
  parseCliArgs,
  runDevDatabaseMaintenance,
} = require('../core.js');

function schemaNameFromTimestamp(date) {
  const millis = BigInt(date.getTime());
  return `test_${millis.toString(16).padStart(12, '0')}7abc0000000000000000`;
}

function writeFixtureFile(filePath) {
  fs.mkdirSync(path.dirname(filePath), { recursive: true });
  fs.writeFileSync(filePath, 'fixture\n', 'utf8');
}

test('parseCliArgs defaults to test schema dry-run with conservative retention', () => {
  assert.deepEqual(parseCliArgs([]), {
    apply: false,
    command: 'test-schemas',
    databaseUrl: null,
    help: false,
    keep: 20,
    olderThanMs: 3 * 24 * 60 * 60 * 1000,
  });
});

test('parseCliArgs keeps one backup by default', () => {
  assert.deepEqual(parseCliArgs(['backups']), {
    apply: false,
    command: 'backups',
    databaseUrl: null,
    help: false,
    keep: 1,
    olderThanMs: 7 * 24 * 60 * 60 * 1000,
  });
});

test('buildTestSchemaPrunePlan keeps recent schemas and newest retained schemas', () => {
  const now = new Date('2026-06-15T12:00:00.000Z');
  const schemas = [
    schemaNameFromTimestamp(new Date('2026-06-01T00:00:00.000Z')),
    schemaNameFromTimestamp(new Date('2026-06-02T00:00:00.000Z')),
    schemaNameFromTimestamp(new Date('2026-06-03T00:00:00.000Z')),
    schemaNameFromTimestamp(new Date('2026-06-14T00:00:00.000Z')),
    'public',
    'test_not_a_uuid_v7',
  ];

  const plan = buildTestSchemaPrunePlan({
    now,
    schemaNames: schemas,
    olderThanMs: 3 * 24 * 60 * 60 * 1000,
    keep: 1,
  });

  assert.deepEqual(plan.dropSchemas, [schemas[0], schemas[1], schemas[2]]);
  assert.deepEqual(plan.keepSchemas.map((schema) => schema.name), [schemas[3]]);
  assert.deepEqual(plan.skippedSchemaNames, ['public', 'test_not_a_uuid_v7']);
});

test('runDevDatabaseMaintenance refuses production and non-local database URLs', () => {
  assert.throws(
    () =>
      runDevDatabaseMaintenance({
        options: parseCliArgs(['test-schemas', '--database-url', 'postgres://postgres:pw@127.0.0.1:35432/1flowbase']),
        sourceEnv: { API_ENV: 'production' },
        loadTestSchemaNames() {
          return [];
        },
      }),
    /拒绝维护 production 数据库/u
  );

  assert.throws(
    () =>
      runDevDatabaseMaintenance({
        options: parseCliArgs(['test-schemas', '--database-url', 'postgres://postgres:pw@db.example.com:5432/1flowbase']),
        sourceEnv: { API_ENV: 'development' },
        loadTestSchemaNames() {
          return [];
        },
      }),
    /只允许维护本地开发数据库/u
  );
});

test('runDevDatabaseMaintenance keeps test schemas during dry-run and drops only with apply', () => {
  const now = new Date('2026-06-15T12:00:00.000Z');
  const schemaNames = [
    schemaNameFromTimestamp(new Date('2026-06-01T00:00:00.000Z')),
    schemaNameFromTimestamp(new Date('2026-06-14T00:00:00.000Z')),
  ];
  const droppedSchemas = [];
  const dryRunOutput = [];
  const databaseUrl = 'postgres://postgres:pw@127.0.0.1:35432/1flowbase';

  const dryRunStatus = runDevDatabaseMaintenance({
    now,
    options: parseCliArgs(['test-schemas', '--database-url', databaseUrl, '--older-than', '3d', '--keep', '0']),
    sourceEnv: { API_ENV: 'development' },
    loadTestSchemaNames() {
      return schemaNames;
    },
    removeTestSchemas({ schemaNames }) {
      droppedSchemas.push(...schemaNames);
    },
    writeStdout(text) {
      dryRunOutput.push(text);
    },
  });

  assert.equal(dryRunStatus, 0);
  assert.deepEqual(droppedSchemas, []);
  assert.match(dryRunOutput.join(''), /dry-run/u);
  assert.match(dryRunOutput.join(''), new RegExp(schemaNames[0], 'u'));

  const applyStatus = runDevDatabaseMaintenance({
    now,
    options: parseCliArgs(['test-schemas', '--database-url', databaseUrl, '--older-than', '3d', '--keep', '0', '--apply']),
    sourceEnv: { API_ENV: 'development' },
    loadTestSchemaNames() {
      return schemaNames;
    },
    removeTestSchemas({ schemaNames }) {
      droppedSchemas.push(...schemaNames);
    },
    writeStdout() {},
  });

  assert.equal(applyStatus, 0);
  assert.deepEqual(droppedSchemas, [schemaNames[0]]);
});

test('AC-001: runDevDatabaseMaintenance drops large test schema plans in bounded transactions', () => {
  const now = new Date('2026-06-15T12:00:00.000Z');
  const schemaNames = Array.from({ length: 25 }, (_value, index) =>
    schemaNameFromTimestamp(new Date(Date.UTC(2026, 4, index + 1)))
  );
  const droppedBatches = [];

  const status = runDevDatabaseMaintenance({
    now,
    options: parseCliArgs([
      'test-schemas',
      '--database-url',
      'postgres://postgres:pw@127.0.0.1:35432/1flowbase',
      '--older-than',
      '0d',
      '--keep',
      '0',
      '--apply',
    ]),
    sourceEnv: { API_ENV: 'development' },
    loadTestSchemaNames() {
      return schemaNames;
    },
    removeTestSchemas({ schemaNames: batch }) {
      droppedBatches.push(batch);
    },
    writeStdout() {},
  });

  assert.equal(status, 0);
  assert.equal(droppedBatches.length, 25);
  assert.equal(droppedBatches.every((batch) => batch.length === 1), true);
  assert.deepEqual(droppedBatches.flat(), [...schemaNames].sort());
});

test('buildBackupPrunePlan keeps active postgres and recent backup directories', () => {
  const repoRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'oneflowbase-dev-db-maintenance-'));
  const volumesDir = path.join(repoRoot, 'docker', 'volumes');
  const activeFile = path.join(volumesDir, 'postgres', 'PG_VERSION');
  const oldBackupFile = path.join(volumesDir, 'postgres.empty-20260601-000000', 'PG_VERSION');
  const keptBackupFile = path.join(volumesDir, 'postgres.backup-20260614-000000', 'PG_VERSION');

  writeFixtureFile(activeFile);
  writeFixtureFile(oldBackupFile);
  writeFixtureFile(keptBackupFile);

  const plan = buildBackupPrunePlan({
    now: new Date('2026-06-15T12:00:00.000Z'),
    repoRoot,
    olderThanMs: 7 * 24 * 60 * 60 * 1000,
    keep: 1,
  });

  assert.deepEqual(
    plan.removeDirectories.map((entry) => entry.relativePath),
    [path.join('docker', 'volumes', 'postgres.empty-20260601-000000')]
  );
  assert.deepEqual(
    plan.keepDirectories.map((entry) => entry.relativePath),
    [path.join('docker', 'volumes', 'postgres.backup-20260614-000000')]
  );
  assert.equal(plan.removeDirectories.some((entry) => entry.relativePath === path.join('docker', 'volumes', 'postgres')), false);
});

test('runDevDatabaseMaintenance removes backup directories only with apply', () => {
  const repoRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'oneflowbase-dev-db-maintenance-'));
  const oldBackupDir = path.join(repoRoot, 'docker', 'volumes', 'postgres.empty-20260601-000000');
  const keptBackupDir = path.join(repoRoot, 'docker', 'volumes', 'postgres.backup-20260614-000000');

  writeFixtureFile(path.join(oldBackupDir, 'PG_VERSION'));
  writeFixtureFile(path.join(keptBackupDir, 'PG_VERSION'));

  const dryRunStatus = runDevDatabaseMaintenance({
    now: new Date('2026-06-15T12:00:00.000Z'),
    options: parseCliArgs(['backups', '--older-than', '7d', '--keep', '1']),
    repoRoot,
    writeStdout() {},
  });

  assert.equal(dryRunStatus, 0);
  assert.equal(fs.existsSync(oldBackupDir), true);

  const applyStatus = runDevDatabaseMaintenance({
    now: new Date('2026-06-15T12:00:00.000Z'),
    options: parseCliArgs(['backups', '--older-than', '7d', '--keep', '1', '--apply']),
    repoRoot,
    writeStdout() {},
  });

  assert.equal(applyStatus, 0);
  assert.equal(fs.existsSync(oldBackupDir), false);
  assert.equal(fs.existsSync(keptBackupDir), true);
});
