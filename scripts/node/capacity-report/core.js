const fs = require('node:fs');
const path = require('node:path');
const { spawnSync } = require('node:child_process');

const {
  collectSchemaInventory,
  evaluateSchemaHygiene,
  loadConfig: loadSchemaHygieneConfig,
} = require('../schema-hygiene/core.js');
const {
  collectGrowthTableReport,
  loadConfig: loadGrowthTableConfig,
} = require('../growth-table-report/core.js');
const {
  collectRawJsonbReport,
  loadConfig: loadRawJsonbConfig,
} = require('../raw-jsonb-report/core.js');
const {
  collectLogQueryContractReport,
  loadConfig: loadLogQueryContractConfig,
} = require('../log-query-contract-report/core.js');
const { getRepoRoot } = require('../testing/warning-capture.js');

const OUTPUT_ROOT = path.join('tmp', 'test-governance');
const JSON_REPORT_FILE = 'capacity-report.json';
const MARKDOWN_REPORT_FILE = 'capacity-report.md';
const DEFAULT_CONFIG_FILE = path.join('scripts', 'node', 'capacity-report', 'config.json');
const DEFAULT_MIGRATIONS_DIR = path.join(
  'api',
  'crates',
  'storage',
  'durable',
  'postgres',
  'migrations'
);
const DEFAULT_POSTGRES_SCHEMA = 'public';
const DEFAULT_THRESHOLDS = {
  partitionCandidateRowEstimate: 10_000_000,
  partitionCandidateTotalSizeBytes: 10 * 1024 * 1024 * 1024,
  indexToTableSizeRatio: 4,
};
const METADATA_FIELDS = [
  {
    field: 'table_name',
    owner: 'schema hygiene',
    sourceOfTruth: 'scan',
    persisted: false,
    userEditable: false,
    historicalImpact: 'identifies the table row in the report',
    evidenceSource: 'migration inventory',
  },
  {
    field: 'table_profile',
    owner: 'schema hygiene',
    sourceOfTruth: 'scan',
    persisted: false,
    userEditable: false,
    historicalImpact: 'affects which hygiene rules apply',
    evidenceSource: 'schema-hygiene table profile',
  },
  {
    field: 'exemption_reason',
    owner: 'schema hygiene config',
    sourceOfTruth: 'manual_reason',
    persisted: false,
    userEditable: true,
    historicalImpact: 'may suppress a bounded hygiene finding until review',
    evidenceSource: 'schema-hygiene exemption config',
  },
  {
    field: 'growth_risk',
    owner: 'growth table report',
    sourceOfTruth: 'scan',
    persisted: false,
    userEditable: false,
    historicalImpact: 'marks routing, uniqueness, index, and backfill work before scale planning',
    evidenceSource: 'growth-table-report',
  },
  {
    field: 'jsonb_risk',
    owner: 'raw JSONB report',
    sourceOfTruth: 'scan',
    persisted: false,
    userEditable: false,
    historicalImpact: 'marks raw payload list-read risks before widening list APIs',
    evidenceSource: 'raw-jsonb-report',
  },
  {
    field: 'retention_archive_state',
    owner: 'capacity report config',
    sourceOfTruth: 'manual_reason',
    persisted: false,
    userEditable: true,
    historicalImpact: 'records whether retention/archive policy has been declared for future capacity work',
    evidenceSource: 'capacity-report config',
  },
  {
    field: 'total_size_bytes',
    owner: 'PostgreSQL inspection',
    sourceOfTruth: 'live_postgres',
    persisted: false,
    userEditable: false,
    historicalImpact: 'point-in-time table plus index size',
    evidenceSource: 'pg_total_relation_size',
  },
  {
    field: 'table_size_bytes',
    owner: 'PostgreSQL inspection',
    sourceOfTruth: 'live_postgres',
    persisted: false,
    userEditable: false,
    historicalImpact: 'point-in-time heap/table size',
    evidenceSource: 'pg_relation_size',
  },
  {
    field: 'index_size_bytes',
    owner: 'PostgreSQL inspection',
    sourceOfTruth: 'live_postgres',
    persisted: false,
    userEditable: false,
    historicalImpact: 'point-in-time derived index size',
    evidenceSource: 'total size minus table size',
  },
  {
    field: 'row_estimate',
    owner: 'PostgreSQL inspection',
    sourceOfTruth: 'live_postgres',
    persisted: false,
    userEditable: false,
    historicalImpact: 'planner estimate used only as a planning signal',
    evidenceSource: 'pg_class.reltuples',
  },
  {
    field: 'collected_at',
    owner: 'PostgreSQL inspection',
    sourceOfTruth: 'live_postgres',
    persisted: false,
    userEditable: false,
    historicalImpact: 'timestamp for interpreting point-in-time metrics',
    evidenceSource: 'database clock',
  },
];

