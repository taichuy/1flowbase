const fs = require('node:fs');
const path = require('node:path');
const { spawnSync } = require('node:child_process');

const { getRepoRoot } = require('../testing/warning-capture.js');
const {
  closeIssueWithGitHubApi,
  closeStaleOpenQualityGateIssues,
  createIssueWithGitHubApi,
  createIssueWithLabelFallback,
  listOpenQualityGateIssuesWithGitHubApi,
  upsertIssueCommentWithMarker,
} = require('./github-api.js');
const {
  buildGateCommand,
  COVERAGE_BACKEND_COMPONENT_SCOPES,
  DEFAULT_AGGREGATE_SCOPES,
  REPO_BACKEND_COMPONENT_SCOPES,
} = require('./commands.js');
const { BACKEND_CONSISTENCY_TARGETS } = require('../verify/index.js');
const {
  buildContainerImageSecurityMarkdownLines,
  formatContainerImageSecurityComponentLine,
  formatContainerImageSecuritySummaryLine,
  formatContainerImageSecurityVulnerabilityLine,
  readContainerImageSecurityReport,
} = require('./container-image-security-report.js');

const OUTPUT_ROOT = path.join('tmp', 'test-governance');
const BACKEND_CONSISTENCY_TARGET_REPORT_FILE = 'backend-consistency-targets.json';
const SECURITY_RISK_REPORT_FILE = 'security-risk.json';
const VALID_REPORT_TYPES = new Set(['ci', 'cd']);
const PR_REPORT_MARKER = '<!-- 1flowbase-quality-gate-pr-report -->';
const MAX_GATE_OUTPUT_BYTES = 64 * 1024 * 1024;
const MAX_GITHUB_ISSUE_BODY_BYTES = 60 * 1024;
const FAILURE_EXCERPT_MAX_LINES = 80;
const ANSI_CONTROL_SEQUENCE_PATTERN = /\u001b(?:\[[0-?]*[ -/]*[@-~]|\][^\u0007]*(?:\u0007|\u001b\\)|[@-Z\\-_])/gu;

function parseBooleanInput(value) {
  if (value === undefined || value === null || value === '') {
    return false;
  }

  if (value === true || value === 'true') {
    return true;
  }

  if (value === false || value === 'false') {
    return false;
  }

  throw new Error(`Expected boolean input, received: ${value}`);
}

function normalizeReportType(reportType) {
  const normalized = reportType || 'ci';

  if (!VALID_REPORT_TYPES.has(normalized)) {
    throw new Error(`Unknown quality gate report type: ${normalized}`);
  }

  return normalized;
}

function ensureOutputDir(repoRoot) {
  const outputDir = path.join(repoRoot, OUTPUT_ROOT);
  fs.mkdirSync(outputDir, { recursive: true });
  return outputDir;
}

function formatTimestamp(date) {
  const year = date.getUTCFullYear();
  const month = String(date.getUTCMonth() + 1).padStart(2, '0');
  const day = String(date.getUTCDate()).padStart(2, '0');
  const hour = String(date.getUTCHours()).padStart(2, '0');
  const minute = String(date.getUTCMinutes()).padStart(2, '0');

  return `${year}-${month}-${day} ${hour}:${minute}`;
}

function shortShaFromEnv(env) {
  return (env.GITHUB_SHA || 'unknown').slice(0, 7);
}

function buildRunUrl(env) {
  const serverUrl = env.GITHUB_SERVER_URL || 'https://github.com';
  const repository = env.GITHUB_REPOSITORY || '';
  const runId = env.GITHUB_RUN_ID || '';

  if (!repository || !runId) {
    return '';
  }

  return `${serverUrl}/${repository}/actions/runs/${runId}`;
}

function buildIssueTitle({
  reportType,
  timestamp,
  branch,
  shortSha,
  status,
  environment,
}) {
  const typeLabel = reportType.toUpperCase();
  const target = environment || branch || 'unknown';

  return `[Quality Gate][${typeLabel}] ${timestamp} ${target} ${shortSha} ${status}`;
}

function buildIssueLabels({ reportType, status }) {
  return [
    'quality-gate',
    `${reportType}-report`,
    status,
  ];
}

function boundIssueBody(markdown, maxBytes = MAX_GITHUB_ISSUE_BODY_BYTES) {
  const body = String(markdown || '');
  if (Buffer.byteLength(body, 'utf8') <= maxBytes) return body;

  const suffix = [
    '',
    '---',
    'Issue report truncated to GitHub’s body limit. Complete component logs and failure details remain in the workflow artifact.',
    '',
  ].join('\n');
  const prefixBudget = maxBytes - Buffer.byteLength(suffix, 'utf8');
  let low = 0;
  let high = body.length;
  while (low < high) {
    const middle = Math.ceil((low + high) / 2);
    if (Buffer.byteLength(body.slice(0, middle), 'utf8') <= prefixBudget) low = middle;
    else high = middle - 1;
  }
  if (
    low > 0
    && low < body.length
    && /[\uD800-\uDBFF]/u.test(body[low - 1])
    && /[\uDC00-\uDFFF]/u.test(body[low])
  ) low -= 1;
  return `${body.slice(0, low).trimEnd()}${suffix}`;
}

