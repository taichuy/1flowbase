const test = require('node:test');
const assert = require('node:assert/strict');
const crypto = require('node:crypto');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');

const { validateAppliedMigrations } = require('../migration-validation.js');

function migrationFixture(sql = 'select 1;\n') {
  const repoRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'oneflowbase-migration-validation-'));
  const migrationsDir = path.join(repoRoot, 'api', 'crates', 'storage', 'durable', 'postgres', 'migrations');
  fs.mkdirSync(migrationsDir, { recursive: true });
  fs.writeFileSync(path.join(migrationsDir, '20260903070000_fixture.sql'), sql);
  return { repoRoot, checksum: crypto.createHash('sha384').update(sql).digest('hex') };
}

test('AC-005 accepts applied migrations whose repository checksum matches', () => {
  const fixture = migrationFixture();
  const result = validateAppliedMigrations({
    repoRoot: fixture.repoRoot,
    databaseUrl: 'postgres://postgres:secret@localhost/flowbase',
    runPsqlImpl: () => ({
      status: 0,
      stdout: `20260903070000|${fixture.checksum}\n`,
      stderr: '',
    }),
  });

  assert.deepEqual(result, { status: 'valid', appliedCount: 1 });
});

test('AC-005 emits the sqlx-compatible modified migration error used by dev-up recovery', () => {
  const fixture = migrationFixture();
  assert.throws(
    () => validateAppliedMigrations({
      repoRoot: fixture.repoRoot,
      databaseUrl: 'postgres://postgres:secret@localhost/flowbase',
      runPsqlImpl: () => ({ status: 0, stdout: `20260903070000|${'0'.repeat(96)}\n`, stderr: '' }),
    }),
    /migration 20260903070000 was previously applied but has been modified/u
  );
});

test('AC-005 emits the sqlx-compatible missing migration error used by dev-up recovery', () => {
  const fixture = migrationFixture();
  assert.throws(
    () => validateAppliedMigrations({
      repoRoot: fixture.repoRoot,
      databaseUrl: 'postgres://postgres:secret@localhost/flowbase',
      runPsqlImpl: () => ({ status: 0, stdout: `20260903070001|${'0'.repeat(96)}\n`, stderr: '' }),
    }),
    /migration 20260903070001 was previously applied but is missing in the resolved migrations/u
  );
});