function normalizePath(filePath) {
  return filePath.split(path.sep).join('/');
}

function toRepoRelative(repoRoot, filePath) {
  return normalizePath(path.relative(repoRoot, filePath));
}

function resolveRepoPath(repoRoot, filePath) {
  return path.isAbsolute(filePath) ? filePath : path.join(repoRoot, filePath);
}

function loadConfig(repoRoot = getRepoRoot(), configPath = DEFAULT_CONFIG_FILE) {
  const absolutePath = resolveRepoPath(repoRoot, configPath);
  if (!fs.existsSync(absolutePath)) {
    return {};
  }
  return JSON.parse(fs.readFileSync(absolutePath, 'utf8'));
}

function normalizeConfig(config = {}) {
  return {
    thresholds: {
      ...DEFAULT_THRESHOLDS,
      ...(config.thresholds || {}),
    },
    retentionArchiveStates: config.retentionArchiveStates || {},
  };
}

function toFiniteNumber(value) {
  if (typeof value === 'number' && Number.isFinite(value)) {
    return value;
  }
  if (typeof value === 'string' && value.trim().length > 0) {
    const parsed = Number(value);
    return Number.isFinite(parsed) ? parsed : null;
  }
  return null;
}

function fieldValue(row, snakeName, camelName) {
  return row[snakeName] ?? row[camelName] ?? null;
}

function normalizeMetric(row, index = 0) {
  const schemaName = fieldValue(row, 'schema_name', 'schemaName') || DEFAULT_POSTGRES_SCHEMA;
  const tableName = fieldValue(row, 'table_name', 'tableName');
  const totalSizeBytes = toFiniteNumber(fieldValue(row, 'total_size_bytes', 'totalSizeBytes'));
  const tableSizeBytes = toFiniteNumber(fieldValue(row, 'table_size_bytes', 'tableSizeBytes'));
  const indexSizeBytes = toFiniteNumber(fieldValue(row, 'index_size_bytes', 'indexSizeBytes'));
  const rowEstimate = toFiniteNumber(fieldValue(row, 'row_estimate', 'rowEstimate'));
  const collectedAt = fieldValue(row, 'collected_at', 'collectedAt');
  const errors = [];

  if (!tableName || typeof tableName !== 'string') {
    errors.push({
      kind: 'invalid_metric',
      message: `capacity metric at index ${index} is missing table_name`,
    });
  }
  for (const [field, value] of [
    ['total_size_bytes', totalSizeBytes],
    ['table_size_bytes', tableSizeBytes],
    ['index_size_bytes', indexSizeBytes],
  ]) {
    if (value === null) {
      errors.push({
        kind: 'statistics_unavailable',
        tableName: tableName || null,
        message: `capacity metric for ${tableName || `index ${index}`} is missing ${field}`,
      });
    }
  }
  if (rowEstimate === null) {
    errors.push({
      kind: 'statistics_unavailable',
      tableName: tableName || null,
      message: `capacity metric for ${tableName || `index ${index}`} is missing row_estimate`,
    });
  }

  return {
    metric: tableName
      ? {
          schemaName,
          tableName,
          totalSizeBytes,
          tableSizeBytes,
          indexSizeBytes,
          rowEstimate,
          collectedAt: typeof collectedAt === 'string' && collectedAt.length > 0 ? collectedAt : null,
        }
      : null,
    errors,
  };
}

function normalizeMetrics(rows = []) {
  const metrics = [];
  const errors = [];

  rows.forEach((row, index) => {
    const normalized = normalizeMetric(row, index);
    if (normalized.metric) {
      metrics.push(normalized.metric);
    }
    errors.push(...normalized.errors);
  });

  return { metrics, errors };
}

function failedInspection({ source, schema, errors }) {
  return {
    version: 'postgres-capacity-inspection/v1',
    status: 'failed',
    source,
    schema,
    metrics: [],
    errors,
    exitCode: 1,
  };
}

