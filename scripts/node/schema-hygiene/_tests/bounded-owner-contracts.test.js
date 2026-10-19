const test = require('node:test');
const assert = require('node:assert/strict');
const path = require('node:path');
const { collectSchemaInventory, evaluateSchemaHygiene, loadConfig } = require('../core.js');

const repoRoot = path.resolve(__dirname, '..', '..', '..', '..');
const names = ['plugin_contribution_authorization_revisions',
  'plugin_contribution_authorizations', 'provider_protocol_capsules'];

// Exercise the actual migration projection, not a separate idealized schema.
// Shared ownership.test.js also removes every declared PK, FK, owner column,
// required lifecycle column and lookup index from these three contracts.
test('authority and capsule declarations retain mixed-scope and run ownership', () => {
  const config = loadConfig(repoRoot);
  const inventory = collectSchemaInventory({ repoRoot });
  const report = evaluateSchemaHygiene({ inventory, config });
  for (const name of names) {
    const table = report.tables.find((candidate) => candidate.name === name);
    assert.ok(table, name);
    assert.deepEqual(table.findings, [], name);
    assert.equal(table.platformReadiness.severity, 'ok', name);
    assert.equal(config.needsOwnerReviewTables[name], undefined);
  }
  assert.deepEqual(config.ownedTableContracts.plugin_contribution_authorization_revisions.primaryKey,
    ['installation_id', 'workspace_id']);
  assert.deepEqual(config.ownedTableContracts.provider_protocol_capsules.primaryKey,
    ['flow_run_id', 'capsule_kind', 'slot_key']);
  assert.equal(config.tableProfiles.provider_protocol_capsules, 'flow_run_owned_table');
  for (const name of names.slice(0, 2)) {
    assert.equal(config.systemScopeTables.includes(name), false, 'authority includes business workspaces');
    const readiness = report.tables.find((table) => table.name === name).platformReadiness;
    assert.equal(readiness.requiredScopeId, false);
    assert.equal(readiness.routingKeyStatus, 'installation_id+workspace_id');
    assert.deepEqual(config.exemptions[name].skip.filter((skip) => !skip.startsWith('managed-table-')), []);
  }
});

test('owner contracts reject changed routing identities and foreign-key targets or cascades', () => {
  const config = loadConfig(repoRoot);
  const inventory = collectSchemaInventory({ repoRoot });
  function rejected(name, change, rule) {
    const tables = inventory.tables.map((table) => table.name === name ? change(structuredClone(table)) : table);
    const report = evaluateSchemaHygiene({ inventory: { ...inventory, tables }, config });
    assert.ok(report.findings.some((finding) => finding.table === name
      && finding.rule === rule && finding.severity === 'error'), `${name}: ${rule}`);
  }
  for (const name of names) {
    rejected(name, (table) => ({ ...table, primaryKey: { ...table.primaryKey,
      columns: [...table.primaryKey.columns, 'unrelated_id'] } }), 'owned-table-primary-key');
    for (const reference of config.ownedTableContracts[name].foreignKeys) {
      const isReference = (key) => key.references.table === reference.table;
      rejected(name, (table) => ({ ...table, foreignKeys: table.foreignKeys.map((key) => isReference(key)
        ? { ...key, references: { ...key.references, table: 'unrelated_owner' } } : key) }), 'owned-table-foreign-key');
      rejected(name, (table) => ({ ...table, foreignKeys: table.foreignKeys.map((key) => isReference(key)
        ? { ...key, onDelete: 'no action' } : key) }), 'owned-table-foreign-key');
    }
  }
});

test('mixed authority ownership fails closed if validation or cleanup trigger is lost or weakened', () => {
  const config = loadConfig(repoRoot);
  const inventory = collectSchemaInventory({ repoRoot });
  const owner = 'plugin_contribution_authorization_revisions';
  const contract = config.ownedTableContracts[owner];
  assert.equal(contract.triggers.length, 2);
  function rejected(required, change) {
    const tables = inventory.tables.map((table) => table.name === required.table
      ? { ...table, triggers: change(structuredClone(table.triggers)) } : table);
    const report = evaluateSchemaHygiene({ inventory: { ...inventory, tables }, config });
    assert.ok(report.findings.some((finding) => finding.table === owner
      && finding.rule === 'owned-table-validation-trigger' && finding.severity === 'error'), required.name);
  }
  for (const required of contract.triggers) {
    rejected(required, (triggers) => triggers.filter((trigger) => trigger.name !== required.name));
    for (const patch of [{ function: 'unrelated_validator' }, { timing: 'instead of' }, { forEach: 'statement' }, { events: [] }]) {
      rejected(required, (triggers) => triggers.map((trigger) => trigger.name === required.name ? { ...trigger, ...patch } : trigger));
    }
    if (required.events.some((event) => event.columns?.length)) {
      rejected(required, (triggers) => triggers.map((trigger) => trigger.name === required.name
        ? { ...trigger, events: trigger.events.map((event) => event.event === 'update'
          ? { ...event, columns: ['workspace_id'] } : event) } : trigger));
    }
  }
});
