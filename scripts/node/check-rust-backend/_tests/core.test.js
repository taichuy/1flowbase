const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');

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
    relativePath: 'api/apps/api-server/src/routes/settings/members.rs',
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

test('baseline suppression survives unrelated line shifts', async () => {
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
  fs.writeFileSync(
    path.join(repoRoot, 'scripts', 'node', 'check-rust-backend', 'baseline.json'),
    JSON.stringify({
      allowedFindings: [
        {
          rule: 'no-production-escape',
          file: 'api/crates/domain/src/existing.rs',
          line: 1,
          snippet: 'pub fn existing() { Some(1).unwrap(); }',
          reason: 'existing panic cleanup is tracked separately',
        },
      ],
    })
  );

  const findings = collectRustBackendFindings({ repoRoot, includeSuppressed: true });

  assert.equal(findings.length, 1);
  assert.equal(findings[0].suppressed, true);
  assert.equal(findings[0].suppressionReason, 'existing panic cleanup is tracked separately');
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
