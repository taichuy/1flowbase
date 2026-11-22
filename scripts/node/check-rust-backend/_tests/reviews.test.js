const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const { createHash } = require('node:crypto');
const { collectRustBackendFindings, computeReviewTargetSha256, scanRustSource, main } = require('../core.js');

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
    const sourceContent = fs.readFileSync(path.join(root, entry.file), 'utf8');
    assert.equal(computeReviewTargetSha256(sourceContent, entry.source_anchor), entry.source_sha256, entry.file);
    assert.equal(
      computeReviewTargetSha256(sourceContent.replace(/\r?\n/gu, '\r\n'), entry.source_anchor),
      entry.source_sha256,
      `${entry.file} (CRLF)`
    );
    assert.ok(entry.evidence.length > 0);
    assert.ok(entry.evidence.some((evidence) => evidence.file !== entry.file));
    for (const evidence of entry.evidence) {
      assert.ok(evidence.reason.trim());
      const evidenceContent = fs.readFileSync(path.join(root, evidence.file), 'utf8');
      assert.equal(computeReviewTargetSha256(evidenceContent, evidence.anchor), evidence.sha256, evidence.file);
      assert.equal(
        computeReviewTargetSha256(evidenceContent.replace(/\r?\n/gu, '\r\n'), evidence.anchor),
        evidence.sha256,
        `${evidence.file} (CRLF)`
      );
    }
  }
});

test('model-provider response mapping converts i18n catalogs without unwrap or baseline exceptions', () => {
  const repoRoot = path.resolve(__dirname, '../../../..');
  const baseline = JSON.parse(fs.readFileSync(path.join(repoRoot, 'scripts/node/check-rust-backend/baseline.json'), 'utf8'));
  assert.equal(
    baseline.allowedFindings.some(entry => ['json-model-provider-catalog', 'json-model-provider-options'].includes(entry.id)),
    false
  );
  const file = 'api/crates/api/plugins-models/src/routes/plugins_and_models/model_providers/response_mapping.rs';
  const content = fs.readFileSync(path.join(repoRoot, file), 'utf8');
  assert.deepEqual(scanRustSource({ relativePath: file, content }), []);
});

test('CRLF checkouts apply all formal repository reviews identically to LF checkouts', (t) => {
  const repoRoot = path.resolve(__dirname, '../../../..');
  const baseline = JSON.parse(fs.readFileSync(path.join(repoRoot, 'scripts/node/check-rust-backend/baseline.json'), 'utf8'));
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'crlf-baseline-reviews-'));
  t.after(() => fs.rmSync(root, { recursive: true, force: true }));
  const writeCrlf = (relative, text) => {
    const target = path.join(root, relative);
    fs.mkdirSync(path.dirname(target), { recursive: true });
    fs.writeFileSync(target, text.replace(/\r?\n/gu, '\r\n'), 'utf8');
  };
  for (const entry of baseline.allowedFindings) {
    for (const file of [entry.file, ...entry.evidence.map(item => item.file)]) {
      writeCrlf(file, fs.readFileSync(path.join(repoRoot, file), 'utf8'));
    }
  }
  writeCrlf('scripts/node/check-rust-backend/baseline.json', JSON.stringify(baseline, null, 2));
  const findings = collectRustBackendFindings({ repoRoot: root, includeSuppressed: true });
  assert.deepEqual(findings.filter(item => /^(?:invalid|retired)-baseline-review$/u.test(item.rule)), []);
  assert.deepEqual(
    findings.filter(item => item.suppressed).map(item => item.reviewId).sort(),
    baseline.allowedFindings.map(entry => entry.id).sort()
  );
  assert.equal(findings.some(item => !item.suppressed && item.severity === 'error'), false);
});

