const fs = require('node:fs');
const { createHash } = require('node:crypto');
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
  return isRustTestPath(relativePath) || relativePath.startsWith('api/plugins/installed/')
    || relativePath.startsWith('api/vendor/');
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

const BASELINE_FILE = 'scripts/node/check-rust-backend/baseline.json';
const REVIEW_VERSION = 'rust-backend-reviews/v1';
const REVIEW_RULES = new Set([
  'no-production-escape', 'no-sensitive-serialize', 'no-sensitive-logging',
  'blocking-in-async-context',
]);
const SHA256 = /^[a-f0-9]{64}$/u;

function findingKey(finding) {
  return [finding.rule, finding.file, finding.snippet].join('\u0000');
}

function reviewDiagnostic(entry, rule, problems) {
  return {
    severity: 'warning', rule,
    file: typeof entry?.file === 'string' ? entry.file : BASELINE_FILE,
    line: 1,
    message: `Baseline review ${entry?.id || '(unidentified)'} not applied: ${problems.join('; ')}`,
    snippet: typeof entry?.snippet === 'string' ? entry.snippet : '',
    suppressed: false,
    reviewId: entry?.id || null,
    reviewProblems: problems,
  };
}

function loadBaseline(repoRoot) {
  const baselinePath = path.join(repoRoot, BASELINE_FILE);
  if (!fs.existsSync(baselinePath)) return { entries: [], diagnostics: [] };
  try {
    const parsed = JSON.parse(fs.readFileSync(baselinePath, 'utf8'));
    if (parsed.version !== REVIEW_VERSION || !Array.isArray(parsed.allowedFindings)) {
      throw new Error(`expected ${REVIEW_VERSION} and allowedFindings array`);
    }
    return { entries: parsed.allowedFindings, diagnostics: [] };
  } catch (error) {
    return { entries: [], diagnostics: [reviewDiagnostic(null,
      'invalid-baseline-review', [`invalid baseline document: ${error.message}`])] };
  }
}

function reviewPath(file) {
  return typeof file === 'string' && file.startsWith('api/') && file.endsWith('.rs')
    && !file.includes('\\') && !file.split('/').some((part) => part === '..' || part === '.')
    && path.posix.normalize(file) === file;
}

function validateReview(entry, repoRoot, hashes) {
  const problems = [];
  const nonempty = (value) => typeof value === 'string' && value.trim().length > 0;
  if (!entry || typeof entry !== 'object' || Array.isArray(entry)) return ['review must be an object'];
  if (!nonempty(entry.id)) problems.push('missing review id');
  if (!REVIEW_RULES.has(entry.rule)) problems.push('unknown diagnostic rule');
  if (!nonempty(entry.snippet)) problems.push('missing exact diagnostic snippet');
  if (!nonempty(entry.reason)) problems.push('missing source-specific reason');
  if (Object.hasOwn(entry, 'line')) problems.push('historical line is not review evidence');

  function bind(file, expectedHash, label) {
    if (!reviewPath(file)) { problems.push(`${label} must be a canonical api Rust path`); return; }
    if (!SHA256.test(expectedHash || '')) { problems.push(`${label} lacks a full SHA256`); return; }
    if (!hashes.has(file)) {
      try {
        const absolute = fs.realpathSync(path.join(repoRoot, file));
        const relative = path.relative(fs.realpathSync(repoRoot), absolute);
        if (path.isAbsolute(relative) || relative === '..' || relative.startsWith(`..${path.sep}`)) {
          throw new Error('path escapes repository');
        }
        hashes.set(file, { hash: createHash('sha256').update(fs.readFileSync(absolute)).digest('hex') });
      } catch (error) { hashes.set(file, { error: error.message }); }
    }
    const actual = hashes.get(file);
    if (actual.error) problems.push(`${label} unavailable: ${file}`);
    else if (actual.hash !== expectedHash) problems.push(`${label} SHA256 changed: ${file}`);
  }
  bind(entry.file, entry.source_sha256, 'finding source');
  if (!Array.isArray(entry.evidence) || entry.evidence.length === 0) {
    problems.push('missing caller/type/owner evidence');
  } else {
    const evidencePaths = new Set();
    for (const evidence of entry.evidence) {
      if (!evidence || typeof evidence !== 'object' || Array.isArray(evidence)) {
        problems.push('evidence must be an object'); continue;
      }
      if (!nonempty(evidence.reason)) problems.push('missing evidence-specific reason');
      if (evidencePaths.has(evidence.file)) problems.push('duplicate evidence file');
      evidencePaths.add(evidence.file);
      bind(evidence.file, evidence.sha256, 'review evidence');
    }
    if (!entry.evidence.some((item) => item?.file !== entry.file)) {
      problems.push('evidence must include a separate caller/type/owner source');
    }
  }
  return problems;
}

function applyBaseline(findings, baseline, repoRoot) {
  const liveKeys = new Set(findings.map(findingKey));
  const reviews = new Map();
  const diagnostics = [...baseline.diagnostics];
  const hashes = new Map();
  const ids = new Map();
  const keys = new Map();
  for (const entry of baseline.entries) {
    const key = findingKey(entry || {});
    ids.set(entry?.id, (ids.get(entry?.id) || 0) + 1);
    keys.set(key, (keys.get(key) || 0) + 1);
  }
  for (const entry of baseline.entries) {
    const problems = validateReview(entry, repoRoot, hashes);
    const key = findingKey(entry || {});
    if (ids.get(entry?.id) !== 1 || keys.get(key) !== 1) problems.push('duplicate review id or diagnostic key');
    const retired = !liveKeys.has(key);
    if (retired) problems.push('exact upstream diagnostic is no longer present; retire this review');
    if (problems.length) {
      diagnostics.push(reviewDiagnostic(entry,
        retired ? 'retired-baseline-review' : 'invalid-baseline-review', problems));
    } else reviews.set(key, entry);
  }
  const annotated = findings.map((finding) => {
    const review = reviews.get(findingKey(finding));
    return {
      ...finding, suppressed: Boolean(review),
      ...(review ? {
        suppressionReason: review.reason,
        reviewId: review.id,
        source_sha256: review.source_sha256,
        reviewEvidence: review.evidence,
      } : {}),
    };
  });
  // Keep original severity, message and snippet even when reviewed. Review
  // failures are additional diagnostics, never replacements for upstream output.
  return [...annotated, ...diagnostics];
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
  const annotated = applyBaseline(findings, baseline, repoRoot);

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
