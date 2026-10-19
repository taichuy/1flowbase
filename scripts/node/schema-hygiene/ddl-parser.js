const { parseAlterTableDropConstraint, parseAlterTableRenameConstraint } = require('./constraint-actions.js');
const { actionForRule } = require("./finding-actions.js");

const COLUMN_CONSTRAINT_KEYWORDS = [
  'not',
  'null',
  'default',
  'primary',
  'unique',
  'references',
  'check',
  'constraint',
  'generated',
  'collate',
];

function stripIdentifierQuotes(identifier) {
  const trimmed = identifier.trim();
  if (trimmed.startsWith('"') && trimmed.endsWith('"')) {
    return trimmed.slice(1, -1).replace(/""/gu, '"');
  }
  return trimmed;
}

function normalizeIdentifier(identifier) {
  const raw = identifier.trim().replace(/^only\s+/iu, '');
  const parts = splitTopLevel(raw, '.');
  return stripIdentifierQuotes(parts[parts.length - 1]).toLowerCase();
}

function readIdentifier(input) {
  const trimmed = input.trimStart();
  if (trimmed.startsWith('"')) {
    let cursor = 1;
    while (cursor < trimmed.length) {
      if (trimmed[cursor] === '"' && trimmed[cursor + 1] === '"') {
        cursor += 2;
        continue;
      }
      if (trimmed[cursor] === '"') {
        return {
          identifier: trimmed.slice(0, cursor + 1),
          rest: trimmed.slice(cursor + 1).trimStart(),
        };
      }
      cursor += 1;
    }
  }

  const match = /^([a-zA-Z_][a-zA-Z0-9_$]*)([\s\S]*)$/u.exec(trimmed);
  if (!match) {
    return null;
  }

  return {
    identifier: match[1],
    rest: match[2].trimStart(),
  };
}

function stripSqlComments(sql) {
  let output = '';
  let index = 0;
  let singleQuoted = false;
  let doubleQuoted = false;
  let dollarQuote = null;

  while (index < sql.length) {
    const current = sql[index];
    const next = sql[index + 1];

    if (!singleQuoted && !doubleQuoted && dollarQuote === null) {
      if (current === '-' && next === '-') {
        while (index < sql.length && sql[index] !== '\n') {
          index += 1;
        }
        output += '\n';
        index += 1;
        continue;
      }
      if (current === '/' && next === '*') {
        index += 2;
        while (index < sql.length && !(sql[index] === '*' && sql[index + 1] === '/')) {
          output += sql[index] === '\n' ? '\n' : ' ';
          index += 1;
        }
        index += 2;
        continue;
      }
      const dollarMatch = /^\$[a-zA-Z0-9_]*\$/u.exec(sql.slice(index));
      if (dollarMatch) {
        dollarQuote = dollarMatch[0];
        output += dollarQuote;
        index += dollarQuote.length;
        continue;
      }
    }

    if (!doubleQuoted && dollarQuote === null && current === "'") {
      output += current;
      if (singleQuoted && next === "'") {
        output += next;
        index += 2;
        continue;
      }
      singleQuoted = !singleQuoted;
      index += 1;
      continue;
    }

    if (!singleQuoted && dollarQuote === null && current === '"') {
      output += current;
      if (doubleQuoted && next === '"') {
        output += next;
        index += 2;
        continue;
      }
      doubleQuoted = !doubleQuoted;
      index += 1;
      continue;
    }

    if (!singleQuoted && !doubleQuoted && dollarQuote !== null && sql.startsWith(dollarQuote, index)) {
      output += dollarQuote;
      index += dollarQuote.length;
      dollarQuote = null;
      continue;
    }

    output += current;
    index += 1;
  }

  return output;
}

