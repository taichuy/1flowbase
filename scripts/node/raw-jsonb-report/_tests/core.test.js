const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');

const {
  collectRawJsonbReport,
  formatRawJsonbMarkdown,
  loadConfig,
  writeRawJsonbReports,
} = require('../core.js');

function writeFile(repoRoot, relativePath, content) {
  const absolutePath = path.join(repoRoot, relativePath);
  fs.mkdirSync(path.dirname(absolutePath), { recursive: true });
  fs.writeFileSync(absolutePath, content, 'utf8');
}

function createRepoWithMigration(sql, source = '') {
  const repoRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'oneflowbase-raw-jsonb-report-'));
  writeFile(
    repoRoot,
    'api/crates/storage-durable/postgres/migrations/20260101000000_fixture.sql',
    sql
  );
  if (source.length > 0) {
    writeFile(
      repoRoot,
      'api/crates/storage-durable/postgres/src/orchestration_runtime_repository/read_methods.rs',
      source
    );
  }
  return repoRoot;
}

test('collectRawJsonbReport flags raw JSONB selected by an unbounded list entrypoint', () => {
  const repoRoot = createRepoWithMigration(`
    create table application_run_log_summaries (
      flow_run_id uuid primary key,
      application_id uuid not null,
      title text not null,
      input_payload jsonb not null default '{}'::jsonb,
      created_at timestamptz not null default now()
    );
  `, `
    async fn list_application_run_logs_page() {
      sqlx::query("
        select
          flow_run_id as id,
          title,
          input_payload,
          created_at
        from application_run_log_summaries
        where application_id = $1
        order by created_at desc
      ");
    }
  `);

  const report = collectRawJsonbReport({
    repoRoot,
    config: {
      sourceSearchDirs: ['api/crates/storage-durable/postgres/src'],
      fields: [
        {
          table: 'application_run_log_summaries',
          column: 'input_payload',
          purpose: 'application run list legacy payload projection',
          payloadKind: 'raw',
          readContract: 'summary_or_preview_only',
          listPolicy: 'forbidden_raw',
          summaryOrPreview: 'title stores the derived summary text',
          protectedBy: ['application_id'],
        },
      ],
    },
  });

  assert.equal(report.status, 'warning');
  assert.equal(report.summary.rawFields, 1);
  assert.equal(report.summary.listRawRisks, 1);
  assert.equal(report.fields[0].appearsInListInterface, true);
  assert.equal(report.fields[0].hasSummaryOrPreview, true);
  assert.equal(report.fields[0].findings[0].rule, 'raw-jsonb-list-read');
  assert.equal(report.fields[0].readEntrypoints[0].functionName, 'list_application_run_logs_page');
  assert.equal(report.fields[0].readEntrypoints[0].readBoundary, 'list');
});

test('collectRawJsonbReport accepts detail and run-scope raw reads without list risk', () => {
  const repoRoot = createRepoWithMigration(`
    create table flow_runs (
      id uuid primary key,
      application_id uuid not null,
      input_payload jsonb not null default '{}'::jsonb,
      output_payload jsonb not null default '{}'::jsonb,
      created_at timestamptz not null default now()
    );

    create table runtime_events (
      id uuid primary key,
      flow_run_id uuid not null,
      sequence bigint not null,
      payload jsonb not null default '{}'::jsonb,
      created_at timestamptz not null default now()
    );
  `, `
    async fn fetch_flow_run_for_application() {
      sqlx::query("
        select id, application_id, input_payload, output_payload, created_at
        from flow_runs
        where application_id = $1 and id = $2
      ");
    }

    async fn list_runtime_events() {
      sqlx::query("
        select id, flow_run_id, sequence, payload, created_at
        from runtime_events
        where flow_run_id = $1 and sequence > $2
        order by sequence asc
      ");
    }
  `);

  const report = collectRawJsonbReport({
    repoRoot,
    config: {
      sourceSearchDirs: ['api/crates/storage-durable/postgres/src'],
      fields: [
        {
          table: 'flow_runs',
          column: 'input_payload',
          purpose: 'flow run input truth',
          payloadKind: 'raw',
          readContract: 'detail_or_run_scope',
          listPolicy: 'forbidden_raw',
          summaryOrPreview: 'FlowRunSummaryResponse exposes title/query/model only',
          protectedBy: ['application_id', 'flow_run_id'],
        },
        {
          table: 'runtime_events',
          column: 'payload',
          purpose: 'runtime event fact payload',
          payloadKind: 'raw',
          readContract: 'run_scope',
          listPolicy: 'run_scope_only',
          summaryOrPreview: 'runtime event metadata columns provide cursor and type summary',
          protectedBy: ['flow_run_id', 'sequence'],
        },
      ],
    },
  });

  assert.equal(report.status, 'passed');
  assert.equal(report.summary.listRawRisks, 0);
  const flowInput = report.fields.find((field) => field.table === 'flow_runs');
  const runtimePayload = report.fields.find((field) => field.table === 'runtime_events');
  assert.equal(flowInput.readEntrypoints[0].readBoundary, 'detail');
  assert.equal(runtimePayload.readEntrypoints[0].readBoundary, 'run_scope');
  assert.equal(runtimePayload.appearsInListInterface, false);
});

