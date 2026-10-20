const fs = require('node:fs');
const path = require('node:path');
const OUTPUT_ROOT = path.join('tmp', 'test-governance');
const JSON_REPORT_FILE = 'console-operation-registry-hygiene.json';
const MARKDOWN_REPORT_FILE = 'console-operation-registry-hygiene.md';
const DEFAULT_MAX_FINDINGS = 200;

function renderMarkdown(report) {
  const lines = [
    '# Console Operation Registry Hygiene',
    '',
    `- Findings: ${report.summary.findings}`,
    `- Errors: ${report.summary.errors}`,
    `- Warnings: ${report.summary.warnings}`,
    `- Missing diff items: ${report.summary.missing}`,
    `- Permission expansion diff items: ${report.summary.expansions}`,
    '',
    '## Compiled Assembly Evidence',
    '',
    `- Authoritative: ${report.compiled_checks.authoritative ? 'yes' : 'no'}`,
    `- Commands passed: ${report.summary.compiledChecksPassed}`,
    `- Commands failed: ${report.summary.compiledChecksFailed}`,
    '',
    '## Missing / Gap Diff',
    '',
  ];

  if (report.diff.missing.length === 0) {
    lines.push('No missing or gap diff items.');
  } else {
    lines.push('| Kind | Key |');
    lines.push('| --- | --- |');
    for (const item of report.diff.missing) {
      lines.push(`| ${item.kind} | ${String(item.key).replace(/\|/gu, '\\|')} |`);
    }
  }

  lines.push('', '## Permission Expansion Diff', '');
  if (report.diff.expansion.length === 0) {
    lines.push('No permission expansion diff items.');
  } else {
    lines.push('| Kind | Key |');
    lines.push('| --- | --- |');
    for (const item of report.diff.expansion) {
      lines.push(`| ${item.kind} | ${String(item.key).replace(/\|/gu, '\\|')} |`);
    }
  }

  lines.push('', '## Regrouping Diff', '');
  if (report.diff.regrouping.length === 0) {
    lines.push('No policy-group regrouping.');
  } else {
    lines.push('| Kind | Key | Before | After |');
    lines.push('| --- | --- | --- | --- |');
    for (const item of report.diff.regrouping) {
      lines.push(`| ${item.kind} | ${item.key} | ${JSON.stringify(item.before)} | ${JSON.stringify(item.after)} |`);
    }
  }

  lines.push('', '## Findings', '');
  if (report.findings.length === 0) {
    lines.push('No findings.');
  } else {
    lines.push('| Severity | Rule | Source | Subject | File | Message |');
    lines.push('| --- | --- | --- | --- | --- | --- |');
    for (const finding of report.findings) {
      const message = String(finding.message).replace(/\|/gu, '\\|').replace(/\n/gu, '<br>');
      lines.push(
        `| ${finding.severity} | ${finding.rule} | ${finding.source} | ${finding.subject || ''} | ${finding.file || ''} | ${message} |`
      );
    }
  }

  return `${lines.join('\n')}\n`;
}

function writeReport({ repoRoot, report, maxFindings = DEFAULT_MAX_FINDINGS } = {}) {
  const outputDir = path.join(repoRoot, OUTPUT_ROOT);
  fs.mkdirSync(outputDir, { recursive: true });
  const reportForDisk = {
    ...report,
    findings: report.findings.slice(0, maxFindings),
    truncated: report.findings.length > maxFindings,
  };
  const jsonReportPath = path.join(outputDir, JSON_REPORT_FILE);
  const markdownReportPath = path.join(outputDir, MARKDOWN_REPORT_FILE);
  fs.writeFileSync(jsonReportPath, `${JSON.stringify(reportForDisk, null, 2)}\n`, 'utf8');
  fs.writeFileSync(markdownReportPath, renderMarkdown(reportForDisk), 'utf8');
  return { jsonReportPath, markdownReportPath, report: reportForDisk };
}

module.exports = { DEFAULT_MAX_FINDINGS, renderMarkdown, writeReport };