function splitSqlStatements(sql) {
  const statements = [];
  let current = '';
  let index = 0;
  let singleQuoted = false;
  let doubleQuoted = false;
  let dollarQuote = null;

  while (index < sql.length) {
    const char = sql[index];
    const next = sql[index + 1];

    if (!singleQuoted && !doubleQuoted && dollarQuote === null) {
      if (char === '-' && next === '-') {
        while (index < sql.length && sql[index] !== '\n') {
          current += sql[index];
          index += 1;
        }
        continue;
      }
      if (char === '/' && next === '*') {
        current += char + next;
        index += 2;
        while (index < sql.length && !(sql[index] === '*' && sql[index + 1] === '/')) {
          current += sql[index];
          index += 1;
        }
        if (index < sql.length) {
          current += sql[index] + sql[index + 1];
          index += 2;
        }
        continue;
      }
      const dollarMatch = /^\$[a-zA-Z0-9_]*\$/u.exec(sql.slice(index));
      if (dollarMatch) {
        dollarQuote = dollarMatch[0];
        current += dollarQuote;
        index += dollarQuote.length;
        continue;
      }
    }

    if (!doubleQuoted && dollarQuote === null && char === "'") {
      current += char;
      if (singleQuoted && next === "'") {
        current += next;
        index += 2;
        continue;
      }
      singleQuoted = !singleQuoted;
      index += 1;
      continue;
    }

    if (!singleQuoted && dollarQuote === null && char === '"') {
      current += char;
      if (doubleQuoted && next === '"') {
        current += next;
        index += 2;
        continue;
      }
      doubleQuoted = !doubleQuoted;
      index += 1;
      continue;
    }

    if (!singleQuoted && !doubleQuoted && dollarQuote !== null && sql.startsWith(dollarQuote, index)) {
      current += dollarQuote;
      index += dollarQuote.length;
      dollarQuote = null;
      continue;
    }

    if (!singleQuoted && !doubleQuoted && dollarQuote === null && char === ';') {
      const statement = current.trim();
      if (statement.length > 0) {
        statements.push(statement);
      }
      current = '';
      index += 1;
      continue;
    }

    current += char;
    index += 1;
  }

  const tail = current.trim();
  if (tail.length > 0) {
    statements.push(tail);
  }

  return statements;
}

function splitTopLevel(input, delimiter = ',') {
  const parts = [];
  let current = '';
  let depth = 0;
  let singleQuoted = false;
  let doubleQuoted = false;
  let dollarQuote = null;
  let index = 0;

  while (index < input.length) {
    const char = input[index];
    const next = input[index + 1];

    if (!singleQuoted && !doubleQuoted && dollarQuote === null) {
      const dollarMatch = /^\$[a-zA-Z0-9_]*\$/u.exec(input.slice(index));
      if (dollarMatch) {
        dollarQuote = dollarMatch[0];
        current += dollarQuote;
        index += dollarQuote.length;
        continue;
      }
    }

    if (!doubleQuoted && dollarQuote === null && char === "'") {
      current += char;
      if (singleQuoted && next === "'") {
        current += next;
        index += 2;
        continue;
      }
      singleQuoted = !singleQuoted;
      index += 1;
      continue;
    }

    if (!singleQuoted && dollarQuote === null && char === '"') {
      current += char;
      if (doubleQuoted && next === '"') {
        current += next;
        index += 2;
        continue;
      }
      doubleQuoted = !doubleQuoted;
      index += 1;
      continue;
    }

    if (!singleQuoted && !doubleQuoted && dollarQuote !== null && input.startsWith(dollarQuote, index)) {
      current += dollarQuote;
      index += dollarQuote.length;
      dollarQuote = null;
      continue;
    }

    if (!singleQuoted && !doubleQuoted && dollarQuote === null) {
      if (char === '(') {
        depth += 1;
      } else if (char === ')') {
        depth = Math.max(0, depth - 1);
      }

      if (depth === 0 && char === delimiter) {
        parts.push(current.trim());
        current = '';
        index += 1;
        continue;
      }
    }

    current += char;
    index += 1;
  }

  const tail = current.trim();
  if (tail.length > 0) {
    parts.push(tail);
  }
  return parts;
}

function createTable(name, source) {
  return {
    name,
    source,
    columns: [],
    primaryKey: null,
    uniqueConstraints: [],
    indexes: [],
    foreignKeys: [],
    triggers: [],
    checks: [],
    jsonbColumns: [],
  };
}

function getOrCreateTable(tables, tableName, source) {
  if (!tables.has(tableName)) {
    tables.set(tableName, createTable(tableName, source));
  }
  return tables.get(tableName);
}

function findColumn(table, columnName) {
  return table.columns.find((column) => column.name === columnName);
}

function upsertColumn(table, column) {
  const existing = findColumn(table, column.name);
  if (existing) {
    Object.assign(existing, column);
  } else {
    table.columns.push(column);
  }
  table.jsonbColumns = table.columns
    .filter((candidate) => /\bjsonb\b/iu.test(candidate.type))
    .map((candidate) => candidate.name);
}