test('collectRawJsonbReport accepts raw reads for a bounded primary-key batch', () => {
  const repoRoot = createRepoWithMigration(`
    create table flow_runs (
      id uuid primary key,
      output_payload jsonb not null default '{}'::jsonb
    );
  `, `
    async fn list_count_tokens_results(flow_run_ids: &[Uuid]) {
      sqlx::query("
        select id, output_payload
        from flow_runs
        where id = any($1)
      ");
    }
  `);

  const report = collectRawJsonbReport({
    repoRoot,
    config: {
      sourceSearchDirs: ['api/crates/storage-durable/postgres/src'],
      fields: [{
        table: 'flow_runs',
        column: 'output_payload',
        purpose: 'flow run output truth',
        payloadKind: 'raw',
        readContract: 'detail_or_run_scope',
        listPolicy: 'forbidden_raw',
        summaryOrPreview: 'overview exposes bounded answer summary fields',
        protectedBy: ['application_id', 'flow_run_id'],
      }],
    },
  });

  assert.equal(report.status, 'passed');
  assert.equal(report.summary.listRawRisks, 0);
  assert.equal(report.fields[0].readEntrypoints[0].readBoundary, 'detail');
});

test('collectRawJsonbReport treats constant projections as summary reads instead of raw column reads', () => {
  const repoRoot = createRepoWithMigration(`
    create table application_run_log_summaries (
      flow_run_id uuid primary key,
      application_id uuid not null,
      title text not null,
      input_payload jsonb not null default '{}'::jsonb,
      created_at timestamptz not null default now()
    );
  `, `
    async fn list_application_run_logs_page() {
      sqlx::query("
        select
          flow_run_id as id,
          title,
          '{}'::jsonb as input_payload,
          created_at
        from application_run_log_summaries
        where application_id = $1
        order by created_at desc
      ");
    }
  `);

  const report = collectRawJsonbReport({
    repoRoot,
    config: {
      sourceSearchDirs: ['api/crates/storage-durable/postgres/src'],
      fields: [
        {
          table: 'application_run_log_summaries',
          column: 'input_payload',
          purpose: 'application run list summary projection',
          payloadKind: 'summary',
          readContract: 'summary_or_preview_only',
          listPolicy: 'constant_projection_only',
          summaryOrPreview: 'title and statistics are the list contract',
          protectedBy: ['application_id'],
        },
      ],
    },
  });

  assert.equal(report.status, 'passed');
  assert.equal(report.fields[0].readEntrypoints[0].columnSource, 'constant_projection');
  assert.equal(report.fields[0].appearsInListInterface, false);
});

test('collectRawJsonbReport recognizes table aliases when scanning raw list reads', () => {
  const repoRoot = createRepoWithMigration(`
    create table flow_runs (
      id uuid primary key,
      application_id uuid not null,
      input_payload jsonb not null default '{}'::jsonb,
      output_payload jsonb not null default '{}'::jsonb,
      created_at timestamptz not null default now()
    );
  `, `
    async fn list_application_conversation_runs_page() {
      sqlx::query("
        select
          prior.id,
          prior.input_payload,
          prior.output_payload,
          prior.created_at
        from flow_runs prior
        where prior.application_id = $1
        order by prior.created_at asc
      ");
    }
  `);

  const report = collectRawJsonbReport({
    repoRoot,
    config: {
      sourceSearchDirs: ['api/crates/storage-durable/postgres/src'],
      fields: [
        {
          table: 'flow_runs',
          column: 'input_payload',
          purpose: 'flow run input truth',
          payloadKind: 'raw',
          readContract: 'detail_or_run_scope',
          listPolicy: 'forbidden_raw',
          summaryOrPreview: 'run summary title/query fields',
          protectedBy: ['application_id', 'flow_run_id'],
        },
      ],
    },
  });

  assert.equal(report.status, 'warning');
  assert.equal(report.summary.listRawRisks, 1);
  assert.equal(report.fields[0].readEntrypoints[0].columnSource, 'raw_column');
  assert.equal(report.fields[0].findings[0].rule, 'raw-jsonb-list-read');
});

