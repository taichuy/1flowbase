const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');

function writeFile(repoRoot, relativePath, content) {
  const absolutePath = path.join(repoRoot, relativePath);
  fs.mkdirSync(path.dirname(absolutePath), { recursive: true });
  fs.writeFileSync(absolutePath, content, 'utf8');
}

function createRepoWithMigrations(migrations) {
  const repoRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'oneflowbase-schema-hygiene-'));
  for (const [fileName, sql] of Object.entries(migrations)) {
    writeFile(
      repoRoot,
      `api/crates/storage/durable/postgres/migrations/${fileName}`,
      sql
    );
  }
  return repoRoot;
}

function createRepoWithMigration(sql) {
  return createRepoWithMigrations({
    '20260101000000_fixture.sql': sql,
  });
}

module.exports = { writeFile, createRepoWithMigrations, createRepoWithMigration };
