const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const { createHash } = require('node:crypto');
const { collectRustBackendFindings, scanRustSource, main } = require('../core.js');

const sourceFile = 'api/crates/host/src/reader.rs';
const callerFile = 'api/crates/host/src/admission.rs';
const source = 'fn read() { let bytes = std::fs::read("artifact"); }\n';
const caller = 'async fn admit() { tokio::task::spawn_blocking(|| read()).await; }\n';
const sha256 = (bytes) => createHash('sha256').update(bytes).digest('hex');

function fixture(t) {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'rust-source-review-'));
  t.after(() => fs.rmSync(root, { recursive: true, force: true }));
  function write(file, content) {
    fs.mkdirSync(path.dirname(path.join(root, file)), { recursive: true });
    fs.writeFileSync(path.join(root, file), content);
  }
  write(sourceFile, source);
  write(callerFile, caller);
  const review = {
    id: 'reader-blocking-pool', rule: 'blocking-in-async-context', file: sourceFile,
    snippet: source.trim(), reason: 'admit moves the exact reader call to spawn_blocking',
    source_sha256: sha256(source),
    evidence: [{ file: callerFile, sha256: sha256(caller), reason: 'actual offload caller' }],
  };
  function baseline(entries = [review], version = 'rust-backend-reviews/v1') {
    write('scripts/node/check-rust-backend/baseline.json', JSON.stringify({ version, allowedFindings: entries }));
  }
  baseline();
  return { root, write, review, baseline };
}

function original(findings) {
  return findings.find((item) => item.file === sourceFile && item.rule === 'blocking-in-async-context');
}

test('only a complete source-and-caller hash match reviews the original diagnostic', (t) => {
  const f = fixture(t);
  const raw = scanRustSource({ relativePath: sourceFile, content: source })[0];
  const findings = collectRustBackendFindings({ repoRoot: f.root, includeSuppressed: true });
  const reviewed = original(findings);
  assert.equal(reviewed.suppressed, true);
  for (const key of ['severity', 'rule', 'file', 'line', 'message', 'snippet']) {
    assert.deepEqual(reviewed[key], raw[key], key);
  }
  assert.equal(reviewed.reviewId, f.review.id);
  assert.equal(reviewed.suppressionReason, f.review.reason);
  assert.deepEqual(reviewed.reviewEvidence, f.review.evidence);
  assert.deepEqual(collectRustBackendFindings({ repoRoot: f.root }), []);
});

for (const [name, file, content] of [
  ['finding source including unrelated line shifts', sourceFile, `// shifted\n${source}`],
  ['actual caller', callerFile, `${caller}// changed caller\n`],
]) {
  test(`changing ${name} invalidates the review and retains the upstream warning`, (t) => {
    const f = fixture(t);
    f.write(file, content);
    const findings = collectRustBackendFindings({ repoRoot: f.root, includeSuppressed: true });
    assert.equal(original(findings).suppressed, false);
    const invalid = findings.find((item) => item.rule === 'invalid-baseline-review');
    assert.ok(invalid.reviewProblems.some((problem) => problem.includes('SHA256 changed')));
  });
}

for (const file of [sourceFile, callerFile]) {
  test(`missing review file fails closed: ${file}`, (t) => {
    const f = fixture(t);
    fs.rmSync(path.join(f.root, file));
    const findings = collectRustBackendFindings({ repoRoot: f.root, includeSuppressed: true });
    assert.equal(findings.some((item) => item.suppressed), false);
    assert.ok(findings.some((item) => item.reviewProblems?.some((problem) => problem.includes('unavailable'))));
    if (file === callerFile) assert.equal(original(findings).suppressed, false);
  });
}

