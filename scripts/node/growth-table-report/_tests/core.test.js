const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');

const {
  collectGrowthTableReport,
  formatGrowthTableMarkdown,
  loadConfig,
  writeGrowthTableReports,
} = require('../core.js');

function writeFile(repoRoot, relativePath, content) {
  const absolutePath = path.join(repoRoot, relativePath);
  fs.mkdirSync(path.dirname(absolutePath), { recursive: true });
  fs.writeFileSync(absolutePath, content, 'utf8');
}

function createRepoWithMigration(sql, source = '') {
  const repoRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'oneflowbase-growth-report-'));
  writeFile(
    repoRoot,
    'api/crates/storage/durable/postgres/migrations/20260101000000_fixture.sql',
    sql
  );
  if (source.length > 0) {
    writeFile(
      repoRoot,
      'api/crates/storage/durable/postgres/src/runtime_queries.rs',
      source
    );
  }
  return repoRoot;
}

test('collectGrowthTableReport flags missing routing columns, unsafe uniqueness, and missing indexes', () => {
  const repoRoot = createRepoWithMigration(`
    create table runtime_events (
      id uuid primary key,
      flow_run_id uuid not null,
      sequence bigint not null,
      payload jsonb not null default '{}'::jsonb,
      created_at timestamptz not null default now(),
      unique(flow_run_id, sequence)
    );

    create index runtime_events_flow_sequence_idx
      on runtime_events (flow_run_id, sequence asc);
  `, `
    async fn list_runtime_events() {
      sqlx::query("select * from runtime_events where flow_run_id = $1 order by sequence asc");
    }
  `);

  const report = collectGrowthTableReport({
    repoRoot,
    config: {
      sourceSearchDirs: ['api/crates/storage/durable/postgres/src'],
      tables: [
        {
          name: 'runtime_events',
          growthType: 'runtime_event',
          requiredRoutingColumns: ['scope_id', 'flow_run_id'],
          requiredTimeColumns: ['created_at'],
          ownerColumns: ['flow_run_id'],
          uniqueRouteKeys: ['scope_id'],
          recommendedIndexes: [
            {
              columns: ['scope_id', 'flow_run_id', 'sequence', 'id'],
              scenario: 'workspace-scoped runtime event replay by flow run cursor',
              priority: 'must_fix',
            },
          ],
          backfill: {
            source: 'flow_runs.application_id -> applications.workspace_id',
            batchBoundary: 'flow_run_id ranges',
            failureRecovery: 'only update rows where scope_id is null',
          },
        },
      ],
    },
  });

  assert.equal(report.status, 'warning');
  assert.equal(report.summary.mustFix, 1);
  assert.equal(report.summary.later, 0);

  const table = report.tables[0];
  assert.equal(table.status, 'must_fix');
  assert.deepEqual(table.jsonbColumns, ['payload']);
  assert.deepEqual(
    table.findings.map((finding) => finding.rule),
    [
      'missing-routing-column',
      'unique-constraint-routing-key',
      'unique-constraint-routing-key',
      'missing-recommended-index',
      'missing-expansion-scope-column',
      'missing-expansion-scope-time-id-index',
      'raw-jsonb-review',
    ]
  );
  assert.equal(table.expansionReadiness.status, 'not_ready');
  assert.equal(table.recommendedIndexes[0].present, false);
  assert.equal(table.recommendedIndexes[0].scenario, 'workspace-scoped runtime event replay by flow run cursor');
  assert.equal(table.queryEntrypoints[0].functionName, 'list_runtime_events');
  assert.equal(table.readEntrypoints[0].functionName, 'list_runtime_events');
  assert.deepEqual(table.writeEntrypoints, []);
  assert.equal(table.backfill.source, 'flow_runs.application_id -> applications.workspace_id');
  assert.equal(table.backfill.followUpMigrationTask, 'required');
  assert.equal(table.downtimeRisk.level, 'high');
  assert.equal(table.constraintReplacementRisk.level, 'high');
});

