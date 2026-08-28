const fs = require('node:fs');
const path = require('node:path');

const { collectSchemaInventory } = require('../schema-hygiene/core.js');
const { getRepoRoot } = require('../testing/warning-capture.js');

const OUTPUT_ROOT = path.join('tmp', 'test-governance');
const JSON_REPORT_FILE = 'growth-table-report.json';
const MARKDOWN_REPORT_FILE = 'growth-table-report.md';
const DEFAULT_CONFIG_FILE = path.join('scripts', 'node', 'growth-table-report', 'config.json');
const DEFAULT_MIGRATIONS_DIR = path.join(
  'api',
  'crates',
  'storage',
  'durable',
  'postgres',
  'migrations'
);
const DEFAULT_SOURCE_SEARCH_DIRS = [
  path.join('api', 'crates', 'storage', 'durable', 'postgres', 'src'),
  path.join('api', 'crates', 'control-plane', 'src'),
  path.join('api', 'crates', 'orchestration-runtime', 'src'),
  path.join('api', 'crates', 'runtime-core', 'src'),
];
const DEFAULT_MAX_EVIDENCE = 6;
const VALID_PRIORITIES = new Set(['must_fix', 'later']);
const VALID_EXPANSION_EXEMPTION_KINDS = new Set(['bounded_projection']);
const EXPANSION_SCOPE_COLUMN = 'scope_id';
const EXPANSION_TIME_COLUMN = 'created_at';
const EXPANSION_TIE_BREAKER_COLUMN = 'id';

function normalizePath(filePath) {
  return filePath.split(path.sep).join('/');
}

function toRepoRelative(repoRoot, filePath) {
  return normalizePath(path.relative(repoRoot, filePath));
}

function loadConfig(repoRoot = getRepoRoot(), configPath = DEFAULT_CONFIG_FILE) {
  const absolutePath = path.isAbsolute(configPath) ? configPath : path.join(repoRoot, configPath);
  return JSON.parse(fs.readFileSync(absolutePath, 'utf8'));
}

function hasColumn(table, columnName) {
  return table.columns.some((column) => column.name === columnName);
}

function matchingColumns(table, columnNames) {
  return columnNames.filter((columnName) => hasColumn(table, columnName));
}

function indexCoversColumns(index, expectedColumns) {
  return expectedColumns.every((columnName, indexPosition) => index.columns[indexPosition] === columnName);
}

function hasExpansionReadinessIndex(table) {
  return table.indexes.some((index) => indexCoversColumns(index, [
    EXPANSION_SCOPE_COLUMN,
    EXPANSION_TIME_COLUMN,
    EXPANSION_TIE_BREAKER_COLUMN,
  ]));
}

function normalizeStringList(value) {
  if (!Array.isArray(value)) {
    return [];
  }
  return value
    .map((item) => (typeof item === 'string' ? item.trim() : ''))
    .filter(Boolean);
}

function normalizeExpansionReadinessExemption(spec = {}) {
  const exemption = spec.expansionReadinessExemption;
  if (!exemption) {
    return null;
  }

  const reason = typeof exemption.reason === 'string' ? exemption.reason.trim() : '';
  if (reason.length === 0) {
    return null;
  }

  if (!VALID_EXPANSION_EXEMPTION_KINDS.has(exemption.kind)) {
    return null;
  }

  return {
    kind: exemption.kind,
    reason,
    boundedBy: normalizeStringList(exemption.boundedBy),
  };
}

function normalizePriority(priority) {
  return VALID_PRIORITIES.has(priority) ? priority : 'later';
}

function finding({ rule, priority = 'later', column = null, constraint = null, index = null, message }) {
  return {
    rule,
    priority: normalizePriority(priority),
    column,
    constraint,
    index,
    message,
  };
}

