const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const { loadConfig, evaluateSchemaHygiene } = require('../core.js');

function fixture(t) {
  const repoRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'schema-owner-config-'));
  t.after(() => fs.rmSync(repoRoot, { recursive: true, force: true }));
  const directory = path.join(repoRoot, 'policy');
  fs.mkdirSync(directory);
  return { repoRoot, write(name, value) { fs.writeFileSync(path.join(directory, name), JSON.stringify(value)); } };
}

test('loadConfig retains custom relative and absolute entries and combines only declared ownership sections', (t) => {
  const files = fixture(t);
  files.write('config.json', { ownershipPolicyFile: 'owners.json', registeredSystemTables: ['users'] });
  files.write('owners.json', { exemptions: { events: { reason: 'explicit owner lifecycle contract', skip: ['managed-table-id'] } },
    ownedTableContracts: { events: { primaryKey: ['owner_id'] } } });
  const relative = loadConfig(files.repoRoot, 'policy/config.json');
  const absolute = loadConfig(files.repoRoot, path.join(files.repoRoot, 'policy/config.json'));
  assert.deepEqual(relative, absolute);
  assert.deepEqual(relative.registeredSystemTables, ['users']);
  assert.deepEqual(relative.ownedTableContracts.events.primaryKey, ['owner_id']);
  assert.deepEqual(relative.exemptions.events.skip, ['managed-table-id']);
  assert.deepEqual(loadConfig(files.repoRoot, 'missing.json'), {});
});

test('injected and self-contained configurations keep the existing evaluation contract', (t) => {
  const files = fixture(t);
  const config = { ownedTableContracts: { events: { primaryKey: ['owner_id'] } } };
  files.write('config.json', config);
  assert.deepEqual(loadConfig(files.repoRoot, 'policy/config.json'), config);
  const report = evaluateSchemaHygiene({ inventory: { tables: [], parseErrors: [] }, config });
  assert.equal(report.summary.errors, 0);
});

test('missing or ambiguous external ownership policy fails rather than silently dropping contracts', (t) => {
  const files = fixture(t);
  files.write('config.json', { ownershipPolicyFile: 'owners.json' });
  assert.throws(() => loadConfig(files.repoRoot, 'policy/config.json'), { code: 'ENOENT' });
  files.write('owners.json', { systemScopeTables: ['everything'] });
  assert.throws(() => loadConfig(files.repoRoot, 'policy/config.json'), /exclusively declare/u);
  files.write('owners.json', { ownedTableContracts: {} });
  files.write('config.json', { ownershipPolicyFile: 'owners.json', ownedTableContracts: {} });
  assert.throws(() => loadConfig(files.repoRoot, 'policy/config.json'), /exclusively declare/u);
});