function listFilesBySuffix(rootDir, suffix) {
  if (!fs.existsSync(rootDir)) {
    return [];
  }

  const collected = [];
  const walk = (currentDir) => {
    for (const entry of fs.readdirSync(currentDir, { withFileTypes: true })) {
      const absolutePath = path.join(currentDir, entry.name);

      if (entry.isDirectory()) {
        walk(absolutePath);
        continue;
      }

      if (entry.isFile() && entry.name.endsWith(suffix)) {
        collected.push(absolutePath);
      }
    }
  };

  walk(rootDir);
  return collected.sort();
}

function readFailureExcerpt(logPath) {
  if (!fs.existsSync(logPath)) {
    return '';
  }

  const lines = stripAnsiControlSequences(fs.readFileSync(logPath, 'utf8'))
    .trimEnd()
    .split(/\r?\n/u);
  return selectFailureExcerpt(lines).trim();
}

function stripAnsiControlSequences(value) {
  return value.replace(ANSI_CONTROL_SEQUENCE_PATTERN, '');
}

function selectFailureExcerpt(lines) {
  const rustFailuresIndex = lines.findIndex((line) => line.trim() === 'failures:');
  if (rustFailuresIndex >= 0) {
    return excerptFromAnchorWithSummary({
      lines,
      anchorIndex: rustFailuresIndex,
      summaryIndex: lines.findIndex((line) => /test result: FAILED/u.test(line)),
    });
  }

  const panicIndex = lines.findIndex((line) => /\bpanicked at\b/u.test(line));
  if (panicIndex >= 0) {
    return excerptFromAnchorWithSummary({
      lines,
      anchorIndex: Math.max(0, panicIndex - 2),
      summaryIndex: lines.findIndex((line) => /test result: FAILED/u.test(line)),
    });
  }

  return lines.slice(-FAILURE_EXCERPT_MAX_LINES).join('\n');
}

function excerptFromAnchorWithSummary({ lines, anchorIndex, summaryIndex }) {
  const summaryWillBeAppended = summaryIndex >= anchorIndex + FAILURE_EXCERPT_MAX_LINES;
  const bodyLineBudget = summaryWillBeAppended
    ? FAILURE_EXCERPT_MAX_LINES - 2
    : FAILURE_EXCERPT_MAX_LINES;
  const endIndex = Math.min(lines.length, anchorIndex + bodyLineBudget);
  const excerptLines = lines.slice(anchorIndex, endIndex);

  if (summaryWillBeAppended) {
    excerptLines.push('...');
    excerptLines.push(lines[summaryIndex]);
  }

  return excerptLines.join('\n');
}

function toRepoRelative(repoRoot, filePath) {
  return path.relative(repoRoot, filePath).replace(/\\/gu, '/');
}

function readJsonFileIfPresent(filePath) {
  if (!fs.existsSync(filePath)) {
    return null;
  }

  try {
    return JSON.parse(fs.readFileSync(filePath, 'utf8'));
  } catch {
    return null;
  }
}

function readFrontendMetricPct(summary, metric) {
  const metricSummary = summary?.total?.[metric];
  if (metricSummary && Number.isFinite(metricSummary.total) && metricSummary.total === 0) {
    return null;
  }

  const value = metricSummary?.pct;
  return Number.isFinite(value) ? value : null;
}

function readBackendMetricPct(summary, metric) {
  const metricSummary = summary?.data?.[0]?.totals?.[metric];
  if (metricSummary && Number.isFinite(metricSummary.count) && metricSummary.count === 0) {
    return null;
  }

  const value = metricSummary?.percent;
  return Number.isFinite(value) ? value : null;
}

function formatCoveragePct(value) {
  return value === null ? 'n/a' : `${value.toFixed(2)}%`;
}

function buildCoverageSummaries({ repoRoot, coverageFiles }) {
  return coverageFiles.flatMap((relativePath) => {
    const absolutePath = path.join(repoRoot, relativePath);
    const summary = readJsonFileIfPresent(absolutePath);

    if (!summary) {
      return [{
        name: path.basename(relativePath, '.json'),
        kind: 'unknown',
        path: relativePath,
        metrics: {},
      }];
    }

    if (relativePath.endsWith('coverage/frontend/coverage-summary.json')) {
      return [{
        name: 'frontend total',
        kind: 'frontend',
        path: relativePath,
        metrics: {
          lines: readFrontendMetricPct(summary, 'lines'),
          functions: readFrontendMetricPct(summary, 'functions'),
          statements: readFrontendMetricPct(summary, 'statements'),
          branches: readFrontendMetricPct(summary, 'branches'),
        },
      }];
    }

    if (relativePath.includes('/coverage/backend/')) {
      return [{
        name: path.basename(relativePath, '.json'),
        kind: 'backend',
        path: relativePath,
        metrics: {
          lines: readBackendMetricPct(summary, 'lines'),
          functions: readBackendMetricPct(summary, 'functions'),
          branches: readBackendMetricPct(summary, 'branches'),
          regions: readBackendMetricPct(summary, 'regions'),
        },
      }];
    }

    return [{
      name: path.basename(relativePath, '.json'),
      kind: 'unknown',
      path: relativePath,
      metrics: {},
    }];
  });
}

