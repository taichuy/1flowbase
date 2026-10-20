const test = require('node:test');
const assert = require('node:assert/strict');

const { collectCapacityReport, formatCapacityMarkdown } = require('../core.js');

function inputs() {
  return {
    config: {},
    schemaReport: {
      tables: [{ name: 'events', profile: 'managed_table', findings: [] }],
      findings: [],
      summary: { findings: 0 },
    },
    growthReport: { tables: [], summary: { findings: 0 } },
    rawJsonbReport: { fields: [], summary: { findings: 0, listRawRisks: 0 } },
    logQueryContractReport: { status: 'passed', summary: { findings: 0 }, endpoints: [] },
    inspection: { status: 'skipped', source: 'none', metrics: [], errors: [] },
  };
}

function planningInputs() {
  const options = inputs();
  // An ordinary unpartitioned PRIMARY KEY (id) remains legal. A future routing
  // topology, missing recommended index and raw-detail review are planning work.
  options.growthReport = {
    tables: [{
      name: 'events',
      status: 'must_fix',
      routingColumns: ['scope_id'],
      findings: [
        { rule: 'unique-constraint-routing-key', priority: 'must_fix', message: 'future routing key is absent from PRIMARY KEY (id)' },
        { rule: 'raw-jsonb-review', priority: 'later', message: 'review raw detail storage before future scale' },
        { rule: 'missing-recommended-index', priority: 'must_fix', message: 'future replay index is recommended' },
      ],
      recommendedIndexes: [{ present: false, columns: ['scope_id', 'created_at'] }],
    }],
    summary: { findings: 3 },
  };
  options.rawJsonbReport.fields = [{
    table: 'events', column: 'payload', payloadKind: 'raw', rawListReadRisk: false,
    readContract: 'primary-key-detail', findings: [],
  }];
  return options;
}

function metric(overrides = {}) {
  return {
    schemaName: 'public', tableName: 'events', totalSizeBytes: 2048,
    tableSizeBytes: 1024, indexSizeBytes: 1024, rowEstimate: 10,
    collectedAt: '2026-10-20T00:00:00.000Z', ...overrides,
  };
}

test('legal unpartitioned growth planning retains every finding without claiming capacity verification', () => {
  const report = collectCapacityReport(planningInputs());
  assert.equal(report.status, 'passed');
  assert.equal(report.exitCode, 0);
  assert.deepEqual(report.assessments.currentContracts, { status: 'passed', findings: 0 });
  assert.deepEqual(report.assessments.growthPlanning, { status: 'planning', findings: 3 });
  assert.equal(report.assessments.measuredCapacity.status, 'unverified');
  assert.equal(report.assessments.measuredCapacity.coveredTables, 0);
  assert.equal(report.assessments.measuredCapacity.totalTables, 1);
  assert.equal(report.inspection.status, 'skipped');
  assert.equal(report.summary.growthFindings, 3);
  assert.equal(report.summary.partitionCandidates, 1);
  assert.equal(report.summary.indexRisks, 1);
  assert.deepEqual(report.tables[0].growthRisk.findings.map(({ rule, severity }) => [rule, severity]), [
    ['unique-constraint-routing-key', 'must_fix'],
    ['raw-jsonb-review', 'later'],
    ['missing-recommended-index', 'must_fix'],
  ]);
  assert.equal(report.tables[0].findings.length, 3);
  const markdown = formatCapacityMarkdown(report);
  assert.match(markdown, /Measured capacity: unverified \(未验证\)/u);
  assert.match(markdown, /does not certify capacity/u);
  assert.match(markdown, /Growth planning details/u);
  for (const rule of ['unique-constraint-routing-key', 'raw-jsonb-review', 'missing-recommended-index']) {
    assert.match(markdown, new RegExp(rule, 'u'));
  }
});