function classifyPostgresInspectionError(message) {
  if (/permission denied/iu.test(message)) {
    return 'permission_denied';
  }
  if (/could not connect|connection refused|connection failure|timeout|no route|password authentication failed|database .* does not exist/iu.test(message)) {
    return 'connection_failure';
  }
  if (/statistics|reltuples|pg_stat|analyze/iu.test(message)) {
    return 'statistics_unavailable';
  }
  return 'postgres_inspection_failed';
}

function parseInspectionInput({ repoRoot, inspectionInputPath, schema = DEFAULT_POSTGRES_SCHEMA }) {
  const absolutePath = resolveRepoPath(repoRoot, inspectionInputPath);
  if (!fs.existsSync(absolutePath)) {
    return failedInspection({
      source: 'inspection_input',
      schema,
      errors: [
        {
          kind: 'inspection_input_missing',
          message: `capacity inspection input does not exist: ${normalizePath(inspectionInputPath)}`,
        },
      ],
    });
  }

  let payload;
  try {
    payload = JSON.parse(fs.readFileSync(absolutePath, 'utf8'));
  } catch (error) {
    return failedInspection({
      source: 'inspection_input',
      schema,
      errors: [
        {
          kind: 'inspection_input_invalid',
          message: `capacity inspection input is not valid JSON: ${error.message}`,
        },
      ],
    });
  }

  const rows = Array.isArray(payload) ? payload : payload.metrics || [];
  const { metrics, errors } = normalizeMetrics(rows);

  return {
    version: 'postgres-capacity-inspection/v1',
    status: 'loaded',
    source: 'inspection_input',
    schema,
    inputPath: toRepoRelative(repoRoot, absolutePath),
    metrics,
    errors,
    exitCode: 0,
  };
}

function validateSchemaName(schema) {
  if (!/^[a-zA-Z_][a-zA-Z0-9_]*$/u.test(schema)) {
    throw new Error('--schema must be a simple PostgreSQL identifier');
  }
}