function formatCoverageSummaryLine(summary) {
  const metricText = Object.entries(summary.metrics)
    .map(([metric, value]) => `${metric} ${formatCoveragePct(value)}`)
    .join(', ');

  return metricText
    ? `- ${summary.name}: ${metricText} (${summary.path})`
    : `- ${summary.name}: see ${summary.path}`;
}

function normalizeSecurityRiskReport({ repoRoot, reportPath, report }) {
  const findings = Array.isArray(report?.findings) ? report.findings : [];
  const changedFiles = Array.isArray(report?.changedFiles) ? report.changedFiles : [];

  return {
    status: report?.status || (findings.length > 0 ? 'review_required' : 'advisory'),
    changedFileCount: changedFiles.length,
    findingCount: findings.length,
    highCount: findings.filter((finding) => finding.severity === 'high').length,
    mediumCount: findings.filter((finding) => finding.severity === 'medium').length,
    reportPath: toRepoRelative(repoRoot, reportPath),
  };
}

function readSecurityRiskReport({ repoRoot, outputDir }) {
  const reportPath = path.join(outputDir, SECURITY_RISK_REPORT_FILE);
  const report = readJsonFileIfPresent(reportPath);

  return report
    ? normalizeSecurityRiskReport({ repoRoot, reportPath, report })
    : null;
}

function formatSecurityRiskLine(securityRisk) {
  return `- Status: ${securityRisk.status}`
    + `; Findings: ${securityRisk.findingCount} (high ${securityRisk.highCount}, medium ${securityRisk.mediumCount})`
    + `; Changed files: ${securityRisk.changedFileCount}`;
}

function formatAggregateSecurityRiskLine(securityRisk) {
  const scope = securityRisk.scope || 'unknown';
  return `- ${scope}: ${securityRisk.status}, findings ${securityRisk.findingCount} `
    + `(high ${securityRisk.highCount}, medium ${securityRisk.mediumCount}), `
    + `changed files ${securityRisk.changedFileCount}`;
}

function normalizeBackendConsistencyTarget(target) {
  return {
    label: target.label,
    packageName: target.packageName,
    filter: target.filter,
    status: target.status || 'not_run',
    exitCode: Number.isFinite(target.exitCode) ? target.exitCode : null,
    durationMs: Number.isFinite(target.durationMs) ? target.durationMs : null,
    passedCount: Number.isFinite(target.passedCount) ? target.passedCount : null,
    failedCount: Number.isFinite(target.failedCount) ? target.failedCount : null,
  };
}

function readBackendConsistencyTargetReport(repoRoot) {
  const report = readJsonFileIfPresent(path.join(
    repoRoot,
    OUTPUT_ROOT,
    BACKEND_CONSISTENCY_TARGET_REPORT_FILE
  ));

  return Array.isArray(report?.targets)
    ? report.targets.map(normalizeBackendConsistencyTarget)
    : [];
}

function buildBackendConsistencyTargets({ repoRoot, scope }) {
  if (scope !== 'ci' && scope !== 'backend-consistency') {
    return [];
  }

  const targetReport = readBackendConsistencyTargetReport(repoRoot);

  if (targetReport.length > 0) {
    return targetReport;
  }

  return BACKEND_CONSISTENCY_TARGETS.map(normalizeBackendConsistencyTarget);
}

function formatDurationMs(durationMs) {
  return durationMs === null ? 'n/a' : `${(durationMs / 1000).toFixed(2)}s`;
}

function formatBackendConsistencyTargetLine(target) {
  return `| \`${target.label}\` | \`${target.packageName}\` | \`${target.filter}\` | `
    + `${target.status} | ${formatDurationMs(target.durationMs)} | `
    + `${target.passedCount ?? 'n/a'} | ${target.failedCount ?? 'n/a'} |`;
}

