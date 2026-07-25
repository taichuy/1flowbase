'use strict';

const fs = require('node:fs');
const path = require('node:path');

const ARTIFACT_RELATIVE_DIRECTORY = path.join('tmp', 'test-governance', 'ai-gateway-concurrency');

function markdownReport(summary) {
  const lines = [
    '# AI Gateway Concurrency Characterize',
    '',
    `- Verdict: **${summary.verdict}**`,
    `- Profile: \`${summary.profile}\``,
    `- Requests: ${summary.totals.requests}`,
    `- Blocking correctness requests: ${summary.totals.blockingRequests ?? summary.totals.requests}`,
    `- Non-blocking performance requests: ${summary.totals.advisoryRequests ?? 0}`,
    `- Blocking contract failures: ${summary.totals.contractFailures}`,
    `- Non-blocking advisories: ${summary.totals.advisoryFailures ?? 0}`,
    `- Durable convergence: ${summary.durableConvergence?.verdict ?? 'not-collected'}`,
    `- Peak observed at mock upstream: ${summary.metrics.mockArrivalPeak}`,
    '',
    'Absolute timing values are characterization observations, not performance budgets.',
    '',
    '| Gate role | Topology | Barrier | Transport | Scenario | Concurrency | Targets (application/provider instance:requests) | Overlap | Pass | Outcomes | TTFT p50 ms | Total p50 ms | Throughput rps | Mock peak | Derived queue max ms |',
    '| --- | --- | --- | --- | --- | ---: | --- | --- | --- | --- | ---: | ---: | ---: | ---: | ---: |',
  ];
  for (const batch of summary.batches) {
    const outcomes = Object.entries(batch.outcomes).map(([name, count]) => `${name}:${count}`).join(', ');
    const targets = Object.entries(batch.targetDistribution ?? {}).map(([id, count]) => `${id}:${count}`).join(', ');
    const overlap = batch.overlapEvidence ? (batch.overlapEvidence.observed ? 'both' : 'missing') : '-';
    lines.push(`| ${batch.gateRole ?? 'blocking-correctness'} | ${batch.topology ?? 'same-pool'} | ${batch.batchBarrierId ?? '-'} | ${batch.transport} | ${batch.scenario} | ${batch.concurrency} | ${targets || '-'} | ${overlap} | ${batch.pass ? 'yes' : 'no'} | ${outcomes} | ${batch.metrics.ttftP50Ms ?? '-'} | ${batch.metrics.totalLatencyP50Ms ?? '-'} | ${batch.metrics.throughputRps} | ${batch.metrics.mockArrivalPeak ?? '-'} | ${batch.metrics.derivedQueueMaxMs ?? '-'} |`);
  }
  lines.push('', '## Contract failures', '');
  if (summary.failures.length === 0) lines.push('- None');
  else for (const failure of summary.failures) lines.push(`- ${failure.batch}: ${failure.message}`);
  lines.push('', '## Non-blocking performance and observability advisories', '');
  if ((summary.advisories ?? []).length === 0) lines.push('- None');
  else for (const advisory of summary.advisories) lines.push(`- ${advisory.batch}: ${advisory.message}`);
  lines.push('');
  return `${lines.join('\n')}\n`;
}

function writeCharacterizeArtifacts({ repoRoot, summary, events, durableLedger = null }) {
  if (!path.isAbsolute(repoRoot)) throw new Error('repoRoot must be an absolute path');
  const outputDirectory = path.join(repoRoot, ARTIFACT_RELATIVE_DIRECTORY);
  fs.mkdirSync(outputDirectory, { recursive: true });
  const reportPath = path.join(outputDirectory, 'report.md');
  const summaryPath = path.join(outputDirectory, 'summary.json');
  const eventsPath = path.join(outputDirectory, 'events.jsonl');
  const durableLedgerPath = path.join(outputDirectory, 'durable-ledger.json');
  fs.writeFileSync(reportPath, markdownReport(summary), 'utf8');
  fs.writeFileSync(summaryPath, `${JSON.stringify(summary, null, 2)}\n`, 'utf8');
  fs.writeFileSync(eventsPath, events.map((event) => JSON.stringify(event)).join('\n') + (events.length ? '\n' : ''), 'utf8');
  fs.writeFileSync(durableLedgerPath, `${JSON.stringify(durableLedger, null, 2)}\n`, 'utf8');
  return { outputDirectory, reportPath, summaryPath, eventsPath, durableLedgerPath };
}

module.exports = { ARTIFACT_RELATIVE_DIRECTORY, markdownReport, writeCharacterizeArtifacts };
