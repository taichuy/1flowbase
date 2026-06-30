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
      'api/crates/storage-durable/postgres/migrations/20260529123000_register_runtime_builtin_read_models.sql',
      '4b744748c1d7648af80b11a1169e9f6ba2934fedd73806fdd3f80ca6e393ba0e5c78efb334da923d7b49c0904eb4561e',
    ],
    [
      'api/crates/storage-durable/postgres/migrations/20260628100000_register_builtin_system_table_contracts.sql',
      'd11841ac0502ff548ea978012ebb99f6db8ca7a716981fb176c2608e639e3fa51f802d84f3215a033da9092c735edf6e',
    ],
  ]);

  for (const [relativePath, checksum] of publishedMigrations) {
    assert.equal(migrationChecksum(relativePath), checksum, relativePath);
  }
});