function findConstraintKeywordIndex(rest) {
  const lower = rest.toLowerCase();
  let best = -1;
  for (const keyword of COLUMN_CONSTRAINT_KEYWORDS) {
    const match = new RegExp(`(^|\\s)${keyword}\\b`, 'u').exec(lower);
    if (!match) {
      continue;
    }
    const index = match.index + match[1].length;
    if (best === -1 || index < best) {
      best = index;
    }
  }
  return best;
}

function parseColumnDefinition(definition) {
  const identifier = readIdentifier(definition);
  if (!identifier) {
    return null;
  }

  const columnName = normalizeIdentifier(identifier.identifier);
  if (['like', 'exclude'].includes(columnName)) {
    return null;
  }

  const constraintIndex = findConstraintKeywordIndex(identifier.rest);
  const type = (constraintIndex === -1 ? identifier.rest : identifier.rest.slice(0, constraintIndex))
    .trim()
    .replace(/\s+/gu, ' ');

  return {
    name: columnName,
    type,
    nullable: !/\bnot\s+null\b/iu.test(identifier.rest) && !/\bprimary\s+key\b/iu.test(identifier.rest),
    default: /\bdefault\b/iu.test(identifier.rest),
    rawDefinition: definition.trim(),
  };
}

function parseColumnList(input) {
  return splitTopLevel(input, ',')
    .map((column) => normalizeIdentifier(column.replace(/\b(?:asc|desc|nulls\s+first|nulls\s+last)\b/giu, '').trim()))
    .filter(Boolean);
}

