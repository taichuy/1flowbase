const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const { main } = require('../../tooling/index.js');

const reportNames = ['schema-hygiene', 'raw-jsonb-report', 'log-query-contract-report', 'capacity-report'];
const migrations = 'api/crates/storage/durable/postgres/migrations';
const migration = `${migrations}/20260101000000_create_events.sql`;

function writeFile(repoRoot, file, content) {
  const target = path.join(repoRoot, file);
  fs.mkdirSync(path.dirname(target), { recursive: true });
  fs.writeFileSync(target, content);
}

function fixture(t) {
  const repoRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'data-contracts-'));
  t.after(() => fs.rmSync(repoRoot, { recursive: true, force: true }));
  for (const name of [...reportNames, 'growth-table-report']) {
    writeFile(repoRoot, `scripts/node/${name}/config.json`, '{}');
  }
  writeFile(repoRoot, 'scripts/node/raw-jsonb-report/config.json', JSON.stringify({
    fields: [{ table: 'scoped_events', column: 'missing_payload' }],
  }));
  writeFile(repoRoot, migration, `
    CREATE TABLE scoped_events (
      id uuid PRIMARY KEY, scope_id uuid NOT NULL,
      payload jsonb NOT NULL DEFAULT '{}'::jsonb,
      created_at timestamptz NOT NULL DEFAULT now(),
      updated_at timestamptz NOT NULL DEFAULT now()
    );
    CREATE INDEX scoped_events_scope_created_idx
      ON scoped_events (scope_id, created_at DESC, id DESC);
  `);
  return { repoRoot, writeStdout() {}, writeStderr() {} };
}

function readReports(repoRoot) {
  return Object.fromEntries(reportNames.flatMap((name) => ['json', 'md'].map((extension) => {
    const file = `${name}.${extension}`;
    const text = fs.readFileSync(path.join(repoRoot, 'tmp/test-governance', file), 'utf8');
    // Only the report's collection timestamp may differ between executions.
    return [file, text.replace(/"generatedAt": "[^"]+"/gu, '"generatedAt": "<collected-at>"')];
  })));
}

test('one data-contract scan preserves all standalone artifacts and advisory outcomes', async (t) => {
  const deps = fixture(t);
  const readFileSync = fs.readFileSync;
  let migrationReads = 0;
  t.mock.method(fs, 'readFileSync', function (file, ...args) {
    if (file === path.join(deps.repoRoot, migration)) migrationReads += 1;
    return readFileSync.call(this, file, ...args);
  });
  for (const name of reportNames) assert.equal(await main([name], deps), 0, name);
  const expected = readReports(deps.repoRoot);
  assert.equal(migrationReads, 3, 'standalone schema, raw JSONB and capacity each collect the inventory');
  migrationReads = 0;
  assert.equal(await main(['data-contracts'], deps), 0);
  assert.equal(migrationReads, 1, 'the shared migration inventory must only be read once');
  assert.deepEqual(readReports(deps.repoRoot), expected);
  const raw = JSON.parse(expected['raw-jsonb-report.json']);
  assert.equal(raw.status, 'warning');
  const capacity = JSON.parse(expected['capacity-report.json']);
  assert.equal(capacity.assessments.measuredCapacity.status, 'unverified');
});

test('data-contracts preserves schema and query failures independently', async (t) => {
  for (const failure of ['schema', 'query']) {
    await t.test(failure, async (t) => {
      const deps = fixture(t);
      if (failure === 'schema') {
        writeFile(deps.repoRoot, `${migrations}/20260101000000_duplicate.sql`, 'SELECT 1;');
      } else {
        writeFile(deps.repoRoot, 'scripts/node/log-query-contract-report/config.json', JSON.stringify({
          endpoints: [{
            id: 'logs', category: 'log_list', method: 'GET', path: '/logs',
            api: { file: 'api/missing.rs', functionName: 'list_logs' },
            repository: { file: 'api/missing.rs', functionName: 'list_logs_page' },
            contract: {},
          }],
        }));
      }
      assert.equal(await main([failure === 'schema' ? 'schema-hygiene' : 'log-query-contract-report'], deps), 1);
      assert.equal(await main(['data-contracts'], deps), 1);
      const reports = readReports(deps.repoRoot);
      if (failure === 'schema') {
        const schema = JSON.parse(reports['schema-hygiene.json']);
        assert.ok(schema.findings.some(({ rule }) => rule === 'postgres-migration-version-duplicate'));
        assert.equal(JSON.parse(reports['log-query-contract-report.json']).exitCode, 0);
      } else {
        assert.equal(JSON.parse(reports['schema-hygiene.json']).summary.errors, 0);
        assert.equal(JSON.parse(reports['log-query-contract-report.json']).exitCode, 1);
      }
    });
  }
});

test('data-contracts fails closed on unreadable policy and supports help without scanning', async (t) => {
  const deps = fixture(t);
  assert.equal(await main(['data-contracts', '--help'], { repoRoot: '/nonexistent', writeStdout() {} }), 0);
  await assert.rejects(main(['data-contracts', '--unknown'], deps), /Unknown data-contracts option/u);
  writeFile(deps.repoRoot, 'scripts/node/log-query-contract-report/config.json', '{invalid');
  await assert.rejects(main(['data-contracts'], deps), SyntaxError);
  fs.unlinkSync(path.join(deps.repoRoot, 'scripts/node/log-query-contract-report/config.json'));
  await assert.rejects(main(['data-contracts'], deps), { code: 'ENOENT' });
});