test('collectGrowthTableReport keeps routed tables without missing recommendations in ok status', () => {
  const repoRoot = createRepoWithMigration(`
    create table application_run_log_summaries (
      id uuid primary key,
      flow_run_id uuid not null,
      scope_id uuid not null,
      application_id uuid not null,
      status text not null,
      created_at timestamptz not null default now(),
      started_at timestamptz not null,
      updated_at timestamptz not null
    );

    create index application_run_log_summaries_scope_application_idx
      on application_run_log_summaries (scope_id, application_id, created_at desc, flow_run_id desc);
    create index application_run_log_summaries_scope_created_idx
      on application_run_log_summaries (scope_id, created_at desc, id desc);
  `);

  const report = collectGrowthTableReport({
    repoRoot,
    config: {
      tables: [
        {
          name: 'application_run_log_summaries',
          growthType: 'log_summary',
          requiredRoutingColumns: ['scope_id', 'application_id', 'flow_run_id'],
          requiredTimeColumns: ['created_at', 'started_at'],
          uniqueRouteKeys: [],
          recommendedIndexes: [
            {
              columns: ['scope_id', 'application_id', 'created_at', 'flow_run_id'],
              scenario: 'workspace application run log list',
              priority: 'must_fix',
            },
          ],
        },
      ],
    },
  });

  assert.equal(report.status, 'passed');
  assert.equal(report.summary.ok, 1);
  assert.equal(report.tables[0].status, 'ok');
  assert.deepEqual(report.tables[0].findings, []);
  assert.equal(report.tables[0].recommendedIndexes[0].present, true);
  assert.equal(report.tables[0].expansionReadiness.status, 'ready');
  assert.equal(report.tables[0].downtimeRisk.level, 'low');
  assert.equal(report.tables[0].constraintReplacementRisk.level, 'none');
});

test('collectGrowthTableReport rejects workspace_id-only expansion readiness', () => {
  const repoRoot = createRepoWithMigration(`
    create table workspace_events (
      id uuid primary key,
      workspace_id uuid not null,
      created_at timestamptz not null default now()
    );

    create index workspace_events_workspace_created_idx
      on workspace_events (workspace_id, created_at desc, id desc);
  `);

  const report = collectGrowthTableReport({
    repoRoot,
    config: {
      tables: [
        {
          name: 'workspace_events',
          growthType: 'workspace_high_growth',
          requiredRoutingColumns: ['workspace_id'],
          requiredTimeColumns: ['created_at'],
          uniqueRouteKeys: ['workspace_id'],
          recommendedIndexes: [
            {
              columns: ['workspace_id', 'created_at', 'id'],
              scenario: 'legacy workspace event list',
              priority: 'must_fix',
            },
          ],
        },
      ],
    },
  });

  const table = report.tables[0];
  assert.equal(table.status, 'must_fix');
  assert.equal(table.expansionReadiness.hasScopeId, false);
  assert.equal(table.expansionReadiness.workspaceIdOnly, true);
  assert.ok(table.findings.some((finding) => finding.rule === 'missing-expansion-scope-column'));
});

