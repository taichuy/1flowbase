const fs = require('node:fs');
const path = require('node:path');

const { getRepoRoot } = require('../testing/warning-capture.js');

const OUTPUT_ROOT = path.join('tmp', 'test-governance');
const JSON_REPORT_FILE = 'log-query-contract-report.json';
const MARKDOWN_REPORT_FILE = 'log-query-contract-report.md';
const DEFAULT_CONFIG_FILE = path.join('scripts', 'node', 'log-query-contract-report', 'config.json');
const CONTRACT_DIMENSIONS = ['scope', 'time', 'cursor', 'limit'];

function normalizePath(filePath) {
  return filePath.split(path.sep).join('/');
}

function loadConfig(repoRoot = getRepoRoot(), configPath = DEFAULT_CONFIG_FILE) {
  const absolutePath = path.isAbsolute(configPath) ? configPath : path.join(repoRoot, configPath);
  return JSON.parse(fs.readFileSync(absolutePath, 'utf8'));
}

function resolveSource(repoRoot, filePath) {
  const absolutePath = path.isAbsolute(filePath) ? filePath : path.join(repoRoot, filePath);
  if (!fs.existsSync(absolutePath)) {
    return {
      file: normalizePath(filePath),
      exists: false,
      text: '',
    };
  }

  return {
    file: normalizePath(filePath),
    exists: true,
    text: fs.readFileSync(absolutePath, 'utf8'),
  };
}

function findFunctionMatch(source, functionName) {
  if (!source.exists || !functionName) {
    return null;
  }

  const pattern = new RegExp(`\\b(?:pub\\s+)?(?:async\\s+)?fn\\s+${functionName}\\s*\\(`, 'u');
  return pattern.exec(source.text);
}

function functionContext(source, functionName) {
  const match = findFunctionMatch(source, functionName);
  if (!match) {
    return '';
  }

  return source.text.slice(
    Math.max(0, match.index - 1200),
    Math.min(source.text.length, match.index + 9000)
  );
}

function includeStringContext(repoRoot, sourceSpec, context) {
  if (!sourceSpec.includeStr) {
    return '';
  }

  const escapedPath = sourceSpec.includeStr.replace(/[.*+?^${}()|[\]\\]/gu, '\\$&');
  const includePattern = new RegExp(`\\binclude_str!\\s*\\(\\s*"${escapedPath}"\\s*\\)`, 'u');
  if (!includePattern.test(context)) {
    return '';
  }

  const includedPath = path.join(path.dirname(sourceSpec.file || ''), sourceSpec.includeStr);
  return resolveSource(repoRoot, includedPath).text;
}

function regexMatches(text, pattern) {
  return new RegExp(pattern, 'u').test(text);
}

function evaluateDimension({ dimension, spec, context }) {
  const patterns = spec.patterns || [];
  const matchedPatterns = patterns.filter((pattern) => regexMatches(context, pattern));
  const missingPatterns = patterns.filter((pattern) => !matchedPatterns.includes(pattern));
  const present = patterns.length > 0 && missingPatterns.length === 0;
  const exemption = spec.exemption || null;

  if (present && !exemption) {
    return {
      dimension,
      status: 'compliant',
      description: spec.description || '',
      matchedPatterns,
      missingPatterns: [],
      exemption: null,
    };
  }

  if (present && exemption) {
    return {
      dimension,
      status: 'exempted',
      description: spec.description || '',
      matchedPatterns,
      missingPatterns: [],
      exemption,
    };
  }

  if (exemption && exemption.reason && exemption.removeBy) {
    return {
      dimension,
      status: 'exempted',
      description: spec.description || '',
      matchedPatterns,
      missingPatterns,
      exemption,
    };
  }

  return {
    dimension,
    status: 'failed',
    description: spec.description || '',
    matchedPatterns,
    missingPatterns,
    exemption,
  };
}

function endpointContext(repoRoot, endpoint) {
  const apiSource = resolveSource(repoRoot, endpoint.api?.file || '');
  const repositorySource = resolveSource(repoRoot, endpoint.repository?.file || '');
  const apiContext = functionContext(apiSource, endpoint.api?.functionName);
  const repositoryContext = functionContext(repositorySource, endpoint.repository?.functionName);
  const extraSources = (endpoint.extraSources || []).map((sourceSpec) => {
    const source = resolveSource(repoRoot, sourceSpec.file || '');
    const functionName = sourceSpec.functionName || '';
    const context = functionContext(source, functionName);
    const includedContext = includeStringContext(repoRoot, sourceSpec, context);
    return {
      source,
      functionName,
      functionFound: Boolean(findFunctionMatch(source, functionName)),
      context: [context, includedContext].filter(Boolean).join('\n'),
    };
  });

  return {
    apiSource,
    repositorySource,
    apiFunctionName: endpoint.api?.functionName || '',
    repositoryFunctionName: endpoint.repository?.functionName || '',
    apiFunctionFound: Boolean(findFunctionMatch(apiSource, endpoint.api?.functionName)),
    repositoryFunctionFound: Boolean(findFunctionMatch(repositorySource, endpoint.repository?.functionName)),
    extraSources,
    text: [apiContext, repositoryContext, ...extraSources.map((source) => source.context)].join('\n'),
  };
}

