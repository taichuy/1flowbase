const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');

const {
  collectLogQueryContractReport,
  formatLogQueryContractMarkdown,
  loadConfig,
  writeLogQueryContractReports,
} = require('../core.js');

function writeFile(repoRoot, relativePath, content) {
  const absolutePath = path.join(repoRoot, relativePath);
  fs.mkdirSync(path.dirname(absolutePath), { recursive: true });
  fs.writeFileSync(absolutePath, content, 'utf8');
}

function createRepoWithSource(source) {
  const repoRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'oneflowbase-log-query-contract-'));
  writeFile(repoRoot, 'api/src/logs.rs', source);
  return repoRoot;
}

function endpointConfig(contract) {
  return {
    endpoints: [
      {
        id: 'logs',
        category: 'log_list',
        method: 'GET',
        path: '/logs',
        api: {
          file: 'api/src/logs.rs',
          functionName: 'list_logs',
        },
        repository: {
          file: 'api/src/logs.rs',
          functionName: 'list_logs_page',
        },
        contract,
      },
    ],
  };
}

function monitoringEndpointConfig(contract) {
  const config = endpointConfig(contract);
  config.endpoints[0].extraSources = [
    {
      file: 'api/src/logs.rs',
      functionName: 'application_run_monitoring_logs_query',
      includeStr: 'queries/monitoring.sql',
    },
  ];
  return config;
}

function createMonitoringRepo(sql) {
  const repoRoot = createRepoWithSource(`
    async fn list_logs() {
      visible_application(actor, application_id).await;
      let started_from = default_started_from(&query);
    }

    async fn list_logs_page() {}

    fn application_run_monitoring_logs_query() -> &'static str {
      include_str!("queries/monitoring.sql")
    }
  `);
  writeFile(repoRoot, 'api/src/queries/monitoring.sql', sql);
  return repoRoot;
}

function monitoringContract() {
  return {
    scope: {
      patterns: [
        'visible_application\\(actor, application_id\\)',
        'where\\s+(?:[A-Za-z_][A-Za-z0-9_]*\\.)?application_id\\s*=\\s*\\$1',
      ],
    },
    time: {
      patterns: [
        'default_started_from\\(&query\\)',
        '(?:[A-Za-z_][A-Za-z0-9_]*\\.)?started_at\\s*>=\\s*\\$2',
      ],
    },
    cursor: { patterns: ['with monitoring_logs as'] },
    limit: { patterns: ['limit 10'] },
  };
}

test('collectLogQueryContractReport fails missing query dimensions without exemption', () => {
  const repoRoot = createRepoWithSource(`
    async fn list_logs() {
      let application_id = id;
      let page_size = query.page_size.unwrap_or(20).clamp(1, 100);
      list_logs_page(application_id, page_size).await;
    }

    async fn list_logs_page() {
      sqlx::query("
        select id
        from application_run_log_summaries
        where application_id = $1
        order by created_at desc, id desc
        limit $2
      ");
    }
  `);

  const report = collectLogQueryContractReport({
    repoRoot,
    config: endpointConfig({
      scope: { patterns: ['application_id', 'where application_id = \\$1'] },
      time: { patterns: ['created_at >= \\$2'] },
      cursor: { patterns: ['cursor'] },
      limit: { patterns: ['limit \\$2'] },
    }),
  });

  assert.equal(report.status, 'failed');
  assert.equal(report.exitCode, 1);
  assert.equal(report.summary.failed, 1);
  assert.equal(report.summary.dimensionFailures, 2);
  assert.deepEqual(
    report.endpoints[0].findings.map((finding) => finding.dimension).sort(),
    ['cursor', 'time']
  );
});

test('collectLogQueryContractReport accepts explicit reasoned exemptions', () => {
  const repoRoot = createRepoWithSource(`
    async fn list_logs() {
      let application_id = id;
      let page_size = query.page_size.unwrap_or(20).clamp(1, 100);
      list_logs_page(application_id, page_size).await;
    }

    async fn list_logs_page() {
      sqlx::query("
        select id
        from application_run_log_summaries
        where application_id = $1
        order by created_at desc, id desc
        limit $2
      ");
    }
  `);

  const report = collectLogQueryContractReport({
    repoRoot,
    config: endpointConfig({
      scope: { patterns: ['application_id', 'where application_id = \\$1'] },
      time: {
        patterns: ['created_at >= \\$2'],
        exemption: {
          reason: 'Fixture endpoint is scoped to a pre-bounded test slice.',
          removeBy: '2026-09-30',
        },
      },
      cursor: {
        patterns: ['cursor'],
        exemption: {
          reason: 'Existing endpoint uses stable page/page_size while cursor migration is planned.',
          removeBy: '2026-09-30',
        },
      },
      limit: { patterns: ['limit \\$2'] },
    }),
  });

  assert.equal(report.status, 'passed');
  assert.equal(report.summary.exempted, 1);
  assert.equal(report.summary.dimensionExemptions, 2);
  assert.equal(report.endpoints[0].dimensions.find((dimension) => dimension.dimension === 'cursor').status, 'exempted');
});