function collectRustFiles(rootDir) {
  if (!fs.existsSync(rootDir)) {
    return [];
  }

  const entries = fs.readdirSync(rootDir, { withFileTypes: true });
  return entries.flatMap((entry) => {
    const absolutePath = path.join(rootDir, entry.name);
    if (entry.isDirectory()) {
      if (entry.name === 'target' || entry.name === '_tests') {
        return [];
      }
      return collectRustFiles(absolutePath);
    }
    if (!entry.isFile() || !entry.name.endsWith('.rs')) {
      return [];
    }
    return [absolutePath];
  });
}

function nearestFunctionName(lines, lineIndex) {
  for (let cursor = lineIndex; cursor >= 0; cursor -= 1) {
    const match = /\b(?:pub\s+)?(?:async\s+)?fn\s+([a-zA-Z0-9_]+)\s*\(/u.exec(lines[cursor]);
    if (match) {
      return match[1];
    }
  }
  return null;
}

function classifyEntrypoint({ tableName, context }) {
  const escapedTable = tableName.replace(/[.*+?^${}()|[\]\\]/gu, '\\$&');
  const writePattern = new RegExp(
    `\\b(insert\\s+into|update|delete\\s+from)\\s+${escapedTable}\\b`,
    'iu'
  );
  const readPattern = new RegExp(
    `\\b(from|join)\\s+${escapedTable}\\b`,
    'iu'
  );

  if (writePattern.test(context)) {
    return 'write';
  }
  if (readPattern.test(context) || /\bselect\b/iu.test(context)) {
    return 'read';
  }
  return 'unknown';
}

function collectQueryEntrypoints({
  repoRoot,
  tableName,
  sourceSearchDirs = DEFAULT_SOURCE_SEARCH_DIRS,
  maxEvidence = DEFAULT_MAX_EVIDENCE,
}) {
  const matches = [];
  for (const searchDir of sourceSearchDirs) {
    const absoluteDir = path.isAbsolute(searchDir) ? searchDir : path.join(repoRoot, searchDir);
    for (const filePath of collectRustFiles(absoluteDir)) {
      const lines = fs.readFileSync(filePath, 'utf8').split(/\r?\n/u);
      for (const [lineIndex, line] of lines.entries()) {
        if (!line.includes(tableName)) {
          continue;
        }
        const context = lines
          .slice(Math.max(0, lineIndex - 3), Math.min(lines.length, lineIndex + 4))
          .join('\n');
        matches.push({
          file: toRepoRelative(repoRoot, filePath),
          line: lineIndex + 1,
          functionName: nearestFunctionName(lines, lineIndex),
          operation: classifyEntrypoint({ tableName, context }),
          snippet: line.trim().slice(0, 180),
        });
        if (matches.length >= maxEvidence) {
          return matches;
        }
      }
    }
  }
  return matches;
}

function applyManualEntrypoints(queryEntrypoints, spec) {
  return [
    ...queryEntrypoints,
    ...(spec.manualEntrypoints || []).map((entry) => ({
      file: entry.file || '',
      line: entry.line || null,
      functionName: entry.functionName || null,
      operation: entry.operation || 'unknown',
      snippet: entry.snippet || entry.note || '',
      manual: true,
    })),
  ];
}

function entrypointCoverage(allEntrypoints) {
  const readEntrypoints = allEntrypoints.filter((entry) => entry.operation === 'read');
  const writeEntrypoints = allEntrypoints.filter((entry) => entry.operation === 'write');
  const unknownEntrypoints = allEntrypoints.filter((entry) => entry.operation === 'unknown');
  return {
    readEntrypoints,
    writeEntrypoints,
    unknownEntrypoints,
    readPath: readEntrypoints.length > 0 ? 'evidence_found' : 'not_found',
    writePath: writeEntrypoints.length > 0 ? 'evidence_found' : 'not_found',
  };
}

function constraintDescriptors(table) {
  const descriptors = [];
  if (table.primaryKey) {
    descriptors.push({
      kind: 'primary_key',
      name: table.primaryKey.name || `${table.name}_pkey`,
      columns: table.primaryKey.columns,
    });
  }
  descriptors.push(...table.uniqueConstraints.map((constraint) => ({
    kind: 'unique',
    name: constraint.name || `${table.name}_unique_${constraint.columns.join('_')}`,
    columns: constraint.columns,
  })));
  return descriptors;
}

function evaluateRecommendedIndexes(table, spec) {
  return (spec.recommendedIndexes || []).map((recommendation) => {
    const present = table.indexes.some((index) => indexCoversColumns(index, recommendation.columns));
    return {
      columns: recommendation.columns,
      scenario: recommendation.scenario,
      priority: normalizePriority(recommendation.priority),
      present,
    };
  });
}

function collectExpansionReadiness(table, spec = {}) {
  const hasId = hasColumn(table, EXPANSION_TIE_BREAKER_COLUMN);
  const hasScopeId = hasColumn(table, EXPANSION_SCOPE_COLUMN);
  const hasWorkspaceId = hasColumn(table, 'workspace_id');
  const hasCreatedAt = hasColumn(table, EXPANSION_TIME_COLUMN);
  const hasScopeTimeIdIndex = hasExpansionReadinessIndex(table);
  const exemption = normalizeExpansionReadinessExemption(spec);
  const recommendedActions = [];

  if (!hasId) {
    recommendedActions.push('add_id');
  }
  if (!hasScopeId) {
    recommendedActions.push('add_scope_id');
  }
  if (!hasCreatedAt) {
    recommendedActions.push('add_created_at');
  }
  if (!hasScopeTimeIdIndex) {
    recommendedActions.push('add_scope_time_index');
  }

  return {
    status: exemption
      ? 'bounded'
      : hasId && hasScopeId && hasCreatedAt && hasScopeTimeIdIndex ? 'ready' : 'not_ready',
    hasId,
    hasScopeId,
    hasWorkspaceId,
    workspaceIdOnly: hasWorkspaceId && !hasScopeId,
    hasCreatedAt,
    hasScopeTimeIdIndex,
    timeKey: EXPANSION_TIME_COLUMN,
    tieBreaker: EXPANSION_TIE_BREAKER_COLUMN,
    recommendedActions: exemption
      ? ['bounded_projection_exempt']
      : recommendedActions.length > 0 ? recommendedActions : ['no_action'],
    exemption,
  };
}

function defaultBackfill(backfill) {
  if (!backfill) {
    return null;
  }
  return {
    followUpMigrationTask: 'required',
    downtimeRisk: 'requires batch backfill and validation outside the report-only task',
    ...backfill,
  };
}

function defaultDowntimeRisk({ spec, missingRoutingColumns, missingTimeColumns, recommendedIndexes }) {
  if (spec.downtimeRisk) {
    return spec.downtimeRisk;
  }
  const requiresBackfill = Boolean(spec.backfill);
  const missingMustFixIndex = recommendedIndexes.some((index) => !index.present && index.priority === 'must_fix');

  if (requiresBackfill) {
    return {
      level: 'high',
      reason: 'Requires column backfill or semantic migration; execute as a separate L3 migration task with batched updates and validation.',
    };
  }
  if (missingRoutingColumns.length > 0 || missingTimeColumns.length > 0 || missingMustFixIndex) {
    return {
      level: 'medium',
      reason: 'Schema/index change may need CONCURRENTLY or lock-budget planning before implementation.',
    };
  }
  return {
    level: 'low',
    reason: 'Report-only assessment found no required backfill or must-fix index replacement.',
  };
}

function defaultConstraintReplacementRisk({ spec, table, uniqueFindings }) {
  if (spec.constraintReplacementRisk) {
    return spec.constraintReplacementRisk;
  }
  if (uniqueFindings.length > 0) {
    return {
      level: 'high',
      reason: 'Unique or primary-key route-key gap requires separate constraint replacement design; keep old constraint until new unique index is validated.',
    };
  }
  if ((table.uniqueConstraints || []).length > 0 || table.primaryKey) {
    return {
      level: 'none',
      reason: 'Existing uniqueness includes an accepted routing key or is currently left unchanged.',
    };
  }
  return {
    level: 'none',
    reason: 'No unique constraint replacement needed for the current report scope.',
  };
}

function evaluateGrowthTable(table, spec, queryEntrypoints) {
  const findings = [];
  const requiredRoutingColumns = spec.requiredRoutingColumns || [];
  const requiredTimeColumns = spec.requiredTimeColumns || [];
  const uniqueRouteKeys = spec.uniqueRouteKeys || [];
  const recommendedIndexes = evaluateRecommendedIndexes(table, spec);
  const expansionReadiness = collectExpansionReadiness(table, spec);

  for (const columnName of requiredRoutingColumns) {
    if (!hasColumn(table, columnName)) {
      findings.push(finding({
        rule: 'missing-routing-column',
        priority: 'must_fix',
        column: columnName,
        message: `${table.name} is missing routing column ${columnName}`,
      }));
    }
  }

  for (const columnName of requiredTimeColumns) {
    if (!hasColumn(table, columnName)) {
      findings.push(finding({
        rule: 'missing-time-column',
        priority: normalizePriority(spec.missingTimePriority || 'must_fix'),
        column: columnName,
        message: `${table.name} is missing time/cursor column ${columnName}`,
      }));
    }
  }

  if (uniqueRouteKeys.length > 0) {
    for (const constraint of constraintDescriptors(table)) {
      if (!uniqueRouteKeys.some((columnName) => constraint.columns.includes(columnName))) {
        findings.push(finding({
          rule: 'unique-constraint-routing-key',
          priority: normalizePriority(spec.uniqueRoutingPriority || 'must_fix'),
          constraint: constraint.name,
          message: `${table.name} ${constraint.kind} (${constraint.columns.join(', ')}) lacks route key ${uniqueRouteKeys.join(' or ')}`,
        }));
      }
    }
  }

  for (const recommendation of recommendedIndexes) {
    if (!recommendation.present) {
      findings.push(finding({
        rule: 'missing-recommended-index',
        priority: recommendation.priority,
        index: recommendation.columns.join(', '),
        message: `${table.name} lacks index for ${recommendation.scenario}`,
      }));
    }
  }

  if (!expansionReadiness.exemption) {
    if (!expansionReadiness.hasId) {
      findings.push(finding({
        rule: 'missing-expansion-id',
        priority: 'must_fix',
        column: EXPANSION_TIE_BREAKER_COLUMN,
        message: `${table.name} is missing id for expansion cursor tie-breakers`,
      }));
    }

    if (!expansionReadiness.hasScopeId) {
      findings.push(finding({
        rule: 'missing-expansion-scope-column',
        priority: 'must_fix',
        column: EXPANSION_SCOPE_COLUMN,
        message: `${table.name} is missing scope_id; workspace_id can only be a backfill source`,
      }));
    }

    if (!expansionReadiness.hasCreatedAt) {
      findings.push(finding({
        rule: 'missing-expansion-created-at',
        priority: 'must_fix',
        column: EXPANSION_TIME_COLUMN,
        message: `${table.name} is missing created_at as the fixed expansion time key`,
      }));
    }

    if (!expansionReadiness.hasScopeTimeIdIndex) {
      findings.push(finding({
        rule: 'missing-expansion-scope-time-id-index',
        priority: 'must_fix',
        index: `${EXPANSION_SCOPE_COLUMN}, ${EXPANSION_TIME_COLUMN}, ${EXPANSION_TIE_BREAKER_COLUMN}`,
        message: `${table.name} lacks (scope_id, created_at, id) expansion readiness index`,
      }));
    }
  }

  if (table.jsonbColumns.length > 0 && spec.rawJsonbPolicy !== 'ignore') {
    findings.push(finding({
      rule: 'raw-jsonb-review',
      priority: normalizePriority(spec.rawJsonbPriority || 'later'),
      message: `${table.name} keeps JSONB payload columns; list/detail read boundaries need explicit review`,
    }));
  }

  const status = findings.some((item) => item.priority === 'must_fix')
    ? 'must_fix'
    : findings.length > 0 ? 'later' : 'ok';
  const missingRoutingColumns = requiredRoutingColumns.filter((columnName) => !hasColumn(table, columnName));
  const missingTimeColumns = requiredTimeColumns.filter((columnName) => !hasColumn(table, columnName));
  const allEntrypoints = applyManualEntrypoints(queryEntrypoints, spec);
  const coverage = entrypointCoverage(allEntrypoints);
  const uniqueFindings = findings.filter((item) => item.rule === 'unique-constraint-routing-key');
  const backfill = defaultBackfill(spec.backfill);

  return {
    name: table.name,
    status,
    growthType: spec.growthType || 'unspecified',
    tableSource: table.source,
    routingColumns: matchingColumns(table, requiredRoutingColumns),
    missingRoutingColumns,
    ownerColumns: matchingColumns(table, spec.ownerColumns || []),
    timeColumns: matchingColumns(table, requiredTimeColumns),
    missingTimeColumns,
    jsonbColumns: table.jsonbColumns,
    primaryKey: table.primaryKey,
    uniqueConstraints: table.uniqueConstraints,
    indexes: table.indexes.map((index) => ({
      name: index.name,
      columns: index.columns,
      predicate: index.predicate,
      source: index.source,
    })),
    recommendedIndexes,
    expansionReadiness,
    queryEntrypoints: allEntrypoints,
    readEntrypoints: coverage.readEntrypoints,
    writeEntrypoints: coverage.writeEntrypoints,
    unknownEntrypoints: coverage.unknownEntrypoints,
    readPath: coverage.readPath,
    writePath: coverage.writePath,
    findings,
    recommendation: spec.recommendation || '',
    backfill,
    downtimeRisk: defaultDowntimeRisk({ spec, missingRoutingColumns, missingTimeColumns, recommendedIndexes }),
    constraintReplacementRisk: defaultConstraintReplacementRisk({ spec, table, uniqueFindings }),
  };
}

function missingTableReport(spec) {
  return {
    name: spec.name,
    status: 'must_fix',
    growthType: spec.growthType || 'unspecified',
    tableSource: null,
    routingColumns: [],
    missingRoutingColumns: spec.requiredRoutingColumns || [],
    ownerColumns: [],
    timeColumns: [],
    missingTimeColumns: spec.requiredTimeColumns || [],
    jsonbColumns: [],
    primaryKey: null,
    uniqueConstraints: [],
    indexes: [],
    recommendedIndexes: [],
    expansionReadiness: {
      status: 'not_ready',
      hasId: false,
      hasScopeId: false,
      hasWorkspaceId: false,
      workspaceIdOnly: false,
      hasCreatedAt: false,
      hasScopeTimeIdIndex: false,
      timeKey: EXPANSION_TIME_COLUMN,
      tieBreaker: EXPANSION_TIE_BREAKER_COLUMN,
      recommendedActions: ['needs_owner_review'],
    },
    queryEntrypoints: [],
    readEntrypoints: [],
    writeEntrypoints: [],
    unknownEntrypoints: [],
    readPath: 'not_found',
    writePath: 'not_found',
    findings: [
      finding({
        rule: 'table-missing',
        priority: 'must_fix',
        message: `${spec.name} is configured as a high-growth table but was not found in migrations`,
      }),
    ],
    recommendation: spec.recommendation || '',
    backfill: defaultBackfill(spec.backfill),
    downtimeRisk: spec.downtimeRisk || {
      level: 'high',
      reason: 'Configured high-growth table is missing from migrations; resolve inventory before planning migration.',
    },
    constraintReplacementRisk: spec.constraintReplacementRisk || {
      level: 'unknown',
      reason: 'Table is missing, so uniqueness cannot be evaluated.',
    },
  };
}

function buildSummary(tables) {
  return {
    tables: tables.length,
    mustFix: tables.filter((table) => table.status === 'must_fix').length,
    later: tables.filter((table) => table.status === 'later').length,
    ok: tables.filter((table) => table.status === 'ok').length,
    findings: tables.reduce((total, table) => total + table.findings.length, 0),
  };
}

function collectGrowthTableReport({
  repoRoot = getRepoRoot(),
  inventory,
  config,
  migrationsDir = DEFAULT_MIGRATIONS_DIR,
  maxEvidence = DEFAULT_MAX_EVIDENCE,
} = {}) {
  const effectiveConfig = config || loadConfig(repoRoot);
  const effectiveInventory = inventory || collectSchemaInventory({ repoRoot, migrationsDir });
  const tableByName = new Map(effectiveInventory.tables.map((table) => [table.name, table]));
  const sourceSearchDirs = effectiveConfig.sourceSearchDirs || DEFAULT_SOURCE_SEARCH_DIRS;
  const tables = (effectiveConfig.tables || []).map((spec) => {
    const table = tableByName.get(spec.name);
    if (!table) {
      return missingTableReport(spec);
    }
    return evaluateGrowthTable(
      table,
      spec,
      collectQueryEntrypoints({
        repoRoot,
        tableName: spec.name,
        sourceSearchDirs,
        maxEvidence,
      })
    );
  });
  const summary = buildSummary(tables);

  return {
    status: summary.mustFix > 0 || summary.later > 0 ? 'warning' : 'passed',
    exitCode: 0,
    source: effectiveInventory.source,
    summary,
    tables,
  };
}

function formatList(items) {
  return items.length === 0 ? '-' : items.map((item) => `\`${item}\``).join(', ');
}

function escapeMarkdownTableCell(value) {
  return String(value).replace(/\|/gu, '\\|');
}

function formatReadinessExemption(exemption) {
  if (!exemption) {
    return '-';
  }
  const boundedBy = exemption.boundedBy.length > 0
    ? `; bounded by ${exemption.boundedBy.join(', ')}`
    : '';
  return escapeMarkdownTableCell(`${exemption.kind}: ${exemption.reason}${boundedBy}`);
}

function formatGrowthTableMarkdown(report) {
  const tableRows = report.tables.map((table) => (
    `| \`${table.name}\` | ${table.status} | ${table.growthType} | ${table.expansionReadiness.status} | `
      + `${formatList(table.routingColumns)} | ${formatList(table.missingRoutingColumns)} | `
      + `${formatList(table.timeColumns)} | ${formatReadinessExemption(table.expansionReadiness.exemption)} | `
      + `${table.findings.length} |`
  ));
  const findingRows = report.tables.flatMap((table) => (
    table.findings.map((item) => (
      `| \`${table.name}\` | ${item.priority} | \`${item.rule}\` | ${item.message} |`
    ))
  ));
  const indexRows = report.tables.flatMap((table) => (
    table.recommendedIndexes.map((item) => (
      `| \`${table.name}\` | ${item.present ? 'present' : item.priority} | `
        + `${formatList(item.columns)} | ${item.scenario} |`
    ))
  ));
  const backfillRows = report.tables
    .filter((table) => table.backfill)
    .map((table) => (
      `| \`${table.name}\` | ${table.backfill.source} | ${table.backfill.batchBoundary} | `
        + `${table.backfill.failureRecovery} | ${table.backfill.followUpMigrationTask} |`
    ));
  const downtimeRows = report.tables.map((table) => (
    `| \`${table.name}\` | ${table.downtimeRisk.level} | ${table.downtimeRisk.reason} | `
      + `${table.constraintReplacementRisk.level} | ${table.constraintReplacementRisk.reason} |`
  ));
  const writeEvidenceRows = report.tables.flatMap((table) => (
    table.writeEntrypoints.map((entry) => (
      `| \`${table.name}\` | \`${entry.file}:${entry.line}\` | `
        + `\`${entry.functionName || 'unknown'}\` | \`${entry.snippet.replace(/`/gu, "'")}\` |`
    ))
  ));
  const readEvidenceRows = report.tables.flatMap((table) => (
    table.readEntrypoints.map((entry) => (
      `| \`${table.name}\` | \`${entry.file}:${entry.line}\` | `
        + `\`${entry.functionName || 'unknown'}\` | \`${entry.snippet.replace(/`/gu, "'")}\` |`
    ))
  ));
  const missingEvidenceRows = report.tables
    .filter((table) => table.readPath === 'not_found' || table.writePath === 'not_found')
    .map((table) => (
      `| \`${table.name}\` | ${table.readPath} | ${table.writePath} |`
    ));

  return [
    '# Growth Table Routing Report',
    '',
    '## Summary',
    '',
    `- Status: ${report.status}`,
    `- Tables: ${report.summary.tables}`,
    `- Must fix: ${report.summary.mustFix}`,
    `- Later: ${report.summary.later}`,
    `- OK: ${report.summary.ok}`,
    `- Findings: ${report.summary.findings}`,
    '',
    '## Tables',
    '',
    '| Table | Status | Growth type | Expansion readiness | Routing columns | Missing routing columns | Time columns | Readiness exemption | Findings |',
    '| --- | --- | --- | --- | --- | --- | --- | --- | ---: |',
    ...tableRows,
    '',
    '## Findings',
    '',
    findingRows.length === 0 ? 'No high-growth table findings.' : null,
    findingRows.length > 0 ? '| Table | Priority | Rule | Message |' : null,
    findingRows.length > 0 ? '| --- | --- | --- | --- |' : null,
    ...findingRows,
    '',
    '## Recommended Indexes',
    '',
    '| Table | Status | Columns | Scenario |',
    '| --- | --- | --- | --- |',
    ...indexRows,
    '',
    '## Backfill Risk',
    '',
    backfillRows.length === 0 ? 'No backfill plans were declared.' : null,
    backfillRows.length > 0 ? '| Table | Data source | Batch boundary | Failure recovery | Follow-up migration task |' : null,
    backfillRows.length > 0 ? '| --- | --- | --- | --- | --- |' : null,
    ...backfillRows,
    '',
    '## Downtime And Constraint Risk',
    '',
    '| Table | Downtime risk | Downtime reason | Constraint risk | Constraint reason |',
    '| --- | --- | --- | --- | --- |',
    ...downtimeRows,
    '',
    '## Write Path Evidence',
    '',
    writeEvidenceRows.length === 0 ? 'No write path evidence was found in configured source directories.' : null,
    writeEvidenceRows.length > 0 ? '| Table | Source | Function | Snippet |' : null,
    writeEvidenceRows.length > 0 ? '| --- | --- | --- | --- |' : null,
    ...writeEvidenceRows,
    '',
    '## Read Path Evidence',
    '',
    readEvidenceRows.length === 0 ? 'No read path evidence was found in configured source directories.' : null,
    readEvidenceRows.length > 0 ? '| Table | Source | Function | Snippet |' : null,
    readEvidenceRows.length > 0 ? '| --- | --- | --- | --- |' : null,
    ...readEvidenceRows,
    '',
    '## Missing Path Evidence',
    '',
    missingEvidenceRows.length === 0 ? 'Every table has read and write path evidence.' : null,
    missingEvidenceRows.length > 0 ? '| Table | Read path | Write path |' : null,
    missingEvidenceRows.length > 0 ? '| --- | --- | --- |' : null,
    ...missingEvidenceRows,
    '',
  ].filter((line) => line !== null).join('\n');
}

