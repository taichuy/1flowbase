const fs = require('node:fs');
const path = require('node:path');

const OUTPUT_ROOT = path.join('tmp', 'test-governance');

const JSON_REPORT_FILE = 'schema-hygiene.json';

const MARKDOWN_REPORT_FILE = 'schema-hygiene.md';

const DEFAULT_MAX_FINDINGS = 400;

function writeMarkdownReport(report, markdownPath) {
  const lines = [
    '# Schema Hygiene',
    '',
    `- Tables: ${report.summary.tables}`,
    `- Findings: ${report.summary.findings}`,
    `- Errors: ${report.summary.errors}`,
    `- Parse errors: ${report.summary.parseErrors}`,
    '',
    '## Findings',
    '',
  ];

  if (report.findings.length === 0) {
    lines.push('No findings.');
  } else {
    lines.push('| Severity | Rule | Table | Reason | Suggested action |');
    lines.push('| --- | --- | --- | --- | --- |');
    for (const item of report.findings) {
      lines.push(`| ${item.severity} | ${item.rule} | ${item.table || item.file || ''} | ${(item.reason || item.message).replace(/\|/gu, '\\|')} | ${(item.action || '').replace(/\|/gu, '\\|')} |`);
    }
  }

  lines.push('', '## Tables', '', '| Table | Profile | Category | Missing platform fields | Actions | Readiness reason | Columns | JSONB | Indexes |');
  lines.push('| --- | --- | --- | --- | --- | --- | ---: | --- | ---: |');
  for (const table of report.tables) {
    lines.push(
      `| ${table.name} | ${table.profile} | ${table.platformReadiness.category} | `
        + `${table.platformReadiness.missingFields.join(', ') || '-'} | `
        + `${table.platformReadiness.recommendedActions.join(', ')} | `
        + `${(table.platformReadiness.reason || '').replace(/\|/gu, '\\|')} | `
        + `${table.columns.length} | ${table.jsonbColumns.join(', ')} | ${table.indexes.length} |`
    );
  }

  fs.writeFileSync(markdownPath, `${lines.join('\n')}\n`, 'utf8');
}

function writeReports({ repoRoot, report, maxFindings = DEFAULT_MAX_FINDINGS }) {
  const outputDir = path.join(repoRoot, OUTPUT_ROOT);
  fs.mkdirSync(outputDir, { recursive: true });

  const reportForDisk = {
    ...report,
    findings: report.findings.slice(0, maxFindings),
    truncated: report.findings.length > maxFindings,
  };

  const jsonPath = path.join(outputDir, JSON_REPORT_FILE);
  const markdownPath = path.join(outputDir, MARKDOWN_REPORT_FILE);
  fs.writeFileSync(jsonPath, `${JSON.stringify(reportForDisk, null, 2)}\n`, 'utf8');
  writeMarkdownReport(reportForDisk, markdownPath);

  return { jsonPath, markdownPath, report: reportForDisk };
}

module.exports = { writeReports, DEFAULT_MAX_FINDINGS };
