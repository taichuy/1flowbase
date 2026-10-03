const test = require('node:test');
const assert = require('node:assert/strict');
const crypto = require('node:crypto');
const fs = require('node:fs');
const path = require('node:path');

const repoRoot = path.resolve(__dirname, '..', '..', '..', '..');

function migrationChecksum(relativePath) {
  const bytes = fs.readFileSync(path.join(repoRoot, ...relativePath.split('/')));
  return crypto.createHash('sha384').update(bytes).digest('hex');
}

test('published PostgreSQL migrations keep their sqlx checksums', () => {
  const publishedMigrations = new Map([
    [
      'api/crates/storage/durable/postgres/migrations/20260529123000_register_runtime_builtin_read_models.sql',
      '4b744748c1d7648af80b11a1169e9f6ba2934fedd73806fdd3f80ca6e393ba0e5c78efb334da923d7b49c0904eb4561e',
    ],
    [
      'api/crates/storage/durable/postgres/migrations/20260628100000_register_builtin_system_table_contracts.sql',
      'd11841ac0502ff548ea978012ebb99f6db8ca7a716981fb176c2608e639e3fa51f802d84f3215a033da9092c735edf6e',
    ],
    [
      'api/crates/storage/durable/postgres/migrations/20260809090000_create_ui_management.sql',
      '2ace8d9fc47c03dd10f671594cee28fd3e41127a5e3edf5bda5048a8ce2c7a2cdf6b59a3828d55322996e69687430e27',
    ],
    [
      'api/crates/storage/durable/postgres/migrations/20260823200000_replace_ui_component_overrides_with_records.sql',
      'd4c1386a2bb62c6d0b3388af2de4d7b461e1fb3a8ba3c1b04eb39f0561ba44d1f0fa3167b733f073573569b01d107c65',
    ],
    [
      'api/crates/storage/durable/postgres/migrations/20260823210000_add_ui_component_catalog_provenance.sql',
      '5b8932d6daa432e57d69eaee3fe38498f469fc5db5c15918693ee1b33fe065e7228e07c242e2798b743fc925d5d33979',
    ],
  ]);

  for (const [relativePath, checksum] of publishedMigrations) {
    assert.equal(migrationChecksum(relativePath), checksum, relativePath);
  }
});

test('template release readiness is repaired additively without exempting the immutable ledger', () => {
  const os = require('node:os');
  const { collectSchemaInventory, evaluateSchemaHygiene } = require('../core.js');
  const original = '20261003010000_application_template_releases.sql';
  const readiness = '20261003170000_application_template_release_readiness.sql';
  const migrationRoot = 'api/crates/storage/durable/postgres/migrations';
  const fixtureRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'template-release-readiness-'));
  const directory = path.join(fixtureRoot, migrationRoot);
  fs.mkdirSync(directory, { recursive: true });
  try {
    fs.copyFileSync(path.join(repoRoot, migrationRoot, original), path.join(directory, original));
    const before = evaluateSchemaHygiene({ inventory: collectSchemaInventory({ repoRoot: fixtureRoot }) });
    assert.deepEqual(before.findings.filter((item) => item.severity === 'error').map((item) => item.rule).sort(), [
      'managed-table-created-at',
      'managed-table-id',
      'managed-table-scope-column',
      'managed-table-scope-time-index',
    ]);
    fs.copyFileSync(path.join(repoRoot, migrationRoot, readiness), path.join(directory, readiness));
    const after = evaluateSchemaHygiene({ inventory: collectSchemaInventory({ repoRoot: fixtureRoot }) });
    assert.equal(after.summary.errors, 0);
    assert.equal(after.tables[0].profile, 'managed_table');
  } finally {
    fs.rmSync(fixtureRoot, { recursive: true, force: true });
  }
});
