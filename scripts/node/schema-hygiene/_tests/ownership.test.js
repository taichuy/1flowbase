const test = require('node:test');
const assert = require('node:assert/strict');
const path = require('node:path');
const { collectSchemaInventory, evaluateSchemaHygiene, loadConfig } = require('../core.js');

const repoRoot = path.resolve(__dirname, '..', '..', '..', '..');

test('specialized logs and organization tables retain real ownership and lookup contracts', () => {
  const config = loadConfig(repoRoot);
  const inventory = collectSchemaInventory({ repoRoot });
  const names = [...Object.keys(config.ownedTableContracts), 'application_run_native_projection_progress'];
  const report = evaluateSchemaHygiene({ inventory, config });
  for (const name of names) {
    assert.ok(inventory.tables.some((table) => table.name === name), name);
    assert.deepEqual(report.findings.filter((item) => item.table === name && item.severity === 'error'), [], name);
  }
  // The tree root legitimately has no parent; all other owner keys are required.
  assert.ok(inventory.tables.find((table) => table.name === 'departments')
    .columns.find((column) => column.name === 'parent_id').nullable);

  for (const [name, contract] of Object.entries(config.ownedTableContracts)) {
    const original = inventory.tables.find((table) => table.name === name);
    const rejected = (table, rule) => {
      const candidate = evaluateSchemaHygiene({ inventory: { tables: [table], parseErrors: [] }, config });
      assert.ok(candidate.findings.some((item) => item.table === name && item.rule === rule), `${name}: ${rule}`);
    };
    rejected({ ...original, primaryKey: null }, 'owned-table-primary-key');
    rejected({ ...original, foreignKeys: [] }, 'owned-table-foreign-key');
    const owner = contract.foreignKeys[0].columns[0];
    rejected({ ...original, columns: original.columns.map((column) => column.name === owner
      ? { ...column, nullable: true } : column) }, 'owned-table-owner-column');
    for (const columns of contract.indexes || []) {
      const matches = (index) => columns.every((column, position) => index.columns[position] === column);
      rejected({ ...original,
        indexes: original.indexes.filter((index) => !matches(index)),
        uniqueConstraints: original.uniqueConstraints.filter((index) => !matches(index)),
      }, 'owned-table-lookup-index');
    }
  }
  const progress = inventory.tables.find((table) => table.name === 'application_run_native_projection_progress');
  const invalid = evaluateSchemaHygiene({ inventory: { tables: [{ ...progress, foreignKeys: [] }], parseErrors: [] }, config });
  assert.ok(invalid.findings.some((item) => item.rule === 'flow-run-owned-table-owner-column'));
});

test('specialized ownership exemptions do not relax ordinary business tables', () => {
  const config = loadConfig(repoRoot);
  const table = {
    name: 'customer_records', columns: [{ name: 'id', nullable: false }],
    primaryKey: { columns: ['id'] }, indexes: [], foreignKeys: [], uniqueConstraints: [],
  };
  const report = evaluateSchemaHygiene({ inventory: { tables: [table], parseErrors: [] }, config });
  for (const rule of ['managed-table-created-at', 'managed-table-updated-at-or-append-only',
    'managed-table-scope-column', 'managed-table-scope-time-index']) {
    assert.ok(report.findings.some((item) => item.table === table.name && item.rule === rule), rule);
  }
});