for (const defect of ['schema', 'raw-jsonb', 'log-query']) {
  test(`current ${defect} violations still warn even with no capacity metrics`, () => {
    const options = planningInputs();
    if (defect === 'schema') {
      const finding = { table: 'events', severity: 'error', rule: 'managed-table-scope-time-index', message: 'current scope index contract is missing' };
      options.schemaReport.findings = [finding];
      options.schemaReport.tables[0].findings = [finding];
      options.schemaReport.summary.findings = 1;
    } else if (defect === 'raw-jsonb') {
      options.rawJsonbReport.fields[0].rawListReadRisk = true;
      options.rawJsonbReport.fields[0].findings = [{ rule: 'raw-jsonb-list-read', message: 'list reads unbounded raw payload' }];
      options.rawJsonbReport.summary = { findings: 1, listRawRisks: 1 };
    } else {
      options.logQueryContractReport = { status: 'warning', summary: { findings: 1, needsFix: 1 }, endpoints: [] };
    }
    const report = collectCapacityReport(options);
    assert.equal(report.status, 'warning');
    assert.equal(report.assessments.currentContracts.status, 'warning');
    assert.equal(report.assessments.currentContracts.findings, 1);
    assert.equal(report.assessments.measuredCapacity.status, 'unverified');
    assert.equal(report.summary.growthFindings, 3);
  });
}

test('measured pressure warns independently of growth table configuration', () => {
  const options = inputs();
  options.inspection = {
    status: 'loaded', source: 'inspection_input', errors: [],
    metrics: [metric({ rowEstimate: 10_000_000, indexSizeBytes: 4096 })],
  };
  const report = collectCapacityReport(options);
  assert.equal(report.status, 'warning');
  assert.equal(report.assessments.currentContracts.status, 'passed');
  assert.equal(report.assessments.growthPlanning.status, 'none');
  assert.equal(report.assessments.measuredCapacity.status, 'warning');
  assert.equal(report.assessments.measuredCapacity.pressureTables, 1);
  assert.deepEqual(report.assessments.measuredCapacity.observations.map(({ kind }) => kind), [
    'row-estimate-threshold', 'index-to-table-size-ratio',
  ]);
  assert.equal(report.assessments.measuredCapacity.observations[0].collectedAt, metric().collectedAt);
});

test('partial metrics remain unverified while retaining observed pressure warnings', () => {
  const options = inputs();
  options.schemaReport.tables.push({ name: 'other_events', profile: 'managed_table', findings: [] });
  options.inspection = {
    status: 'loaded', source: 'inspection_input', errors: [],
    metrics: [metric({ totalSizeBytes: 10 * 1024 * 1024 * 1024 })],
  };
  const report = collectCapacityReport(options);
  assert.equal(report.status, 'warning');
  assert.equal(report.assessments.measuredCapacity.status, 'unverified');
  assert.equal(report.assessments.measuredCapacity.coveredTables, 1);
  assert.equal(report.assessments.measuredCapacity.totalTables, 2);
  assert.equal(report.assessments.measuredCapacity.pressureTables, 1);
  assert.equal(report.assessments.measuredCapacity.observations[0].kind, 'total-size-threshold');
});

test('observed metrics with no threshold pressure do not certify query performance', () => {
  const options = inputs();
  options.inspection = { status: 'loaded', source: 'inspection_input', metrics: [metric()], errors: [] };
  const report = collectCapacityReport(options);
  assert.equal(report.status, 'passed');
  assert.equal(report.assessments.measuredCapacity.status, 'observed');
  assert.deepEqual(report.assessments.measuredCapacity.observations, []);
  assert.match(formatCapacityMarkdown(report), /does not certify capacity/u);
});

test('metrics without collection time stay unverified', () => {
  const options = inputs();
  options.inspection = { status: 'loaded', source: 'inspection_input', metrics: [metric({ collectedAt: null })], errors: [] };
  const report = collectCapacityReport(options);
  assert.equal(report.assessments.measuredCapacity.status, 'unverified');
  assert.equal(report.assessments.measuredCapacity.coveredTables, 0);
  assert.equal(report.summary.withCapacityMetrics, 1);
});

test('inspection failures and invalid metric diagnostics retain failed or warning status', () => {
  const failedOptions = planningInputs();
  failedOptions.inspection = { status: 'failed', source: 'live_postgres', metrics: [], errors: [{ kind: 'permission_denied', message: 'permission denied for pg_class' }] };
  const failed = collectCapacityReport(failedOptions);
  assert.equal(failed.status, 'failed');
  assert.equal(failed.exitCode, 1);
  assert.equal(failed.summary.inspectionErrors, 1);
  const invalidOptions = inputs();
  invalidOptions.inspection = { status: 'loaded', source: 'inspection_input', metrics: [], errors: [{ kind: 'statistics_unavailable', message: 'missing row_estimate' }] };
  const invalid = collectCapacityReport(invalidOptions);
  assert.equal(invalid.status, 'warning');
  assert.equal(invalid.assessments.measuredCapacity.status, 'unverified');
});
