const path = require('node:path');
const { getRepoRoot } = require('../testing/warning-capture.js');
const { writeReports: writeSchemaReports } = require('../schema-hygiene/core.js');
const { writeRawJsonbReports } = require('../raw-jsonb-report/core.js');
const { writeLogQueryContractReports } = require('../log-query-contract-report/core.js');
const { buildReportInputs, writeCapacityReports } = require('./core.js');

// One source snapshot feeds the standalone reports and the capacity projection.
// Schema/query errors still block; capacity planning and raw JSONB stay advisory.
async function main(argv = [], deps = {}) {
  const writeStdout = deps.writeStdout || ((text) => process.stdout.write(text));
  const writeStderr = deps.writeStderr || ((text) => process.stderr.write(text));
  if (argv.includes('--help') || argv.includes('-h')) {
    writeStdout('Usage: node scripts/node/tooling.js data-contracts\n');
    return 0;
  }
  if (argv.length > 0) throw new Error(`Unknown data-contracts option: ${argv[0]}`);

  const repoRoot = deps.repoRoot || getRepoRoot();
  const reports = buildReportInputs({ repoRoot });
  const schema = writeSchemaReports({ repoRoot, report: reports.schemaReport });
  const raw = writeRawJsonbReports({ repoRoot, report: reports.rawJsonbReport });
  const query = writeLogQueryContractReports({ repoRoot, report: reports.logQueryContractReport });
  const capacity = writeCapacityReports({ repoRoot, ...reports });

  writeStdout(
    `[1flowbase-data-contracts] schema_errors ${reports.schemaReport.summary.errors}, `
      + `query_contract ${query.report.status}, raw_jsonb ${raw.report.status}, `
      + `capacity ${capacity.report.status}, measured_capacity ${capacity.report.assessments.measuredCapacity.status}.\n`
  );
  for (const result of [schema, raw, query, capacity]) {
    writeStdout(`Reports: ${path.relative(repoRoot, result.jsonPath || result.reportPath)}, `
      + `${path.relative(repoRoot, result.markdownPath)}\n`);
  }
  for (const item of reports.schemaReport.findings.filter((finding) => finding.severity === 'error')) {
    writeStderr(`[schema-hygiene:${item.rule}] ${item.table || item.file || 'schema'} ${item.message}\n`);
  }

  return Number(reports.schemaReport.summary.errors > 0)
    || query.report.exitCode || capacity.report.exitCode;
}

module.exports = { main };
