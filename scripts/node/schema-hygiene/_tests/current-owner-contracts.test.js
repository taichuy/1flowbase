const test = require('node:test');
const assert = require('node:assert/strict');
const path = require('node:path');
const { collectSchemaInventory, evaluateSchemaHygiene, loadConfig } = require('../core.js');

const repoRoot = path.resolve(__dirname, '..', '..', '..', '..');
const workspaceResources = ['custom_frontend_blocks', 'event_definitions',
  'plugin_record_permission_policies', 'workflow_event_triggers'];
const boundedResources = ['event_catalog', 'plugin_record_permission_grants',
  'plugin_record_permission_scope_revisions', 'workflow_run_identity_snapshots'];

test('workspace configurations retain physical scope and chronology without exemptions', () => {
  const config = loadConfig(repoRoot);
  const inventory = collectSchemaInventory({ repoRoot });
  const errors = (candidate) => evaluateSchemaHygiene({ inventory: candidate, config }).findings
    .filter((finding) => workspaceResources.includes(finding.table) && finding.severity === 'error');
  assert.deepEqual(errors(inventory), []);
  for (const name of workspaceResources) {
    const table = inventory.tables.find((candidate) => candidate.name === name);
    assert.equal(config.exemptions[name], undefined, name);
    assert.equal(config.systemScopeTables.includes(name), false, name);
    const scope = table.columns.find((column) => column.name === 'scope_id');
    assert.equal(scope.nullable, false, name);
    assert.match(scope.rawDefinition, /generated always as \(workspace_id\) stored/i, name);
    assert.deepEqual(table.primaryKey.columns, ['id'], name);
    for (const [mutate, rule] of [
      [(candidate) => { candidate.columns = candidate.columns.filter((column) => column.name !== 'scope_id'); }, 'managed-table-scope-column'],
      [(candidate) => { candidate.indexes = []; }, 'managed-table-scope-time-index'],
      [(candidate) => { candidate.columns = candidate.columns.filter((column) => column.name !== 'created_at'); }, 'managed-table-created-at'],
    ]) {
      const candidate = structuredClone(inventory);
      mutate(candidate.tables.find((item) => item.name === name));
      assert.ok(errors(candidate).some((finding) => finding.table === name && finding.rule === rule), `${name}: ${rule}`);
    }
  }
  const trigger = inventory.tables.find((table) => table.name === 'workflow_event_triggers');
  assert.equal(trigger.columns.find((column) => column.name === 'created_at').nullable, false);
  assert.equal(trigger.columns.find((column) => column.name === 'created_at').default, true);
});