function endpointFinding(endpoint, dimension, message) {
  return {
    endpointId: endpoint.id,
    dimension,
    severity: 'fail',
    message,
  };
}

function evaluateEndpoint({ repoRoot, endpoint }) {
  const context = endpointContext(repoRoot, endpoint);
  const dimensions = CONTRACT_DIMENSIONS.map((dimension) => {
    const spec = endpoint.contract?.[dimension] || {};
    return evaluateDimension({ dimension, spec, context: context.text });
  });
  const findings = [];

  if (!context.apiSource.exists) {
    findings.push(endpointFinding(endpoint, 'source', `API source file is missing: ${context.apiSource.file}`));
  } else if (context.apiFunctionName && !context.apiFunctionFound) {
    findings.push(endpointFinding(endpoint, 'source', `API function is missing: ${context.apiFunctionName}`));
  }
  if (!context.repositorySource.exists) {
    findings.push(endpointFinding(endpoint, 'source', `Repository source file is missing: ${context.repositorySource.file}`));
  } else if (context.repositoryFunctionName && !context.repositoryFunctionFound) {
    findings.push(endpointFinding(endpoint, 'source', `Repository function is missing: ${context.repositoryFunctionName}`));
  }
  for (const extraSource of context.extraSources) {
    if (!extraSource.source.exists) {
      findings.push(endpointFinding(endpoint, 'source', `Extra source file is missing: ${extraSource.source.file}`));
    } else if (extraSource.functionName && !extraSource.functionFound) {
      findings.push(endpointFinding(endpoint, 'source', `Extra source function is missing: ${extraSource.functionName}`));
    }
  }
  for (const dimension of dimensions) {
    if (dimension.status === 'failed') {
      findings.push(endpointFinding(
        endpoint,
        dimension.dimension,
        `${endpoint.id} is missing ${dimension.dimension} query contract evidence`
      ));
    }
    if (dimension.status === 'exempted' && (!dimension.exemption?.reason || !dimension.exemption?.removeBy)) {
      findings.push(endpointFinding(
        endpoint,
        dimension.dimension,
        `${endpoint.id} ${dimension.dimension} exemption must include reason and removeBy`
      ));
    }
  }

  const status = findings.length > 0 ? 'failed' : (
    dimensions.some((dimension) => dimension.status === 'exempted') ? 'exempted' : 'compliant'
  );

  return {
    id: endpoint.id,
    category: endpoint.category || 'log_query',
    method: endpoint.method || 'internal',
    path: endpoint.path || '',
    api: endpoint.api || null,
    repository: endpoint.repository || null,
    dimensions,
    status,
    reviewStatus: status === 'failed' ? 'needs-fix' : status,
    findings,
  };
}

function buildSummary(endpoints) {
  const dimensions = endpoints.flatMap((endpoint) => endpoint.dimensions);

  return {
    endpoints: endpoints.length,
    compliant: endpoints.filter((endpoint) => endpoint.status === 'compliant').length,
    exempted: endpoints.filter((endpoint) => endpoint.status === 'exempted').length,
    failed: endpoints.filter((endpoint) => endpoint.status === 'failed').length,
    needsFix: endpoints.filter((endpoint) => endpoint.reviewStatus === 'needs-fix').length,
    dimensionFailures: dimensions.filter((dimension) => dimension.status === 'failed').length,
    dimensionExemptions: dimensions.filter((dimension) => dimension.status === 'exempted').length,
    findings: endpoints.reduce((total, endpoint) => total + endpoint.findings.length, 0),
  };
}

function collectLogQueryContractReport({
  repoRoot = getRepoRoot(),
  config,
} = {}) {
  const effectiveConfig = config || loadConfig(repoRoot);
  const endpoints = (effectiveConfig.endpoints || []).map((endpoint) => evaluateEndpoint({
    repoRoot,
    endpoint,
  }));
  const summary = buildSummary(endpoints);

  return {
    version: 'log-query-contract-report/v1',
    status: summary.findings > 0 ? 'failed' : 'passed',
    exitCode: summary.findings > 0 ? 1 : 0,
    defaultPolicy: effectiveConfig.defaultPolicy || {},
    summary,
    endpoints,
  };
}

function formatList(items) {
  return items.length === 0 ? '-' : items.map((item) => `\`${item}\``).join(', ');
}

