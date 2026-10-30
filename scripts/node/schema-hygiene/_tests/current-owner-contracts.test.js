const test = require('node:test');
const assert = require('node:assert/strict');
const path = require('node:path');
const { collectSchemaInventory, evaluateSchemaHygiene, loadConfig } = require('../core.js');

const repoRoot = path.resolve(__dirname, '..', '..', '..', '..');
const names = ['communication_inputs', 'communication_messages',
  'runtime_internal_compaction_contents', 'runtime_internal_compaction_heads'];

// Expectations come from the timeline/session and native internal compaction
// migrations, exercised through the production migration inventory and gate.
function fixture() {
  const config = loadConfig(repoRoot);
  const inventory = collectSchemaInventory({ repoRoot });
  const findings = (candidate = inventory) => evaluateSchemaHygiene({ inventory: candidate, config })
    .findings.filter((finding) => names.includes(finding.table) && finding.severity === 'error');
  const rejected = (name, mutate, rule) => {
    const candidate = structuredClone(inventory);
    mutate(candidate.tables.find((table) => table.name === name));
    assert.ok(findings(candidate).some((finding) => finding.table === name && finding.rule === rule),
      `${name}: ${rule}`);
  };
  return { config, inventory, findings, rejected };
}

test('current timeline and session compaction schemas satisfy their actual owner contracts', () => {
  const { config, inventory, findings } = fixture();
  for (const name of names) assert.ok(inventory.tables.some((table) => table.name === name), name);
  assert.deepEqual(findings(), []);
  assert.equal(config.ownedTableContracts.communication_attachments, undefined);
  // Both receipt and timeline history intentionally survive owner deletion.
  for (const name of names.slice(0, 2)) {
    assert.equal(config.ownedTableContracts[name].foreignKeys, undefined);
    assert.equal(config.exemptions[name], undefined, 'generic managed checks remain active');
  }
  const timeline = inventory.tables.find((table) => table.name === 'communication_messages');
  assert.deepEqual(timeline.foreignKeys, []);
  for (const column of ['processing_status', 'send_status', 'claim_expires_at']) {
    assert.equal(timeline.columns.some((item) => item.name === column), false, column);
    assert.ok(config.ownedTableContracts.communication_inputs.requiredColumns.includes(column));
  }
  const heads = inventory.tables.find((table) => table.name === 'runtime_internal_compaction_heads');
  assert.ok(heads.columns.find((column) => column.name === 'checkpoint_content_id').nullable);
  assert.equal(heads.columns.some((column) => column.name === 'created_at'), false);
});

test('current owner contracts reject identity, routing and lifecycle damage', () => {
  const { config, rejected } = fixture();
  for (const name of names) {
    const contract = config.ownedTableContracts[name];
    rejected(name, (table) => { table.primaryKey.columns = ['unrelated_id']; }, 'owned-table-primary-key');
    for (const owner of new Set([...(contract.ownerColumns || []), ...(contract.routingColumns || [])])) {
      rejected(name, (table) => { table.columns = table.columns.filter((column) => column.name !== owner); },
        'owned-table-owner-column');
      rejected(name, (table) => { table.columns.find((column) => column.name === owner).nullable = true; },
        'owned-table-owner-column');
    }
    for (const required of contract.requiredColumns) {
      rejected(name, (table) => { table.columns = table.columns.filter((column) => column.name !== required); },
        'owned-table-required-column');
    }
    for (const columns of [...(contract.indexes || []), ...(contract.unconditionalIndexes || [])]) {
      const matches = (index) => columns.every((column, indexPosition) => index.columns[indexPosition] === column);
      rejected(name, (table) => {
        table.indexes = table.indexes.filter((index) => !matches(index));
        table.uniqueConstraints = table.uniqueConstraints.filter((index) => !matches(index));
      }, 'owned-table-lookup-index');
    }
    for (const required of contract.triggers || []) {
      rejected(name, (table) => { table.triggers = table.triggers.filter((trigger) => trigger.name !== required.name); },
        'owned-table-validation-trigger');
    }
  }
});

test('compaction content references retain the exact application scope and deletion behavior', () => {
  const { config, rejected } = fixture();
  for (const name of names.slice(2)) {
    for (const reference of config.ownedTableContracts[name].foreignKeys) {
      const matches = (key) => key.columns.join(',') === reference.columns.join(',');
      rejected(name, (table) => { table.foreignKeys = table.foreignKeys.filter((key) => !matches(key)); },
        'owned-table-foreign-key');
      rejected(name, (table) => {
        table.foreignKeys.find(matches).references.columns = ['id'];
      }, 'owned-table-foreign-key');
      rejected(name, (table) => {
        table.foreignKeys.find(matches).onDelete = reference.onDelete === 'cascade' ? 'no action' : 'cascade';
      }, 'owned-table-foreign-key');
      for (const owner of reference.columns) {
        if ((config.ownedTableContracts[name].nullableOwnerColumns || []).includes(owner)) continue;
        rejected(name, (table) => { table.columns.find((column) => column.name === owner).nullable = true; },
          'owned-table-owner-column');
      }
    }
  }
});
