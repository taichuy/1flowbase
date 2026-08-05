const fs = require('node:fs');
const path = require('node:path');

const { collectSchemaInventory } = require('../schema-hygiene/core.js');
const { getRepoRoot } = require('../testing/warning-capture.js');

const OUTPUT_ROOT = path.join('tmp', 'test-governance');
const JSON_REPORT_FILE = 'raw-jsonb-report.json';
const MARKDOWN_REPORT_FILE = 'raw-jsonb-report.md';
const DEFAULT_CONFIG_FILE = path.join('scripts', 'node', 'raw-jsonb-report', 'config.json');
const DEFAULT_MIGRATIONS_DIR = path.join(
  'api',
  'crates',
  'storage-durable',
  'postgres',
  'migrations'
);
const DEFAULT_SOURCE_SEARCH_DIRS = [
  path.join('api', 'apps', 'api-server', 'src'),
  path.join('api', 'crates', 'storage-durable', 'postgres', 'src'),
  path.join('api', 'crates', 'control-plane', 'src'),
  path.join('api', 'crates', 'orchestration-runtime', 'src'),
];
const DEFAULT_MAX_EVIDENCE = 8;
const SQL_CONTEXT_LINE_RADIUS = 24;
const DEFAULT_CONTRACT = {
  summary:
    'Small, bounded fields derived from raw payload or stable metadata. List and overview APIs should prefer these fields.',
  preview:
    'Truncated or artifact-backed display value with explicit size/ref metadata. UI snippets must use preview instead of raw.',
  raw:
    'Complete JSONB truth retained in PostgreSQL. It may be read only through primary-key, run-scope, or detail entrypoints.',
};
const RAW_PAYLOAD_KINDS = new Set(['raw']);

function normalizePath(filePath) {
  return filePath.split(path.sep).join('/');
}

function toRepoRelative(repoRoot, filePath) {
  return normalizePath(path.relative(repoRoot, filePath));
}

function escapeRegExp(input) {
  return input.replace(/[.*+?^${}()|[\]\\]/gu, '\\$&');
}

