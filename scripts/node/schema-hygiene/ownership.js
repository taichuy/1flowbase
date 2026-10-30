// Specialized tables retain their real owner identity instead of manufacturing
// an independent workspace chronology. Exemptions never skip these checks.
function sameColumns(left = [], right = []) {
  return left.length === right.length && left.every((value, index) => value === right[index]);
}

function ownedTableFindings(table, contract, tables = [table]) {
  if (!contract) return [];
  const findings = [];
  const report = (rule, message, column = null) => findings.push({ rule, message, column });
  if (!sameColumns(table.primaryKey?.columns, contract.primaryKey)) {
    report('owned-table-primary-key', 'specialized table must retain its declared owner-keyed row identity');
  }
  for (const owner of contract.foreignKeys || []) {
    const present = table.foreignKeys.some((key) => sameColumns(key.columns, owner.columns)
      && key.references.table === owner.table && sameColumns(key.references.columns, owner.references)
      && (!owner.onDelete || (key.onDelete || 'no action') === owner.onDelete));
    if (!present) report('owned-table-foreign-key', `required owner reference to ${owner.table} is missing`);
    for (const name of owner.columns) {
      const column = table.columns.find((item) => item.name === name);
      if (!column || (column.nullable && !(contract.nullableOwnerColumns || []).includes(name))) {
        report('owned-table-owner-column', 'owner routing columns must exist and be non-null', name);
      }
    }
  }
  // Some retained facts intentionally outlive a deleted owner. Their routing
  // identity remains mandatory even when a cascading FK would destroy history.
  for (const name of new Set([...(contract.ownerColumns || []), ...(contract.routingColumns || [])])) {
    const column = table.columns.find((item) => item.name === name);
    if (!column || column.nullable) {
      report('owned-table-owner-column', 'retained owner identity must exist and be non-null', name);
    }
  }
  for (const name of contract.requiredColumns || []) {
    if (!table.columns.some((item) => item.name === name)) {
      report('owned-table-required-column', 'specialized lifecycle column must remain present', name);
    }
  }
  const indexes = [table.primaryKey, ...table.indexes, ...table.uniqueConstraints].filter(Boolean);
  for (const columns of contract.indexes || []) {
    if (!indexes.some((index) => sameColumns(index.columns.slice(0, columns.length), columns))) {
      report('owned-table-lookup-index', `required owner lookup index (${columns.join(', ')}) is missing`);
    }
  }
  // Workspace-wide keyset pages include every row. A partial index cannot
  // satisfy this contract, even when its leading columns match the query.
  for (const columns of contract.unconditionalIndexes || []) {
    if (!indexes.some((index) => !index.predicate
      && sameColumns(index.columns.slice(0, columns.length), columns))) {
      report('owned-table-lookup-index', `required unfiltered lookup index (${columns.join(', ')}) is missing`);
    }
  }
  // ON CONFLICT identities require an unfiltered unique key over exactly these
  // columns; a longer unique key does not establish uniqueness of its prefix.
  const uniqueKeys = [table.primaryKey, ...table.uniqueConstraints,
    ...table.indexes.filter((index) => index.unique)].filter(Boolean);
  for (const columns of contract.uniqueIndexes || []) {
    if (!uniqueKeys.some((index) => !index.predicate
      && sameColumns([...index.columns].sort(), [...columns].sort()))) {
      report('owned-table-lookup-index', `required unique identity (${columns.join(', ')}) is missing`);
    }
  }
  // Predicates are the parser's SQL text, including literal case. Never fold
  // case or remove quotes: that could accept a different pending-row set.
  for (const required of contract.partialIndexes || []) {
    if (!indexes.some((index) => sameColumns(index.columns.slice(0, required.columns.length), required.columns)
      && index.predicate === required.predicate)) {
      report('owned-table-lookup-index', `required filtered lookup (${required.columns.join(', ')}) is missing or changed`);
    }
  }
  for (const required of contract.triggers || []) {
    const target = tables.find((candidate) => candidate.name === required.table);
    const present = (target?.triggers || []).some((trigger) => trigger.name === required.name
      && trigger.timing === required.timing && trigger.forEach === 'row'
      && !trigger.when && !trigger.constraint && !(trigger.transitionTables || []).length
      && Array.isArray(trigger.arguments) && !trigger.arguments.length
      && Array.isArray(trigger.events)
      && trigger.function === required.function
      && trigger.events.length === required.events.length
      && required.events.every((event) => trigger.events.some((candidate) => candidate.event === event.event
        && sameColumns([...(candidate.columns || [])].sort(), [...(event.columns || [])].sort()))));
    if (!present) {
      report('owned-table-validation-trigger', `required owner lifecycle trigger ${required.table}.${required.name} is missing or changed`);
    }
  }
  return findings;
}

module.exports = { ownedTableFindings };
