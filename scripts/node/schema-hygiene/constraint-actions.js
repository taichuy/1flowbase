function parseAlterTableDropConstraint(table, action, normalizeIdentifier) {
  const match = /^drop\s+constraint\s+(?:if\s+exists\s+)?("[^"]+"|[a-zA-Z_][a-zA-Z0-9_$]*)/iu.exec(action.trim());
  if (!match) {
    return false;
  }
  const constraintName = normalizeIdentifier(match[1]);
  table.uniqueConstraints = table.uniqueConstraints.filter((constraint) => constraint.name !== constraintName);
  table.foreignKeys = table.foreignKeys.filter((constraint) => constraint.name !== constraintName);
  table.checks = table.checks.filter((constraint) => constraint.name !== constraintName);
  if (table.primaryKey && table.primaryKey.name === constraintName) {
    table.primaryKey = null;
  }
  return true;
}

// Constraint names change identity, not columns or referential semantics.
function parseAlterTableRenameConstraint(table, action, normalizeIdentifier) {
  const match = /^rename\s+constraint\s+("[^"]+"|[a-zA-Z_][a-zA-Z0-9_$]*)\s+to\s+("[^"]+"|[a-zA-Z_][a-zA-Z0-9_$]*)$/iu.exec(action.trim());
  if (!match) return false;
  const previous = normalizeIdentifier(match[1]);
  const next = normalizeIdentifier(match[2]);
  for (const constraint of [table.primaryKey, ...table.uniqueConstraints, ...table.foreignKeys, ...table.checks, ...table.indexes]) {
    if (constraint?.name === previous) constraint.name = next;
  }
  // PostgreSQL-generated names are not all represented by the static inventory.
  // Recognizing the grammar must not invent columns or a new constraint.
  return true;
}

module.exports = { parseAlterTableDropConstraint, parseAlterTableRenameConstraint };