function loadConfig(repoRoot = getRepoRoot(), configPath = DEFAULT_CONFIG_FILE) {
  const absolutePath = path.isAbsolute(configPath) ? configPath : path.join(repoRoot, configPath);
  return JSON.parse(fs.readFileSync(absolutePath, 'utf8'));
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

function queryOperation(context) {
  if (/\b(insert\s+into|update|delete\s+from)\b/iu.test(context)) {
    return 'write';
  }
  if (/\bselect\b/iu.test(context)) {
    return 'read';
  }
  return 'unknown';
}

function columnSourceForLine({ line, table, column }) {
  const normalized = line.toLowerCase();
  const escapedColumn = escapeRegExp(column);
  const qualifiedColumn = new RegExp(`\\b${escapeRegExp(table)}\\s*\\.\\s*${escapedColumn}\\b`, 'iu');
  if (qualifiedColumn.test(normalized)) {
    return 'raw_column';
  }

  const aliasPattern = new RegExp(`\\bas\\s+${escapedColumn}\\b`, 'iu');
  if (aliasPattern.test(normalized)) {
    return 'constant_projection';
  }

  const bareColumn = new RegExp(`(^|[^a-zA-Z0-9_\\.])${escapedColumn}\\b`, 'u');
  if (bareColumn.test(normalized)) {
    return 'raw_column';
  }

  return null;
}

function tableAliasesFromContext({ context, table }) {
  const aliases = new Set([table]);
  const pattern = new RegExp(
    `\\b(?:from|join)\\s+${escapeRegExp(table)}\\s+(?:as\\s+)?([a-zA-Z_][a-zA-Z0-9_]*)\\b`,
    'giu'
  );
  let match = pattern.exec(context);
  while (match) {
    const alias = match[1].toLowerCase();
    if (!['where', 'on', 'using', 'order', 'group', 'limit'].includes(alias)) {
      aliases.add(alias);
    }
    match = pattern.exec(context);
  }
  return aliases;
}

function classifyColumnSource({ line, table, column, context }) {
  const aliases = tableAliasesFromContext({ context, table });
  const escapedColumn = escapeRegExp(column);
  const source = columnSourceForLine({ line, table, column });
  if (source === 'raw_column') {
    return 'raw_column';
  }
  if (source === 'constant_projection') {
    return 'constant_projection';
  }
  const normalized = line.toLowerCase();
  for (const alias of aliases) {
    const aliasColumn = new RegExp(`\\b${escapeRegExp(alias)}\\s*\\.\\s*${escapedColumn}\\b`, 'iu');
    if (aliasColumn.test(normalized)) {
      return 'raw_column';
    }
  }
  return 'not_referenced';
}

function readBoundaryForContext({ functionName, context }) {
  const name = (functionName || '').toLowerCase();
  const normalized = context.toLowerCase();
  const hasFlowRunScope = /\bwhere\b[\s\S]*\bflow_run_id\s*=\s*\$/u.test(normalized);
  const hasNodeRunScope = /\bwhere\b[\s\S]*\bnode_run_id\s*=\s*\$/u.test(normalized);
  const hasApplicationAndRunId =
    /\bwhere\b[\s\S]*\bapplication_id\s*=\s*\$/u.test(normalized)
    && /\b(?:id|flow_run_id)\s*=\s*\$/u.test(normalized);
  const hasTraceNodeId = /\btrace_node_id\s*=\s*\$/u.test(normalized);
  const hasPrimaryKeyBatch = /\bwhere\b[\s\S]*\bid\s*=\s*any\s*\(\s*\$/u.test(normalized);
  const isPageOrList =
    name.startsWith('list_')
    || name.includes('_page')
    || name.includes('_children')
    || name.includes('_roots');

  if (name.includes('stitched_trace')) {
    return 'detail';
  }
  if (hasApplicationAndRunId || hasTraceNodeId || hasPrimaryKeyBatch || /\b(detail|content|fetch|get_latest)\b/u.test(name)) {
    return isPageOrList && (hasFlowRunScope || hasNodeRunScope) ? 'run_scope' : 'detail';
  }
  if (hasFlowRunScope || hasNodeRunScope) {
    return 'run_scope';
  }
  if (isPageOrList) {
    return 'list';
  }
  return 'read';
}

function collectFieldEntrypoints({
  repoRoot,
  table,
  column,
  sourceSearchDirs = DEFAULT_SOURCE_SEARCH_DIRS,
  maxEvidence = DEFAULT_MAX_EVIDENCE,
}) {
  const candidates = [];
  const seen = new Set();
  const tablePattern = new RegExp(`\\b${escapeRegExp(table)}\\b`, 'u');
  const columnPattern = new RegExp(`\\b${escapeRegExp(column)}\\b`, 'u');

  for (const searchDir of sourceSearchDirs) {
    const absoluteDir = path.isAbsolute(searchDir) ? searchDir : path.join(repoRoot, searchDir);
    for (const filePath of collectRustFiles(absoluteDir)) {
      const lines = fs.readFileSync(filePath, 'utf8').split(/\r?\n/u);
      for (const [lineIndex, line] of lines.entries()) {
        if (!tablePattern.test(line) && !columnPattern.test(line)) {
          continue;
        }

        const contextLines = lines.slice(
          Math.max(0, lineIndex - SQL_CONTEXT_LINE_RADIUS),
          Math.min(lines.length, lineIndex + SQL_CONTEXT_LINE_RADIUS + 1)
        );
        const context = contextLines.join('\n');
        if (!tablePattern.test(context) || !columnPattern.test(context)) {
          continue;
        }

        const functionName = nearestFunctionName(lines, lineIndex);
        const operation = queryOperation(context);
        const columnSource = classifyColumnSource({ line, table, column, context });
        const key = [
          toRepoRelative(repoRoot, filePath),
          functionName || 'unknown',
          operation,
          columnSource,
          readBoundaryForContext({ functionName, context }),
        ].join(':');
        if (seen.has(key)) {
          continue;
        }
        seen.add(key);

        candidates.push({
          file: toRepoRelative(repoRoot, filePath),
          line: lineIndex + 1,
          functionName,
          operation,
          readBoundary: operation === 'read'
            ? readBoundaryForContext({ functionName, context })
            : 'not_read',
          columnSource,
          snippet: line.trim().slice(0, 180),
        });
      }
    }
  }

  const priority = (entry) => {
    if (entry.operation === 'read' && entry.columnSource === 'raw_column') {
      return 0;
    }
    if (entry.operation === 'read') {
      return 1;
    }
    if (entry.operation === 'write') {
      return 2;
    }
    return 3;
  };
  const sqlEvidenceRank = (entry) => (
    /\b(sqlx::query|select|from|join|returning|insert\s+into|update|delete\s+from)\b/iu
      .test(entry.snippet)
      ? 0
      : 1
  );
  const readerFunctionRank = (entry) => (
    /^(list|get|fetch)_/u.test(entry.functionName || '') ? 0 : 1
  );
  const columnSourceRank = (entry) => {
    if (entry.columnSource === 'raw_column') {
      return 0;
    }
    if (entry.columnSource === 'constant_projection') {
      return 1;
    }
    return 2;
  };

  return candidates
    .sort((left, right) => (
      priority(left) - priority(right)
        || readerFunctionRank(left) - readerFunctionRank(right)
        || columnSourceRank(left) - columnSourceRank(right)
        || sqlEvidenceRank(left) - sqlEvidenceRank(right)
        || left.file.localeCompare(right.file)
        || left.line - right.line
    ))
    .slice(0, maxEvidence);
}

function applyManualEntrypoints(entrypoints, spec) {
  return [
    ...entrypoints,
    ...(spec.manualEntrypoints || []).map((entry) => ({
      file: entry.file || '',
      line: entry.line || null,
      functionName: entry.functionName || null,
      operation: entry.operation || 'read',
      readBoundary: entry.readBoundary || 'read',
      columnSource: entry.columnSource || 'raw_column',
      snippet: entry.snippet || entry.note || '',
      manual: true,
    })),
  ];
}

function specKey(spec) {
  return `${spec.table}.${spec.column}`;
}

function inventoryFields(inventory) {
  return inventory.tables.flatMap((table) => (
    table.jsonbColumns.map((column) => ({
      table: table.name,
      column,
      tableSource: table.source,
    }))
  ));
}

function configuredSpec(specs, table, column) {
  return specs.find((spec) => spec.table === table && spec.column === column) || null;
}

function missingConfiguredFields(inventory, specs) {
  const available = new Set(inventoryFields(inventory).map((field) => `${field.table}.${field.column}`));
  return specs
    .filter((spec) => !available.has(specKey(spec)))
    .map((spec) => ({
      table: spec.table,
      column: spec.column,
      tableSource: null,
      configured: true,
      payloadKind: spec.payloadKind || 'unclassified',
      purpose: spec.purpose || '',
      isRaw: RAW_PAYLOAD_KINDS.has(spec.payloadKind),
      readContract: spec.readContract || '',
      listPolicy: spec.listPolicy || '',
      summaryOrPreview: spec.summaryOrPreview || '',
      hasSummaryOrPreview: Boolean(spec.summaryOrPreview),
      protectedBy: spec.protectedBy || [],
      readEntrypoints: [],
      writeEntrypoints: [],
      unknownEntrypoints: [],
      appearsInListInterface: false,
      rawListReadRisk: false,
      findings: [{
        rule: 'configured-jsonb-field-missing',
        severity: 'warning',
        message: `${spec.table}.${spec.column} is configured for raw JSONB boundary reporting but was not found in migrations`,
      }],
      status: 'warning',
    }));
}

function fieldFinding({ rule, message }) {
  return {
    rule,
    severity: 'warning',
    message,
  };
}

function evaluateField({ repoRoot, sourceField, spec, sourceSearchDirs, maxEvidence }) {
  const effectiveSpec = spec || {};
  const configured = Boolean(spec);
  const payloadKind = effectiveSpec.payloadKind || 'unclassified';
  const isRaw = RAW_PAYLOAD_KINDS.has(payloadKind);
  const entrypoints = applyManualEntrypoints(
    collectFieldEntrypoints({
      repoRoot,
      table: sourceField.table,
      column: sourceField.column,
      sourceSearchDirs,
      maxEvidence,
    }),
    effectiveSpec
  );
  const readEntrypoints = entrypoints.filter((entry) => entry.operation === 'read');
  const rawReadEntrypoints = readEntrypoints.filter((entry) => entry.columnSource === 'raw_column');
  const rawListEntrypoints = rawReadEntrypoints.filter((entry) => entry.readBoundary === 'list');
  const listEntrypoints = readEntrypoints.filter(
    (entry) => entry.readBoundary === 'list' && entry.columnSource === 'raw_column'
  );
  const writeEntrypoints = entrypoints.filter((entry) => entry.operation === 'write');
  const unknownEntrypoints = entrypoints.filter((entry) => entry.operation === 'unknown');
  const hasSummaryOrPreview = Boolean(effectiveSpec.summaryOrPreview);
  const protectedBy = effectiveSpec.protectedBy || [];
  const readContract = effectiveSpec.readContract || '';
  const listPolicy = effectiveSpec.listPolicy || '';
  const findings = [];

  if (isRaw && readContract.length === 0) {
    findings.push(fieldFinding({
      rule: 'raw-jsonb-contract-missing',
      message: `${sourceField.table}.${sourceField.column} is raw JSONB but has no read contract`,
    }));
  }
  if (isRaw && rawListEntrypoints.length > 0 && listPolicy !== 'allow_list_raw') {
    findings.push(fieldFinding({
      rule: 'raw-jsonb-list-read',
      message: `${sourceField.table}.${sourceField.column} is read as raw JSONB from list entrypoints`,
    }));
  }
  if (isRaw && protectedBy.length === 0) {
    findings.push(fieldFinding({
      rule: 'raw-jsonb-protection-missing',
      message: `${sourceField.table}.${sourceField.column} has no primary-key, run-scope, or detail protection declared`,
    }));
  }
  if (isRaw && listPolicy === 'forbidden_raw' && !hasSummaryOrPreview) {
    findings.push(fieldFinding({
      rule: 'raw-jsonb-summary-preview-missing',
      message: `${sourceField.table}.${sourceField.column} forbids list raw reads but has no summary/preview source`,
    }));
  }

  return {
    table: sourceField.table,
    column: sourceField.column,
    tableSource: sourceField.tableSource,
    configured,
    payloadKind,
    purpose: effectiveSpec.purpose || 'unclassified JSONB field outside the raw payload boundary config',
    isRaw,
    readContract,
    listPolicy,
    summaryOrPreview: effectiveSpec.summaryOrPreview || '',
    hasSummaryOrPreview,
    protectedBy,
    readEntrypoints,
    writeEntrypoints,
    unknownEntrypoints,
    appearsInListInterface: listEntrypoints.length > 0,
    rawListReadRisk: isRaw && rawListEntrypoints.length > 0,
    findings,
    status: findings.length > 0 ? 'warning' : 'ok',
  };
}

function buildSummary(fields) {
  return {
    jsonbFields: fields.length,
    configuredFields: fields.filter((field) => field.configured).length,
    rawFields: fields.filter((field) => field.isRaw).length,
    summaryFields: fields.filter((field) => field.payloadKind === 'summary').length,
    previewFields: fields.filter((field) => field.payloadKind === 'preview').length,
    listRawRisks: fields.filter((field) => field.rawListReadRisk).length,
    findings: fields.reduce((total, field) => total + field.findings.length, 0),
  };
}

function collectRawJsonbReport({
  repoRoot = getRepoRoot(),
  inventory,
  config,
  migrationsDir = DEFAULT_MIGRATIONS_DIR,
  maxEvidence = DEFAULT_MAX_EVIDENCE,
} = {}) {
  const effectiveConfig = config || loadConfig(repoRoot);
  const effectiveInventory = inventory || collectSchemaInventory({ repoRoot, migrationsDir });
  const specs = effectiveConfig.fields || [];
  const sourceSearchDirs = effectiveConfig.sourceSearchDirs || DEFAULT_SOURCE_SEARCH_DIRS;
  const fields = [
    ...inventoryFields(effectiveInventory).map((field) => (
      evaluateField({
        repoRoot,
        sourceField: field,
        spec: configuredSpec(specs, field.table, field.column),
        sourceSearchDirs,
        maxEvidence,
      })
    )),
    ...missingConfiguredFields(effectiveInventory, specs),
  ].sort((left, right) => (
    `${left.table}.${left.column}`.localeCompare(`${right.table}.${right.column}`)
  ));
  const summary = buildSummary(fields);

  return {
    version: 'raw-jsonb-report/v1',
    status: summary.findings > 0 ? 'warning' : 'passed',
    exitCode: 0,
    source: effectiveInventory.source,
    contract: {
      ...DEFAULT_CONTRACT,
      ...(effectiveConfig.contract || {}),
    },
    summary,
    fields,
  };
}

function formatList(items) {
  return items.length === 0 ? '-' : items.map((item) => `\`${item}\``).join(', ');
}

function formatRawJsonbMarkdown(report) {
  const fieldRows = report.fields
    .filter((field) => field.configured || field.isRaw)
    .map((field) => (
      `| \`${field.table}.${field.column}\` | ${field.status} | ${field.payloadKind} | `
        + `${field.appearsInListInterface ? 'yes' : 'no'} | `
        + `${field.hasSummaryOrPreview ? field.summaryOrPreview : '-'} | `
        + `${formatList(field.protectedBy)} | ${field.readContract || '-'} |`
    ));
  const findingRows = report.fields.flatMap((field) => (
    field.findings.map((finding) => (
      `| \`${field.table}.${field.column}\` | \`${finding.rule}\` | ${finding.message} |`
    ))
  ));
  const readRows = report.fields.flatMap((field) => (
    field.readEntrypoints.map((entry) => (
      `| \`${field.table}.${field.column}\` | ${entry.readBoundary} | ${entry.columnSource} | `
        + `\`${entry.file}:${entry.line}\` | \`${entry.functionName || 'unknown'}\` | `
        + `\`${entry.snippet.replace(/`/gu, "'")}\` |`
    ))
  ));

  return [
    '# Raw JSONB Boundary Report',
    '',
    '## Summary',
    '',
    `- Status: ${report.status}`,
    `- JSONB fields: ${report.summary.jsonbFields}`,
    `- Configured fields: ${report.summary.configuredFields}`,
    `- Raw fields: ${report.summary.rawFields}`,
    `- Summary fields: ${report.summary.summaryFields}`,
    `- Preview fields: ${report.summary.previewFields}`,
    `- List raw risks: ${report.summary.listRawRisks}`,
    `- Findings: ${report.summary.findings}`,
    '',
    '## Contract',
    '',
    '| Kind | Rule |',
    '| --- | --- |',
    `| summary | ${report.contract.summary} |`,
    `| preview | ${report.contract.preview} |`,
    `| raw | ${report.contract.raw} |`,
    '',
    '## Configured Fields',
    '',
    fieldRows.length === 0 ? 'No configured raw JSONB fields.' : null,
    fieldRows.length > 0 ? '| Field | Status | Kind | Raw in list | Summary/preview | Protected by | Read contract |' : null,
    fieldRows.length > 0 ? '| --- | --- | --- | --- | --- | --- | --- |' : null,
    ...fieldRows,
    '',
    '## Findings',
    '',
    findingRows.length === 0 ? 'No raw JSONB boundary findings.' : null,
    findingRows.length > 0 ? '| Field | Rule | Message |' : null,
    findingRows.length > 0 ? '| --- | --- | --- |' : null,
    ...findingRows,
    '',
    '## Read Evidence',
    '',
    readRows.length === 0 ? 'No read evidence was found in configured source directories.' : null,
    readRows.length > 0 ? '| Field | Boundary | Column source | Source | Function | Snippet |' : null,
    readRows.length > 0 ? '| --- | --- | --- | --- | --- | --- |' : null,
    ...readRows,
    '',
  ].filter((line) => line !== null).join('\n');
}