function writeGrowthTableReports({
  repoRoot = getRepoRoot(),
  report,
  config,
  migrationsDir = DEFAULT_MIGRATIONS_DIR,
  outputRoot = OUTPUT_ROOT,
  maxEvidence = DEFAULT_MAX_EVIDENCE,
} = {}) {
  const effectiveReport = report || collectGrowthTableReport({
    repoRoot,
    config,
    migrationsDir,
    maxEvidence,
  });
  const outputDir = path.join(repoRoot, outputRoot);
  fs.mkdirSync(outputDir, { recursive: true });
  const reportPath = path.join(outputDir, JSON_REPORT_FILE);
  const markdownPath = path.join(outputDir, MARKDOWN_REPORT_FILE);
  fs.writeFileSync(reportPath, `${JSON.stringify(effectiveReport, null, 2)}\n`, 'utf8');
  fs.writeFileSync(markdownPath, formatGrowthTableMarkdown(effectiveReport), 'utf8');
  return {
    report: effectiveReport,
    reportPath,
    markdownPath,
  };
}

function parseCliArgs(argv) {
  const options = {
    help: false,
    configPath: DEFAULT_CONFIG_FILE,
    migrationsDir: DEFAULT_MIGRATIONS_DIR,
    maxEvidence: DEFAULT_MAX_EVIDENCE,
  };

  for (let index = 0; index < argv.length; index += 1) {
    const arg = argv[index];
    if (arg === '-h' || arg === '--help') {
      options.help = true;
      continue;
    }
    if (arg === '--config') {
      options.configPath = argv[index + 1];
      index += 1;
      continue;
    }
    if (arg === '--migrations-dir') {
      options.migrationsDir = argv[index + 1];
      index += 1;
      continue;
    }
    if (arg === '--max-evidence') {
      options.maxEvidence = Number.parseInt(argv[index + 1], 10);
      index += 1;
      continue;
    }
    throw new Error(`Unknown growth-table-report option: ${arg}`);
  }

  if (!Number.isInteger(options.maxEvidence) || options.maxEvidence < 1) {
    throw new Error('--max-evidence must be a positive integer');
  }

  return options;
}