function parseReference(raw) {
  const match = /\breferences\s+([a-zA-Z0-9_."$]+)(?:\s*\(([^)]+)\))?/iu.exec(raw);
  if (!match) {
    return null;
  }
  return {
    table: normalizeIdentifier(match[1]),
    columns: match[2] ? parseColumnList(match[2]) : [],
  };
}

function foreignKeyDeleteAction(definition) {
  const match = /\bon\s+delete\s+(cascade|restrict|no\s+action|set\s+null|set\s+default)\b/iu.exec(definition);
  return match ? { onDelete: match[1].toLowerCase().replace(/\s+/gu, ' ') } : {};
}

function addInlineConstraints(table, column, raw) {
  if (/\bprimary\s+key\b/iu.test(raw)) {
    table.primaryKey = {
      columns: [column.name],
      name: null,
      source: 'inline',
    };
  }

  if (/\bunique\b/iu.test(raw)) {
    table.uniqueConstraints.push({
      columns: [column.name],
      name: null,
      source: 'inline',
    });
  }

  const reference = parseReference(raw);
  if (reference) {
    table.foreignKeys.push({
      columns: [column.name],
      references: reference,
      ...foreignKeyDeleteAction(raw),
      name: null,
      source: 'inline',
    });
  }
}

function parseTableConstraint(table, definition) {
  const normalized = definition
    .trim()
    .replace(/^constraint\s+("[^"]+"|[a-zA-Z_][a-zA-Z0-9_$]*)\s+/iu, '');
  const constraintNameMatch = /^constraint\s+("[^"]+"|[a-zA-Z_][a-zA-Z0-9_$]*)\s+/iu.exec(definition.trim());
  const constraintName = constraintNameMatch ? normalizeIdentifier(constraintNameMatch[1]) : null;

  const primaryKeyMatch = /^primary\s+key\s*\(([\s\S]+)\)$/iu.exec(normalized);
  if (primaryKeyMatch) {
    table.primaryKey = {
      columns: parseColumnList(primaryKeyMatch[1]),
      name: constraintName,
      source: 'table',
    };
    return true;
  }

  const uniqueMatch = /^unique\s*\(([\s\S]+)\)$/iu.exec(normalized);
  if (uniqueMatch) {
    table.uniqueConstraints.push({
      columns: parseColumnList(uniqueMatch[1]),
      name: constraintName,
      source: 'table',
    });
    return true;
  }

  const foreignKeyMatch = /^foreign\s+key\s*\(([^)]+)\)\s+references\s+([a-zA-Z0-9_."$]+)(?:\s*\(([^)]+)\))?/iu.exec(normalized);
  if (foreignKeyMatch) {
    table.foreignKeys.push({
      columns: parseColumnList(foreignKeyMatch[1]),
      references: {
        table: normalizeIdentifier(foreignKeyMatch[2]),
        columns: foreignKeyMatch[3] ? parseColumnList(foreignKeyMatch[3]) : [],
      },
      ...foreignKeyDeleteAction(normalized),
      name: constraintName,
      source: 'table',
    });
    return true;
  }

  if (/^check\s*\(/iu.test(normalized)) {
    table.checks.push({
      name: constraintName,
      definition: definition.trim(),
    });
    return true;
  }

  return false;
}

function parseCreateTable(statement, context) {
  const cleaned = stripSqlComments(statement).trim();
  const match = /^create\s+table\s+(?:if\s+not\s+exists\s+)?([a-zA-Z0-9_."$]+)\s*\(([\s\S]*)\)$/iu.exec(cleaned);
  if (!match) {
    context.parseErrors.push(createParseError(context, 'unsupported-create-table', statement));
    return;
  }

  const tableName = normalizeIdentifier(match[1]);
  const table = createTable(tableName, context.relativePath);
  context.tables.set(tableName, table);

  for (const definition of splitTopLevel(match[2], ',')) {
    if (/^(constraint|primary\s+key|unique|foreign\s+key|check)\b/iu.test(definition.trim())) {
      if (!parseTableConstraint(table, definition)) {
        context.parseErrors.push(createParseError(context, 'unsupported-table-constraint', definition));
      }
      continue;
    }

    const column = parseColumnDefinition(definition);
    if (!column || column.type.length === 0) {
      context.parseErrors.push(createParseError(context, 'unsupported-table-element', definition));
      continue;
    }
    upsertColumn(table, column);
    addInlineConstraints(table, column, definition);
  }
}

function parseCreateIndex(statement, context) {
  const cleaned = stripSqlComments(statement).replace(/\s+/gu, ' ').trim();
  const match = /^create\s+(unique\s+)?index\s+(?:concurrently\s+)?(?:if\s+not\s+exists\s+)?("[^"]+"|[a-zA-Z_][a-zA-Z0-9_$]*)\s+on\s+(?:only\s+)?([a-zA-Z0-9_."$]+)(?:\s+using\s+([a-zA-Z_][a-zA-Z0-9_$]*))?\s*\(([\s\S]+)\)(?:\s+nulls\s+not\s+distinct)?(?:\s+where\s+([\s\S]+))?$/iu.exec(cleaned);
  if (!match) {
    context.parseErrors.push(createParseError(context, 'unsupported-create-index', statement));
    return;
  }

  const tableName = normalizeIdentifier(match[3]);
  if (context.temporaryTables.has(tableName)) {
    return;
  }
  const table = getOrCreateTable(context.tables, tableName, context.relativePath);
  const columns = parseColumnList(match[5]);
  const index = {
    name: normalizeIdentifier(match[2]),
    unique: Boolean(match[1]),
    table: tableName,
    method: match[4] ? match[4].toLowerCase() : 'btree',
    columns,
    predicate: match[6] ? match[6].trim() : null,
    source: context.relativePath,
  };
  table.indexes.push(index);

  if (index.unique) {
    table.uniqueConstraints.push({
      columns,
      name: index.name,
      source: 'index',
      predicate: index.predicate,
    });
  }
}

function parseAlterTableAddColumn(table, action) {
  const columnDefinition = action.replace(/^add\s+column\s+(?:if\s+not\s+exists\s+)?/iu, '');
  const column = parseColumnDefinition(columnDefinition);
  if (!column || column.type.length === 0) {
    return false;
  }
  upsertColumn(table, column);
  addInlineConstraints(table, column, columnDefinition);
  return true;
}

function parseAlterTableAddConstraint(table, action) {
  const definition = action.replace(/^add\s+/iu, '');
  return parseTableConstraint(table, definition);
}

function parseAlterTableDropColumn(table, action) {
  const match = /^drop\s+column\s+(?:if\s+exists\s+)?("[^"]+"|[a-zA-Z_][a-zA-Z0-9_$]*)/iu.exec(action.trim());
  if (!match) {
    return false;
  }
  const columnName = normalizeIdentifier(match[1]);
  table.columns = table.columns.filter((column) => column.name !== columnName);
  table.jsonbColumns = table.jsonbColumns.filter((column) => column !== columnName);
  return true;
}

function parseAlterTableRenameColumn(table, action) {
  const match = /^rename\s+column\s+("[^"]+"|[a-zA-Z_][a-zA-Z0-9_$]*)\s+to\s+("[^"]+"|[a-zA-Z_][a-zA-Z0-9_$]*)$/iu.exec(action.trim());
  if (!match) {
    return false;
  }
  const from = normalizeIdentifier(match[1]);
  const to = normalizeIdentifier(match[2]);
  const column = findColumn(table, from);
  if (column) {
    column.name = to;
  }
  table.jsonbColumns = table.jsonbColumns.map((columnName) => (columnName === from ? to : columnName));
  for (const index of table.indexes) {
    index.columns = index.columns.map((columnName) => (columnName === from ? to : columnName));
  }
  return true;
}

function findUnquotedUsingClause(typeDefinition) {
  let quotedIdentifier = false;
  for (let index = 0; index < typeDefinition.length; index += 1) {
    const char = typeDefinition[index];
    if (char === '"') {
      if (quotedIdentifier && typeDefinition[index + 1] === '"') {
        index += 1;
      } else {
        quotedIdentifier = !quotedIdentifier;
      }
      continue;
    }
    if (!quotedIdentifier && /\s/u.test(char) && /^\s+using\b/iu.test(typeDefinition.slice(index))) {
      return index;
    }
  }
  return undefined;
}

function parseAlterTableAlterColumn(table, action) {
  const match = /^alter\s+column\s+("[^"]+"|[a-zA-Z_][a-zA-Z0-9_$]*)\s+([\s\S]+)$/iu.exec(action.trim());
  if (!match) {
    return false;
  }

  const columnName = normalizeIdentifier(match[1]);
  const column = findColumn(table, columnName);
  if (!column) {
    return false;
  }

  const operation = match[2].trim();
  if (/^set\s+not\s+null$/iu.test(operation)) {
    column.nullable = false;
    return true;
  }
  if (/^drop\s+not\s+null$/iu.test(operation)) {
    column.nullable = true;
    return true;
  }
  if (/^set\s+default\b/iu.test(operation)) {
    column.default = true;
    return true;
  }
  if (/^drop\s+default$/iu.test(operation)) {
    column.default = false;
    return true;
  }
  if (/^drop\s+expression$/iu.test(operation)) {
    return true;
  }
  const typeMatch = /^(?:set\s+data\s+)?type\s+([\s\S]+)$/iu.exec(operation);
  if (typeMatch) {
    let typeDefinition = typeMatch[1].trim();
    const usingIndex = findUnquotedUsingClause(typeDefinition);
    if (usingIndex !== undefined) {
      const usingClause = typeDefinition.slice(usingIndex).trim();
      if (!/^using\s+\S[\s\S]*$/iu.test(usingClause)) {
        return false;
      }
      typeDefinition = typeDefinition.slice(0, usingIndex).trim();
    }

    const parsedColumn = parseColumnDefinition(`${columnName} ${typeDefinition}`);
    if (!parsedColumn || parsedColumn.type.length === 0) {
      return false;
    }

    column.type = parsedColumn.type;
    table.jsonbColumns = table.columns
      .filter((candidate) => /\bjsonb\b/iu.test(candidate.type))
      .map((candidate) => candidate.name);
    return true;
  }

  return false;
}

function parseAlterTableRenameTable({ context, table, tableName, action }) {
  const match = /^rename\s+to\s+("[^"]+"|[a-zA-Z_][a-zA-Z0-9_$]*)$/iu.exec(action.trim());
  if (!match) {
    return false;
  }
  const renamedTableName = normalizeIdentifier(match[1]);
  context.tables.delete(tableName);
  table.name = renamedTableName;
  context.tables.set(renamedTableName, table);
  return true;
}

function parseAlterTable(statement, context) {
  const cleaned = stripSqlComments(statement).trim();
  const match = /^alter\s+table\s+(?:if\s+exists\s+)?(?:only\s+)?([a-zA-Z0-9_."$]+)\s+([\s\S]+)$/iu.exec(cleaned);
  if (!match) {
    context.parseErrors.push(createParseError(context, 'unsupported-alter-table', statement));
    return;
  }

  const tableName = normalizeIdentifier(match[1]);
  if (context.temporaryTables.has(tableName)) {
    return;
  }
  const table = getOrCreateTable(context.tables, tableName, context.relativePath);

  for (const action of splitTopLevel(match[2], ',')) {
    const trimmed = action.trim();
    let parsed = false;
    if (/^add\s+column\b/iu.test(trimmed)) {
      parsed = parseAlterTableAddColumn(table, trimmed);
    } else if (/^add\s+constraint\b/iu.test(trimmed)) {
      parsed = parseAlterTableAddConstraint(table, trimmed);
    } else if (/^add\s+(?:primary\s+key|unique|foreign\s+key|check)\b/iu.test(trimmed)) {
      parsed = parseTableConstraint(table, trimmed.replace(/^add\s+/iu, ''));
    } else if (/^drop\s+column\b/iu.test(trimmed)) {
      parsed = parseAlterTableDropColumn(table, trimmed);
    } else if (/^drop\s+constraint\b/iu.test(trimmed)) {
      parsed = parseAlterTableDropConstraint(table, trimmed, normalizeIdentifier);
    } else if (/^alter\s+column\b/iu.test(trimmed)) {
      parsed = parseAlterTableAlterColumn(table, trimmed);
    } else if (/^rename\s+constraint\b/iu.test(trimmed)) {
      parsed = parseAlterTableRenameConstraint(table, trimmed, normalizeIdentifier);
    } else if (/^rename\s+column\b/iu.test(trimmed)) {
      parsed = parseAlterTableRenameColumn(table, trimmed);
    } else if (/^rename\s+to\b/iu.test(trimmed)) {
      parsed = parseAlterTableRenameTable({ context, table, tableName, action: trimmed });
    }

    if (!parsed) {
      context.parseErrors.push(createParseError(context, 'unsupported-alter-table-action', trimmed));
    }
  }
}

function parseDropTable(statement, context) {
  const match = /^drop\s+table\s+(?:if\s+exists\s+)?([a-zA-Z0-9_."$]+)(?:\s+cascade|\s+restrict)?$/iu.exec(stripSqlComments(statement).trim());
  if (match) {
    const tableName = normalizeIdentifier(match[1]);
    context.temporaryTables.delete(tableName);
    context.tables.delete(tableName);
  }
}

function parseDropIndex(statement, context) {
  const match = /^drop\s+index\s+(?:if\s+exists\s+)?("[^"]+"|[a-zA-Z_][a-zA-Z0-9_$]*)/iu.exec(stripSqlComments(statement).trim());
  if (!match) {
    return;
  }
  const indexName = normalizeIdentifier(match[1]);
  for (const table of context.tables.values()) {
    table.indexes = table.indexes.filter((index) => index.name !== indexName);
    table.uniqueConstraints = table.uniqueConstraints.filter((constraint) => constraint.name !== indexName);
  }
}

// Track current trigger attachments while replaying migrations. This projects
// DDL identity and invocation shape; it does not verify PL/pgSQL function bodies.
function parseCreateTrigger(statement, context) {
  const match = /^create\s+(?:or\s+replace\s+)?(constraint\s+)?trigger\s+([a-zA-Z0-9_."$]+)\s+(before|after|instead\s+of)\s+([\s\S]+?)\s+on\s+([a-zA-Z0-9_."$]+)\s+([\s\S]+?)\s+execute\s+(?:function|procedure)\s+([a-zA-Z0-9_."$]+)\s*\(([\s\S]*)\)$/iu.exec(statement);
  const row = match && /^(?:(?:not\s+)?deferrable\s+)?(?:initially\s+(?:deferred|immediate)\s+)?for\s+each\s+(row|statement)(?:\s+when\s*\(([\s\S]*)\))?$/iu.exec(match[6].trim());
  const events = match && match[4].split(/\s+or\s+/iu).map((raw) => {
    const event = /^(insert|delete|truncate|update)(?:\s+of\s+([a-zA-Z0-9_."$,\s]+))?$/iu.exec(raw.trim());
    if (!event || (event[2] && event[1].toLowerCase() !== 'update')) return null;
    return { event: event[1].toLowerCase(), columns: event[2] ? parseColumnList(event[2]) : [] };
  });
  if (!match || !row || events.some((event) => !event)) {
    context.parseErrors.push(createParseError(context, 'unsupported-create-trigger', statement));
    return;
  }
  // Views are outside the table inventory; do not manufacture a managed table.
  const table = context.tables.get(normalizeIdentifier(match[5]));
  if (!table) return;
  const trigger = {
    name: normalizeIdentifier(match[2]), timing: match[3].toLowerCase().replace(/\s+/gu, ' '),
    events, forEach: row[1].toLowerCase(), when: row[2]?.trim() || null,
    constraint: Boolean(match[1]), function: normalizeIdentifier(match[7]),
    arguments: match[8].trim() ? splitTopLevel(match[8], ',').map((value) => value.trim()) : [],
    source: context.relativePath,
  };
  table.triggers = [...(table.triggers || []).filter((item) => item.name !== trigger.name), trigger];
}

function parseDropTrigger(statement, context) {
  const match = /^drop\s+trigger\s+(?:if\s+exists\s+)?([a-zA-Z0-9_."$]+)\s+on\s+([a-zA-Z0-9_."$]+)(?:\s+(?:cascade|restrict))?$/iu.exec(statement);
  if (!match) {
    context.parseErrors.push(createParseError(context, 'unsupported-drop-trigger', statement));
    return;
  }
  const table = context.tables.get(normalizeIdentifier(match[2]));
  if (table) table.triggers = (table.triggers || []).filter((trigger) => trigger.name !== normalizeIdentifier(match[1]));
}

function parseDropTriggerFunction(statement, context) {
  // A trigger function has no declared arguments. Dropping it with CASCADE
  // removes its dependent attachments; a historical CREATE is no longer proof.
  const match = /^drop\s+function\s+(?:if\s+exists\s+)?([a-zA-Z0-9_."$]+)\s*\(\s*\)\s+cascade$/iu.exec(statement);
  if (!match) {
    context.parseErrors.push(createParseError(context, 'unsupported-drop-trigger-function', statement));
    return;
  }
  for (const table of context.tables.values()) {
    table.triggers = (table.triggers || []).filter((trigger) => trigger.function !== normalizeIdentifier(match[1]));
  }
}

function createParseError(context, rule, statement) {
  const message = `Unable to parse schema DDL statement in ${context.relativePath}`;
  return {
    severity: 'error',
    rule,
    file: context.relativePath,
    reason: message,
    message,
    action: actionForRule(rule),
    snippet: statement.trim().replace(/\s+/gu, ' ').slice(0, 240),
  };
}

function parseSchemaStatement(statement, context) {
  const normalized = stripSqlComments(statement).trim();
  if (normalized.length === 0) {
    return;
  }

  if (/^create\s+(?:or\s+replace\s+)?(?:constraint\s+)?trigger\b/iu.test(normalized)) {
    parseCreateTrigger(normalized, context);
    return;
  }
  if (/^drop\s+trigger\b/iu.test(normalized)) {
    parseDropTrigger(normalized, context);
    return;
  }
  if (/^alter\s+trigger\b/iu.test(normalized)) {
    context.parseErrors.push(createParseError(context, 'unsupported-alter-trigger', statement));
    return;
  }
  if (/^drop\s+function\b[\s\S]*\bcascade\s*$/iu.test(normalized)) {
    parseDropTriggerFunction(normalized, context);
    return;
  }
  const temporaryTableMatch = /^create\s+(?:temporary|temp)\s+table\s+(?:if\s+not\s+exists\s+)?([a-zA-Z0-9_."$]+)/iu.exec(normalized);
  if (temporaryTableMatch) {
    context.temporaryTables.add(normalizeIdentifier(temporaryTableMatch[1]));
    return;
  }

  if (/^create\s+table\b/iu.test(normalized)) {
    parseCreateTable(normalized, context);
    return;
  }
  if (/^create\s+(unique\s+)?index\b/iu.test(normalized)) {
    parseCreateIndex(normalized, context);
    return;
  }
  if (/^alter\s+table\b/iu.test(normalized)) {
    parseAlterTable(normalized, context);
    return;
  }
  if (/^drop\s+table\b/iu.test(normalized)) {
    parseDropTable(normalized, context);
    return;
  }
  if (/^drop\s+index\b/iu.test(normalized)) {
    parseDropIndex(normalized, context);
  }
}

module.exports = { splitSqlStatements, splitTopLevel, parseSchemaStatement };
