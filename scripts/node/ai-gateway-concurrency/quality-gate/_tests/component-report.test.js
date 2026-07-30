'use strict';

const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const test = require('node:test');

const {
  buildComponentReport,
  writeComponentReport,
} = require('../component-report');

const passingGateResult = Object.freeze({
  status: 'pass',
  failures: [],
  main_source_sha: 'candidate-sha',
  official_source_sha: 'official-sha',
  blocking_transports: [
    { id: 'chat-completions-sse', label: 'OpenAI Chat' },
    { id: 'anthropic-sse', label: 'Anthropic' },
    { id: 'responses-sse', label: 'Responses SSE' },
    { id: 'responses-websocket', label: 'Responses WebSocket' },
  ],
  official_provider_codes: ['openai', 'anthropic', 'openai_compatible'],
  protocol_result: {
    protocol_conformance: {
      oracle: {
        rows: 16,
        protocol_context_profiles: { rows: 9 },
        error_fidelity: { rows: 20 },
      },
      runtime_provenance: { verdict: 'PASS' },
    },
  },
});

test('AC-012: passing gateway evidence becomes an aggregate-compatible component report', () => {
  const report = buildComponentReport({ commandOutcome: 'success', gateResult: passingGateResult });
  assert.equal(report.status, 'passed');
  assert.equal(report.scope, 'ai-gateway-protocol-conformance');
  assert.equal(report.exitCode, 0);
  assert.equal(report.protocolConformance.oracleRows, 16);
  assert.equal(report.protocolConformance.profileRows, 9);
  assert.equal(report.protocolConformance.errorRows, 20);
  assert.equal(report.protocolConformance.provenance, 'PASS');
});

test('AC-012 controlled negatives: command failure or missing evidence fails the aggregate component', () => {
  assert.equal(buildComponentReport({
    commandOutcome: 'failure',
    gateResult: passingGateResult,
  }).status, 'failed');
  const missing = buildComponentReport({ commandOutcome: 'success', gateResult: null });
  assert.equal(missing.status, 'failed');
  assert.match(missing.failures.join('\n'), /missing quality-gate\.json/u);
});

test('AC-012: component report writer emits the standard aggregate filenames', () => {
  const repoRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'ai-gateway-component-report-'));
  const paths = writeComponentReport({
    repoRoot,
    commandOutcome: 'success',
    gateResult: passingGateResult,
  });
  assert.equal(path.basename(paths.reportPath), 'quality-gate-report.json');
  assert.equal(path.basename(paths.logPath), 'quality-gate.latest.log');
  assert.equal(JSON.parse(fs.readFileSync(paths.reportPath, 'utf8')).status, 'passed');
  assert.match(fs.readFileSync(paths.logPath, 'utf8'), /status=passed/u);
});