function usage(writeStdout = (text) => process.stdout.write(text)) {
  writeStdout(
    'Usage: node scripts/node/tooling.js growth-table-report [--config <path>] [--migrations-dir <path>] [--max-evidence <n>]\n'
      + 'Writes high-growth table routing, unique constraint, index, and backfill risk reports.\n'
  );
}

async function main(argv = [], deps = {}) {
  const options = parseCliArgs(argv);
  const writeStdout = deps.writeStdout || ((text) => process.stdout.write(text));

  if (options.help) {
    usage(writeStdout);
    return 0;
  }

  const repoRoot = deps.repoRoot || getRepoRoot();
  const config = deps.config || loadConfig(repoRoot, options.configPath);
  const result = writeGrowthTableReports({
    repoRoot,
    config,
    migrationsDir: options.migrationsDir,
    maxEvidence: options.maxEvidence,
  });

  writeStdout(
    `[1flowbase-growth-table-report] ${result.report.status} `
      + `(must_fix ${result.report.summary.mustFix}, later ${result.report.summary.later}, ok ${result.report.summary.ok}). `
      + `Reports: ${toRepoRelative(repoRoot, result.reportPath)}, ${toRepoRelative(repoRoot, result.markdownPath)}\n`
  );

  return 0;
}

module.exports = {
  collectGrowthTableReport,
  collectQueryEntrypoints,
  evaluateGrowthTable,
  formatGrowthTableMarkdown,
  loadConfig,
  main,
  parseCliArgs,
  writeGrowthTableReports,
};
