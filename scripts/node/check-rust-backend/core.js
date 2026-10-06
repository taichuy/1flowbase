const fs = require('node:fs');
const path = require('node:path');
const { maskRustText, maskTestOnlyItems } = require('./source.js');
const { sensitiveOccurrences } = require('./sensitive.js');

const REPORT_FILE = 'rust-backend-static-gate.json';
const PRODUCTION_ESCAPE_PATTERNS = [
  { name: 'unwrap', pattern: /\.unwrap\s*\(/u },
  { name: 'panic', pattern: /\bpanic!\s*\(/u },
  { name: 'dbg', pattern: /\bdbg!\s*\(/u },
  { name: 'todo', pattern: /\btodo!\s*\(/u },
  { name: 'unimplemented', pattern: /\bunimplemented!\s*\(/u },
];
const BLOCKING_PATTERNS = [
  /\bstd::fs::(?:read|read_to_string|write|create_dir_all|metadata|set_permissions)\s*\(/u,
  /\bstd::thread::sleep\s*\(/u,
  /\breqwest::blocking\b/u,
];

function normalizePath(filePath) {
  return filePath.split(path.sep).join('/');
}

function getRepoRoot() {
  return path.resolve(__dirname, '..', '..', '..');
}

function isRustTestPath(relativePath) {
  return /(?:^|\/)(?:_tests|tests|benches)\//u.test(relativePath)
    || /(?:^|\/)_?tests\.rs$/u.test(relativePath);
}

function isSkippedRustPath(relativePath) {
  return isRustTestPath(relativePath) || relativePath.startsWith('api/plugins/installed/');
}

function walkFiles(currentDir, collected = []) {
  if (!fs.existsSync(currentDir)) {
    return collected;
  }

  const entries = fs.readdirSync(currentDir, { withFileTypes: true });

  for (const entry of entries) {
    if (entry.name === 'target') {
      continue;
    }

    const absolutePath = path.join(currentDir, entry.name);

    if (entry.isDirectory()) {
      walkFiles(absolutePath, collected);
      continue;
    }

    if (entry.isFile() && entry.name.endsWith('.rs')) {
      collected.push(absolutePath);
    }
  }

  return collected;
}

function createFinding({ severity, rule, file, line, message, snippet }) {
  return {
    severity,
    rule,
    file,
    line,
    message,
    snippet: snippet.trim(),
  };
}

function scanRustSource({ relativePath, content }) {
  if (isSkippedRustPath(relativePath)) {
    return [];
  }

  const lines = content.split(/\r?\n/u);
  const code = maskTestOnlyItems(maskRustText(content));
  const codeLines = code.split(/\r?\n/u);
  const findings = [];

  codeLines.forEach((line, index) => {
    const lineNumber = index + 1;

    for (const { name, pattern } of PRODUCTION_ESCAPE_PATTERNS) {
      if (pattern.test(line)) {
        findings.push(createFinding({
          severity: 'error',
          rule: 'no-production-escape',
          file: relativePath,
          line: lineNumber,
          message: `production Rust code uses ${name}`,
          snippet: lines[index],
        }));
      }
    }

    if (BLOCKING_PATTERNS.some((pattern) => pattern.test(line))) {
      findings.push(createFinding({
        severity: 'warning',
        rule: 'blocking-in-async-context',
        file: relativePath,
        line: lineNumber,
        message: 'Rust backend code uses blocking IO or blocking sleep; confirm this is outside request async paths',
        snippet: lines[index],
      }));
    }

  });

  for (const { rule, offset } of sensitiveOccurrences(code)) {
    const lineNumber = code.slice(0, offset).split('\n').length;
    findings.push(createFinding({
      severity: 'error', rule, file: relativePath, line: lineNumber,
      message: rule === 'no-sensitive-logging'
        ? 'logging call appears to include sensitive material'
        : 'serialized Rust struct exposes a sensitive field',
      snippet: lines[lineNumber - 1],
    }));
  }

  return findings;
}

function loadBaseline(repoRoot) {
  const baselinePath = path.join(repoRoot, 'scripts', 'node', 'check-rust-backend', 'baseline.json');

  if (!fs.existsSync(baselinePath)) {
    return new Set();
  }

  const parsed = JSON.parse(fs.readFileSync(baselinePath, 'utf8'));
  return new Map(
    (parsed.allowedFindings || []).map((entry) => [findingKey(entry), entry.reason || null])
  );
}

function findingKey(finding) {
  return [
    finding.rule,
    finding.file,
    finding.snippet,
  ].join('\u0000');
}

function applyBaseline(findings, baseline) {
  return findings.map((finding) => {
    const key = findingKey(finding);
    const suppressed = baseline.has(key);
    return {
      ...finding,
      suppressed,
      ...(suppressed && baseline.get(key) ? { suppressionReason: baseline.get(key) } : {}),
    };
  });
}

function collectRustBackendFindings({ repoRoot = getRepoRoot(), includeSuppressed = false } = {}) {
  const apiRoot = path.join(repoRoot, 'api');
  const baseline = loadBaseline(repoRoot);
  const findings = walkFiles(apiRoot)
    .map((absolutePath) => {
      const relativePath = normalizePath(path.relative(repoRoot, absolutePath));
      return scanRustSource({
        relativePath,
        content: fs.readFileSync(absolutePath, 'utf8'),
      });
    })
    .flat();
  const annotated = applyBaseline(findings, baseline);

  if (includeSuppressed) {
    return annotated;
  }

  return annotated.filter((finding) => !finding.suppressed);
}

function resolveReportPath(repoRoot, env = process.env) {
  const outputDir = env.ONEFLOWBASE_WARNING_OUTPUT_DIR
    ? path.resolve(repoRoot, env.ONEFLOWBASE_WARNING_OUTPUT_DIR)
    : path.join(repoRoot, 'tmp', 'test-governance');

  fs.mkdirSync(outputDir, { recursive: true });
  return path.join(outputDir, REPORT_FILE);
}

function formatFinding(finding) {
  return `${finding.file}:${finding.line} ${finding.rule} ${finding.message}`;
}

async function main(_argv = [], deps = {}) {
  const repoRoot = deps.repoRoot || getRepoRoot();
  const env = deps.env || process.env;
  const writeStdout = deps.writeStdout || ((text) => process.stdout.write(text));
  const writeStderr = deps.writeStderr || ((text) => process.stderr.write(text));
  const findings = collectRustBackendFindings({ repoRoot, includeSuppressed: true });
  const activeFindings = findings.filter((finding) => !finding.suppressed);
  const errors = activeFindings.filter((finding) => finding.severity === 'error');
  const warnings = activeFindings.filter((finding) => finding.severity === 'warning');
  const suppressed = findings.filter((finding) => finding.suppressed);
  const report = {
    summary: {
      errors: errors.length,
      warnings: warnings.length,
      suppressed: suppressed.length,
    },
    findings,
  };

  fs.writeFileSync(resolveReportPath(repoRoot, env), `${JSON.stringify(report, null, 2)}\n`, 'utf8');

  if (warnings.length > 0) {
    writeStdout(`[rust-backend-static-gate] warnings=${warnings.length}; report=${REPORT_FILE}\n`);
  }

  if (errors.length > 0) {
    writeStderr(
      errors
        .slice(0, 20)
        .map(formatFinding)
        .join('\n') + '\n'
    );
    return 1;
  }

  writeStdout(`[rust-backend-static-gate] passed; warnings=${warnings.length}; suppressed=${suppressed.length}\n`);
  return 0;
}

module.exports = {
  collectRustBackendFindings,
  main,
  scanRustSource,
};
