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
    for (const reference of contract.foreignKeys || []) {
      rejected({ ...original, foreignKeys: [] }, 'owned-table-foreign-key');
      for (const owner of reference.columns) {
        if ((contract.nullableOwnerColumns || []).includes(owner)) continue;
        rejected({ ...original, columns: original.columns.map((column) => column.name === owner
          ? { ...column, nullable: true } : column) }, 'owned-table-owner-column');
      }
    }
    for (const owner of new Set([...(contract.ownerColumns || []), ...(contract.routingColumns || [])])) {
      rejected({ ...original, columns: original.columns.filter((column) => column.name !== owner) }, 'owned-table-owner-column');
      rejected({ ...original, columns: original.columns.map((column) => column.name === owner
        ? { ...column, nullable: true } : column) }, 'owned-table-owner-column');
    }
    for (const columnName of contract.requiredColumns || []) {
      rejected({ ...original, columns: original.columns.filter((column) => column.name !== columnName) }, 'owned-table-required-column');
    }
    for (const columns of contract.indexes || []) {
      const matches = (index) => columns.every((column, position) => index.columns[position] === column);
      rejected({ ...original,
        primaryKey: matches(original.primaryKey) ? null : original.primaryKey,
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


test('plugin credential lookup keeps scope and lifecycle checks without requiring chronology pagination', () => {
  const config = loadConfig(repoRoot);
  const inventory = collectSchemaInventory({ repoRoot });
  const original = inventory.tables.find((table) => table.name === 'plugin_credentials');
  assert.ok(original);
  assert.deepEqual(config.exemptions.plugin_credentials.skip.sort(), ['managed-table-id', 'managed-table-scope-time-index']);
  for (const [column, rule] of [
    ['scope_id', 'managed-table-scope-column'],
    ['created_at', 'managed-table-created-at'],
    ['updated_at', 'managed-table-updated-at-or-append-only'],
  ]) {
    const report = evaluateSchemaHygiene({
      inventory: { tables: [{ ...original, columns: original.columns.filter((item) => item.name !== column) }], parseErrors: [] },
      config,
    });
    assert.ok(report.findings.some((item) => item.table === original.name && item.rule === rule), column);
  }
});


test('recipient history retains identities and read acknowledgement without cascading owner deletion', () => {
  const config = loadConfig(repoRoot);
  const inventory = collectSchemaInventory({ repoRoot });
  const inbox = inventory.tables.find((table) => table.name === 'system_notifications');
  assert.ok(inbox);
  assert.ok(inbox.columns.find((column) => column.name === 'read_at').nullable);
  assert.equal(inbox.foreignKeys.some((key) => ['users', 'notification_instances'].includes(key.references.table)), false);
  assert.deepEqual(config.ownedTableContracts.system_notifications.ownerColumns,
    ['workspace_id', 'recipient_user_id', 'instance_id']);
  assert.equal(config.systemScopeTables.includes('system_notifications'), false);
  assert.equal(config.appendOnlyTables.includes('system_notifications'), false);
});


test('send attempts retain workspace routing and terminal completion without pretending append-only', () => {
  const config = loadConfig(repoRoot);
  const inventory = collectSchemaInventory({ repoRoot });
  const attempt = inventory.tables.find((table) => table.name === 'notification_send_logs');
  assert.ok(attempt);
  assert.equal(config.appendOnlyTables.includes(attempt.name), false);
  assert.equal(config.systemScopeTables.includes(attempt.name), false);
  assert.equal(attempt.foreignKeys.length, 0);
  assert.ok(attempt.columns.find((column) => column.name === 'completed_at').nullable);
  const report = evaluateSchemaHygiene({ inventory: { tables: [attempt], parseErrors: [] }, config });
  assert.deepEqual(report.findings.filter((item) => item.severity === 'error'), []);
  assert.equal(report.tables[0].platformReadiness.category, 'workspace_owned_notification_send_attempt');
  // Shared owned-table mutation fixtures above reject removed identity, lifecycle
  // columns and either real lookup index; these generic checks remain active too.
  for (const [column, rule] of [['id', 'managed-table-id'], ['created_at', 'managed-table-created-at']]) {
    const invalid = evaluateSchemaHygiene({
      inventory: { tables: [{ ...attempt, columns: attempt.columns.filter((item) => item.name !== column) }], parseErrors: [] }, config,
    });
    assert.ok(invalid.findings.some((item) => item.rule === rule), column);
  }
});


test('credit ledger retains workspace-wide and user-filtered chronology indexes', () => {
  const config = loadConfig(repoRoot);
  const inventory = collectSchemaInventory({ repoRoot });
  const original = inventory.tables.find((table) => table.name === 'runtime_credit_ledger');
  assert.ok(original);
  const workspaceChronology = ['workspace_id', 'created_at', 'id'];
  const userChronology = ['workspace_id', 'user_id', 'created_at', 'id'];
  const matches = (index, columns) => columns.every((column, position) => index.columns[position] === column);
  const evaluate = (table) => evaluateSchemaHygiene({
    inventory: { ...inventory, tables: inventory.tables.map((candidate) => candidate.name === table.name ? table : candidate) },
    config,
  }).findings.filter((finding) => finding.table === table.name && finding.severity === 'error');

  assert.deepEqual(evaluate(original), []);
  assert.ok(original.indexes.some((index) => matches(index, workspaceChronology)
    && index.source.endsWith('20261020030000_index_credit_ledger_workspace_chronology.sql')
    && !index.predicate));
  assert.ok(original.indexes.some((index) => matches(index, userChronology)));
  // Existing per-user lookup contracts keep their previous predicate semantics.
  assert.deepEqual(evaluate({ ...original, indexes: original.indexes.map((index) => matches(index, userChronology)
    ? { ...index, predicate: 'user_id is not null' } : index) }), []);

  // Removing either access path fails even while generic scope chronology and
  // the other query's index remain available.
  for (const columns of [workspaceChronology, userChronology]) {
    const findings = evaluate({ ...original, indexes: original.indexes.filter((index) => !matches(index, columns)) });
    assert.ok(findings.some((finding) => finding.rule === 'owned-table-lookup-index'), columns.join(','));
  }
  for (const patch of [
    { columns: ['workspace_id', 'user_id', 'created_at', 'id'] },
    { columns: ['scope_id', 'created_at', 'id'] },
    { columns: ['workspace_id', 'id', 'created_at'] },
    { predicate: 'user_id is not null' },
  ]) {
    const findings = evaluate({ ...original, indexes: original.indexes.map((index) => matches(index, workspaceChronology)
      ? { ...index, ...patch } : index) });
    assert.ok(findings.some((finding) => finding.rule === 'owned-table-lookup-index'), JSON.stringify(patch));
  }
});