function buildReport({
  repoRoot,
  reportType,
  scope,
  status,
  exitCode,
  issueUrl,
  environmentName,
  timestamp,
  env,
}) {
  const outputDir = path.join(repoRoot, OUTPUT_ROOT);
  const logPath = path.join(outputDir, 'quality-gate.latest.log');
  const warningFiles = listFilesBySuffix(outputDir, '.warnings.log')
    .map((filePath) => toRepoRelative(repoRoot, filePath));
  const coverageFiles = listFilesBySuffix(path.join(outputDir, 'coverage'), '.json')
    .map((filePath) => toRepoRelative(repoRoot, filePath));
  const coverageSummaries = buildCoverageSummaries({ repoRoot, coverageFiles });
  const securityRisk = readSecurityRiskReport({ repoRoot, outputDir });
  const containerImageSecurity = readContainerImageSecurityReport({ repoRoot, outputDir });
  const backendConsistencyTargets = buildBackendConsistencyTargets({ repoRoot, scope });
  const runUrl = buildRunUrl(env);
  const shortSha = shortShaFromEnv(env);
  const failureExcerpt = status === 'failed' ? readFailureExcerpt(logPath) : '';

  const report = {
    reportType,
    status,
    scope,
    exitCode,
    branch: env.GITHUB_REF_NAME || '',
    commit: env.GITHUB_SHA || '',
    shortSha,
    actor: env.GITHUB_ACTOR || '',
    workflow: env.GITHUB_WORKFLOW || '',
    runUrl,
    environment: environmentName || '',
    timestamp,
    issueUrl,
    logPath: toRepoRelative(repoRoot, logPath),
    warningFiles,
    coverageFiles,
    coverageSummaries,
    securityRisk,
    containerImageSecurity,
    backendConsistencyTargets,
  };

  const markdown = [
    '# Quality Gate Report',
    '',
    '## Result Summary',
    '',
    `- Type: ${reportType.toUpperCase()}`,
    `- Status: ${report.status}`,
    `- Exit code: ${report.exitCode}`,
    `- Scope: ${scope}`,
    environmentName ? `- Environment: ${environmentName}` : null,
    `- Branch: ${report.branch || 'unknown'}`,
    `- Commit: ${report.commit || 'unknown'}`,
    `- Actor: ${report.actor || 'unknown'}`,
    runUrl ? `- Run: ${runUrl}` : null,
    '',
    '## Warnings',
    '',
    warningFiles.length === 0 ? 'No warning logs were captured.' : null,
    ...warningFiles.map((filePath) => `- ${filePath}`),
    '',
    '## Coverage',
    '',
    coverageSummaries.length === 0 ? 'No coverage summaries were captured for this scope.' : null,
    ...coverageSummaries.map(formatCoverageSummaryLine),
    '',
    '## Security Risk',
    '',
    securityRisk ? formatSecurityRiskLine(securityRisk) : 'No security risk report was captured for this scope.',
    securityRisk ? `- Report: ${securityRisk.reportPath}` : null,
    '',
    ...(containerImageSecurity
      ? [
        '## Container Image Security',
        '',
        formatContainerImageSecuritySummaryLine(containerImageSecurity),
        '',
        '| Component | Status | HIGH | CRITICAL | Image |',
        '| --- | --- | ---: | ---: | --- |',
        ...containerImageSecurity.components.map(formatContainerImageSecurityComponentLine),
        '',
        ...((containerImageSecurity.components || []).some((component) => (
          (component.topVulnerabilities || []).length > 0
        ))
          ? [
            '### Top Vulnerabilities',
            '',
            '| Component | Severity | Vulnerability | Package | Installed | Fixed |',
            '| --- | --- | --- | --- | --- | --- |',
            ...containerImageSecurity.components.flatMap((component) => (
              (component.topVulnerabilities || []).slice(0, 5).map((vulnerability) => (
                formatContainerImageSecurityVulnerabilityLine({ component, vulnerability })
              ))
            )),
            '',
          ]
          : []),
      ]
      : [
        '## Container Image Security',
        '',
        'No container image security report was captured for this scope.',
        '',
      ]),
    backendConsistencyTargets.length > 0 ? '## Backend Consistency Targets' : null,
    backendConsistencyTargets.length > 0 ? '' : null,
    backendConsistencyTargets.length > 0 ? '| Label | Package | Rust test filter | Status | Duration | Passed | Failed |' : null,
    backendConsistencyTargets.length > 0 ? '| --- | --- | --- | --- | ---: | ---: | ---: |' : null,
    ...backendConsistencyTargets.map(formatBackendConsistencyTargetLine),
    '',
    '## Evidence',
    '',
    `- Main log: ${report.logPath}`,
    '- Artifact: test-governance-artifacts',
    ...warningFiles.map((filePath) => `- Warning log: ${filePath}`),
    ...coverageFiles.map((filePath) => `- Coverage summary file: ${filePath}`),
    securityRisk ? `- Security risk report: ${securityRisk.reportPath}` : null,
    containerImageSecurity ? `- Container image security report: ${containerImageSecurity.markdownPath}` : null,
    containerImageSecurity ? `- Container image security JSON: ${containerImageSecurity.reportPath}` : null,
    failureExcerpt ? '' : null,
    failureExcerpt ? '## Failure Excerpt' : null,
    failureExcerpt ? '' : null,
    failureExcerpt ? '```text' : null,
    failureExcerpt || null,
    failureExcerpt ? '```' : null,
  ].filter((line) => line !== null).join('\n');

  return {
    markdown: `${markdown}\n`,
    json: report,
  };
}