for (const [name, mutate] of [
  ['missing reason', (entry) => { delete entry.reason; }],
  ['blank reason', (entry) => { entry.reason = ' '; }],
  ['missing source hash', (entry) => { delete entry.source_sha256; }],
  ['missing evidence', (entry) => { entry.evidence = []; }],
  ['missing evidence hash', (entry) => { delete entry.evidence[0].sha256; }],
  ['missing evidence reason', (entry) => { delete entry.evidence[0].reason; }],
  ['historical line metadata', (entry) => { entry.line = 99; }],
  ['same-file-only evidence', (entry) => {
    entry.evidence = [{ file: sourceFile, sha256: sha256(source), reason: 'does not bind a separate caller' }];
  }],
  ['noncanonical caller path', (entry) => { entry.evidence[0].file = 'api/crates/host/src/../src/admission.rs'; }],
]) {
  test(`${name} never applies a review`, (t) => {
    const f = fixture(t);
    mutate(f.review);
    f.baseline();
    const findings = collectRustBackendFindings({ repoRoot: f.root, includeSuppressed: true });
    assert.equal(original(findings).suppressed, false);
    assert.ok(findings.some((item) => item.rule === 'invalid-baseline-review'));
  });
}

test('legacy no-evidence baseline documents do not bypass the formal review schema', (t) => {
  const f = fixture(t);
  f.write('scripts/node/check-rust-backend/baseline.json', JSON.stringify({
    allowedFindings: [{ rule: f.review.rule, file: sourceFile, snippet: source.trim() }],
  }));
  const findings = collectRustBackendFindings({ repoRoot: f.root, includeSuppressed: true });
  assert.equal(original(findings).suppressed, false);
  assert.ok(findings.some((item) => item.rule === 'invalid-baseline-review'));
});

test('retired upstream diagnostics emit an unsuppressed review retirement warning', async (t) => {
  const f = fixture(t);
  const retired = 'fn read() {}\n';
  f.write(sourceFile, retired);
  f.review.source_sha256 = sha256(retired);
  f.baseline();
  const findings = collectRustBackendFindings({ repoRoot: f.root, includeSuppressed: true });
  assert.equal(findings.length, 1);
  assert.equal(findings[0].rule, 'retired-baseline-review');
  assert.equal(findings[0].suppressed, false);
  assert.match(findings[0].message, /retire this review/u);
  await main([], { repoRoot: f.root, writeStdout() {}, writeStderr() {} });
  const report = JSON.parse(fs.readFileSync(path.join(f.root, 'tmp/test-governance/rust-backend-static-gate.json')));
  assert.equal(report.summary.warnings, 1);
  assert.equal(report.summary.suppressed, 0);
});

test('duplicate review IDs or diagnostic keys cannot silently overwrite each other', (t) => {
  const f = fixture(t);
  f.baseline([f.review, { ...f.review, reason: 'duplicate' }]);
  const findings = collectRustBackendFindings({ repoRoot: f.root, includeSuppressed: true });
  assert.equal(original(findings).suppressed, false);
  assert.equal(findings.filter((item) => item.rule === 'invalid-baseline-review').length, 2);
});

test('formal repository reviews all bind source and distinct evidence without historical line fields', () => {
  const root = path.resolve(__dirname, '../../../..');
  const baseline = JSON.parse(fs.readFileSync(path.join(root, 'scripts/node/check-rust-backend/baseline.json')));
  assert.equal(baseline.version, 'rust-backend-reviews/v1');
  const ids = new Set();
  for (const entry of baseline.allowedFindings) {
    assert.ok(entry.id && entry.reason.trim());
    assert.equal(ids.has(entry.id), false);
    ids.add(entry.id);
    assert.equal(Object.hasOwn(entry, 'line'), false);
    assert.equal(sha256(fs.readFileSync(path.join(root, entry.file))), entry.source_sha256);
    assert.ok(entry.evidence.length > 0);
    assert.ok(entry.evidence.some((evidence) => evidence.file !== entry.file));
    for (const evidence of entry.evidence) {
      assert.ok(evidence.reason.trim());
      assert.equal(sha256(fs.readFileSync(path.join(root, evidence.file))), evidence.sha256);
    }
  }
});
