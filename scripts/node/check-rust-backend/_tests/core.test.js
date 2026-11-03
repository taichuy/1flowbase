const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const { createHash } = require('node:crypto');
const os = require('node:os');
const path = require('node:path');
const { createRequire } = require('node:module');

const {
  collectRustBackendFindings,
  main,
  scanRustSource,
} = require('../core.js');

test('scanRustSource flags production escape hatches while ignoring cfg test modules', () => {
  const findings = scanRustSource({
    relativePath: 'api/crates/domain/src/order.rs',
    content: [
      'pub fn create() {',
      '    let value = build_order().unwrap();',
      '}',
      '',
      '#[cfg(test)]',
      'mod tests {',
      '    #[test]',
      '    fn accepts_unwrap_in_test() {',
      '        Some(1).unwrap();',
      '    }',
      '}',
      '',
      '#[cfg(all(test, unix))]',
      'mod unix_tests {',
      '    #[test]',
      '    fn accepts_escape_hatches_in_cfg_all_test_module() {',
      '        Some(1).unwrap();',
      '        panic!("test-only failure");',
      '    }',
      '}',
    ].join('\n'),
  });

  assert.deepEqual(
    findings.map((finding) => ({
      severity: finding.severity,
      rule: finding.rule,
      line: finding.line,
    })),
    [
      {
        severity: 'error',
        rule: 'no-production-escape',
        line: 2,
      },
    ]
  );
});

test('scanRustSource ignores a cfg test function outside a test module', () => {
  const findings = scanRustSource({
    relativePath: 'api/apps/api-server/src/routes/settings/file_storages.rs',
    content: [
      '#[cfg(test)]',
      'pub(crate) fn projection_fixture() {',
      '    Some(1).unwrap();',
      '}',
    ].join('\n'),
  });

  assert.deepEqual(findings, []);
});

test('scanRustSource flags sensitive serialized fields', () => {
  const findings = scanRustSource({
    relativePath: 'api/crates/domain/src/auth.rs',
    content: [
      '#[derive(Debug, Clone, Serialize, Deserialize)]',
      'pub struct UserResponse {',
      '    pub id: UserId,',
      '    pub password_hash: String,',
      '}',
    ].join('\n'),
  });

  assert.equal(findings.length, 1);
  assert.equal(findings[0].severity, 'error');
  assert.equal(findings[0].rule, 'no-sensitive-serialize');
  assert.equal(findings[0].line, 4);
});

test('current auth hash records are not serializable or baseline-suppressed', () => {
  const repoRoot = path.resolve(__dirname, '..', '..', '..', '..');
  const authRelativePath = 'api/crates/domain/src/auth/mod.rs';
  const authPath = path.join(repoRoot, ...authRelativePath.split('/'));
  const baselinePath = path.join(repoRoot, 'scripts', 'node', 'check-rust-backend', 'baseline.json');
  const authFindings = scanRustSource({
    relativePath: authRelativePath,
    content: fs.readFileSync(authPath, 'utf8'),
  }).filter(
    (finding) =>
      finding.rule === 'no-sensitive-serialize'
      && /(?:password_hash|token_hash)/u.test(finding.snippet)
  );
  const baseline = JSON.parse(fs.readFileSync(baselinePath, 'utf8'));
  const authBaselineSuppressions = (baseline.allowedFindings || []).filter(
    (finding) =>
      finding.rule === 'no-sensitive-serialize'
      && finding.file === authRelativePath
      && /(?:password_hash|token_hash)/u.test(finding.snippet)
  );

  assert.deepEqual(authFindings, []);
  assert.deepEqual(authBaselineSuppressions, []);
});

test('scanRustSource limits sensitive serialization checks to the current struct', () => {
  const findings = scanRustSource({
    relativePath: 'api/apps/api-server/src/routes/settings/identity_access/members.rs',
    content: [
      '#[derive(Serialize)]',
      'pub struct MemberResponse {',
      '    pub id: UserId,',
      '}',
      '',
      'pub fn update_password() {',
      '    let password_hash = hash_password("secret")?;',
      '}',
    ].join('\n'),
  });

  assert.deepEqual(findings, []);
});