function readQualityGateArtifactReports({ repoRoot, artifactRoot }) {
  const absoluteArtifactRoot = path.isAbsolute(artifactRoot)
    ? artifactRoot
    : path.join(repoRoot, artifactRoot);

  if (!fs.existsSync(absoluteArtifactRoot)) {
    return [];
  }

  const reports = listFilesBySuffix(absoluteArtifactRoot, 'quality-gate-report.json')
    .map((reportPath) => {
      const report = readJsonFileIfPresent(reportPath);

      if (!report) {
        return null;
      }

      const artifactPath = path.dirname(reportPath);
      const artifactName = path.relative(absoluteArtifactRoot, artifactPath).split(path.sep)[0]
        || path.basename(artifactPath);
      const scope = report.scope || artifactName.replace(/^test-governance-/u, '');

      return {
        artifactName,
        artifactPath,
        reportPath,
        scope,
        report,
      };
    })
    .filter(Boolean);

  return reports.sort((left, right) => left.scope.localeCompare(right.scope));
}

function dedupeBy(items, keyForItem) {
  const seen = new Set();
  const deduped = [];

  for (const item of items) {
    const key = keyForItem(item);

    if (seen.has(key)) {
      continue;
    }

    seen.add(key);
    deduped.push(item);
  }

  return deduped;
}

function normalizeComponentReport({ repoRoot, artifact }) {
  const exitCode = Number.isFinite(artifact.report.exitCode) ? artifact.report.exitCode : 1;
  const status = artifact.report.status === 'passed' && exitCode === 0 ? 'passed' : 'failed';
  const logPath = artifact.artifactPath
    ? path.join(artifact.artifactPath, 'quality-gate.latest.log')
    : '';

  return {
    artifactName: artifact.artifactName,
    scope: artifact.scope,
    status,
    exitCode,
    reportPath: artifact.reportPath ? toRepoRelative(repoRoot, artifact.reportPath) : '',
    logPath: fs.existsSync(logPath) ? toRepoRelative(repoRoot, logPath) : '',
    failureExcerpt: status === 'failed'
      ? (artifact.missing ? `No quality gate artifact was downloaded for scope: ${artifact.scope}` : readFailureExcerpt(logPath))
      : '',
  };
}

function addMissingAggregateScopes({ componentArtifacts, expectedScopes }) {
  const seenScopes = new Set(componentArtifacts.map((artifact) => artifact.scope));
  const missingArtifacts = expectedScopes
    .filter((scope) => !seenScopes.has(scope))
    .map((scope) => ({
      artifactName: `missing-${scope}`,
      artifactPath: '',
      reportPath: '',
      scope,
      missing: true,
      report: {
        status: 'failed',
        scope,
        exitCode: 1,
        coverageSummaries: [],
        backendConsistencyTargets: [],
        warningFiles: [],
        securityRisk: null,
        containerImageSecurity: null,
      },
    }));

  return [...componentArtifacts, ...missingArtifacts]
    .sort((left, right) => left.scope.localeCompare(right.scope));
}