test('event directory and permission internals retain bounded owner identities and lifecycle guards', () => {
  const config = loadConfig(repoRoot);
  const inventory = collectSchemaInventory({ repoRoot });
  const errors = (candidate) => evaluateSchemaHygiene({ inventory: candidate, config }).findings
    .filter((finding) => boundedResources.includes(finding.table) && finding.severity === 'error');
  assert.deepEqual(errors(inventory), []);
  const expectedKeys = [['identity'], ['policy_id', 'role_id'],
    ['workspace_id', 'publisher_namespace', 'plugin_code'], ['flow_run_id']];
  boundedResources.forEach((name, position) => {
    assert.deepEqual(config.ownedTableContracts[name].primaryKey, expectedKeys[position], name);
    assert.equal(config.systemScopeTables.includes(name), false, name);
    assert.equal(config.appendOnlyTables.includes(name), false, name);
    const rejected = (mutate, rule) => {
      const candidate = structuredClone(inventory);
      mutate(candidate.tables.find((table) => table.name === name));
      assert.ok(errors(candidate).some((finding) => finding.table === name && finding.rule === rule), `${name}: ${rule}`);
    };
    const contract = config.ownedTableContracts[name];
    for (const reference of contract.foreignKeys || []) {
      rejected((table) => { table.foreignKeys.find((key) => key.references.table === reference.table).onDelete = 'no action'; },
        'owned-table-foreign-key');
    }
  });
  assert.equal(config.tableProfiles.workflow_run_identity_snapshots, 'flow_run_owned_table');
  assert.deepEqual(config.exemptions.event_catalog.skip,
    ['managed-table-id', 'managed-table-scope-time-index']);
  const grants = config.ownedTableContracts.plugin_record_permission_grants;
  assert.deepEqual(grants.foreignKeys.map((key) => [key.columns, key.table, key.references, key.onDelete]), [
    [['policy_id'], 'plugin_record_permission_policies', ['id'], 'cascade'],
    [['role_id'], 'roles', ['id'], 'cascade'],
  ]);
  assert.deepEqual(config.ownedTableContracts.plugin_record_permission_scope_revisions.foreignKeys,
    [{ columns: ['workspace_id'], table: 'workspaces', references: ['id'], onDelete: 'cascade' }]);
  assert.deepEqual(config.ownedTableContracts.workflow_run_identity_snapshots.foreignKeys,
    [{ columns: ['flow_run_id'], table: 'flow_runs', references: ['id'], onDelete: 'cascade' }]);
  const guards = {
    event_catalog: ['event_definitions_catalog_projection'],
    plugin_record_permission_grants: ['plugin_record_permission_grant_scope', 'plugin_record_permission_grant_revision'],
    plugin_record_permission_scope_revisions: ['plugin_record_permission_policy_revision', 'plugin_record_permission_grant_revision'],
  };
  for (const [name, expected] of Object.entries(guards)) {
    assert.deepEqual(config.ownedTableContracts[name].triggers.map((trigger) => trigger.name), expected, name);
  }
  const catalog = config.ownedTableContracts.event_catalog;
  assert.deepEqual(catalog.partialIndexes,
    [{ columns: ["(metadata->>'id')"], predicate: "origin = 'custom'" }]);
  for (const mutation of [
    (table) => { table.indexes = table.indexes.filter((index) => index.name !== 'event_catalog_custom_definition'); },
    (table) => { table.indexes.find((index) => index.name === 'event_catalog_custom_definition').predicate = "origin = 'core'"; },
    (table) => { table.indexes.find((index) => index.name === 'event_catalog_scope_updated').predicate = 'available'; },
  ]) {
    const candidate = structuredClone(inventory);
    mutation(candidate.tables.find((table) => table.name === 'event_catalog'));
    assert.ok(errors(candidate).some((finding) => finding.table === 'event_catalog'
      && finding.rule === 'owned-table-lookup-index'));
  }
  // Mutating a trigger on its actual owning table must fail even when the
  // contract belongs to the revision counter that the trigger maintains.
  for (const name of boundedResources) {
    for (const trigger of config.ownedTableContracts[name].triggers || []) {
      for (const patch of [null, { function: 'unrelated_guard' }, { events: [] }, { timing: 'instead of' }]) {
        const candidate = structuredClone(inventory);
        const owner = candidate.tables.find((table) => table.name === trigger.table);
        owner.triggers = owner.triggers.flatMap((item) => item.name !== trigger.name ? [item]
          : patch ? [{ ...item, ...patch }] : []);
        assert.ok(errors(candidate).some((finding) => finding.table === name
          && finding.rule === 'owned-table-validation-trigger'), `${name}: ${trigger.name}`);
      }
    }
  }
});
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
  assert.equal(timeline.columns.find((column) => column.name === 'input_id').nullable, true,
    'proactive outbound messages have no inbound input');
  assert.ok(config.ownedTableContracts.communication_messages.requiredColumns.includes('input_id'));
  for (const column of ['processing_status', 'send_status', 'claim_expires_at']) {
    assert.equal(timeline.columns.some((item) => item.name === column), false, column);
    assert.ok(config.ownedTableContracts.communication_inputs.requiredColumns.includes(column));
  }
  const heads = inventory.tables.find((table) => table.name === 'runtime_internal_compaction_heads');
  assert.ok(heads.columns.find((column) => column.name === 'checkpoint_content_id').nullable);
  assert.equal(heads.columns.some((column) => column.name === 'created_at'), false);
});

test('channel send receipts retain send-log ownership and scoped history lookup', () => {
  const config = loadConfig(repoRoot);
  const inventory = collectSchemaInventory({ repoRoot });
  const errors = (candidate) => evaluateSchemaHygiene({ inventory: candidate, config }).findings
    .filter((finding) => finding.table === 'communication_message_sends' && finding.severity === 'error');
  assert.deepEqual(errors(inventory), []);
  const reject = (mutate, rule) => {
    const candidate = structuredClone(inventory);
    mutate(candidate.tables.find((table) => table.name === 'communication_message_sends'));
    assert.ok(errors(candidate).some((finding) => finding.rule === rule), rule);
  };
  reject((table) => { table.primaryKey.columns = ['id']; }, 'owned-table-primary-key');
  reject((table) => { table.foreignKeys = []; }, 'owned-table-foreign-key');
  reject((table) => { table.columns.find((column) => column.name === 'scope_id').nullable = true; },
    'owned-table-owner-column');
  reject((table) => { table.indexes = []; }, 'owned-table-lookup-index');
  reject((table) => { table.columns = table.columns.filter((column) => column.name !== 'receipt_json'); },
    'owned-table-required-column');
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
