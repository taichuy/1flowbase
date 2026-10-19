const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const { collectSchemaInventory, evaluateSchemaHygiene } = require('../core.js');

function inventory(t, migrations) {
  const repoRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'schema-trigger-contract-'));
  t.after(() => fs.rmSync(repoRoot, { recursive: true, force: true }));
  const directory = path.join(repoRoot, 'api/crates/storage/durable/postgres/migrations');
  fs.mkdirSync(directory, { recursive: true });
  migrations.forEach((sql, index) => fs.writeFileSync(path.join(directory, `20261019000${index}_fixture.sql`), sql));
  return collectSchemaInventory({ repoRoot });
}
const schema = `create table authority (id uuid primary key, workspace_id uuid not null,
  installation_id uuid not null, scope_id uuid not null, created_at timestamptz, updated_at timestamptz);
  create index authority_scope on authority(scope_id,created_at,id);`;
const create = `create trigger validate_scope before insert or update of workspace_id, installation_id
  on authority for each row execute function validate_authority_scope();`;
const config = { ownedTableContracts: { authority: { primaryKey: ['id'], triggers: [{
  table: 'authority', name: 'validate_scope', timing: 'before',
  events: [{ event: 'insert' }, { event: 'update', columns: ['workspace_id', 'installation_id'] }],
  function: 'validate_authority_scope',
}] } } };
function errors(result) { return evaluateSchemaHygiene({ inventory: result, config }).findings.filter((item) => item.severity === 'error'); }

test('migration replay records the current trigger shape and removes a subsequently dropped attachment', (t) => {
  const present = inventory(t, [schema, create]);
  assert.deepEqual(present.parseErrors, []);
  assert.deepEqual(errors(present), []);
  assert.deepEqual(present.tables[0].triggers[0].events, config.ownedTableContracts.authority.triggers[0].events
    .map((event) => ({ ...event, columns: event.columns || [] })));
  for (const drop of ['drop trigger validate_scope on authority', 'drop trigger if exists validate_scope on authority cascade']) {
    const removed = inventory(t, [schema, create, `${drop};`]);
    assert.deepEqual(removed.parseErrors, []);
    assert.deepEqual(removed.tables[0].triggers, []);
    assert.ok(errors(removed).some((item) => item.rule === 'owned-table-validation-trigger'));
  }
});

test('dropping a trigger function with cascade removes its current attachments', (t) => {
  const result = inventory(t, [schema, create, 'drop function if exists validate_authority_scope() cascade;']);
  assert.deepEqual(result.parseErrors, []);
  assert.deepEqual(result.tables[0].triggers, []);
  assert.ok(errors(result).some((item) => item.rule === 'owned-table-validation-trigger'));
});

test('replacing, filtering or adding arguments to a required trigger cannot preserve its owner contract', (t) => {
  for (const changed of [
    create.replace('create trigger', 'create or replace trigger').replace('validate_authority_scope()', 'unrelated_scope()'),
    create.replace('for each row', 'for each row when (new.workspace_id is not null)'),
    create.replace('validate_authority_scope()', "validate_authority_scope('narrow')"),
    create.replace('update of workspace_id, installation_id', 'update of workspace_id'),
  ]) {
    const result = inventory(t, [schema, create, changed]);
    assert.deepEqual(result.parseErrors, []);
    assert.ok(errors(result).some((item) => item.rule === 'owned-table-validation-trigger'));
  }
});

test('unsupported create/drop/alter trigger variants remain parse errors instead of evidence of a valid owner', (t) => {
  for (const [sql, rule] of [
    [create.replace('for each row', 'referencing new row as changed for each row'), 'unsupported-create-trigger'],
    ['drop trigger validate_scope on authority unexpected;', 'unsupported-drop-trigger'],
    ['alter trigger validate_scope on authority rename to renamed_scope;', 'unsupported-alter-trigger'],
    ['drop function validate_authority_scope(), other_scope() cascade;', 'unsupported-drop-trigger-function'],
  ]) {
    const result = inventory(t, [schema, create, sql]);
    assert.ok(errors(result).some((item) => item.rule === rule && item.severity === 'error'));
  }
});