test('scanRustSource does not treat static log message text as sensitive leakage', () => {
  const findings = scanRustSource({
    relativePath: 'api/apps/api-server/src/bin/reset_root_password.rs',
    content: 'pub fn main() { println!("reset root password for {}", account); }\n',
  });

  assert.deepEqual(findings, []);
});

test('scanRustSource reports async blocking patterns as warnings', () => {
  const findings = scanRustSource({
    relativePath: 'api/apps/api-server/src/routes/files.rs',
    content: [
      'pub async fn upload() -> Result<(), AppError> {',
      '    let bytes = std::fs::read("upload.bin")?;',
      '    Ok(())',
      '}',
    ].join('\n'),
  });

  assert.equal(findings.length, 1);
  assert.equal(findings[0].severity, 'warning');
  assert.equal(findings[0].rule, 'blocking-in-async-context');
});

test('collectRustBackendFindings skips Rust files under test directories and test modules', () => {
  const repoRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'oneflowbase-rust-gate-'));
  fs.mkdirSync(path.join(repoRoot, 'api', 'crates', 'domain', 'src', '_tests'), { recursive: true });
  fs.mkdirSync(path.join(repoRoot, 'api', 'apps', 'api-server', 'src', 'routes', 'foo'), { recursive: true });
  fs.mkdirSync(path.join(repoRoot, 'api', 'crates', 'domain', 'src'), { recursive: true });
  fs.writeFileSync(
    path.join(repoRoot, 'api', 'crates', 'domain', 'src', '_tests', 'order_tests.rs'),
    'fn test_helper() { Some(1).unwrap(); }\n'
  );
  fs.writeFileSync(
    path.join(repoRoot, 'api', 'apps', 'api-server', 'src', 'routes', 'foo', 'tests.rs'),
    'fn route_test_helper() { Some(1).unwrap(); }\n'
  );
  fs.writeFileSync(
    path.join(repoRoot, 'api', 'crates', 'domain', 'src', 'order.rs'),
    'pub fn create() { Some(1).unwrap(); }\n'
  );

  const findings = collectRustBackendFindings({ repoRoot });

  assert.equal(findings.length, 1);
  assert.equal(findings[0].file, 'api/crates/domain/src/order.rs');
});