test('collectRawJsonbReport keeps storage SQL reads when non-SQL references exceed max evidence', () => {
  const noisyReferences = Array.from({ length: 12 }, (_, index) => `
    fn api_payload_reference_${index}() {
      let _field = "input_payload";
      let _table = "node_runs";
    }
  `).join('\n');
  const repoRoot = createRepoWithMigration(`
    create table node_runs (
      id uuid primary key,
      flow_run_id uuid not null,
      input_payload jsonb not null default '{}'::jsonb,
      created_at timestamptz not null default now()
    );
  `, `
    ${noisyReferences}

    async fn list_node_runs_for_flow_run() {
      sqlx::query("
        select
          id,
          flow_run_id,
          input_payload,
          created_at
        from node_runs
        where flow_run_id = $1
      ");
    }
  `);

  const report = collectRawJsonbReport({
    repoRoot,
    maxEvidence: 4,
    config: {
      sourceSearchDirs: ['api/crates/storage-durable/postgres/src'],
      fields: [
        {
          table: 'node_runs',
          column: 'input_payload',
          purpose: 'node input truth',
          payloadKind: 'raw',
          readContract: 'detail_or_run_scope',
          listPolicy: 'forbidden_raw',
          summaryOrPreview: 'trace node summary metadata',
          protectedBy: ['flow_run_id', 'node_run_id'],
        },
      ],
    },
  });

  assert.equal(report.status, 'passed');
  assert.equal(
    report.fields[0].readEntrypoints.some(
      (entry) => entry.functionName === 'list_node_runs_for_flow_run'
        && entry.readBoundary === 'run_scope'
        && entry.columnSource === 'raw_column'
    ),
    true
  );
});

test('writeRawJsonbReports writes JSON and Markdown under tmp/test-governance', () => {
  const repoRoot = createRepoWithMigration(`
    create table application_run_trace_node_contents (
      trace_node_id uuid primary key,
      payload jsonb not null default '{}'::jsonb,
      source_refs jsonb not null default '[]'::jsonb
    );
  `);

  const result = writeRawJsonbReports({
    repoRoot,
    config: {
      fields: [
        {
          table: 'application_run_trace_node_contents',
          column: 'payload',
          purpose: 'trace node raw content',
          payloadKind: 'raw',
          readContract: 'detail_by_trace_node_id',
          listPolicy: 'forbidden_raw',
          summaryOrPreview: 'application_run_trace_nodes carries summary metadata',
          protectedBy: ['flow_run_id', 'trace_node_id'],
        },
      ],
    },
  });

  assert.equal(result.reportPath, path.join(repoRoot, 'tmp', 'test-governance', 'raw-jsonb-report.json'));
  assert.equal(result.markdownPath, path.join(repoRoot, 'tmp', 'test-governance', 'raw-jsonb-report.md'));
  assert.equal(fs.existsSync(result.reportPath), true);
  assert.equal(fs.existsSync(result.markdownPath), true);

  const markdown = formatRawJsonbMarkdown(result.report);
  assert.match(markdown, /# Raw JSONB Boundary Report/u);
  assert.match(markdown, /summary/u);
  assert.match(markdown, /preview/u);
  assert.match(markdown, /raw/u);
  assert.match(markdown, /application_run_trace_node_contents/u);
  assert.match(markdown, /trace_node_id/u);
});

test('default config covers issue-required raw runtime payload tables', () => {
  const repoRoot = path.resolve(__dirname, '..', '..', '..', '..');
  const config = loadConfig(repoRoot);
  const fieldKeys = new Set(config.fields.map((field) => `${field.table}.${field.column}`));

  for (const key of [
    'flow_runs.input_payload',
    'flow_runs.output_payload',
    'node_runs.input_payload',
    'node_runs.debug_payload',
    'flow_run_events.payload',
    'runtime_events.payload',
    'runtime_usage_ledger.raw_usage',
    'application_run_log_summaries.input_payload',
    'application_run_trace_node_contents.payload',
  ]) {
    assert.equal(fieldKeys.has(key), true, `${key} must be covered`);
  }
});

test('default report includes storage read evidence for runtime raw payloads', () => {
  const repoRoot = path.resolve(__dirname, '..', '..', '..', '..');
  const report = collectRawJsonbReport({
    repoRoot,
    config: loadConfig(repoRoot),
  });

  assert.equal(report.status, 'passed');
  assert.equal(report.summary.listRawRisks, 0);

  for (const key of [
    'node_runs.input_payload',
    'node_runs.output_payload',
    'node_runs.error_payload',
    'node_runs.debug_payload',
    'runtime_events.payload',
  ]) {
    const [table, column] = key.split('.');
    const field = report.fields.find((candidate) => (
      candidate.table === table && candidate.column === column
    ));
    assert.ok(field, `${key} must be in the report`);
    assert.equal(
      field.readEntrypoints.some((entry) => (
        entry.file.includes('orchestration_runtime_repository/')
          && entry.operation === 'read'
          && entry.columnSource === 'raw_column'
      )),
      true,
      `${key} must keep storage read evidence`
    );
  }
});
