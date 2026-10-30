const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const { collectSchemaInventory, evaluateSchemaHygiene } = require('../core.js');

function inventory(t, sql) {
  const repoRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'schema-constraint-rename-'));
  t.after(() => fs.rmSync(repoRoot, { recursive: true, force: true }));
  const directory = path.join(repoRoot, 'api/crates/storage/durable/postgres/migrations');
  fs.mkdirSync(directory, { recursive: true });
  fs.writeFileSync(path.join(directory, '20261019000000_fixture.sql'), sql);
  return collectSchemaInventory({ repoRoot });
}

const schema = `
  create table parent_rows (id uuid primary key);
  create table event_rows (
    id uuid,
    parent_id uuid,
    sequence bigint,
    constraint event_pk primary key (id),
    constraint event_sequence unique (sequence),
    constraint event_parent foreign key (parent_id) references parent_rows(id),
    constraint event_positive check (sequence > 0)
  );
`;

test('constraint renames preserve keys, checks and references after table retirement', (t) => {
  const result = inventory(t, `${schema}
    alter table event_rows rename to retained_events;
    alter table retained_events rename constraint event_pk to retained_pk;
    alter table retained_events rename constraint event_sequence to retained_sequence;
    alter table retained_events rename constraint event_parent to retained_parent;
    alter table retained_events rename constraint event_positive to "retained positive";
  `);
  assert.deepEqual(result.parseErrors, []);
  const table = result.tables.find((entry) => entry.name === 'retained_events');
  assert.deepEqual(table.primaryKey, { columns: ['id'], name: 'retained_pk', source: 'table' });
  assert.equal(table.uniqueConstraints[0].name, 'retained_sequence');
  assert.deepEqual(table.uniqueConstraints[0].columns, ['sequence']);
  assert.equal(table.foreignKeys[0].name, 'retained_parent');
  assert.deepEqual(table.foreignKeys[0].references, { table: 'parent_rows', columns: ['id'] });
  assert.equal(table.checks[0].name, 'retained positive');
  assert.match(table.checks[0].definition, /sequence > 0/u);
});

test('dropping a renamed constraint removes its current identity', (t) => {
  const result = inventory(t, `${schema}
    alter table event_rows rename constraint event_pk to new_pk;
    alter table event_rows rename constraint event_positive to new_positive;
    alter table event_rows drop constraint new_pk;
    alter table event_rows drop constraint new_positive;
  `);
  assert.deepEqual(result.parseErrors, []);
  const table = result.tables.find((entry) => entry.name === 'event_rows');
  assert.equal(table.primaryKey, null);
  assert.deepEqual(table.checks, []);
  assert.equal(table.foreignKeys.length, 1);
});

test('malformed rename syntax and unrelated unsupported actions remain errors', (t) => {
  const result = inventory(t, `${schema}
    alter table event_rows rename constraint event_pk to;
    alter table event_rows rename constraint event_sequence to new_name trailing;
    alter table event_rows alter column sequence set statistics 1;
  `);
  assert.deepEqual(result.parseErrors.map((error) => error.rule), [
    'unsupported-alter-table-action',
    'unsupported-alter-table-action',
    'unsupported-alter-table-action',
  ]);
  assert.equal(result.tables.find((entry) => entry.name === 'event_rows').primaryKey.name, 'event_pk');
});


test('table rename moves existing inbound and self foreign keys before the old name is reused', (t) => {
  const result = inventory(t, `
    create table parent_rows (
      id uuid primary key,
      scope_id uuid not null,
      predecessor_id uuid references parent_rows(id),
      unique(scope_id, id)
    );
    create table child_rows (
      id uuid primary key,
      scope_id uuid not null,
      parent_id uuid not null,
      constraint child_parent foreign key(scope_id, parent_id)
        references parent_rows(scope_id, id) on delete cascade
    );
    alter table parent_rows rename to retained_parents;
    create table parent_rows (id uuid primary key);
    create table later_rows (id uuid primary key, parent_id uuid references parent_rows(id));
    alter table retained_parents rename to "archived_parents";
  `);
  assert.deepEqual(result.parseErrors, []);
  const child = result.tables.find((table) => table.name === 'child_rows');
  const archived = result.tables.find((table) => table.name === 'archived_parents');
  const later = result.tables.find((table) => table.name === 'later_rows');
  assert.ok(result.tables.some((table) => table.name === 'parent_rows'));
  assert.deepEqual(child.foreignKeys[0], {
    name: 'child_parent', columns: ['scope_id', 'parent_id'],
    references: { table: 'archived_parents', columns: ['scope_id', 'id'] },
    onDelete: 'cascade', source: 'table',
  });
  assert.equal(archived.foreignKeys[0].references.table, 'archived_parents');
  assert.equal(later.foreignKeys[0].references.table, 'parent_rows');

  const config = { ownedTableContracts: { child_rows: {
    primaryKey: ['id'], foreignKeys: [{
      columns: ['scope_id', 'parent_id'], table: 'archived_parents',
      references: ['scope_id', 'id'], onDelete: 'cascade',
    }],
  } } };
  const ownerErrors = (candidate) => evaluateSchemaHygiene({ inventory: candidate, config })
    .findings.filter((finding) => finding.table === 'child_rows'
      && finding.rule === 'owned-table-foreign-key' && finding.severity === 'error');
  assert.deepEqual(ownerErrors(result), []);
  // Rebinding the old FK to the new, unrelated relation must be rejected even
  // though that relation exists under the original SQL name.
  const wrongOwner = structuredClone(result);
  wrongOwner.tables.find((table) => table.name === 'child_rows')
    .foreignKeys[0].references.table = 'parent_rows';
  assert.equal(ownerErrors(wrongOwner).length, 1);
});
