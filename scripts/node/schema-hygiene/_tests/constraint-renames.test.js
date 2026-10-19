const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const { collectSchemaInventory } = require('../core.js');

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
