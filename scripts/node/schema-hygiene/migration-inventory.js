const fs = require('node:fs');
const path = require('node:path');
const { splitSqlStatements, parseSchemaStatement } = require("./ddl-parser.js");
const { actionForRule } = require("./finding-actions.js");

const DEFAULT_MIGRATIONS_DIR = path.join(
  'api',
  'crates',
  'storage',
  'durable',
  'postgres',
  'migrations'
);

function getRepoRoot() {
  return path.resolve(__dirname, '..', '..', '..');
}

function normalizePath(filePath) {
  return filePath.split(path.sep).join('/');
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

module.exports = { getRepoRoot, normalizePath, collectSchemaInventory, DEFAULT_MIGRATIONS_DIR };