function buildAggregateReport({
  repoRoot,
  reportType,
  componentArtifacts,
  issueUrl,
  environmentName,
  timestamp,
  env,
}) {
  const components = componentArtifacts.map((artifact) => normalizeComponentReport({ repoRoot, artifact }));
  const warningFiles = dedupeBy(
    componentArtifacts.flatMap((artifact) => artifact.report.warningFiles || []),
    (filePath) => filePath
  );
  const status = components.every((component) => component.status === 'passed')
    ? 'passed'
    : 'failed';
  const exitCode = status === 'passed'
    ? 0
    : components.find((component) => component.exitCode !== 0)?.exitCode || 1;
  const coverageSummaries = dedupeBy(
    componentArtifacts.flatMap((artifact) => artifact.report.coverageSummaries || []),
    (summary) => `${summary.kind || 'unknown'}:${summary.name}:${summary.path}`
  );
  const securityRiskReports = dedupeBy(
    componentArtifacts
      .flatMap((artifact) => (artifact.report.securityRisk
        ? [{ ...artifact.report.securityRisk, scope: artifact.scope }]
        : [])),
    (securityRisk) => `${securityRisk.scope}:${securityRisk.reportPath}`
  );
  const containerImageSecurityReports = dedupeBy(
    componentArtifacts
      .flatMap((artifact) => (artifact.report.containerImageSecurity
        ? [{ ...artifact.report.containerImageSecurity, scope: artifact.scope }]
        : [])),
    (containerImageSecurity) => `${containerImageSecurity.scope}:${containerImageSecurity.reportPath}`
  );
  const backendConsistencyTargets = componentArtifacts.flatMap(
    (artifact) => artifact.report.backendConsistencyTargets || []
  );
  const runUrl = buildRunUrl(env);
  const shortSha = shortShaFromEnv(env);
  const outputDir = path.join(repoRoot, OUTPUT_ROOT);
  const logPath = path.join(outputDir, 'quality-gate.latest.log');

  const report = {
    reportType,
    status,
    scope: 'ci',
    exitCode,
    branch: env.GITHUB_REF_NAME || '',
    commit: env.GITHUB_SHA || '',
    shortSha,
    actor: env.GITHUB_ACTOR || '',
    workflow: env.GITHUB_WORKFLOW || '',
    runUrl,
    environment: environmentName || '',
    timestamp,
    issueUrl,
    logPath: toRepoRelative(repoRoot, logPath),
    warningFiles,
    coverageFiles: coverageSummaries.map((summary) => summary.path).filter(Boolean),
    coverageSummaries,
    securityRiskReports,
    containerImageSecurityReports,
    backendConsistencyTargets,
    components,
  };

  const failedComponents = components.filter((component) => component.status !== 'passed');
  const markdown = [
    '# Quality Gate Report',
    '',
    '## Result Summary',
    '',
    `- Type: ${reportType.toUpperCase()}`,
    `- Status: ${status}`,
    `- Exit code: ${exitCode}`,
    '- Scope: ci',
    environmentName ? `- Environment: ${environmentName}` : null,
    `- Branch: ${report.branch || 'unknown'}`,
    `- Commit: ${report.commit || 'unknown'}`,
    `- Actor: ${report.actor || 'unknown'}`,
    runUrl ? `- Run: ${runUrl}` : null,
    '',
    '## Component Results',
    '',
    '| Scope | Status | Exit code | Artifact |',
    '| --- | --- | ---: | --- |',
    ...components.map((component) => (
      `| \`${component.scope}\` | ${component.status} | ${component.exitCode} | `
        + `\`${component.artifactName}\` |`
    )),
    '',
    '## Warnings',
    '',
    warningFiles.length === 0 ? 'No warning logs were captured.' : null,
    ...warningFiles.map((filePath) => `- ${filePath}`),
    '',
    '## Coverage',
    '',
    coverageSummaries.length === 0 ? 'No coverage summaries were captured for this scope.' : null,
    ...coverageSummaries.map(formatCoverageSummaryLine),
    '',
    '## Security Risk',
    '',
    securityRiskReports.length === 0 ? 'No security risk reports were captured.' : null,
    ...securityRiskReports.map(formatAggregateSecurityRiskLine),
    '',
    ...buildContainerImageSecurityMarkdownLines(containerImageSecurityReports),
    backendConsistencyTargets.length > 0 ? '## Backend Consistency Targets' : null,
    backendConsistencyTargets.length > 0 ? '' : null,
    backendConsistencyTargets.length > 0 ? '| Label | Package | Rust test filter | Status | Duration | Passed | Failed |' : null,
    backendConsistencyTargets.length > 0 ? '| --- | --- | --- | --- | ---: | ---: | ---: |' : null,
    ...backendConsistencyTargets.map(formatBackendConsistencyTargetLine),
    '',
    '## Evidence',
    '',
    `- Main log: ${report.logPath}`,
    '- Artifact: test-governance-artifacts',
    ...components.flatMap((component) => [
      component.reportPath ? `- Component report: ${component.reportPath}` : null,
      component.logPath ? `- Component log: ${component.logPath}` : null,
    ]),
    ...warningFiles.map((filePath) => `- Warning log: ${filePath}`),
    ...coverageSummaries.map((summary) => `- Coverage summary file: ${summary.path}`),
    ...securityRiskReports.map((securityRisk) => `- Security risk report: ${securityRisk.reportPath}`),
    ...containerImageSecurityReports.flatMap((containerImageSecurity) => [
      containerImageSecurity.markdownPath
        ? `- Container image security report: ${containerImageSecurity.markdownPath}`
        : null,
      containerImageSecurity.reportPath
        ? `- Container image security JSON: ${containerImageSecurity.reportPath}`
        : null,
    ]),
    ...failedComponents.flatMap((component) => (
      component.failureExcerpt
        ? [
          '',
          `## Failure Excerpt: ${component.scope}`,
          '',
          '```text',
          component.failureExcerpt,
          '```',
        ]
        : []
    )),
  ].filter((line) => line !== null).join('\n');

  return {
    markdown: `${markdown}\n`,
    json: report,
  };
}

function writeReports({ repoRoot, report }) {
  const outputDir = ensureOutputDir(repoRoot);
  const markdownPath = path.join(outputDir, 'quality-gate-report.md');
  const jsonPath = path.join(outputDir, 'quality-gate-report.json');

  fs.writeFileSync(markdownPath, report.markdown, 'utf8');
  fs.writeFileSync(jsonPath, `${JSON.stringify(report.json, null, 2)}\n`, 'utf8');

  return { markdownPath, jsonPath };
}

function writeActionOutputs(outputs, outputPath = process.env.GITHUB_OUTPUT) {
  if (!outputPath) {
    return;
  }

  const content = Object.entries(outputs)
    .map(([key, value]) => `${key}=${String(value).replace(/\n/gu, ' ')}`)
    .join('\n');

  fs.appendFileSync(outputPath, `${content}\n`, 'utf8');
}