test('collectGrowthTableReport exempts bounded plugin projections from expansion blockers', () => {
  const catalogReason = 'bounded plugin package catalog projection keyed by plugin installation';
  const artifactReason = 'bounded plugin artifact projection keyed by installation and node';
  const repoRoot = createRepoWithMigration(`
    create table plugin_package_catalog_projection (
      installation_id uuid primary key,
      package_code text not null,
      package_version text not null,
      catalog_snapshot_json jsonb not null default '{}'::jsonb,
      projection_status text not null,
      refreshed_at timestamptz,
      updated_at timestamptz not null default now()
    );

    create index plugin_package_catalog_projection_package_idx
      on plugin_package_catalog_projection (package_code, package_version);

    create index plugin_package_catalog_projection_status_idx
      on plugin_package_catalog_projection (projection_status, updated_at desc);

    create table plugin_artifact_instances (
      node_id text not null,
      installation_id uuid not null,
      artifact_status text not null,
      runtime_status text not null default 'inactive',
      checked_at timestamptz not null default now(),
      primary key (node_id, installation_id)
    );

    create index plugin_artifact_instances_installation_id_idx
      on plugin_artifact_instances (installation_id);
  `);

  const report = collectGrowthTableReport({
    repoRoot,
    config: {
      tables: [
        {
          name: 'plugin_package_catalog_projection',
          growthType: 'plugin_catalog_projection',
          requiredRoutingColumns: ['installation_id', 'package_code'],
          requiredTimeColumns: ['updated_at', 'refreshed_at'],
          missingTimePriority: 'later',
          uniqueRouteKeys: ['installation_id'],
          expansionReadinessExemption: {
            kind: 'bounded_projection',
            reason: catalogReason,
            boundedBy: ['plugin_installations'],
          },
          recommendedIndexes: [
            {
              columns: ['package_code', 'package_version'],
              scenario: 'package catalog projection lookup',
              priority: 'must_fix',
            },
            {
              columns: ['projection_status', 'updated_at'],
              scenario: 'catalog projection repair scan',
              priority: 'must_fix',
            },
          ],
        },
        {
          name: 'plugin_artifact_instances',
          growthType: 'plugin_artifact_projection',
          requiredRoutingColumns: ['installation_id', 'node_id'],
          requiredTimeColumns: ['checked_at'],
          uniqueRouteKeys: ['installation_id'],
          expansionReadinessExemption: {
            kind: 'bounded_projection',
            reason: artifactReason,
            boundedBy: ['plugin_installations'],
          },
          recommendedIndexes: [
            {
              columns: ['installation_id'],
              scenario: 'plugin installation artifact status lookup',
              priority: 'must_fix',
            },
          ],
        },
      ],
    },
  });

  assert.equal(report.summary.mustFix, 0);
  const catalog = report.tables.find((table) => table.name === 'plugin_package_catalog_projection');
  const artifact = report.tables.find((table) => table.name === 'plugin_artifact_instances');

  assert.equal(catalog.expansionReadiness.status, 'bounded');
  assert.equal(catalog.expansionReadiness.exemption.reason, catalogReason);
  assert.deepEqual(catalog.expansionReadiness.recommendedActions, ['bounded_projection_exempt']);
  assert.equal(catalog.findings.some((finding) => finding.rule.startsWith('missing-expansion-')), false);
  assert.equal(catalog.status, 'later');

  assert.equal(artifact.expansionReadiness.status, 'bounded');
  assert.equal(artifact.expansionReadiness.exemption.reason, artifactReason);
  assert.deepEqual(artifact.expansionReadiness.recommendedActions, ['bounded_projection_exempt']);
  assert.deepEqual(artifact.findings, []);
  assert.equal(artifact.status, 'ok');

  const markdown = formatGrowthTableMarkdown(report);
  assert.match(markdown, /Readiness exemption/u);
  assert.match(markdown, new RegExp(catalogReason, 'u'));
  assert.match(markdown, new RegExp(artifactReason, 'u'));
});

test('collectGrowthTableReport splits write and read entrypoints from SQL evidence', () => {
  const repoRoot = createRepoWithMigration(`
    create table runtime_events (
      id uuid primary key,
      scope_id uuid not null,
      flow_run_id uuid not null,
      sequence bigint not null,
      created_at timestamptz not null default now()
    );

    create index runtime_events_scope_created_idx
      on runtime_events (scope_id, created_at desc, id desc);
  `, `
    async fn append_runtime_event() {
      sqlx::query("insert into runtime_events (id, scope_id, flow_run_id, sequence) values ($1, $2, $3, $4)");
    }

    async fn list_runtime_events() {
      sqlx::query("select * from runtime_events where scope_id = $1 order by sequence asc");
    }
  `);

  const report = collectGrowthTableReport({
    repoRoot,
    config: {
      sourceSearchDirs: ['api/crates/storage/durable/postgres/src'],
      tables: [
        {
          name: 'runtime_events',
          growthType: 'runtime_event',
          requiredRoutingColumns: ['scope_id', 'flow_run_id'],
          requiredTimeColumns: ['created_at'],
          uniqueRouteKeys: ['scope_id'],
          recommendedIndexes: [],
        },
      ],
    },
  });

  const table = report.tables[0];
  assert.equal(table.writeEntrypoints[0].functionName, 'append_runtime_event');
  assert.equal(table.readEntrypoints[0].functionName, 'list_runtime_events');
});

