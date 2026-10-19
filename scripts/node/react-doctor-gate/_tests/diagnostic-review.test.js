const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const { REVIEW_FILE, sourceDigest, evaluateDiagnosticReviews } = require('../diagnostic-review.js');

function fixture(t) {
  const repoRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'doctor-source-review-'));
  t.after(() => fs.rmSync(repoRoot, { recursive: true, force: true }));
  const file = 'web/app/src/owner.tsx';
  const source = 'export const renderBody = () => null;\n';
  fs.mkdirSync(path.dirname(path.join(repoRoot, file)), { recursive: true });
  fs.writeFileSync(path.join(repoRoot, file), source);
  const review = {
    file, rule: 'react-doctor/no-render-in-render', sourceSha256: sourceDigest(source),
    reason: 'This is a plain function returning elements; element type and key identity remain stable.',
    evidence: [{ file, sha256: sourceDigest(source) }],
  };
  const manifest = { schemaVersion: 1, toolPackage: 'react-doctor@0.2.16', reviews: [review] };
  const writeManifest = () => {
    fs.mkdirSync(path.dirname(path.join(repoRoot, REVIEW_FILE)), { recursive: true });
    fs.writeFileSync(path.join(repoRoot, REVIEW_FILE), JSON.stringify(manifest));
  };
  writeManifest();
  const diagnostic = { filePath: 'src/owner.tsx', plugin: 'react-doctor', rule: 'no-render-in-render', line: 1 };
  const doctorReport = { diagnostics: [diagnostic], summary: { totalDiagnosticCount: 1 }, error: null };
  const evaluate = () => evaluateDiagnosticReviews({ repoRoot, doctorReport, toolPackage: 'react-doctor@0.2.16' });
  return { repoRoot, file, manifest, writeManifest, doctorReport, diagnostic, evaluate };
}

test('source review preserves diagnostics while accepting a current explicit counterexample', (t) => {
  const f = fixture(t);
  assert.deepEqual(f.evaluate(), { status: 'reviewed', reviewed: 1, findings: [] });
  assert.equal(f.doctorReport.diagnostics.length, 1);
});

test('a source change invalidates the review even when rule and location remain unchanged', (t) => {
  const f = fixture(t);
  fs.appendFileSync(path.join(f.repoRoot, f.file), 'export const NestedComponent = () => null;\n');
  assert.equal(f.evaluate().status, 'requires-review');
  assert.match(f.evaluate().findings[0].reason, /source changed/u);
});

test('a new rule in an unchanged file is not authorized by another reviewed rule', (t) => {
  const f = fixture(t);
  f.diagnostic.rule = 'no-adjust-state-on-prop-change';
  assert.equal(f.evaluate().reviewed, 0);
  assert.match(f.evaluate().findings[0].reason, /no explicit/u);
});

test('incomplete output cannot turn a green child exit into accepted evidence', (t) => {
  const f = fixture(t);
  f.doctorReport.summary.totalDiagnosticCount = 2;
  assert.equal(f.evaluate().status, 'invalid-output');
  delete f.doctorReport.diagnostics;
  assert.equal(f.evaluate().status, 'invalid-output');
});

test('tool-reported suppression cannot be accepted as a complete inventory', (t) => {
  const f = fixture(t);
  f.doctorReport.summary.suppressedDiagnosticCount = 1;
  assert.equal(f.evaluate().status, 'suppressed-output');
});

test('review records require extant evidence, a pinned tool, and an unambiguous identity', (t) => {
  const f = fixture(t);
  f.manifest.reviews[0].evidence = [{ file: 'web/app/src/deleted.tsx', sha256: 'missing' }];
  f.writeManifest();
  assert.match(f.evaluate().findings[0].reason, /disappeared|invalid/u);
  f.manifest.reviews[0].evidence = [{ file: f.file, sha256: sourceDigest(fs.readFileSync(path.join(f.repoRoot, f.file))) }];
  f.manifest.toolPackage = 'react-doctor@future';
  f.writeManifest();
  assert.match(f.evaluate().findings[0].reason, /no explicit/u);
  f.manifest.toolPackage = 'react-doctor@0.2.16';
  f.manifest.reviews.push(f.manifest.reviews[0]);
  f.writeManifest();
  assert.match(f.evaluate().findings[0].reason, /no explicit/u);
});


test('changing a contract fixture invalidates evidence for unchanged component code', (t) => {
  const f = fixture(t);
  const proof = 'web/app/src/owner.test.tsx';
  const original = 'test("preserves element identity", verifyIdentity);\n';
  fs.writeFileSync(path.join(f.repoRoot, proof), original);
  f.manifest.reviews[0].evidence = [{ file: proof, sha256: sourceDigest(original) }];
  f.writeManifest();
  assert.equal(f.evaluate().status, 'reviewed');
  fs.writeFileSync(path.join(f.repoRoot, proof), 'test("only renders", verifyRender);\n');
  assert.equal(f.evaluate().status, 'requires-review');
  assert.match(f.evaluate().findings[0].reason, /evidence changed/u);
});