function buildPostgresCapacitySql(schema = DEFAULT_POSTGRES_SCHEMA) {
  validateSchemaName(schema);
  const escapedSchema = schema.replace(/'/gu, "''");
  return `
select coalesce(json_agg(row_to_json(capacity_rows)), '[]'::json)
from (
  select
    n.nspname as schema_name,
    c.relname as table_name,
    pg_total_relation_size(c.oid)::bigint as total_size_bytes,
    pg_relation_size(c.oid)::bigint as table_size_bytes,
    (pg_total_relation_size(c.oid) - pg_relation_size(c.oid))::bigint as index_size_bytes,
    greatest(c.reltuples, 0)::bigint as row_estimate,
    to_char(now() at time zone 'utc', 'YYYY-MM-DD"T"HH24:MI:SS.MS"Z"') as collected_at
  from pg_class c
  join pg_namespace n on n.oid = c.relnamespace
  where c.relkind in ('r', 'p')
    and n.nspname = '${escapedSchema}'
  order by pg_total_relation_size(c.oid) desc, c.relname asc
) capacity_rows;
`;
}

function parsePsqlJsonOutput(stdout) {
  const trimmed = (stdout || '').trim();
  if (trimmed.length === 0) {
    throw new Error('psql returned no inspection JSON');
  }
  return JSON.parse(trimmed);
}

function runLivePostgresInspection({
  databaseUrl,
  psqlPath = 'psql',
  schema = DEFAULT_POSTGRES_SCHEMA,
  spawnSyncImpl = spawnSync,
}) {
  const result = spawnSyncImpl(
    psqlPath,
    [
      '--no-psqlrc',
      '--set',
      'ON_ERROR_STOP=1',
      '--tuples-only',
      '--no-align',
      '--command',
      buildPostgresCapacitySql(schema),
      databaseUrl,
    ],
    {
      encoding: 'utf8',
    }
  );

  if (result.error) {
    return failedInspection({
      source: 'live_postgres',
      schema,
      errors: [
        {
          kind: 'connection_failure',
          message: result.error.message,
        },
      ],
    });
  }

  if ((result.status ?? 1) !== 0) {
    const message = (result.stderr || result.stdout || 'PostgreSQL inspection failed').trim();
    return failedInspection({
      source: 'live_postgres',
      schema,
      errors: [
        {
          kind: classifyPostgresInspectionError(message),
          message,
        },
      ],
    });
  }

  try {
    const rows = parsePsqlJsonOutput(result.stdout);
    const { metrics, errors } = normalizeMetrics(Array.isArray(rows) ? rows : []);
    return {
      version: 'postgres-capacity-inspection/v1',
      status: 'collected',
      source: 'live_postgres',
      schema,
      metrics,
      errors,
      exitCode: 0,
    };
  } catch (error) {
    return failedInspection({
      source: 'live_postgres',
      schema,
      errors: [
        {
          kind: 'statistics_unavailable',
          message: error.message,
        },
      ],
    });
  }
}

function collectPostgresCapacityInspection({
  repoRoot = getRepoRoot(),
  inspectionInputPath = null,
  databaseUrl = null,
  psqlPath = 'psql',
  schema = DEFAULT_POSTGRES_SCHEMA,
  spawnSyncImpl = spawnSync,
} = {}) {
  if (inspectionInputPath && databaseUrl) {
    throw new Error('--inspection-input and --database-url cannot be used together');
  }
  if (inspectionInputPath) {
    return parseInspectionInput({ repoRoot, inspectionInputPath, schema });
  }
  if (databaseUrl) {
    return runLivePostgresInspection({ databaseUrl, psqlPath, schema, spawnSyncImpl });
  }
  return {
    version: 'postgres-capacity-inspection/v1',
    status: 'skipped',
    source: 'none',
    schema,
    metrics: [],
    errors: [],
    exitCode: 0,
  };
}

function buildReportInputs({
  repoRoot,
  migrationsDir,
  schemaReport,
  growthReport,
  rawJsonbReport,
  logQueryContractReport,
  inventory,
}) {
  if (schemaReport && growthReport && rawJsonbReport && logQueryContractReport) {
    return {
      schemaReport,
      growthReport,
      rawJsonbReport,
      logQueryContractReport,
    };
  }

  const effectiveInventory = inventory || collectSchemaInventory({ repoRoot, migrationsDir });
  return {
    schemaReport: schemaReport || evaluateSchemaHygiene({
      inventory: effectiveInventory,
      config: loadSchemaHygieneConfig(repoRoot),
    }),
    growthReport: growthReport || collectGrowthTableReport({
      repoRoot,
      inventory: effectiveInventory,
      config: loadGrowthTableConfig(repoRoot),
    }),
    rawJsonbReport: rawJsonbReport || collectRawJsonbReport({
      repoRoot,
      inventory: effectiveInventory,
      config: loadRawJsonbConfig(repoRoot),
    }),
    logQueryContractReport: logQueryContractReport || collectLogQueryContractReport({
      repoRoot,
      config: loadLogQueryContractConfig(repoRoot),
    }),
  };
}

function byName(items = []) {
  return new Map(items.map((item) => [item.name, item]));
}

function groupByTable(fields = []) {
  const grouped = new Map();
  for (const field of fields) {
    const existing = grouped.get(field.table) || [];
    existing.push(field);
    grouped.set(field.table, existing);
  }
  return grouped;
}

function metricByTable(metrics = []) {
  const grouped = new Map();
  for (const metric of metrics) {
    const existing = grouped.get(metric.tableName);
    if (!existing || (metric.totalSizeBytes || 0) > (existing.totalSizeBytes || 0)) {
      grouped.set(metric.tableName, metric);
    }
  }
  return grouped;
}

function buildFinding(source, tableName, finding) {
  return {
    source,
    table: tableName,
    rule: finding.rule || finding.dimension || 'capacity-risk',
    severity: finding.severity || finding.priority || 'warning',
    message: finding.message || finding.reason || '',
  };
}

function rawJsonbRiskForFields(fields = []) {
  const findings = fields.flatMap((field) => (
    (field.findings || []).map((finding) => buildFinding(
      'raw-jsonb-report',
      field.table,
      finding
    ))
  ));
  return {
    fields: fields.map((field) => ({
      column: field.column,
      payloadKind: field.payloadKind || (field.isRaw ? 'raw' : 'summary'),
      rawListReadRisk: Boolean(field.rawListReadRisk),
      readContract: field.readContract || null,
      listPolicy: field.listPolicy || null,
    })),
    rawFields: fields.filter((field) => field.payloadKind === 'raw' || field.isRaw).length,
    listRawRisks: fields.filter((field) => field.rawListReadRisk).length,
    findings,
  };
}

function growthRiskForTable(growthTable) {
  if (!growthTable) {
    return {
      status: 'not_configured',
      growthType: null,
      routingColumns: [],
      missingRoutingColumns: [],
      recommendedIndexes: [],
      backfill: null,
      downtimeRisk: null,
      constraintReplacementRisk: null,
      findings: [],
    };
  }
  return {
    status: growthTable.status,
    growthType: growthTable.growthType || null,
    routingColumns: growthTable.routingColumns || [],
    missingRoutingColumns: growthTable.missingRoutingColumns || [],
    recommendedIndexes: growthTable.recommendedIndexes || [],
    backfill: growthTable.backfill || null,
    downtimeRisk: growthTable.downtimeRisk || null,
    constraintReplacementRisk: growthTable.constraintReplacementRisk || null,
    findings: (growthTable.findings || []).map((finding) => buildFinding(
      'growth-table-report',
      growthTable.name,
      finding
    )),
  };
}

function capacityForMetric(metric, inspection) {
  if (!metric) {
    return {
      source: 'none',
      totalSizeBytes: null,
      tableSizeBytes: null,
      indexSizeBytes: null,
      rowEstimate: null,
      collectedAt: null,
    };
  }
  return {
    source: inspection.source,
    totalSizeBytes: metric.totalSizeBytes,
    tableSizeBytes: metric.tableSizeBytes,
    indexSizeBytes: metric.indexSizeBytes,
    rowEstimate: metric.rowEstimate,
    collectedAt: metric.collectedAt,
  };
}

function retentionArchiveStateForTable(config, tableName) {
  const state = config.retentionArchiveStates[tableName];
  if (state) {
    return {
      source: 'manual_reason',
      state: state.state || 'declared',
      reason: state.reason || '',
    };
  }
  return {
    source: 'not_declared',
    state: 'not_declared',
    reason: 'No retention/archive state declared in capacity-report config.',
  };
}

function hasIndexRisk({ growthRisk, capacity, thresholds }) {
  if (growthRisk.recommendedIndexes.some((index) => index.present === false)) {
    return true;
  }
  if (growthRisk.findings.some((finding) => /index|unique|constraint/iu.test(finding.rule))) {
    return true;
  }
  if (
    capacity.tableSizeBytes !== null
    && capacity.tableSizeBytes > 0
    && capacity.indexSizeBytes !== null
    && capacity.indexSizeBytes / capacity.tableSizeBytes >= thresholds.indexToTableSizeRatio
  ) {
    return true;
  }
  return false;
}

function hasPartitionCandidate({ growthRisk, capacity, thresholds }) {
  if (growthRisk.status === 'not_configured') {
    return false;
  }
  if (growthRisk.status === 'must_fix' || growthRisk.status === 'later') {
    return true;
  }
  if (
    capacity.rowEstimate !== null
    && capacity.rowEstimate >= thresholds.partitionCandidateRowEstimate
  ) {
    return true;
  }
  return capacity.totalSizeBytes !== null
    && capacity.totalSizeBytes >= thresholds.partitionCandidateTotalSizeBytes;
}

function buildCapacityTable({
  tableName,
  schemaTable,
  growthTable,
  rawFields,
  metric,
  inspection,
  config,
}) {
  const growthRisk = growthRiskForTable(growthTable);
  const jsonbRisk = rawJsonbRiskForFields(rawFields);
  const capacity = capacityForMetric(metric, inspection);
  const schemaFindings = (schemaTable?.findings || []).map((finding) => buildFinding(
    'schema-hygiene',
    tableName,
    finding
  ));
  const flags = {
    partitionCandidate: hasPartitionCandidate({
      growthRisk,
      capacity,
      thresholds: config.thresholds,
    }),
    indexRisk: hasIndexRisk({
      growthRisk,
      capacity,
      thresholds: config.thresholds,
    }),
    rawPayloadListRisk: jsonbRisk.listRawRisks > 0,
  };

  return {
    name: tableName,
    schemaName: metric?.schemaName || DEFAULT_POSTGRES_SCHEMA,
    profile: schemaTable?.profile || 'unknown',
    sources: {
      profile: schemaTable ? 'scan' : 'unknown',
      capacity: capacity.source,
      exemption: schemaTable?.exemption ? 'manual_reason' : 'none',
      retentionArchiveState: retentionArchiveStateForTable(config, tableName).source,
    },
    exemption: schemaTable?.exemption || null,
    retentionArchiveState: retentionArchiveStateForTable(config, tableName),
    capacity,
    growthRisk,
    jsonbRisk,
    flags,
    findings: [
      ...schemaFindings,
      ...growthRisk.findings,
      ...jsonbRisk.findings,
    ],
  };
}

function summarizeTables({
  tables,
  schemaReport,
  growthReport,
  rawJsonbReport,
  logQueryContractReport,
  inspection,
}) {
  return {
    tables: tables.length,
    withCapacityMetrics: tables.filter((table) => table.capacity.totalSizeBytes !== null).length,
    partitionCandidates: tables.filter((table) => table.flags.partitionCandidate).length,
    indexRisks: tables.filter((table) => table.flags.indexRisk).length,
    rawPayloadListRisks: tables.filter((table) => table.flags.rawPayloadListRisk).length,
    schemaHygieneFindings: schemaReport.findings?.length || schemaReport.summary?.findings || 0,
    growthFindings: growthReport.summary?.findings || 0,
    rawJsonbFindings: rawJsonbReport.summary?.findings || 0,
    logQueryFindings: logQueryContractReport.summary?.findings || 0,
    inspectionErrors: inspection.errors?.length || 0,
  };
}

function reportStatus({ summary, inspection }) {
  if (inspection.status === 'failed') {
    return 'failed';
  }
  if (
    summary.partitionCandidates > 0
    || summary.indexRisks > 0
    || summary.rawPayloadListRisks > 0
    || summary.schemaHygieneFindings > 0
    || summary.growthFindings > 0
    || summary.rawJsonbFindings > 0
    || summary.logQueryFindings > 0
    || summary.inspectionErrors > 0
  ) {
    return 'warning';
  }
  return 'passed';
}

function collectCapacityReport({
  repoRoot = getRepoRoot(),
  config,
  configPath = DEFAULT_CONFIG_FILE,
  migrationsDir = DEFAULT_MIGRATIONS_DIR,
  schemaReport = null,
  growthReport = null,
  rawJsonbReport = null,
  logQueryContractReport = null,
  inventory = null,
  inspection = null,
  inspectionInputPath = null,
  databaseUrl = null,
  psqlPath = 'psql',
  schema = DEFAULT_POSTGRES_SCHEMA,
  spawnSyncImpl = spawnSync,
  nowImpl = () => new Date(),
} = {}) {
  const effectiveConfig = normalizeConfig(config || loadConfig(repoRoot, configPath));
  const reports = buildReportInputs({
    repoRoot,
    migrationsDir,
    schemaReport,
    growthReport,
    rawJsonbReport,
    logQueryContractReport,
    inventory,
  });
  const effectiveInspection = inspection || collectPostgresCapacityInspection({
    repoRoot,
    inspectionInputPath,
    databaseUrl,
    psqlPath,
    schema,
    spawnSyncImpl,
  });
  const schemaByName = byName(reports.schemaReport.tables || []);
  const growthByName = byName(reports.growthReport.tables || []);
  const rawByTable = groupByTable(reports.rawJsonbReport.fields || []);
  const metricMap = metricByTable(effectiveInspection.metrics || []);
  const tableNames = new Set([
    ...schemaByName.keys(),
    ...growthByName.keys(),
    ...rawByTable.keys(),
    ...metricMap.keys(),
  ]);
  const tables = [...tableNames].sort().map((tableName) => buildCapacityTable({
    tableName,
    schemaTable: schemaByName.get(tableName),
    growthTable: growthByName.get(tableName),
    rawFields: rawByTable.get(tableName) || [],
    metric: metricMap.get(tableName),
    inspection: effectiveInspection,
    config: effectiveConfig,
  }));
  const summary = summarizeTables({
    tables,
    schemaReport: reports.schemaReport,
    growthReport: reports.growthReport,
    rawJsonbReport: reports.rawJsonbReport,
    logQueryContractReport: reports.logQueryContractReport,
    inspection: effectiveInspection,
  });
  const status = reportStatus({ summary, inspection: effectiveInspection });

  return {
    version: 'capacity-report/v1',
    generatedAt: nowImpl().toISOString(),
    status,
    exitCode: status === 'failed' ? 1 : 0,
    metadataFields: METADATA_FIELDS,
    inspection: effectiveInspection,
    queryContract: {
      status: reports.logQueryContractReport.status || 'unknown',
      findings: reports.logQueryContractReport.summary?.findings || 0,
      needsFix: reports.logQueryContractReport.summary?.needsFix || 0,
      endpoints: reports.logQueryContractReport.summary?.endpoints || 0,
    },
    summary,
    tables,
  };
}

function formatNullable(value) {
  return value === null || value === undefined ? '-' : String(value);
}

function formatFlags(flags) {
  const enabled = [];
  if (flags.partitionCandidate) {
    enabled.push('partition_candidate');
  }
  if (flags.indexRisk) {
    enabled.push('index_risk');
  }
  if (flags.rawPayloadListRisk) {
    enabled.push('raw_payload_list_risk');
  }
  return enabled.length === 0 ? '-' : enabled.map((flag) => `\`${flag}\``).join(', ');
}

function formatMarkdownCell(value) {
  return String(value || '-').replace(/\|/gu, '\\|');
}

function formatCapacityMarkdown(report) {
  const tableRows = report.tables.map((table) => (
    `| \`${table.name}\` | ${table.profile} | ${table.capacity.source} | `
      + `${formatNullable(table.capacity.totalSizeBytes)} | `
      + `${formatNullable(table.capacity.tableSizeBytes)} | `
      + `${formatNullable(table.capacity.indexSizeBytes)} | `
      + `${formatNullable(table.capacity.rowEstimate)} | `
      + `${formatNullable(table.capacity.collectedAt)} | `
      + `${formatFlags(table.flags)} |`
  ));
  const metadataRows = report.metadataFields.map((field) => (
    `| \`${field.field}\` | ${formatMarkdownCell(field.owner)} | `
      + `${field.sourceOfTruth} | ${field.persisted ? 'yes' : 'no'} | `
      + `${field.userEditable ? 'yes' : 'no'} | ${formatMarkdownCell(field.historicalImpact)} | `
      + `${formatMarkdownCell(field.evidenceSource)} |`
  ));
  const findingRows = report.tables.flatMap((table) => (
    table.findings.map((finding) => (
      `| \`${table.name}\` | ${finding.source} | \`${finding.rule}\` | `
        + `${finding.severity} | ${formatMarkdownCell(finding.message)} |`
    ))
  ));
  const inspectionErrorRows = (report.inspection.errors || []).map((error) => (
    `| ${error.kind} | ${formatMarkdownCell(error.tableName || '-')} | ${formatMarkdownCell(error.message)} |`
  ));

  return [
    '# Capacity Report',
    '',
    '## Summary',
    '',
    `- Status: ${report.status}`,
    `- Tables: ${report.summary.tables}`,
    `- With capacity metrics: ${report.summary.withCapacityMetrics}`,
    `- Partition candidates: ${report.summary.partitionCandidates}`,
    `- Index risks: ${report.summary.indexRisks}`,
    `- Raw payload list risks: ${report.summary.rawPayloadListRisks}`,
    `- Schema hygiene findings: ${report.summary.schemaHygieneFindings}`,
    `- Growth findings: ${report.summary.growthFindings}`,
    `- Raw JSONB findings: ${report.summary.rawJsonbFindings}`,
    `- Log query findings: ${report.summary.logQueryFindings}`,
    `- Inspection errors: ${report.summary.inspectionErrors}`,
    '',
    '## Inspection',
    '',
    `- Status: ${report.inspection.status}`,
    `- Source: ${report.inspection.source}`,
    '',
    inspectionErrorRows.length === 0 ? 'No inspection errors.' : '| Kind | Table | Message |',
    inspectionErrorRows.length > 0 ? '| --- | --- | --- |' : null,
    ...inspectionErrorRows,
    '',
    '## Tables',
    '',
    '| Table | Profile | Capacity source | Total size bytes | Table size bytes | Index size bytes | Row estimate | Collected at | Flags |',
    '| --- | --- | --- | ---: | ---: | ---: | ---: | --- | --- |',
    ...tableRows,
    '',
    '## Metadata Fields',
    '',
    '| Field | Owner | Source of truth | Persisted | User editable | Historical impact | Evidence source |',
    '| --- | --- | --- | --- | --- | --- | --- |',
    ...metadataRows,
    '',
    '## Findings',
    '',
    findingRows.length === 0 ? 'No table-level capacity findings.' : '| Table | Source | Rule | Severity | Message |',
    findingRows.length > 0 ? '| --- | --- | --- | --- | --- |' : null,
    ...findingRows,
    '',
  ].filter((line) => line !== null).join('\n');
}

function writeCapacityReports({
  repoRoot = getRepoRoot(),
  report,
  outputRoot = OUTPUT_ROOT,
  ...collectOptions
} = {}) {
  const effectiveReport = report || collectCapacityReport({ repoRoot, ...collectOptions });
  const outputDir = path.join(repoRoot, outputRoot);
  fs.mkdirSync(outputDir, { recursive: true });
  const reportPath = path.join(outputDir, JSON_REPORT_FILE);
  const markdownPath = path.join(outputDir, MARKDOWN_REPORT_FILE);
  fs.writeFileSync(reportPath, `${JSON.stringify(effectiveReport, null, 2)}\n`, 'utf8');
  fs.writeFileSync(markdownPath, `${formatCapacityMarkdown(effectiveReport)}\n`, 'utf8');
  return {
    report: effectiveReport,
    reportPath,
    markdownPath,
  };
}

function parseCapacityCliArgs(argv = []) {
  const options = {
    help: false,
    inspectionInputPath: null,
    databaseUrl: null,
    psqlPath: 'psql',
    schema: DEFAULT_POSTGRES_SCHEMA,
    configPath: DEFAULT_CONFIG_FILE,
  };

  for (let index = 0; index < argv.length; index += 1) {
    const arg = argv[index];
    if (arg === '-h' || arg === '--help') {
      options.help = true;
      continue;
    }
    if (arg === '--inspection-input') {
      options.inspectionInputPath = argv[index + 1];
      index += 1;
      continue;
    }
    if (arg === '--database-url') {
      options.databaseUrl = argv[index + 1];
      index += 1;
      continue;
    }
    if (arg === '--psql') {
      options.psqlPath = argv[index + 1];
      index += 1;
      continue;
    }
    if (arg === '--schema') {
      options.schema = argv[index + 1];
      index += 1;
      continue;
    }
    if (arg === '--config') {
      options.configPath = argv[index + 1];
      index += 1;
      continue;
    }
    throw new Error(`Unknown capacity-report option: ${arg}`);
  }

  if (options.inspectionInputPath && options.databaseUrl) {
    throw new Error('--inspection-input and --database-url cannot be used together');
  }
  validateSchemaName(options.schema);
  return options;
}

function usage(writeStdout = (text) => process.stdout.write(text)) {
  writeStdout(
    'Usage: node scripts/node/tooling.js capacity-report [--inspection-input <path>] [--database-url <url>] [--psql <path>] [--schema <name>] [--config <path>]\n'
      + 'Writes schema hygiene, capacity metrics, growth risk, and raw JSONB aggregate reports.\n'
  );
}

async function main(argv = [], deps = {}) {
  const options = parseCapacityCliArgs(argv);
  const writeStdout = deps.writeStdout || ((text) => process.stdout.write(text));
  const writeStderr = deps.writeStderr || ((text) => process.stderr.write(text));

  if (options.help) {
    usage(writeStdout);
    return 0;
  }

  const repoRoot = deps.repoRoot || getRepoRoot();
  const result = writeCapacityReports({
    repoRoot,
    config: deps.config || loadConfig(repoRoot, options.configPath),
    inspectionInputPath: options.inspectionInputPath,
    databaseUrl: options.databaseUrl,
    psqlPath: options.psqlPath,
    schema: options.schema,
    spawnSyncImpl: deps.spawnSyncImpl,
    nowImpl: deps.nowImpl,
  });

  writeStdout(
    `[1flowbase-capacity-report] ${result.report.status} `
      + `(tables ${result.report.summary.tables}, capacity_metrics ${result.report.summary.withCapacityMetrics}, `
      + `partition_candidates ${result.report.summary.partitionCandidates}, index_risks ${result.report.summary.indexRisks}, `
      + `raw_payload_list_risks ${result.report.summary.rawPayloadListRisks}). `
      + `Reports: ${toRepoRelative(repoRoot, result.reportPath)}, ${toRepoRelative(repoRoot, result.markdownPath)}\n`
  );

  for (const error of result.report.inspection.errors || []) {
    writeStderr(`[capacity-report:${error.kind}] ${error.message}\n`);
  }

  return result.report.exitCode;
}

module.exports = {
  buildPostgresCapacitySql,
  collectCapacityReport,
  collectPostgresCapacityInspection,
  formatCapacityMarkdown,
  loadConfig,
  main,
  parseCapacityCliArgs,
  writeCapacityReports,
};