function formatLogQueryContractMarkdown(report) {
  const endpointRows = report.endpoints.map((endpoint) => {
    const dimensionSummary = endpoint.dimensions
      .map((dimension) => `${dimension.dimension}:${dimension.status}`)
      .join(', ');
    return `| \`${endpoint.id}\` | ${endpoint.status} | ${endpoint.reviewStatus} | ${endpoint.category} | ${endpoint.method} | \`${endpoint.path}\` | ${dimensionSummary} |`;
  });
  const dimensionRows = report.endpoints.flatMap((endpoint) => (
    endpoint.dimensions.map((dimension) => (
      `| \`${endpoint.id}\` | ${dimension.dimension} | ${dimension.status} | ${dimension.description || '-'} | `
        + `${dimension.exemption?.reason || '-'} | ${dimension.exemption?.removeBy || '-'} | `
        + `${formatList(dimension.missingPatterns)} |`
    ))
  ));
  const findingRows = report.endpoints.flatMap((endpoint) => (
    endpoint.findings.map((finding) => (
      `| \`${endpoint.id}\` | ${finding.dimension} | ${finding.severity} | ${finding.message} |`
    ))
  ));

  return [
    '# Log Query Contract Report',
    '',
    '## Summary',
    '',
    `- Status: ${report.status}`,
    `- Endpoints: ${report.summary.endpoints}`,
    `- Compliant: ${report.summary.compliant}`,
    `- Exempted: ${report.summary.exempted}`,
    `- Failed: ${report.summary.failed}`,
    `- Needs fix: ${report.summary.needsFix}`,
    `- Dimension failures: ${report.summary.dimensionFailures}`,
    `- Dimension exemptions: ${report.summary.dimensionExemptions}`,
    `- Findings: ${report.summary.findings}`,
    '',
    '## Endpoints',
    '',
    '| Endpoint | Gate status | Review status | Category | Method | Path | Dimensions |',
    '| --- | --- | --- | --- | --- | --- | --- |',
    ...endpointRows,
    '',
    '## Dimensions',
    '',
    '| Endpoint | Dimension | Status | Description | Exemption reason | Remove by | Missing patterns |',
    '| --- | --- | --- | --- | --- | --- | --- |',
    ...dimensionRows,
    '',
    '## Findings',
    '',
    findingRows.length === 0 ? 'No unbounded log query findings.' : null,
    findingRows.length > 0 ? '| Endpoint | Dimension | Severity | Message |' : null,
    findingRows.length > 0 ? '| --- | --- | --- | --- |' : null,
    ...findingRows,
    '',
  ].filter((line) => line !== null).join('\n');
}

function writeLogQueryContractReports({
  repoRoot = getRepoRoot(),
  config,
  outputRoot = OUTPUT_ROOT,
} = {}) {
  const report = collectLogQueryContractReport({ repoRoot, config });
  const outputDir = path.join(repoRoot, outputRoot);
  fs.mkdirSync(outputDir, { recursive: true });
  const reportPath = path.join(outputDir, JSON_REPORT_FILE);
  const markdownPath = path.join(outputDir, MARKDOWN_REPORT_FILE);
  fs.writeFileSync(reportPath, `${JSON.stringify(report, null, 2)}\n`, 'utf8');
  fs.writeFileSync(markdownPath, `${formatLogQueryContractMarkdown(report)}\n`, 'utf8');

  return {
    report,
    reportPath,
    markdownPath,
  };
}

function parseCliArgs(argv = []) {
  if (argv.includes('-h') || argv.includes('--help')) {
    return { help: true };
  }

  return { help: false };
}

function usage(writeStdout = (text) => process.stdout.write(text)) {
  writeStdout('Usage: node scripts/node/tooling.js log-query-contract-report\n');
}

async function main(argv = [], deps = {}) {
  const options = parseCliArgs(argv);
  if (options.help) {
    usage(deps.writeStdout);
    return 0;
  }

  const repoRoot = deps.repoRoot || getRepoRoot();
  const result = writeLogQueryContractReports({ repoRoot, config: deps.config });
  const writeStdout = deps.writeStdout || ((text) => process.stdout.write(text));
  writeStdout(
    `[1flowbase-log-query-contract-report] ${result.report.status} `
      + `(endpoints ${result.report.summary.endpoints}, failed ${result.report.summary.failed}, `
      + `exempted ${result.report.summary.exempted}, findings ${result.report.summary.findings}). `
      + `Reports: ${path.relative(repoRoot, result.reportPath)}, ${path.relative(repoRoot, result.markdownPath)}\n`
  );

  return result.report.exitCode;
}

module.exports = {
  collectLogQueryContractReport,
  formatLogQueryContractMarkdown,
  loadConfig,
  main,
  writeLogQueryContractReports,
};