test('anchor-scoped reviews tolerate unrelated multi-file edits while failing closed on anchor mutations', (t) => {
  const f = fixture(t);
  const sourceAnchor = source.trim();
  const callerAnchor = caller.trim();
  f.write(sourceFile, `pub fn unrelated_helper() -> usize { 1 }\n${source}`);
  f.write(callerFile, `pub fn unrelated_caller_helper() {}\n${caller}`);
  f.baseline([{
    ...f.review,
    source_anchor: sourceAnchor,
    source_sha256: sha256(sourceAnchor),
    evidence: [{
      file: callerFile,
      anchor: callerAnchor,
      sha256: sha256(callerAnchor),
      reason: 'actual offload caller',
    }],
  }]);
  let findings = collectRustBackendFindings({ repoRoot: f.root, includeSuppressed: true });
  assert.equal(original(findings).suppressed, true);
  assert.deepEqual(findings.filter(item => item.rule === 'invalid-baseline-review'), []);

  // Unrelated edits outside the anchored items in both files do not break the review.
  f.write(sourceFile, `pub fn unrelated_helper() -> usize { 42 }\n${source}\npub fn extra() {}\n`);
  f.write(callerFile, `pub fn unrelated_caller_helper() { let _x = 9; }\n${caller}\n// extra footer\n`);
  findings = collectRustBackendFindings({ repoRoot: f.root, includeSuppressed: true });
  assert.equal(original(findings).suppressed, true);
  assert.deepEqual(findings.filter(item => item.rule === 'invalid-baseline-review'), []);

  // Mutating the reviewed caller boundary fails closed immediately.
  f.write(callerFile, 'async fn admit() { read(); }\n');
  findings = collectRustBackendFindings({ repoRoot: f.root, includeSuppressed: true });
  assert.equal(original(findings).suppressed, false);
  assert.ok(findings.some(item => item.rule === 'invalid-baseline-review'
    && item.reviewProblems.some(problem => problem.includes(`SHA256 changed: ${callerFile}`))));
});

test('host construction and managed hashing reviews reject changed runtime boundaries', (t) => {
  const repoRoot = path.resolve(__dirname, '../../../..');
  const baseline = JSON.parse(fs.readFileSync(path.join(repoRoot, 'scripts/node/check-rust-backend/baseline.json'), 'utf8'));
  const entries = baseline.allowedFindings.filter(entry =>
    ['host-fingerprint-construction', 'managed-executable-blocking-pool'].includes(entry.id));
  assert.equal(entries.length, 2);
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'host-boundary-reviews-'));
  t.after(() => fs.rmSync(root, { recursive: true, force: true }));
  function write(file, bytes) {
    fs.mkdirSync(path.dirname(path.join(root, file)), { recursive: true });
    fs.writeFileSync(path.join(root, file), bytes);
  }
  for (const entry of entries) {
    for (const [file, hash, anchor] of [[entry.file, entry.source_sha256, entry.source_anchor],
      ...entry.evidence.map(item => [item.file, item.sha256, item.anchor])]) {
      const bytes = fs.readFileSync(path.join(repoRoot, file));
      assert.equal(computeReviewTargetSha256(bytes, anchor), hash, file);
      write(file, bytes);
    }
  }
  write('scripts/node/check-rust-backend/baseline.json', JSON.stringify({ ...baseline, allowedFindings: entries }));
  const findings = () => collectRustBackendFindings({ repoRoot: root, includeSuppressed: true });
  const reviewed = (report, entry) => report.find(item => item.file === entry.file
    && item.rule === entry.rule && item.snippet.trim() === entry.snippet.trim());
  const current = findings();
  assert.deepEqual(current.filter(item => item.suppressed).map(item => item.reviewId).sort(),
    entries.map(entry => entry.id).sort());
  assert.deepEqual(current.filter(item => /^(?:invalid|retired)-baseline-review$/u.test(item.rule)), []);
  for (const [file, before, after, reviewId] of [
    ['api/crates/runtime-profile/src/profile.rs', 'host_fingerprint: self.host_fingerprint.clone()',
      'host_fingerprint: detect_host_fingerprint()?', 'host-fingerprint-construction'],
    ['api/crates/runtime-extension-host/src/runtime_host.rs',
      'tokio::task::spawn_blocking(move || {\n            let _permit = hash_permit;\n            binding.verify_executable()\n        })',
      'binding.verify_executable()', 'managed-executable-blocking-pool'],
  ]) {
    const original = fs.readFileSync(path.join(root, file), 'utf8');
    assert.ok(original.includes(before), `${reviewId}: current reviewed boundary`);
    write(file, original.replace(before, after));
    const changed = findings();
    assert.equal(reviewed(changed, entries.find(entry => entry.id === reviewId)).suppressed, false);
    assert.ok(changed.some(item => item.rule === 'invalid-baseline-review' && item.reviewId === reviewId
      && item.reviewProblems.some(problem => problem.includes(`SHA256 changed: ${file}`))));
    write(file, original);
  }
});