function writeRawJsonbReports({
  repoRoot = getRepoRoot(),
  report,
  config,
  migrationsDir = DEFAULT_MIGRATIONS_DIR,
  outputRoot = OUTPUT_ROOT,
  maxEvidence = DEFAULT_MAX_EVIDENCE,
} = {}) {
  const effectiveReport = report || collectRawJsonbReport({
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
  fs.writeFileSync(markdownPath, formatRawJsonbMarkdown(effectiveReport), 'utf8');
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
    throw new Error(`Unknown raw-jsonb-report option: ${arg}`);
  }

  if (!Number.isInteger(options.maxEvidence) || options.maxEvidence < 1) {
    throw new Error('--max-evidence must be a positive integer');
  }

  return options;
}

function usage(writeStdout = (text) => process.stdout.write(text)) {
  writeStdout(
    'Usage: node scripts/node/tooling.js raw-jsonb-report [--config <path>] [--migrations-dir <path>] [--max-evidence <n>]\n'
      + 'Writes raw JSONB summary, preview, raw-read boundary reports.\n'
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
  const result = writeRawJsonbReports({
    repoRoot,
    config,
    migrationsDir: options.migrationsDir,
    maxEvidence: options.maxEvidence,
  });

  writeStdout(
    `[1flowbase-raw-jsonb-report] ${result.report.status} `
      + `(raw ${result.report.summary.rawFields}, list_raw_risks ${result.report.summary.listRawRisks}, findings ${result.report.summary.findings}). `
      + `Reports: ${toRepoRelative(repoRoot, result.reportPath)}, ${toRepoRelative(repoRoot, result.markdownPath)}\n`
  );

  return 0;
}

module.exports = {
  collectFieldEntrypoints,
  collectRawJsonbReport,
  formatRawJsonbMarkdown,
  loadConfig,
  main,
  parseCliArgs,
  writeRawJsonbReports,
};
