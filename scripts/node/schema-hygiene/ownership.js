// Specialized tables retain their real owner identity instead of manufacturing
// an independent workspace chronology. Exemptions never skip these checks.
function sameColumns(left = [], right = []) {
  return left.length === right.length && left.every((value, index) => value === right[index]);
}

function ownedTableFindings(table, contract) {
  if (!contract) return [];
  const findings = [];
  const report = (rule, message, column = null) => findings.push({ rule, message, column });
  if (!sameColumns(table.primaryKey?.columns, contract.primaryKey)) {
    report('owned-table-primary-key', 'specialized table must retain its declared owner-keyed row identity');
  }
  for (const owner of contract.foreignKeys || []) {
    const present = table.foreignKeys.some((key) => sameColumns(key.columns, owner.columns)
      && key.references.table === owner.table && sameColumns(key.references.columns, owner.references));
    if (!present) report('owned-table-foreign-key', `required owner reference to ${owner.table} is missing`);
    for (const name of owner.columns) {
      const column = table.columns.find((item) => item.name === name);
      if (!column || (column.nullable && !(contract.nullableOwnerColumns || []).includes(name))) {
        report('owned-table-owner-column', 'owner routing columns must exist and be non-null', name);
      }
    }
  }
  const indexes = [table.primaryKey, ...table.indexes, ...table.uniqueConstraints].filter(Boolean);
  for (const columns of contract.indexes || []) {
    if (!indexes.some((index) => sameColumns(index.columns.slice(0, columns.length), columns))) {
      report('owned-table-lookup-index', `required owner lookup index (${columns.join(', ')}) is missing`);
    }
  }
  return findings;
}

module.exports = { ownedTableFindings };
