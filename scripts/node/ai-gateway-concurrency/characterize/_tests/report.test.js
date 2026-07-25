'use strict';

const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const test = require('node:test');
const { markdownReport, writeCharacterizeArtifacts } = require('../report');

function fixtureSummary() {
  return {
    schemaVersion: 1,
    profile: 'characterize',
    verdict: 'PASS',
    performanceBudgetApplied: false,
    totals: {
      requests: 1,
      blockingRequests: 1,
      advisoryRequests: 0,
      contractFailures: 0,
      advisoryFailures: 0,
    },
    metrics: { mockArrivalPeak: 1 },
    failures: [],
    advisories: [],
    batches: [{
      gateRole: 'blocking-correctness',
      topology: 'multi-pool',
      batchBarrierId: 'batch-001',
      transport: 'responses-sse',
      scenario: 'normal',
      concurrency: 1,
      pass: true,
      outcomes: { completed: 1 },
      targetDistribution: { 'application-1/instance-1': 1 },
      overlapEvidence: { observed: true },
      failures: [],
      metrics: {
        ttftP50Ms: 1.25,
        totalLatencyP50Ms: 2.5,
        throughputRps: 10,
        mockArrivalPeak: 1,
        derivedQueueMaxMs: 0,
      },
    }],
  };
}

test('AC-007: artifacts use the fixed governance paths and valid JSON/JSONL', () => {
  const repoRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'ai-gateway-characterize-'));
  try {
    const summary = fixtureSummary();
    const events = [{ kind: 'request', clientNonce: 'load-000001', outcome: 'completed' }];
    const durableLedger = { schemaVersion: 1, verdict: 'PASS', requests: [], polls: [], failures: [] };
    const artifacts = writeCharacterizeArtifacts({ repoRoot, summary, events, durableLedger });
    assert.equal(
      path.relative(repoRoot, artifacts.outputDirectory),
      path.join('tmp', 'test-governance', 'ai-gateway-concurrency'),
    );
    assert.equal(path.basename(artifacts.reportPath), 'report.md');
    assert.equal(path.basename(artifacts.summaryPath), 'summary.json');
    assert.equal(path.basename(artifacts.eventsPath), 'events.jsonl');
    assert.equal(path.basename(artifacts.durableLedgerPath), 'durable-ledger.json');
    assert.deepEqual(JSON.parse(fs.readFileSync(artifacts.summaryPath, 'utf8')), summary);
    assert.deepEqual(
      fs.readFileSync(artifacts.eventsPath, 'utf8').trim().split('\n').map((line) => JSON.parse(line)),
      events,
    );
    assert.deepEqual(JSON.parse(fs.readFileSync(artifacts.durableLedgerPath, 'utf8')), durableLedger);
  } finally {
    fs.rmSync(repoRoot, { recursive: true, force: true });
  }
});

test('AC-007: report labels timings as observations without an absolute budget', () => {
  const report = markdownReport(fixtureSummary());
  assert.match(report, /Absolute timing values are characterization observations, not performance budgets\./u);
  assert.match(report, /blocking-correctness \| multi-pool \| batch-001 \| responses-sse \| normal \| 1/u);
  assert.match(report, /application-1\/instance-1:1 \| both/u);
});