test('collection skips reads while preserving the legacy ordered findings and source-bound reviews', (t) => {
  // Reconstruct only the pre-G1 read-before-scan pipeline. Keep the real scanner,
  // traversal and baseline validation so the oracle compares complete findings.
  const corePath = require.resolve('../core.js');
  const coreSource = fs.readFileSync(corePath, 'utf8');
  const earlySkip = '      if (isSkippedRustPath(relativePath)) return [];\n';
  assert.equal(coreSource.split(earlySkip).length, 2);
  const legacyModule = { exports: {} };
  new Function('require', 'module', '__dirname', coreSource.replace(earlySkip, ''))(
    createRequire(corePath), legacyModule, path.dirname(corePath)
  );
  const legacyCollect = legacyModule.exports.collectRustBackendFindings;
  const repoRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'rust-gate-read-equivalence-'));
  t.after(() => fs.rmSync(repoRoot, { recursive: true, force: true }));
  const skipped = [
    'api/crates/domain/src/_tests/fixture.rs',
    'api/crates/domain/tests/fixture.rs',
    'api/crates/domain/benches/fixture.rs',
    'api/crates/domain/src/tests.rs',
    'api/crates/domain/src/_tests.rs',
    'api/vendor/dependency/src/lib.rs',
    'api/plugins/installed/plugin/src/lib.rs',
  ];
  const production = {
    'api/crates/domain/src/a_reviewed.rs': 'pub fn reviewed() { Some(1).unwrap(); }\n',
    'api/crates/domain/src/b_errors.rs': [
      'pub fn broken() { Some(1).unwrap(); panic!("bad"); }',
      'pub async fn blocking() { std::fs::read("file"); }',
      '#[derive(Serialize)]',
      'pub struct Response {',
      '    pub token_hash: String,',
      '}',
    ].join('\n'),
    'api/crates/domain/src/c_clean.rs': 'pub fn caller() { reviewed(); }\n',
    // Similar names are production paths and must still be read and checked.
    'api/crates/domain/src/tests_support.rs': 'pub fn helper() { todo!(); }\n',
    'api/vendor_local/src/lib.rs': 'pub fn local() { dbg!(1); }\n',
    'api/plugins/installed_local/src/lib.rs': 'pub fn local() { unimplemented!(); }\n',
  };
  for (const [file, content] of [
    ...skipped.map((file) => [file, 'fn ignored() { Some(1).unwrap(); }\n']),
    ...Object.entries(production),
  ]) {
    const absolute = path.join(repoRoot, file);
    fs.mkdirSync(path.dirname(absolute), { recursive: true });
    fs.writeFileSync(absolute, content);
  }
  const baselinePath = path.join(repoRoot, 'scripts/node/check-rust-backend/baseline.json');
  fs.mkdirSync(path.dirname(baselinePath), { recursive: true });
  const sourceFile = 'api/crates/domain/src/a_reviewed.rs';
  const evidenceFile = 'api/crates/domain/src/c_clean.rs';
  const sha = (content) => createHash('sha256').update(content).digest('hex');
  const review = {
    id: 'read-equivalence', rule: 'no-production-escape', file: sourceFile,
    snippet: production[sourceFile].trim(), reason: 'fixed Some literal in fixture',
    source_sha256: sha(production[sourceFile]),
    evidence: [{ file: evidenceFile, sha256: sha(production[evidenceFile]), reason: 'fixture caller' }],
  };
  const originalRead = fs.readFileSync;
  let reads = [];
  t.mock.method(fs, 'readFileSync', function (file, ...args) {
    if (typeof file === 'string' && file.startsWith(`${repoRoot}${path.sep}`)) {
      reads.push([path.relative(repoRoot, file).split(path.sep).join('/'), args[0]]);
    }
    return originalRead.call(this, file, ...args);
  });
  for (const state of ['absent', 'valid', 'source-changed', 'evidence-changed', 'malformed']) {
    if (state !== 'absent') {
      const entry = structuredClone(review);
      if (state === 'source-changed') entry.source_sha256 = '0'.repeat(64);
      if (state === 'evidence-changed') entry.evidence[0].sha256 = '0'.repeat(64);
      fs.writeFileSync(baselinePath, state === 'malformed' ? '{' : JSON.stringify({
        version: 'rust-backend-reviews/v1', allowedFindings: [entry],
      }));
    }
    for (const includeSuppressed of [false, true]) {
      reads = [];
      const before = legacyCollect({ repoRoot, includeSuppressed });
      const beforeReads = [...reads];
      reads = [];
      const after = collectRustBackendFindings({ repoRoot, includeSuppressed });
      assert.deepEqual(after, before, `${state}: complete findings, including order and severity`);
      assert.equal(beforeReads.length - reads.length, skipped.length, state);
      for (const file of skipped) {
        assert.equal(beforeReads.filter(([name]) => name === file).length, 1, file);
        assert.equal(reads.filter(([name]) => name === file).length, 0, file);
        assert.deepEqual(scanRustSource({ relativePath: file, content: production[sourceFile] }), []);
      }
      for (const file of Object.keys(production)) {
        assert.equal(reads.filter(([name, encoding]) => name === file && encoding === 'utf8').length, 1, file);
      }
      assert.ok(after.some((item) => item.file.endsWith('/b_errors.rs')
        && item.rule === 'no-production-escape' && item.severity === 'error' && !item.suppressed));
      assert.ok(after.some((item) => item.rule === 'blocking-in-async-context' && item.severity === 'warning'));
      assert.ok(after.some((item) => item.rule === 'no-sensitive-serialize' && item.severity === 'error'));
      if (state === 'valid' && includeSuppressed) {
        assert.equal(after.find((item) => item.file === sourceFile).suppressed, true);
      }
      if (['source-changed', 'evidence-changed', 'malformed'].includes(state)) {
        assert.ok(after.some((item) => item.rule === 'invalid-baseline-review' && !item.suppressed));
        assert.equal(after.find((item) => item.file === sourceFile).suppressed, false);
      }
    }
  }
});

