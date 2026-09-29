const fs = require('node:fs');
const path = require('node:path');

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
const SENSITIVE_FIELD_PATTERN = /\b(?:password_hash|token_hash|encrypted_secret_json|secret_value|api_key_secret)\b/u;
const SENSITIVE_LOG_PATTERN = /\b(?:password|token|secret|api_key)\b/iu;
const LOGGING_PATTERN = /\b(?:tracing::(?:trace|debug|info|warn|error)!|println!|eprintln!)\s*\(/u;

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

function stripStringLiterals(line) {
  return line.replace(/"([^"\\]|\\.)*"/gu, '""');
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

function countChar(line, char) {
  return [...line].filter((candidate) => candidate === char).length;
}

// Keep offsets while ignoring punctuation inside literals and nested comments.
function tokenizeRustStructure(content) {
  const tokens = [];
  let offset = 0;
  while (offset < content.length) {
    const rest = content.slice(offset);
    const whitespace = /^\s+/u.exec(rest);
    if (whitespace) {
      offset += whitespace[0].length;
      continue;
    }
    if (rest.startsWith('//')) {
      const end = content.indexOf('\n', offset);
      offset = end < 0 ? content.length : end;
      continue;
    }
    if (rest.startsWith('/*')) {
      let depth = 1;
      offset += 2;
      while (offset < content.length && depth > 0) {
        if (content.startsWith('/*', offset)) {
          depth += 1;
          offset += 2;
        } else if (content.startsWith('*/', offset)) {
          depth -= 1;
          offset += 2;
        } else {
          offset += 1;
        }
      }
      continue;
    }
    const rawString = /^(?:br|r)(#*)"/u.exec(rest);
    const quoted = /^(?:b?"(?:[^"\\]|\\[\s\S])*"|b?'(?:[^'\\\n]|\\(?:u\{[\da-fA-F]+\}|x[\da-fA-F]{2}|[^\n]))')/u.exec(rest);
    if (rawString || quoted) {
      const end = rawString
        ? content.indexOf(`"${rawString[1]}`, offset + rawString[0].length)
        : offset + quoted[0].length;
      const length = rawString
        ? (end < 0 ? content.length : end + rawString[1].length + 1) - offset
        : quoted[0].length;
      tokens.push({ value: '<literal>', start: offset, end: offset + length });
      offset += length;
      continue;
    }
    const value = /^[A-Za-z_]\w*/u.exec(rest)?.[0] || rest[0];
    tokens.push({ value, start: offset, end: offset + value.length });
    offset += value.length;
  }
  return tokens;
}

// Evaluate whether the predicate can hold when test=false. Unknown features and
// platforms can be either true or false; any(test, feature) is production code.
function cfgCanEnableWithoutTest(tokens) {
  let cursor = 0;
  function parsePredicate() {
    const name = tokens[cursor++]?.value;
    if (!name) return { yes: true, no: true };
    if (tokens[cursor]?.value !== '(') {
      if (tokens[cursor]?.value === '=') cursor += 2;
      return name === 'test' ? { yes: false, no: true } : { yes: true, no: true };
    }
    cursor += 1;
    const children = [];
    while (cursor < tokens.length && tokens[cursor].value !== ')') {
      children.push(parsePredicate());
      if (tokens[cursor]?.value === ',') cursor += 1;
      else if (tokens[cursor]?.value !== ')') return { yes: true, no: true };
    }
    if (tokens[cursor++]?.value !== ')') return { yes: true, no: true };
    if (name === 'all') return { yes: children.every((child) => child.yes), no: children.some((child) => child.no) };
    if (name === 'any') return { yes: children.some((child) => child.yes), no: children.every((child) => child.no) };
    if (name === 'not' && children.length === 1) return { yes: children[0].no, no: children[0].yes };
    return { yes: true, no: true };
  }
  const predicate = parsePredicate();
  return cursor !== tokens.length || predicate.yes;
}

function attributeEnd(tokens, start) {
  const open = tokens[start + 1]?.value === '!' ? start + 2 : start + 1;
  if (tokens[start]?.value !== '#' || tokens[open]?.value !== '[') return null;
  let depth = 1;
  for (let index = open + 1; index < tokens.length; index += 1) {
    if (tokens[index].value === '[') depth += 1;
    if (tokens[index].value === ']') depth -= 1;
    if (depth === 0) return { open, end: index };
  }
  return null;
}

function cfgItemEnd(tokens, start) {
  let itemStart = start;
  let attribute;
  while ((attribute = attributeEnd(tokens, itemStart))) itemStart = attribute.end + 1;
  // These items end at a semicolon/comma even when they contain a brace group.
  // This covers imports, const/static initializers and cfg-gated struct fields.
  const header = [];
  for (let index = itemStart; index < tokens.length; index += 1) {
    const value = tokens[index].value;
    if (['{', ';', ',', '='].includes(value)) break;
    header.push(value);
  }
  const blockItem = header.some((value) => ['fn', 'mod', 'struct', 'enum', 'impl', 'trait', 'union', 'extern'].includes(value));
  const delimited = !blockItem && (
    header.some((value) => ['use', 'const', 'static', 'type'].includes(value)) || header.includes(':')
  );
  const depths = { '(': 0, '[': 0, '{': 0 };
  const closes = { ')': '(', ']': '[', '}': '{' };
  let angleDepth = 0;
  let inInitializer = false;
  const balanced = () => angleDepth === 0 && Object.values(depths).every((depth) => depth === 0);
  for (let index = itemStart; index < tokens.length; index += 1) {
    const value = tokens[index].value;
    if (value === '=' && balanced() && !blockItem && header.some((token) => ['const', 'static', 'let'].includes(token))) inInitializer = true;
    if (!inInitializer && depths['{'] === 0) {
      if (value === '<') angleDepth += 1;
      else if (value === '>' && angleDepth > 0) angleDepth -= 1;
    }
    if (Object.hasOwn(depths, value)) depths[value] += 1;
    else if (closes[value]) {
      const open = closes[value];
      depths[open] -= 1;
      if (depths[open] < 0) return index - 1;
      if (value === '}' && !delimited && balanced()) return index;
    }
    if ((value === ';' || value === ',') && balanced()) return index;
  }
  return tokens.length - 1;
}

function precedingAttributeStart(tokens, start) {
  let cursor = start;
  while (tokens[cursor - 1]?.value === ']') {
    let depth = 1;
    let open = cursor - 2;
    for (; open >= 0; open -= 1) {
      if (tokens[open].value === ']') depth += 1;
      if (tokens[open].value === '[') depth -= 1;
      if (depth === 0) break;
    }
    if (tokens[open - 1]?.value !== '#') break;
    cursor = open - 1;
  }
  return cursor;
}

function maskCfgTestCode(content) {
  const tokens = tokenizeRustStructure(content);
  const ranges = [];
  for (let index = 0; index < tokens.length; index += 1) {
    const attribute = attributeEnd(tokens, index);
    if (!attribute) continue;
    const expression = tokens.slice(attribute.open + 1, attribute.end);
    if (expression[0]?.value !== 'cfg' || expression[1]?.value !== '(' || expression.at(-1)?.value !== ')') continue;
    if (cfgCanEnableWithoutTest(expression.slice(2, -1))) continue;
    const inner = tokens[index + 1]?.value === '!';
    if (inner && index !== 0) continue;
    const end = inner ? tokens.length - 1 : cfgItemEnd(tokens, attribute.end + 1);
    ranges.push([tokens[precedingAttributeStart(tokens, index)].start, tokens[end]?.end || tokens[attribute.end].end]);
    index = end;
  }
  let cursor = 0;
  const parts = [];
  for (const [start, end] of ranges) {
    parts.push(content.slice(cursor, start), content.slice(start, end).replace(/[^\r\n]/gu, ' '));
    cursor = end;
  }
  parts.push(content.slice(cursor));
  return parts.join('');
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
  const productionLines = maskCfgTestCode(content).split(/\r?\n/u);
  const findings = [];
  let pendingSerializeDerive = false;
  let inSerializeStruct = false;
  let serializeStructDepth = 0;

  productionLines.forEach((line, index) => {
    const lineNumber = index + 1;

    if (!line.trim()) {
      return;
    }

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

    if (LOGGING_PATTERN.test(line) && SENSITIVE_LOG_PATTERN.test(stripStringLiterals(line))) {
      findings.push(createFinding({
        severity: 'error',
        rule: 'no-sensitive-logging',
        file: relativePath,
        line: lineNumber,
        message: 'logging call appears to include sensitive material',
        snippet: lines[index],
      }));
    }

    if (pendingSerializeDerive && /\bstruct\s+\w+/u.test(line)) {
      inSerializeStruct = true;
      serializeStructDepth = countChar(line, '{') - countChar(line, '}');
      pendingSerializeDerive = false;

      if (serializeStructDepth <= 0 && line.includes('}')) {
        inSerializeStruct = false;
        serializeStructDepth = 0;
      }

      return;
    }

    if (inSerializeStruct) {
      if (SENSITIVE_FIELD_PATTERN.test(line)) {
        findings.push(createFinding({
          severity: 'error',
          rule: 'no-sensitive-serialize',
          file: relativePath,
          line: lineNumber,
          message: 'serialized Rust struct exposes a sensitive field',
          snippet: lines[index],
        }));
      }

      serializeStructDepth += countChar(line, '{') - countChar(line, '}');

      if (serializeStructDepth <= 0 && line.includes('}')) {
        inSerializeStruct = false;
        serializeStructDepth = 0;
      }

      return;
    }

    if (/#\[derive\([^\]]*\bSerialize\b[^\]]*\)\]/u.test(line)) {
      pendingSerializeDerive = true;
      return;
    }

    if (line.trim().length > 0 && !line.trim().startsWith('#[') && !/\bstruct\s+\w+/u.test(line)) {
      pendingSerializeDerive = false;
    }
  });

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