function appendStepSummary(markdown, summaryPath = process.env.GITHUB_STEP_SUMMARY) {
  if (!summaryPath) {
    return;
  }

  fs.appendFileSync(summaryPath, `\n${markdown}\n`, 'utf8');
}

function buildPullRequestReportComment(markdown) {
  return `${PR_REPORT_MARKER}\n${markdown}`;
}

function runGateCommand({
  commandSpec,
  env,
  logPath,
  spawnSyncImpl = spawnSync,
  writeStdout = (text) => process.stdout.write(text),
  writeStderr = (text) => process.stderr.write(text),
}) {
  const result = spawnSyncImpl(commandSpec.command, commandSpec.args, {
    cwd: commandSpec.cwd,
    env,
    encoding: 'utf8',
    maxBuffer: MAX_GATE_OUTPUT_BYTES,
    stdio: ['inherit', 'pipe', 'pipe'],
  });

  if (result.error) {
    throw result.error;
  }

  const stdout = result.stdout || '';
  const stderr = result.stderr || '';

  if (stdout) {
    writeStdout(stdout);
  }

  if (stderr) {
    writeStderr(stderr);
  }

  fs.writeFileSync(logPath, `${stdout}${stderr}`, 'utf8');

  return result.status ?? 1;
}

async function runQualityGateAggregate({
  repoRoot = getRepoRoot(),
  artifactRoot = path.join(OUTPUT_ROOT, 'parallel'),
  expectedScopes = DEFAULT_AGGREGATE_SCOPES,
  reportType = 'ci',
  publishIssue = false,
  publishPrComment = false,
  prNumber = 0,
  githubToken = '',
  environmentName = '',
  env = process.env,
  nowImpl = () => new Date(),
  createIssueImpl = createIssueWithGitHubApi,
  listOpenQualityGateIssuesImpl = listOpenQualityGateIssuesWithGitHubApi,
  closeIssueImpl = closeIssueWithGitHubApi,
  upsertPrCommentImpl = upsertIssueCommentWithMarker,
  writeStderr = (text) => process.stderr.write(text),
} = {}) {
  const normalizedReportType = normalizeReportType(reportType);
  const outputDir = ensureOutputDir(repoRoot);
  const componentArtifacts = addMissingAggregateScopes({
    componentArtifacts: readQualityGateArtifactReports({ repoRoot, artifactRoot }),
    expectedScopes,
  });
  const timestamp = formatTimestamp(nowImpl());
  const aggregateLogPath = path.join(outputDir, 'quality-gate.latest.log');
  fs.writeFileSync(
    aggregateLogPath,
    componentArtifacts
      .map((artifact) => {
        const componentLogPath = path.join(artifact.artifactPath, 'quality-gate.latest.log');
        const componentLog = fs.existsSync(componentLogPath)
          ? fs.readFileSync(componentLogPath, 'utf8').trimEnd()
          : '';

        return [
          `===== ${artifact.scope} (${artifact.artifactName}) =====`,
          componentLog,
        ].filter(Boolean).join('\n');
      })
      .join('\n\n'),
    'utf8'
  );

  let issueUrl = '';
  const report = buildAggregateReport({
    repoRoot,
    reportType: normalizedReportType,
    componentArtifacts,
    issueUrl,
    environmentName,
    timestamp,
    env,
  });
  const reportPaths = writeReports({ repoRoot, report });

  if (publishIssue) {
    if (!githubToken) {
      throw new Error('github_token is required when publish_issue is true');
    }

    const issue = await createIssueWithLabelFallback({
      createIssueImpl,
      issue: {
        token: githubToken,
        repository: env.GITHUB_REPOSITORY,
        title: buildIssueTitle({
          reportType: normalizedReportType,
          timestamp,
          branch: env.GITHUB_REF_NAME || '',
          shortSha: shortShaFromEnv(env),
          status: report.json.status,
          environment: environmentName,
        }),
        body: boundIssueBody(report.markdown),
        labels: buildIssueLabels({
          reportType: normalizedReportType,
          status: report.json.status,
        }),
      },
    });
    issueUrl = issue.html_url || '';
    await closeStaleOpenQualityGateIssues({
      token: githubToken,
      repository: env.GITHUB_REPOSITORY,
      latestIssue: issue,
      listOpenQualityGateIssuesImpl,
      closeIssueImpl,
    });
  }

  const finalReport = issueUrl
    ? buildAggregateReport({
      repoRoot,
      reportType: normalizedReportType,
      componentArtifacts,
      issueUrl,
      environmentName,
      timestamp,
      env,
    })
    : report;

  if (issueUrl) {
    writeReports({ repoRoot, report: finalReport });
  }

  let prCommentUrl = '';
  if (publishPrComment) {
    if (!githubToken || !prNumber) {
      writeStderr('[1flowbase-quality-gate] skipped PR report comment: github_token and pr_number are required\n');
    } else {
      try {
        const comment = await upsertPrCommentImpl({
          token: githubToken,
          repository: env.GITHUB_REPOSITORY,
          number: prNumber,
          marker: PR_REPORT_MARKER,
          body: buildPullRequestReportComment(finalReport.markdown),
        });
        prCommentUrl = comment.html_url || '';
      } catch (error) {
        writeStderr(`[1flowbase-quality-gate] failed to publish PR report comment: ${error.message}\n`);
      }
    }
  }

  appendStepSummary(finalReport.markdown);
  writeActionOutputs({
    status: finalReport.json.status,
    exit_code: finalReport.json.exitCode,
    report_path: toRepoRelative(repoRoot, reportPaths.markdownPath),
    report_json_path: toRepoRelative(repoRoot, reportPaths.jsonPath),
    issue_url: issueUrl,
    pr_comment_url: prCommentUrl,
  });

  return {
    status: finalReport.json.status,
    exitCode: finalReport.json.exitCode,
    issueUrl,
    prCommentUrl,
    reportPath: reportPaths.markdownPath,
    reportJsonPath: reportPaths.jsonPath,
  };
}