test('collection propagates production read failures instead of silently dropping files', (t) => {
  const repoRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'rust-gate-read-failure-'));
  t.after(() => fs.rmSync(repoRoot, { recursive: true, force: true }));
  const file = path.join(repoRoot, 'api/crates/domain/src/broken.rs');
  fs.mkdirSync(path.dirname(file), { recursive: true });
  fs.writeFileSync(file, 'pub fn broken() { Some(1).unwrap(); }\n');
  const originalRead = fs.readFileSync;
  const failure = new Error('controlled production read failure');
  t.mock.method(fs, 'readFileSync', function (candidate, ...args) {
    if (candidate === file) throw failure;
    return originalRead.call(this, candidate, ...args);
  });
  assert.throws(() => collectRustBackendFindings({ repoRoot }), (error) => error === failure);
});

test('main writes a report and fails when error findings exist', async () => {
  const repoRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'oneflowbase-rust-gate-main-'));
  fs.mkdirSync(path.join(repoRoot, 'api', 'crates', 'domain', 'src'), { recursive: true });
  fs.writeFileSync(
    path.join(repoRoot, 'api', 'crates', 'domain', 'src', 'auth.rs'),
    [
      '#[derive(Serialize)]',
      'pub struct UserResponse {',
      '    pub token_hash: String,',
      '}',
    ].join('\n')
  );

  let stderr = '';
  const status = await main([], {
    repoRoot,
    writeStdout() {},
    writeStderr(text) {
      stderr += text;
    },
  });

  assert.equal(status, 1);
  assert.match(stderr, /no-sensitive-serialize/u);
  assert.equal(
    fs.existsSync(path.join(repoRoot, 'tmp', 'test-governance', 'rust-backend-static-gate.json')),
    true
  );
});

test('source-bound review matches complete current content without historical line metadata', async () => {
  const repoRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'oneflowbase-rust-gate-baseline-'));
  fs.mkdirSync(path.join(repoRoot, 'api', 'crates', 'domain', 'src'), { recursive: true });
  fs.mkdirSync(path.join(repoRoot, 'scripts', 'node', 'check-rust-backend'), { recursive: true });
  fs.writeFileSync(
    path.join(repoRoot, 'api', 'crates', 'domain', 'src', 'existing.rs'),
    [
      '',
      '',
      'pub fn existing() { Some(1).unwrap(); }',
    ].join('\n')
  );
  fs.writeFileSync(path.join(repoRoot, 'api/crates/domain/src/caller.rs'), 'fn caller() { existing(); }\n');
  fs.writeFileSync(
    path.join(repoRoot, 'scripts', 'node', 'check-rust-backend', 'baseline.json'),
    JSON.stringify({
      version: 'rust-backend-reviews/v1',
      allowedFindings: [
        {
          id: 'constant-fixture',
          rule: 'no-production-escape',
          file: 'api/crates/domain/src/existing.rs',
          snippet: 'pub fn existing() { Some(1).unwrap(); }',
          reason: 'the fixed Some literal cannot be None in this fixture',
          source_sha256: createHash('sha256').update(fs.readFileSync(
            path.join(repoRoot, 'api/crates/domain/src/existing.rs')
          )).digest('hex'),
          evidence: [{
            file: 'api/crates/domain/src/caller.rs',
            sha256: createHash('sha256').update('fn caller() { existing(); }\n').digest('hex'),
            reason: 'fixture caller reaches the exact fixed-literal function',
          }],
        },
      ],
    })
  );

  const findings = collectRustBackendFindings({ repoRoot, includeSuppressed: true });

  assert.equal(findings.length, 1);
  assert.equal(findings[0].suppressed, true);
  assert.equal(findings[0].suppressionReason, 'the fixed Some literal cannot be None in this fixture');
});

