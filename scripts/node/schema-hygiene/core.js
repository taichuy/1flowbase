const fs = require('node:fs');
const path = require('node:path');

const {
  EXPANSION_SCOPE_COLUMN,
  hasColumn,
  hasScopeColumn,
  hasScopeTimeIndex,
  platformReadinessForTable,
} = require('./readiness.js');

const OUTPUT_ROOT = path.join('tmp', 'test-governance');
const JSON_REPORT_FILE = 'schema-hygiene.json';
const MARKDOWN_REPORT_FILE = 'schema-hygiene.md';
const DEFAULT_MAX_FINDINGS = 400;
const DEFAULT_MIGRATIONS_DIR = path.join(
  'api',
  'crates',
  'storage-durable',
  'postgres',
  'migrations'
);
const DEFAULT_CONFIG_FILE = path.join('scripts', 'node', 'schema-hygiene', 'config.json');
const DEFAULT_REASON_POLICY = {
  minLength: 12,
  minWords: 3,
  forbidden: ['misc', 'legacy', 'todo', 'n/a', 'none'],
};
const BROAD_EXEMPTION_SKIPS = new Set([
  'all',
  'managed_table',
  'dynamic_model_table',
  'registered_system_table',
]);
const BOUNDED_PROJECTION_EXEMPTION_KIND = 'bounded_projection';
const BOUNDED_PROJECTION_EXEMPT_ACTION = 'bounded_projection_exempt';
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

function getRepoRoot() {
  return path.resolve(__dirname, '..', '..', '..');
}

function normalizePath(filePath) {
  return filePath.split(path.sep).join('/');
}

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
  const match = /^create\s+(unique\s+)?index\s+(?:concurrently\s+)?(?:if\s+not\s+exists\s+)?("[^"]+"|[a-zA-Z_][a-zA-Z0-9_$]*)\s+on\s+(?:only\s+)?([a-zA-Z0-9_."$]+)(?:\s+using\s+([a-zA-Z_][a-zA-Z0-9_$]*))?\s*\(([\s\S]+)\)(?:\s+where\s+([\s\S]+))?$/iu.exec(cleaned);
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

  return false;
}