test('writeGrowthTableReports writes JSON and Markdown under tmp/test-governance', () => {
  const repoRoot = createRepoWithMigration(`
    create table runtime_debug_artifacts (
      id uuid primary key,
      workspace_id uuid not null,
      application_id uuid not null,
      flow_run_id uuid,
      artifact_kind text not null,
      created_at timestamptz not null default now()
    );

    create index runtime_debug_artifacts_workspace_created_idx
      on runtime_debug_artifacts (workspace_id, created_at desc, id desc);
  `);

  const result = writeGrowthTableReports({
    repoRoot,
    config: {
      tables: [
        {
          name: 'runtime_debug_artifacts',
          growthType: 'artifact',
          requiredRoutingColumns: ['workspace_id', 'application_id', 'flow_run_id'],
          requiredTimeColumns: ['created_at'],
          uniqueRouteKeys: ['workspace_id'],
          recommendedIndexes: [
            {
              columns: ['workspace_id', 'created_at', 'id'],
              scenario: 'workspace artifact retention and capacity inspection',
              priority: 'must_fix',
            },
          ],
        },
      ],
    },
  });

  assert.equal(result.reportPath, path.join(repoRoot, 'tmp', 'test-governance', 'growth-table-report.json'));
  assert.equal(result.markdownPath, path.join(repoRoot, 'tmp', 'test-governance', 'growth-table-report.md'));
  assert.equal(fs.existsSync(result.reportPath), true);
  assert.equal(fs.existsSync(result.markdownPath), true);

  const markdown = formatGrowthTableMarkdown(result.report);
  assert.match(markdown, /# Growth Table Routing Report/u);
  assert.match(markdown, /runtime_debug_artifacts/u);
  assert.match(markdown, /workspace artifact retention and capacity inspection/u);
  assert.match(markdown, /## Write Path Evidence/u);
  assert.match(markdown, /## Read Path Evidence/u);
  assert.match(markdown, /## Downtime And Constraint Risk/u);
});

test('default config covers issue-required and active legacy high-growth tables', () => {
  const repoRoot = path.resolve(__dirname, '..', '..', '..', '..');
  const config = loadConfig(repoRoot);
  const tableNames = new Set(config.tables.map((table) => table.name));

  for (const tableName of [
    'flow_runs',
    'flow_run_events',
    'runtime_events',
    'runtime_debug_artifacts',
    'application_run_log_summaries',
    'application_public_conversations',
    'application_run_trace_projection_statuses',
    'application_run_trace_nodes',
    'application_run_trace_node_contents',
    'plugin_package_catalog_projection',
    'plugin_artifact_instances',
  ]) {
    assert.equal(tableNames.has(tableName), true, `${tableName} must be covered`);
  }

  for (const tableName of ['plugin_package_catalog_projection', 'plugin_artifact_instances']) {
    const spec = config.tables.find((table) => table.name === tableName);
    assert.equal(spec.expansionReadinessExemption.kind, 'bounded_projection');
    assert.match(spec.expansionReadinessExemption.reason, /bounded plugin/u);
  }

  for (const tableName of [
    'application_run_trace_projection_statuses',
    'application_run_trace_nodes',
    'application_run_trace_node_contents',
  ]) {
    const spec = config.tables.find((table) => table.name === tableName);
    assert.match(spec.backfill.source, /flow_runs\.scope_id/u);
  }
});