test('every current Rust baseline exception matches a live finding', () => {
  const repoRoot = path.resolve(__dirname, '..', '..', '..', '..');
  const baseline = JSON.parse(fs.readFileSync(
    path.join(repoRoot, 'scripts', 'node', 'check-rust-backend', 'baseline.json'),
    'utf8'
  ));
  const findings = collectRustBackendFindings({ repoRoot, includeSuppressed: true });
  const key = (entry) => JSON.stringify([entry.rule, entry.file, entry.snippet]);
  const liveKeys = new Set(findings.map(key));
  const stale = baseline.allowedFindings.filter((entry) => !liveKeys.has(key(entry)));

  assert.deepEqual(stale, [], 'retired code must retire its Rust lint exception');
  assert.deepEqual(findings.filter((item) => /^(?:invalid|retired)-baseline-review$/u.test(item.rule)), []);
  assert.equal(findings.filter((item) => item.suppressed).length, baseline.allowedFindings.length);
});

test('current api routes do not contain active blocking IO warnings', () => {
  const repoRoot = path.resolve(__dirname, '..', '..', '..', '..');
  const findings = collectRustBackendFindings({ repoRoot }).filter(
    (finding) =>
      finding.rule === 'blocking-in-async-context'
      && finding.file.startsWith('api/apps/api-server/src/routes/')
  );

  assert.deepEqual(findings, []);
});

test('OAuth consent hash persistence exception does not exempt response credentials', () => {
  const repoRoot = path.resolve(__dirname, '..', '..', '..', '..');
  const relativePath = 'api/crates/control-plane/src/mcp_oauth/mod.rs';
  const source = fs.readFileSync(path.join(repoRoot, relativePath), 'utf8');
  const approval = source.match(/struct Approval \{([\s\S]*?)\n\}/u)?.[1];
  assert.match(approval, /\n    token_hash: String,/u);
  assert.doesNotMatch(approval, /pub(?:\([^)]*\))?\s+token_hash/u);
  for (const name of ['AuthorizationView', 'VerificationView', 'TokenResponse']) {
    const fields = source.match(new RegExp(`pub struct ${name} \\{([\\s\\S]*?)\\n\\}`, 'u'))?.[1];
    assert.equal(typeof fields, 'string');
    assert.doesNotMatch(fields, /\b(?:token_hash|password_hash|api_key_secret)\b/u);
  }
  const findings = collectRustBackendFindings({ repoRoot, includeSuppressed: true });
  const persisted = findings.filter((item) => item.file === relativePath && item.rule === 'no-sensitive-serialize');
  assert.equal(persisted.length, 1);
  assert.equal(persisted[0].snippet, 'token_hash: String,');
  assert.equal(persisted[0].suppressed, true);
  assert.match(persisted[0].suppressionReason, /Private Approval persistence payload/u);
  const fixtureRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'oauth-serialize-negative-'));
  try {
    const file = path.join(fixtureRoot, relativePath);
    fs.mkdirSync(path.dirname(file), { recursive: true });
    fs.writeFileSync(file, '#[derive(Serialize)]\npub struct UnsafeResponse {\n    pub token_hash: String,\n}\n');
    const baselineDir = path.join(fixtureRoot, 'scripts/node/check-rust-backend');
    fs.mkdirSync(baselineDir, { recursive: true });
    fs.copyFileSync(path.join(repoRoot, 'scripts/node/check-rust-backend/baseline.json'), path.join(baselineDir, 'baseline.json'));
    const rejected = collectRustBackendFindings({ repoRoot: fixtureRoot })
      .filter((item) => item.rule === 'no-sensitive-serialize');
    assert.equal(rejected.length, 1);
    assert.equal(rejected[0].rule, 'no-sensitive-serialize');
    assert.equal(rejected[0].suppressed, false);
  } finally {
    fs.rmSync(fixtureRoot, { recursive: true, force: true });
  }
});

