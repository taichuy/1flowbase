const test = require('node:test');
const assert = require('node:assert/strict');
const path = require('node:path');
const { collectSchemaInventory, evaluateSchemaHygiene, loadConfig } = require('../core.js');

const repoRoot = path.resolve(__dirname, '..', '..', '..', '..');
const names = ['flow_container_checkpoints', 'flow_container_executions', 'flow_container_rounds',
  'flow_container_control_failures', 'flow_container_observations'];

// Source contracts: 20261029120000_bounded_container_state.sql and
// 20261029121000_bounded_container_observations.sql, consumed by the container
// transition/observation repository. Shared ownership.test.js removes every
// declared PK, FK, non-null owner, lifecycle column and lookup index, and proves
// ordinary business tables do not inherit these named exceptions.
function fixture() {
  const config = loadConfig(repoRoot);
  const inventory = collectSchemaInventory({ repoRoot });
  const findings = (candidate = inventory) => evaluateSchemaHygiene({ inventory: candidate, config })
    .findings.filter((finding) => names.includes(finding.table) && finding.severity === 'error');
  function rejected(name, mutate, rule) {
    const candidate = structuredClone(inventory);
    mutate(candidate.tables.find((table) => table.name === name));
    assert.ok(findings(candidate).some((finding) => finding.table === name && finding.rule === rule),
      `${name}: ${rule}`);
  }
  return { config, inventory, findings, rejected };
}

test('container migration inventory retains direct and execution-mediated run ownership', () => {
  const { config, inventory, findings } = fixture();
  assert.deepEqual(findings(), []);
  const primaryKeys = [['flow_run_id'], ['flow_run_id', 'execution_id'], ['flow_run_id', 'execution_id'],
    ['flow_run_id', 'execution_id', 'after_round_index'], ['id']];
  names.forEach((name, index) => {
    const table = inventory.tables.find((candidate) => candidate.name === name);
    assert.ok(table, name);
    assert.deepEqual(table.primaryKey.columns, primaryKeys[index], name);
    assert.deepEqual(config.ownedTableContracts[name].primaryKey, primaryKeys[index], name);
    assert.equal(config.systemScopeTables.includes(name), false, name);
    assert.equal(config.appendOnlyTables.includes(name), false, name);
    assert.equal(config.needsOwnerReviewTables[name], undefined, name);
    assert.equal(table.columns.some((column) => column.name === 'scope_id'), false, name);
    if (index < 3) {
      assert.equal(config.tableProfiles[name], 'flow_run_owned_table', name);
      assert.equal(config.exemptions[name], undefined, name);
      assert.deepEqual(config.ownedTableContracts[name].foreignKeys[0], {
        columns: ['flow_run_id'], table: 'flow_runs', references: ['id'], onDelete: 'cascade',
      });
    } else {
      assert.equal(config.tableProfiles[name], undefined, 'composite execution FK must stay active');
      assert.deepEqual(config.ownedTableContracts[name].foreignKeys, [{
        columns: ['flow_run_id', 'execution_id'], table: 'flow_container_executions',
        references: ['flow_run_id', 'execution_id'], onDelete: 'cascade',
      }]);
      assert.ok(config.exemptions[name].skip.every((rule) => rule.startsWith('managed-table-')));
    }
  });
  const executions = inventory.tables.find((table) => table.name === 'flow_container_executions');
  assert.ok(executions.columns.find((column) => column.name === 'node_run_id').nullable);
  assert.deepEqual(config.ownedTableContracts.flow_container_executions.nullableOwnerColumns, ['node_run_id']);
  const observations = inventory.tables.find((table) => table.name === 'flow_container_observations');
  assert.ok(observations.columns.find((column) => column.name === 'completed_at').nullable);
});

test('container owner contracts reject altered row identity and exact reference or deletion changes', () => {
  const { config, rejected } = fixture();
  for (const name of names) {
    rejected(name, (table) => { table.primaryKey.columns = ['unrelated_id']; }, 'owned-table-primary-key');
    for (const reference of config.ownedTableContracts[name].foreignKeys) {
      const matches = (key) => key.columns.join(',') === reference.columns.join(',');
      for (const mutate of [
        (table) => { table.foreignKeys = table.foreignKeys.filter((key) => !matches(key)); },
        (table) => { table.foreignKeys.find(matches).columns = ['unrelated_owner']; },
        (table) => { table.foreignKeys.find(matches).references.table = 'unrelated_owner'; },
        (table) => { table.foreignKeys.find(matches).references.columns = ['unrelated_id']; },
        (table) => { table.foreignKeys.find(matches).onDelete = 'no action'; },
      ]) rejected(name, mutate, 'owned-table-foreign-key');
    }
  }
});

// These declarations depend on the generic partialIndexes / uniqueIndexes
// checks in ownership.js. Column-prefix presence alone cannot protect pending
// delivery selection or the repository's ON CONFLICT identities.
test('pending observation lookup keeps the incomplete-only predicate and sequence order', () => {
  const { config, rejected } = fixture();
  const name = 'flow_container_observations';
  const columns = ['flow_run_id', 'sequence'];
  assert.deepEqual(config.ownedTableContracts[name].partialIndexes, [{
    columns, predicate: 'completed_at is null',
  }]);
  const matches = (index) => index.columns.join(',') === columns.join(',');
  rejected(name, (table) => { table.indexes = table.indexes.filter((index) => !matches(index)); },
    'owned-table-lookup-index');
  for (const patch of [
    { predicate: null },
    { predicate: 'completed_at is not null' },
    { predicate: 'completed_at is null and trace_payload is not null' },
    { columns: ['sequence', 'flow_run_id'] },
  ]) {
    rejected(name, (table) => {
      table.indexes = table.indexes.map((index) => matches(index) ? { ...index, ...patch } : index);
    }, 'owned-table-lookup-index');
  }
});

test('container receipt and round identities reject uniqueness downgraded to ordinary indexes', () => {
  const { config, rejected } = fixture();
  const expected = {
    flow_container_executions: [['node_run_id']],
    flow_container_rounds: [['flow_run_id', 'container_execution_id', 'round_index']],
    flow_container_observations: [['sequence'], ['flow_run_id', 'execution_id', 'observation_key']],
  };
  for (const [name, identities] of Object.entries(expected)) {
    assert.deepEqual(config.ownedTableContracts[name].uniqueIndexes, identities, name);
    for (const columns of identities) {
      const matches = (index) => index.columns.join(',') === columns.join(',');
      rejected(name, (table) => {
        const constraints = table.uniqueConstraints.filter(matches);
        assert.ok(constraints.length || table.indexes.some((index) => matches(index) && index.unique),
          `${name}: migration must declare the expected unique identity`);
        table.uniqueConstraints = table.uniqueConstraints.filter((index) => !matches(index));
        table.indexes = table.indexes.map((index) => matches(index) ? { ...index, unique: false } : index);
        table.indexes.push(...constraints.map((index) => ({ ...index, unique: false })));
      }, 'owned-table-lookup-index');
    }
  }
});