async function runQualityGate({
  repoRoot = getRepoRoot(),
  scope = 'ci',
  reportType = 'ci',
  publishIssue = false,
  githubToken = '',
  environmentName = '',
  env = process.env,
  nowImpl = () => new Date(),
  spawnSyncImpl = spawnSync,
  createIssueImpl = createIssueWithGitHubApi,
  listOpenQualityGateIssuesImpl = listOpenQualityGateIssuesWithGitHubApi,
  closeIssueImpl = closeIssueWithGitHubApi,
  writeStdout = (text) => process.stdout.write(text),
  writeStderr = (text) => process.stderr.write(text),
} = {}) {
  const normalizedReportType = normalizeReportType(reportType);
  const outputDir = ensureOutputDir(repoRoot);
  const logPath = path.join(outputDir, 'quality-gate.latest.log');
  const commandSpec = buildGateCommand({ repoRoot, scope });
  const gateExitCode = runGateCommand({
    commandSpec,
    env,
    logPath,
    spawnSyncImpl,
    writeStdout,
    writeStderr,
  });
  const status = gateExitCode === 0 ? 'passed' : 'failed';
  const timestamp = formatTimestamp(nowImpl());
  let issueUrl = '';

  const report = buildReport({
    repoRoot,
    reportType: normalizedReportType,
    scope,
    status,
    exitCode: gateExitCode,
    issueUrl,
    environmentName,
    timestamp,
    env,
  });
  const reportPaths = writeReports({ repoRoot, report });
  const reportStatus = report.json.status;

  if (publishIssue) {
    if (!githubToken) {
      throw new Error('github_token is required when publish_issue is true');
    }

    const issue = await createIssueWithLabelFallback({
      createIssueImpl,
      issue: {
        token: githubToken,
        repository: env.GITHUB_REPOSITORY,
        title: buildIssueTitle({
          reportType: normalizedReportType,
          timestamp,
          branch: env.GITHUB_REF_NAME || '',
          shortSha: shortShaFromEnv(env),
          status: reportStatus,
          environment: environmentName,
        }),
        body: boundIssueBody(report.markdown),
        labels: buildIssueLabels({
          reportType: normalizedReportType,
          status: reportStatus,
        }),
      },
    });
    issueUrl = issue.html_url || '';
    await closeStaleOpenQualityGateIssues({
      token: githubToken,
      repository: env.GITHUB_REPOSITORY,
      latestIssue: issue,
      listOpenQualityGateIssuesImpl,
      closeIssueImpl,
    });
  }

  const finalReport = issueUrl
    ? buildReport({
      repoRoot,
      reportType: normalizedReportType,
      scope,
      status,
      exitCode: gateExitCode,
      issueUrl,
      environmentName,
      timestamp,
      env,
    })
    : report;

  if (issueUrl) {
    writeReports({ repoRoot, report: finalReport });
  }

  appendStepSummary(finalReport.markdown);
  writeActionOutputs({
    status: finalReport.json.status,
    exit_code: finalReport.json.exitCode,
    report_path: toRepoRelative(repoRoot, reportPaths.markdownPath),
    report_json_path: toRepoRelative(repoRoot, reportPaths.jsonPath),
    issue_url: issueUrl,
  });

  return {
    status: finalReport.json.status,
    exitCode: finalReport.json.exitCode,
    issueUrl,
    reportPath: reportPaths.markdownPath,
    reportJsonPath: reportPaths.jsonPath,
  };
}

module.exports = {
  buildAggregateReport,
  buildGateCommand,
  boundIssueBody,
  buildIssueLabels,
  buildIssueTitle,
  buildReport,
  closeIssueWithGitHubApi,
  COVERAGE_BACKEND_COMPONENT_SCOPES,
  createIssueWithGitHubApi,
  DEFAULT_AGGREGATE_SCOPES,
  listOpenQualityGateIssuesWithGitHubApi,
  parseBooleanInput,
  REPO_BACKEND_COMPONENT_SCOPES,
  runQualityGateAggregate,
  runQualityGate,
  upsertIssueCommentWithMarker,
};
