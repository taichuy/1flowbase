#!/usr/bin/env node
'use strict';

const fs = require('node:fs');
const path = require('node:path');

const COMPONENT_SCOPE = 'ai-gateway-protocol-conformance';

function buildComponentReport({ commandOutcome, gateResult }) {
  const gateFailures = Array.isArray(gateResult?.failures)
    ? gateResult.failures.map((failure) => failure?.message || failure?.name || String(failure))
    : [];
  const failures = [];
  if (commandOutcome !== 'success') failures.push(`blocking command outcome: ${commandOutcome || 'missing'}`);
  if (!gateResult) failures.push('missing quality-gate.json');
  if (gateResult && gateResult.status !== 'pass') failures.push(`quality gate status: ${gateResult.status || 'missing'}`);
  failures.push(...gateFailures);

  const passed = failures.length === 0;
  const oracle = gateResult?.protocol_result?.protocol_conformance?.oracle;
  const provenance = gateResult?.protocol_result?.protocol_conformance?.runtime_provenance;
  return {
    reportType: 'ci',
    status: passed ? 'passed' : 'failed',
    scope: COMPONENT_SCOPE,
    exitCode: passed ? 0 : 1,
    mainSourceSha: gateResult?.main_source_sha || '',
    officialSourceSha: gateResult?.official_source_sha || '',
    failures,
    warningFiles: [],
    coverageSummaries: [],
    backendConsistencyTargets: [],
    protocolConformance: {
      transports: gateResult?.blocking_transports?.map((transport) => transport.id) || [],
      providers: gateResult?.official_provider_codes || [],
      oracleRows: oracle?.rows ?? null,
      profileRows: oracle?.protocol_context_profiles?.rows ?? null,
      errorRows: oracle?.error_fidelity?.rows ?? null,
      provenance: provenance?.verdict || null,
    },
  };
}

function writeComponentReport({ repoRoot, commandOutcome, gateResult }) {
  const outputDir = path.join(
    repoRoot,
    'tmp',
    'test-governance',
    COMPONENT_SCOPE,
  );
  fs.mkdirSync(outputDir, { recursive: true });
  const report = buildComponentReport({ commandOutcome, gateResult });
  const reportPath = path.join(outputDir, 'quality-gate-report.json');
  const logPath = path.join(outputDir, 'quality-gate.latest.log');
  fs.writeFileSync(reportPath, `${JSON.stringify(report, null, 2)}\n`, 'utf8');
  fs.writeFileSync(logPath, [
    `scope=${report.scope}`,
    `status=${report.status}`,
    `exit_code=${report.exitCode}`,
    `main_source_sha=${report.mainSourceSha || 'missing'}`,
    ...report.failures.map((failure) => `failure=${failure}`),
  ].join('\n') + '\n', 'utf8');
  return { report, reportPath, logPath };
}

function parseArgs(argv) {
  const values = {};
  for (let index = 0; index < argv.length; index += 2) {
    const name = argv[index];
    const value = argv[index + 1];
    if (!name?.startsWith('--') || value === undefined) throw new Error(`invalid argument: ${name || 'missing'}`);
    values[name.slice(2)] = value;
  }
  if (!values['repo-root']) throw new Error('--repo-root is required');
  if (!values['command-outcome']) throw new Error('--command-outcome is required');
  return { repoRoot: path.resolve(values['repo-root']), commandOutcome: values['command-outcome'] };
}

function readGateResult(repoRoot) {
  const gatePath = path.join(
    repoRoot,
    'tmp',
    'test-governance',
    'ai-gateway-quality-gate',
    'quality-gate.json',
  );
  if (!fs.existsSync(gatePath)) return null;
  return JSON.parse(fs.readFileSync(gatePath, 'utf8'));
}

function main(argv = process.argv.slice(2)) {
  const inputs = parseArgs(argv);
  const result = writeComponentReport({
    ...inputs,
    gateResult: readGateResult(inputs.repoRoot),
  });
  process.stdout.write(`[ai-gateway-component-report] ${result.report.status}: ${result.reportPath}\n`);
  return result.report.exitCode;
}

if (require.main === module) {
  try {
    process.exitCode = main();
  } catch (error) {
    process.stderr.write(`[ai-gateway-component-report] ${error.message}\n`);
    process.exitCode = 1;
  }
}

module.exports = {
  buildComponentReport,
  main,
  parseArgs,
  readGateResult,
  writeComponentReport,
};