function parseAlterTableDropConstraint(table, action) {
  const match = /^drop\s+constraint\s+(?:if\s+exists\s+)?("[^"]+"|[a-zA-Z_][a-zA-Z0-9_$]*)/iu.exec(action.trim());
  if (!match) {
    return false;
  }
  const constraintName = normalizeIdentifier(match[1]);
  table.uniqueConstraints = table.uniqueConstraints.filter((constraint) => constraint.name !== constraintName);
  table.foreignKeys = table.foreignKeys.filter((constraint) => constraint.name !== constraintName);
  if (table.primaryKey && table.primaryKey.name === constraintName) {
    table.primaryKey = null;
  }
  return true;
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
      parsed = parseAlterTableDropConstraint(table, trimmed);
    } else if (/^alter\s+column\b/iu.test(trimmed)) {
      parsed = parseAlterTableAlterColumn(table, trimmed);
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

function collectMigrationFiles({ repoRoot, migrationsDir = DEFAULT_MIGRATIONS_DIR }) {
  const absoluteDir = path.join(repoRoot, migrationsDir);
  if (!fs.existsSync(absoluteDir)) {
    throw new Error(`PostgreSQL migrations directory not found: ${migrationsDir}`);
  }

  const files = fs.readdirSync(absoluteDir)
    .filter((entry) => entry.endsWith('.sql'))
    .sort((left, right) => left.localeCompare(right))
    .map((entry) => path.join(absoluteDir, entry));

  if (files.length === 0) {
    throw new Error(`No PostgreSQL migration SQL files found under ${migrationsDir}`);
  }

  return files;
}

function collectMigrationVersionFindings({ repoRoot, files }) {
  const byVersion = new Map();

  for (const absolutePath of files) {
    const relativePath = normalizePath(path.relative(repoRoot, absolutePath));
    const fileName = path.basename(absolutePath);
    const reversibleMatch = /^(\d+)_(.+)\.(up|down)\.sql$/u.exec(fileName);
    const match = reversibleMatch || /^(\d+)_(.+)\.sql$/u.exec(fileName);
    if (!match) {
      continue;
    }

    const version = match[1];
    if (!byVersion.has(version)) {
      byVersion.set(version, []);
    }
    byVersion.get(version).push({
      relativePath,
      description: match[2],
      direction: reversibleMatch ? reversibleMatch[3] : null,
    });
  }

  const findings = [];
  for (const [version, entries] of byVersion) {
    if (entries.length < 2) {
      continue;
    }

    const reversiblePair = entries.length === 2
      && entries[0].description === entries[1].description
      && new Set(entries.map((entry) => entry.direction)).size === 2
      && entries.every((entry) => entry.direction === 'up' || entry.direction === 'down');
    if (reversiblePair) {
      continue;
    }

    const relativePaths = entries.map((entry) => entry.relativePath);

    const message = `PostgreSQL migration version ${version} is used by multiple files: ${relativePaths.join(', ')}`;
    findings.push({
      severity: 'error',
      rule: 'postgres-migration-version-duplicate',
      file: relativePaths.join(', '),
      version,
      reason: message,
      message,
      action: actionForRule('postgres-migration-version-duplicate'),
    });
  }

  return findings;
}

function collectSchemaInventory({ repoRoot = getRepoRoot(), migrationsDir = DEFAULT_MIGRATIONS_DIR } = {}) {
  const tables = new Map();
  const temporaryTables = new Set();
  const parseErrors = [];
  const files = collectMigrationFiles({ repoRoot, migrationsDir });
  const migrationErrors = collectMigrationVersionFindings({ repoRoot, files });

  for (const absolutePath of files) {
    if (path.basename(absolutePath).endsWith('.down.sql')) {
      continue;
    }
    const relativePath = normalizePath(path.relative(repoRoot, absolutePath));
    const sql = fs.readFileSync(absolutePath, 'utf8');
    const context = {
      tables,
      temporaryTables,
      parseErrors,
      relativePath,
    };

    for (const statement of splitSqlStatements(sql)) {
      parseSchemaStatement(statement, context);
    }
  }

  if (tables.size === 0) {
    throw new Error(`No PostgreSQL tables discovered under ${migrationsDir}`);
  }

  return {
    source: {
      kind: 'migrations',
      migrationsDir,
      files: files.map((file) => normalizePath(path.relative(repoRoot, file))),
    },
    tables: [...tables.values()].sort((left, right) => left.name.localeCompare(right.name)),
    migrationErrors,
    parseErrors,
  };
}

function loadConfig(repoRoot, configPath = DEFAULT_CONFIG_FILE) {
  const absolutePath = path.isAbsolute(configPath) ? configPath : path.join(repoRoot, configPath);
  if (!fs.existsSync(absolutePath)) {
    return {};
  }

  return JSON.parse(fs.readFileSync(absolutePath, 'utf8'));
}

function normalizeConfig(config = {}) {
  return {
    tableProfiles: config.tableProfiles || {},
    dynamicModelTablePatterns: (config.dynamicModelTablePatterns || []).map((pattern) => new RegExp(pattern, 'u')),
    registeredSystemTables: new Set(config.registeredSystemTables || []),
    registeredSystemTableTemplates: config.registeredSystemTableTemplates || {},
    appendOnlyTables: new Set(config.appendOnlyTables || []),
    systemScopeTables: new Set(config.systemScopeTables || []),
    defaultTableReadiness: config.defaultTableReadiness || {},
    tableReadiness: config.tableReadiness || {},
    needsOwnerReviewTables: config.needsOwnerReviewTables || {},
    reasonPolicy: {
      ...DEFAULT_REASON_POLICY,
      ...(config.reasonPolicy || {}),
      forbidden: (config.reasonPolicy && config.reasonPolicy.forbidden) || DEFAULT_REASON_POLICY.forbidden,
    },
    exemptions: config.exemptions || {},
  };
}

function actionForRule(rule) {
  if (rule.startsWith('exemption-')) {
    return 'Update scripts/node/schema-hygiene/config.json with a specific reason and concrete skip entries.';
  }
  if (rule.startsWith('dynamic-model-')) {
    return 'Update the dynamic model table template to include scope_id, timestamps, and a (scope_id, created_at, id) index.';
  }
  if (rule.startsWith('managed-table-')) {
    return 'Add the missing physical schema property or add a concrete, reasoned exemption for a bounded special table.';
  }
  if (rule.startsWith('postgres-migration-')) {
    return 'Give every PostgreSQL migration SQL file a unique leading version number before running sqlx migrations.';
  }
  if (rule.startsWith('unsupported-')) {
    return 'Extend schema-hygiene SQL parsing for this DDL shape or use live DB introspection for this case.';
  }
  return 'Inspect the schema hygiene rule and fix the table or config entry.';
}

function finding({ rule, table, message, column = null, severity = 'error' }) {
  return {
    severity,
    rule,
    table: table.name,
    column,
    reason: message,
    message,
    action: actionForRule(rule),
  };
}

function isSkipped(exemption, rule, column = null) {
  if (!exemption || !Array.isArray(exemption.skip)) {
    return false;
  }
  return exemption.skip.includes(rule)
    || (column !== null && exemption.skip.includes(column));
}

function validateExemptionReason(exemption, config) {
  const reason = typeof exemption.reason === 'string' ? exemption.reason.trim() : '';
  if (reason.length === 0) {
    return 'missing';
  }

  const policy = config.reasonPolicy;
  const words = reason.split(/\s+/u).filter(Boolean);
  const normalized = reason.toLowerCase();
  const forbidden = policy.forbidden.some((candidate) => {
    const escaped = candidate.replace(/[.*+?^${}()|[\]\\]/gu, '\\$&');
    return new RegExp(`(^|[^a-z0-9])${escaped}([^a-z0-9]|$)`, 'u').test(normalized);
  });
  if (reason.length < policy.minLength || words.length < policy.minWords || forbidden) {
    return 'format';
  }

  return null;
}

function collectExemptionPolicyFindings(table, exemption, config) {
  if (!exemption) {
    return [];
  }

  const findings = [];
  const reasonIssue = validateExemptionReason(exemption, config);
  if (reasonIssue === 'missing') {
    findings.push(finding({
      rule: 'exemption-reason-required',
      table,
      message: 'schema hygiene exemptions must include a reason',
    }));
  } else if (reasonIssue === 'format') {
    findings.push(finding({
      rule: 'exemption-reason-format',
      table,
      message: `schema hygiene exemption reason must be specific: min ${config.reasonPolicy.minLength} chars and ${config.reasonPolicy.minWords} words`,
    }));
  }

  const skips = Array.isArray(exemption.skip) ? exemption.skip : [];
  for (const skip of skips) {
    if (BROAD_EXEMPTION_SKIPS.has(skip)) {
      findings.push(finding({
        rule: 'exemption-skip-too-broad',
        table,
        message: `schema hygiene exemption skip must name a concrete rule or column, not ${skip}`,
      }));
    }
  }

  return findings;
}

function exemptionForReport(exemption) {
  if (!exemption) {
    return null;
  }
  const report = {
    reason: exemption.reason || null,
    skip: exemption.skip || [],
  };
  if (typeof exemption.kind === 'string' && exemption.kind.trim().length > 0) {
    report.kind = exemption.kind.trim();
  }
  return report;
}

function applyPlatformReadinessExemption({ platformReadiness, exemption, tableFindings }) {
  if (!exemption || typeof exemption.kind !== 'string' || exemption.kind.trim().length === 0) {
    return platformReadiness;
  }
  if (tableFindings.some((item) => item.severity === 'error')) {
    return platformReadiness;
  }

  const kind = exemption.kind.trim();
  return {
    ...platformReadiness,
    category: kind,
    recommendedActions: [
      kind === BOUNDED_PROJECTION_EXEMPTION_KIND
        ? BOUNDED_PROJECTION_EXEMPT_ACTION
        : `${kind}_declared`,
    ],
    severity: 'ok',
    reason: exemption.reason.trim(),
  };
}

function profileForTable(table, config) {
  if (config.tableProfiles[table.name]) {
    return config.tableProfiles[table.name];
  }
  if (config.registeredSystemTables.has(table.name)) {
    return 'registered_system_table';
  }
  if (config.dynamicModelTablePatterns.some((pattern) => pattern.test(table.name))) {
    return 'dynamic_model_table';
  }
  return 'managed_table';
}

function evaluateManagedTable(table, config, exemption) {
  const findings = [];
  const appendOnly = config.appendOnlyTables.has(table.name);
  const systemScope = config.systemScopeTables.has(table.name);

  if (!hasColumn(table, 'id') && !isSkipped(exemption, 'managed-table-id', 'id')) {
    findings.push(finding({
      rule: 'managed-table-id',
      table,
      column: 'id',
      message: 'managed_table requires an id column',
    }));
  }

  if (!hasColumn(table, 'created_at') && !isSkipped(exemption, 'managed-table-created-at', 'created_at')) {
    findings.push(finding({
      rule: 'managed-table-created-at',
      table,
      column: 'created_at',
      message: 'managed_table requires created_at',
    }));
  }

  if (!appendOnly && !hasColumn(table, 'updated_at') && !isSkipped(exemption, 'managed-table-updated-at-or-append-only', 'updated_at')) {
    findings.push(finding({
      rule: 'managed-table-updated-at-or-append-only',
      table,
      column: 'updated_at',
      message: 'managed_table requires updated_at unless the table is configured append-only',
    }));
  }

  if (!systemScope && !hasScopeColumn(table) && !isSkipped(exemption, 'managed-table-scope-column')) {
    findings.push(finding({
      rule: 'managed-table-scope-column',
      table,
      column: EXPANSION_SCOPE_COLUMN,
      message: 'managed_table requires scope_id for expansion readiness unless configured as system scope',
    }));
  }

  if (!systemScope && hasScopeColumn(table) && !hasScopeTimeIndex(table) && !isSkipped(exemption, 'managed-table-scope-time-index')) {
    findings.push(finding({
      rule: 'managed-table-scope-time-index',
      table,
      message: 'managed_table requires a (scope_id, created_at, id) index for expansion readiness',
    }));
  }

  if (!systemScope && !hasScopeColumn(table) && !isSkipped(exemption, 'managed-table-scope-time-index')) {
    findings.push(finding({
      rule: 'managed-table-scope-time-index',
      table,
      message: 'managed_table cannot satisfy (scope_id, created_at, id) index without scope_id',
    }));
  }

  return findings;
}

function evaluateDynamicModelTable(table, exemption) {
  const findings = [];
  for (const columnName of ['id', 'created_at', 'updated_at']) {
    if (!hasColumn(table, columnName) && !isSkipped(exemption, `dynamic-model-${columnName}`, columnName)) {
      findings.push(finding({
        rule: `dynamic-model-${columnName}`,
        table,
        column: columnName,
        message: `dynamic_model_table requires ${columnName}`,
      }));
    }
  }

  if (!hasScopeColumn(table) && !isSkipped(exemption, 'dynamic-model-scope-column')) {
    findings.push(finding({
      rule: 'dynamic-model-scope-column',
      table,
      column: EXPANSION_SCOPE_COLUMN,
      message: 'dynamic_model_table requires scope_id',
    }));
  }

  if (!hasScopeTimeIndex(table) && !isSkipped(exemption, 'dynamic-model-scope-time-index')) {
    findings.push(finding({
      rule: 'dynamic-model-scope-time-index',
      table,
      message: 'dynamic_model_table requires a (scope_id, created_at, id) index',
    }));
  }

  return findings;
}

function evaluateRegisteredSystemTableTemplate(table, config, exemption) {
  const template = config.registeredSystemTableTemplates[table.name];
  if (!template) {
    return [
      finding({
        rule: 'registered-system-table-template-missing',
        table,
        message: 'registered_system_table requires a fixed field template declaration',
      }),
    ];
  }

  const findings = [];
  for (const columnName of template.requiredColumns || []) {
    if (!hasColumn(table, columnName) && !isSkipped(exemption, 'registered-system-table-required-column', columnName)) {
      findings.push(finding({
        rule: 'registered-system-table-required-column',
        table,
        column: columnName,
        message: `registered_system_table fixed template requires column ${columnName}`,
      }));
    }
  }
  return findings;
}

function evaluateSchemaHygiene({ inventory, config = {} }) {
  const normalizedConfig = normalizeConfig(config);
  const migrationErrors = inventory.migrationErrors || [];
  const findings = [...migrationErrors, ...inventory.parseErrors];
  const tables = inventory.tables.map((table) => {
    const profile = profileForTable(table, normalizedConfig);
    const exemption = normalizedConfig.exemptions[table.name];
    const tableFindings = [];
    const needsOwnerReviewReason = normalizedConfig.needsOwnerReviewTables[table.name];

    tableFindings.push(...collectExemptionPolicyFindings(table, exemption, normalizedConfig));

    if (needsOwnerReviewReason) {
      tableFindings.push(finding({
        rule: 'managed-table-needs-owner-review',
        table,
        severity: 'warning',
        message: needsOwnerReviewReason,
      }));
    } else if (profile === 'dynamic_model_table') {
      tableFindings.push(...evaluateDynamicModelTable(table, exemption));
    } else {
      if (profile === 'registered_system_table') {
        tableFindings.push(...evaluateRegisteredSystemTableTemplate(table, normalizedConfig, exemption));
      }
      tableFindings.push(...evaluateManagedTable(table, normalizedConfig, exemption));
    }

    findings.push(...tableFindings);

    const platformReadiness = applyPlatformReadinessExemption({
      platformReadiness: platformReadinessForTable({
        table,
        profile,
        config: normalizedConfig,
        tableFindings,
      }),
      exemption,
      tableFindings,
    });

    return {
      ...table,
      profile,
      exemption: exemptionForReport(exemption),
      platformReadiness,
      findings: tableFindings,
    };
  });

  const errors = findings.filter((candidate) => candidate.severity === 'error').length;
  const warnings = findings.filter((candidate) => candidate.severity === 'warning').length;

  return {
    version: 'schema-hygiene/v1',
    generatedAt: new Date().toISOString(),
    source: inventory.source,
    summary: {
      tables: tables.length,
      findings: findings.length,
      errors,
      warnings,
      migrationErrors: migrationErrors.length,
      parseErrors: inventory.parseErrors.length,
      platformReadiness: {
        ok: tables.filter((table) => table.platformReadiness.severity === 'ok').length,
        warnings: tables.filter((table) => table.platformReadiness.severity === 'warning').length,
        errors: tables.filter((table) => table.platformReadiness.severity === 'error').length,
        needsOwnerReview: tables.filter((table) => (
          table.platformReadiness.recommendedActions.includes('needs_owner_review')
        )).length,
        missingScopeId: tables.filter((table) => !table.platformReadiness.fields.scope_id.present).length,
        missingScopeTimeIdIndex: tables.filter((table) => !table.platformReadiness.hasScopeTimeIdIndex).length,
      },
    },
    tables,
    findings,
  };
}

function writeMarkdownReport(report, markdownPath) {
  const lines = [
    '# Schema Hygiene',
    '',
    `- Tables: ${report.summary.tables}`,
    `- Findings: ${report.summary.findings}`,
    `- Errors: ${report.summary.errors}`,
    `- Parse errors: ${report.summary.parseErrors}`,
    '',
    '## Findings',
    '',
  ];

  if (report.findings.length === 0) {
    lines.push('No findings.');
  } else {
    lines.push('| Severity | Rule | Table | Reason | Suggested action |');
    lines.push('| --- | --- | --- | --- | --- |');
    for (const item of report.findings) {
      lines.push(`| ${item.severity} | ${item.rule} | ${item.table || item.file || ''} | ${(item.reason || item.message).replace(/\|/gu, '\\|')} | ${(item.action || '').replace(/\|/gu, '\\|')} |`);
    }
  }

  lines.push('', '## Tables', '', '| Table | Profile | Category | Missing platform fields | Actions | Readiness reason | Columns | JSONB | Indexes |');
  lines.push('| --- | --- | --- | --- | --- | --- | ---: | --- | ---: |');
  for (const table of report.tables) {
    lines.push(
      `| ${table.name} | ${table.profile} | ${table.platformReadiness.category} | `
        + `${table.platformReadiness.missingFields.join(', ') || '-'} | `
        + `${table.platformReadiness.recommendedActions.join(', ')} | `
        + `${(table.platformReadiness.reason || '').replace(/\|/gu, '\\|')} | `
        + `${table.columns.length} | ${table.jsonbColumns.join(', ')} | ${table.indexes.length} |`
    );
  }

  fs.writeFileSync(markdownPath, `${lines.join('\n')}\n`, 'utf8');
}

function writeReports({ repoRoot, report, maxFindings = DEFAULT_MAX_FINDINGS }) {
  const outputDir = path.join(repoRoot, OUTPUT_ROOT);
  fs.mkdirSync(outputDir, { recursive: true });

  const reportForDisk = {
    ...report,
    findings: report.findings.slice(0, maxFindings),
    truncated: report.findings.length > maxFindings,
  };

  const jsonPath = path.join(outputDir, JSON_REPORT_FILE);
  const markdownPath = path.join(outputDir, MARKDOWN_REPORT_FILE);
  fs.writeFileSync(jsonPath, `${JSON.stringify(reportForDisk, null, 2)}\n`, 'utf8');
  writeMarkdownReport(reportForDisk, markdownPath);

  return { jsonPath, markdownPath, report: reportForDisk };
}

function parseSchemaHygieneCliArgs(argv) {
  const options = {
    help: false,
    maxFindings: DEFAULT_MAX_FINDINGS,
    migrationsDir: DEFAULT_MIGRATIONS_DIR,
    configPath: DEFAULT_CONFIG_FILE,
  };

  for (let index = 0; index < argv.length; index += 1) {
    const arg = argv[index];
    if (arg === '-h' || arg === '--help') {
      options.help = true;
      continue;
    }
    if (arg === '--max-findings') {
      options.maxFindings = Number.parseInt(argv[index + 1], 10);
      index += 1;
      continue;
    }
    if (arg === '--migrations-dir') {
      options.migrationsDir = argv[index + 1];
      index += 1;
      continue;
    }
    if (arg === '--config') {
      options.configPath = argv[index + 1];
      index += 1;
      continue;
    }
    throw new Error(`Unknown schema-hygiene option: ${arg}`);
  }

  if (!Number.isInteger(options.maxFindings) || options.maxFindings < 1) {
    throw new Error('--max-findings must be a positive integer');
  }

  return options;
}

function usage(writeStdout = (text) => process.stdout.write(text)) {
  writeStdout(
    'Usage: node scripts/node/tooling.js schema-hygiene [--max-findings <n>] [--migrations-dir <path>] [--config <path>]\n'
      + 'Scans PostgreSQL migrations for schema inventory and expansion-friendly hygiene rules.\n'
  );
}

async function main(argv = [], deps = {}) {
  const options = parseSchemaHygieneCliArgs(argv);
  const writeStdout = deps.writeStdout || ((text) => process.stdout.write(text));
  const writeStderr = deps.writeStderr || ((text) => process.stderr.write(text));

  if (options.help) {
    usage(writeStdout);
    return 0;
  }

  const repoRoot = deps.repoRoot || getRepoRoot();
  const inventory = (deps.collectSchemaInventoryImpl || collectSchemaInventory)({
    repoRoot,
    migrationsDir: options.migrationsDir,
  });
  const config = deps.config || loadConfig(repoRoot, options.configPath);
  const report = evaluateSchemaHygiene({ inventory, config });
  const { jsonPath, markdownPath } = writeReports({
    repoRoot,
    report,
    maxFindings: options.maxFindings,
  });

  writeStdout(
    `[1flowbase-schema-hygiene] ${report.summary.findings} findings `
      + `(${report.summary.errors} errors, ${report.summary.warnings} warnings). `
      + `Reports: ${normalizePath(path.relative(repoRoot, jsonPath))}, `
      + `${normalizePath(path.relative(repoRoot, markdownPath))}\n`
  );

  for (const item of report.findings.filter((candidate) => candidate.severity === 'error')) {
    writeStderr(
      `[schema-hygiene:${item.rule}] ${item.table || item.file || 'schema'} ${item.message}\n`
    );
  }

  return report.summary.errors > 0 ? 1 : 0;
}

module.exports = {
  collectSchemaInventory,
  evaluateSchemaHygiene,
  loadConfig,
  main,
  parseSchemaHygieneCliArgs,
  splitSqlStatements,
  splitTopLevel,
  writeReports,
};