// Legal test escape hatches must not hide the next production item.
test('cfg exemption ends at inline blocks and external module declarations', () => {
  for (const item of ['fn fixture() { Some(1).unwrap(); }', 'mod tests;']) {
    const findings = scanRustSource({ relativePath: 'api/crates/domain/src/example.rs',
      content: `#[cfg(test)]\n${item}\npub fn production() { Some(1).unwrap(); }` });
    assert.deepEqual(findings.map(f => [f.rule, f.line]), [['no-production-escape', 3]]);
  }
});

test('cfg logic exempts only conditions requiring test in every configuration', () => {
  for (const [condition, exempt] of [
    ['test', true], ['all(test, unix)', true], ['any(test, feature = "prod")', false],
    ['all(unix, any(test, feature = "prod"))', false], ['any(test, all(test, unix))', true],
    ['all(test, not(feature = "fixture"))', true], ['not(test)', false],
  ]) {
    const findings = scanRustSource({ relativePath: 'api/crates/domain/src/example.rs',
      content: `#[cfg(${condition})]\nfn selected() { Some(1).unwrap(); }\npub fn next() { panic!("production"); }` });
    assert.deepEqual(findings.map(f => f.line), exempt ? [3] : [2, 3], condition);
  }
});

test('comments and Rust literals are not calls or test-module delimiters', () => {
  const findings = scanRustSource({ relativePath: 'api/crates/domain/src/example.rs', content: [
    '// panic!("comment")', '/* nested /* dbg!(1) */ todo!() */',
    'const DOC: &str = r##"panic!() { \" }"##;',
    'const TEXT: &str = "unimplemented!()";', "const BRACE: char = '}';",
    '#[cfg(test)]', 'mod tests {', 'let message = "{";', 'Some(1).unwrap();', '}',
    'pub fn production() { Some(1).unwrap(); }',
  ].join('\n') });
  assert.deepEqual(findings.map(f => [f.rule, f.line]), [['no-production-escape', 11]]);
});

test('raw-string and multiline cfg values retain legal test-only exemptions', () => {
  for (const attribute of ['#[cfg(all(test, feature = r#"fixture"#))]', '#[cfg(\n all(test, not(feature = "fixture"))\n)]']) {
    const findings = scanRustSource({ relativePath: 'api/crates/domain/src/example.rs',
      content: `${attribute}\nfn fixture() { Some(1).unwrap(); }\nfn production() { panic!("bad"); }` });
    assert.equal(findings.length, 1, attribute);
    assert.equal(findings[0].snippet, 'fn production() { panic!("bad"); }');
  }
});

test('test-only item and field exemptions preserve adjacent production syntax', () => {
  for (const content of [
    '#[cfg(test)]\nfn fixture() { Some(1).unwrap(); } pub fn production() { panic!("bad"); }',
    '#[cfg(test)] mod tests; pub fn production() { panic!("bad"); }',
    'let state = State {\n#[cfg(test)]\ntest_resources: None,\nvalue: Some(1).unwrap(),\n};',
  ]) {
    const findings = scanRustSource({ relativePath: 'api/crates/domain/src/example.rs', content });
    assert.equal(findings.length, 1, content);
    assert.equal(findings[0].rule, 'no-production-escape');
  }
});

test('test-only enum variants do not mask following production discriminants', () => {
  for (const variant of ['Test', 'Test(u8)', 'Test = { Some(0isize).unwrap() }']) {
    const findings = scanRustSource({ relativePath: 'api/crates/domain/src/example.rs',
      content: `enum E { #[cfg(test)] ${variant}, Production = { Some(1isize).unwrap() }, }` });
    assert.equal(findings.length, 1, variant);
    assert.equal(findings[0].rule, 'no-production-escape');
  }
});

test('const-generic headers are not the body of test-only declarations', () => {
  const findings = scanRustSource({ relativePath: 'api/crates/domain/src/example.rs',
    content: '#[cfg(test)] impl Trait<{ 1 }> for Fixture { fn fixture() { Some(1).unwrap(); } }\nfn production() { Some(2).unwrap(); }' });
  assert.deepEqual(findings.map(f => f.line), [2]);
});