test('collectLogQueryContractReport fails when configured function is missing', () => {
  const repoRoot = createRepoWithSource(`
    async fn list_logs() {}
    async fn list_logs_page() {}
  `);
  const config = endpointConfig({
    scope: { patterns: ['list_logs'] },
    time: {
      patterns: [],
      exemption: {
        reason: 'Fixture checks source resolution only.',
        removeBy: 'not_required',
      },
    },
    cursor: {
      patterns: [],
      exemption: {
        reason: 'Fixture checks source resolution only.',
        removeBy: 'not_required',
      },
    },
    limit: {
      patterns: [],
      exemption: {
        reason: 'Fixture checks source resolution only.',
        removeBy: 'not_required',
      },
    },
  });
  config.endpoints[0].api.functionName = 'missing_logs_handler';

  const report = collectLogQueryContractReport({
    repoRoot,
    config,
  });

  assert.equal(report.status, 'failed');
  assert.equal(report.summary.needsFix, 1);
  assert.deepEqual(report.endpoints[0].findings, [
    {
      endpointId: 'logs',
      dimension: 'source',
      severity: 'fail',
      message: 'API function is missing: missing_logs_handler',
    },
  ]);
});

test('writeLogQueryContractReports writes JSON and Markdown under tmp/test-governance', () => {
  const repoRoot = createRepoWithSource(`
    async fn list_logs() {
      let application_id = id;
      let created_after = application_runs_created_after(&query);
      let page_size = query.page_size.unwrap_or(20).clamp(1, 100);
      list_logs_page(application_id, created_after, page_size).await;
    }

    async fn list_logs_page() {
      sqlx::query("
        select id
        from application_run_log_summaries
        where application_id = $1
          and created_at >= $2
        order by created_at desc, id desc
        limit $3
      ");
    }
  `);
  const result = writeLogQueryContractReports({
    repoRoot,
    config: endpointConfig({
      scope: { patterns: ['application_id', 'where application_id = \\$1'] },
      time: { patterns: ['created_after', 'created_at >= \\$2'] },
      cursor: {
        patterns: ['order by created_at desc, id desc'],
        exemption: {
          reason: 'Stable offset page is accepted for this fixture.',
          removeBy: '2026-09-30',
        },
      },
      limit: { patterns: ['clamp\\(1, 100\\)', 'limit \\$3'] },
    }),
  });

  assert.equal(result.reportPath, path.join(repoRoot, 'tmp', 'test-governance', 'log-query-contract-report.json'));
  assert.equal(result.markdownPath, path.join(repoRoot, 'tmp', 'test-governance', 'log-query-contract-report.md'));
  assert.equal(fs.existsSync(result.reportPath), true);
  assert.equal(fs.existsSync(result.markdownPath), true);

  const markdown = formatLogQueryContractMarkdown(result.report);
  assert.match(markdown, /# Log Query Contract Report/u);
  assert.match(markdown, /logs/u);
});

test('monitoring contract scans qualified predicates from its included SQL', () => {
  const repoRoot = createMonitoringRepo(`
    with monitoring_logs as (
      select t.application_id, t.started_at
      from application_run_logs t
      where t.application_id = $1
        and t.started_at >= $2
    )
    select * from monitoring_logs
    limit 10
  `);

  const report = collectLogQueryContractReport({
    repoRoot,
    config: monitoringEndpointConfig(monitoringContract()),
  });

  assert.equal(report.status, 'passed');
  assert.equal(report.endpoints[0].dimensions.find((dimension) => dimension.dimension === 'scope').status, 'compliant');
  assert.equal(report.endpoints[0].dimensions.find((dimension) => dimension.dimension === 'time').status, 'compliant');
});

test('monitoring contract still fails when included SQL omits application scope', () => {
  const repoRoot = createMonitoringRepo(`
    with monitoring_logs as (
      select t.application_id, t.started_at
      from application_run_logs t
      where t.started_at >= $2
    )
    select * from monitoring_logs
    limit 10
  `);

  const report = collectLogQueryContractReport({
    repoRoot,
    config: monitoringEndpointConfig(monitoringContract()),
  });

  assert.equal(report.status, 'failed');
  assert.equal(report.endpoints[0].dimensions.find((dimension) => dimension.dimension === 'scope').status, 'failed');
  assert.equal(report.endpoints[0].dimensions.find((dimension) => dimension.dimension === 'time').status, 'compliant');
});

test('monitoring contract still fails when included SQL omits the time bound', () => {
  const repoRoot = createMonitoringRepo(`
    with monitoring_logs as (
      select t.application_id, t.started_at
      from application_run_logs t
      where t.application_id = $1
    )
    select * from monitoring_logs
    limit 10
  `);

  const report = collectLogQueryContractReport({
    repoRoot,
    config: monitoringEndpointConfig(monitoringContract()),
  });

  assert.equal(report.status, 'failed');
  assert.equal(report.endpoints[0].dimensions.find((dimension) => dimension.dimension === 'scope').status, 'compliant');
  assert.equal(report.endpoints[0].dimensions.find((dimension) => dimension.dimension === 'time').status, 'failed');
});

test('default config covers issue-required log query families and passes current repo', () => {
  const repoRoot = path.resolve(__dirname, '..', '..', '..', '..');
  const config = loadConfig(repoRoot);
  const endpointIds = new Set(config.endpoints.map((endpoint) => endpoint.id));

  for (const id of [
    'application_run_logs_page',
    'application_run_monitoring_report',
    'application_conversation_messages_page',
    'application_run_trace_roots',
    'application_run_trace_children_page',
    'runtime_debug_stream_json_page',
    'runtime_debug_stream_events',
    'runtime_usage_ledger_run_scope',
    'runtime_debug_artifact_resolve_batch',
    'runtime_debug_artifact_detail',
  ]) {
    assert.equal(endpointIds.has(id), true, `${id} must be covered`);
  }

  const report = collectLogQueryContractReport({
    repoRoot,
    config,
  });

  assert.equal(report.status, 'passed');
  assert.equal(report.summary.findings, 0);
});