test('trigger metadata preserves quoted identifiers and constraint conditions without treating views as tables', (t) => {
  const result = inventory(t, [schema, `
    create constraint trigger "deferred_guard" after delete on authority deferrable initially deferred
      for each row when (old.workspace_id is not null) execute function "guard"();
    create trigger view_writer instead of insert on view_only for each row execute function write_view();
  `]);
  assert.deepEqual(result.parseErrors, []);
  assert.equal(result.tables.length, 1);
  const trigger = result.tables[0].triggers[0];
  assert.equal(trigger.name, 'deferred_guard');
  assert.equal(trigger.constraint, true);
  assert.equal(trigger.when, 'old.workspace_id is not null');
  assert.equal(trigger.function, 'guard');
});

test('the existing sequence-high-water migration retains its statement transition-table trigger', () => {
  const repoRoot = path.resolve(__dirname, '..', '..', '..', '..');
  const migration = 'api/crates/storage/durable/postgres/migrations/20260929100000_client_semantic_storage.sql';
  const result = collectSchemaInventory({ repoRoot });
  assert.deepEqual(result.parseErrors.filter((error) => error.file === migration), []);
  const trigger = result.tables.find((table) => table.name === 'runtime_events').triggers
    .find((entry) => entry.name === 'runtime_event_sequence_high_water');
  assert.ok(trigger);
  assert.equal(trigger.source, migration);
  assert.equal(trigger.timing, 'after');
  assert.equal(trigger.forEach, 'statement');
  assert.deepEqual(trigger.events, [{ event: 'insert', columns: [] }]);
  assert.deepEqual(trigger.transitionTables, [{ kind: 'new', name: 'appended_runtime_events' }]);
  assert.equal(trigger.function, 'maintain_runtime_event_sequence_high_water');
});

test('transition relations preserve OLD/NEW aliases and do not turn statement or transition triggers into a row owner guard', (t) => {
  const after = `create trigger validate_scope after insert on authority
    for each row execute function validate_authority_scope();`;
  const rowConfig = { ownedTableContracts: { authority: { primaryKey: ['id'], triggers: [{
    ...config.ownedTableContracts.authority.triggers[0], timing: 'after', events: [{ event: 'insert' }],
  }] } } };
  const evaluate = (sql) => {
    const result = inventory(t, [schema, sql]);
    assert.deepEqual(result.parseErrors, []);
    return evaluateSchemaHygiene({ inventory: result, config: rowConfig });
  };
  assert.deepEqual(evaluate(after).findings, []);
  for (const clause of ['for each statement',
    'referencing new table as appended for each statement',
    'referencing new table as appended for each row']) {
    const report = evaluate(after.replace('for each row', clause));
    assert.ok(report.findings.some((finding) => finding.rule === 'owned-table-validation-trigger'));
  }
  const updated = inventory(t, [schema, after.replace('after insert', 'after update')
    .replace('for each row', 'referencing old table as previous new table as appended for each statement')]);
  assert.deepEqual(updated.parseErrors, []);
  assert.deepEqual(updated.tables[0].triggers[0].transitionTables,
    [{ kind: 'old', name: 'previous' }, { kind: 'new', name: 'appended' }]);
});

test('unknown or invalid transition-table declarations fail closed', (t) => {
  const statement = `create trigger changed after insert on authority
    referencing new table as appended for each statement execute function consume_changes();`;
  for (const changed of [
    statement.replace('new table', 'new relation'),
    statement.replace('as appended', 'as appended new table as duplicate'),
    statement.replace('as appended', 'as appended trailing'),
    statement.replace('as appended', 'as'),
    statement.replace('after insert', 'before insert'),
    statement.replace('after insert', 'after delete'),
    statement.replace('after insert', 'after insert or update'),
    statement.replace('after insert', 'after update of workspace_id'),
    statement.replace('create trigger', 'create constraint trigger'),
  ]) {
    const result = inventory(t, [schema, changed]);
    assert.ok(result.parseErrors.some((error) => error.rule === 'unsupported-create-trigger'));
  }
});
