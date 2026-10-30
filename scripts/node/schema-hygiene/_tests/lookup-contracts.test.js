const test = require('node:test');
const assert = require('node:assert/strict');
const { ownedTableFindings } = require('../ownership.js');

function table(patch = {}) {
  return { name: 'owned_rows', primaryKey: { columns: ['id'] }, columns: [],
    foreignKeys: [], indexes: [], uniqueConstraints: [], ...patch };
}
const contract = { primaryKey: ['id'], uniqueIndexes: [['owner_id', 'operation_id']] };
const identity = ['owner_id', 'operation_id'];
const error = (source, rule) => ownedTableFindings(source, rule)
  .filter((finding) => finding.rule === 'owned-table-lookup-index');

test('unique owner identities accept constraints and unconditional unique indexes in either key order', () => {
  for (const source of [
    table({ uniqueConstraints: [{ columns: identity }] }),
    table({ indexes: [{ columns: identity, unique: true }] }),
    table({ indexes: [{ columns: [...identity].reverse(), unique: true }] }),
  ]) assert.deepEqual(error(source, contract), []);
  assert.deepEqual(error(table(), { primaryKey: ['id'], uniqueIndexes: [['id']] }), []);
});

test('ordinary, partial and longer unique indexes cannot authorize an ON CONFLICT identity', () => {
  for (const source of [
    table({ indexes: [{ columns: identity, unique: false }] }),
    table({ indexes: [{ columns: identity, unique: true, predicate: 'completed_at is null' }] }),
    table({ uniqueConstraints: [{ columns: identity, predicate: 'completed_at is null' }] }),
    table({ indexes: [{ columns: [...identity, 'sequence'], unique: true }] }),
    table({ uniqueConstraints: [{ columns: ['owner_id'] }] }),
  ]) assert.equal(error(source, contract).length, 1);
});

test('filtered lookup retains key prefix and exact predicate without changing literal semantics', () => {
  const required = { primaryKey: ['id'], partialIndexes: [
    { columns: ['owner_id', 'sequence'], predicate: "status = 'Pending'" },
  ] };
  const index = { columns: ['owner_id', 'sequence', 'id'], predicate: "status = 'Pending'" };
  assert.deepEqual(error(table({ indexes: [index] }), required), []);
  for (const patch of [
    { predicate: null }, { predicate: "status = 'pending'" },
    { predicate: "status != 'Pending'" }, { columns: ['sequence', 'owner_id'] },
  ]) assert.equal(error(table({ indexes: [{ ...index, ...patch }] }), required).length, 1);
  assert.equal(error(table(), required).length, 1);
});
