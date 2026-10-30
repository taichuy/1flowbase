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


test('owner-grouped reviews reject missing, duplicated and escaping inventories', (t) => {
  const f = fixture(t);
  const file = 'scripts/node/react-doctor-gate/reviews/owner.json';
  fs.mkdirSync(path.dirname(path.join(f.repoRoot, file)), { recursive: true });
  fs.writeFileSync(path.join(f.repoRoot, file), JSON.stringify({ toolPackage: f.manifest.toolPackage, reviews: f.manifest.reviews }));
  f.manifest.schemaVersion = 2;
  delete f.manifest.reviews;
  f.manifest.reviewFiles = [file];
  f.writeManifest();
  assert.equal(f.evaluate().status, 'reviewed');
  f.manifest.reviewFiles.push(file);
  f.writeManifest();
  assert.equal(f.evaluate().status, 'requires-review');
  f.manifest.reviewFiles = [file, 'scripts/node/react-doctor-gate/reviews/missing.json'];
  f.writeManifest();
  assert.equal(f.evaluate().status, 'requires-review');
  f.manifest.reviewFiles = ['scripts/node/react-doctor-gate/reviews/../reviews/owner.json'];
  f.writeManifest();
  assert.equal(f.evaluate().status, 'requires-review');
});

test('full scans can review build contracts while rejecting paths outside the application', (t) => {
  const f = fixture(t);
  const buildFile = 'web/app/build/owner.ts';
  fs.mkdirSync(path.dirname(path.join(f.repoRoot, buildFile)), { recursive: true });
  fs.copyFileSync(path.join(f.repoRoot, f.file), path.join(f.repoRoot, buildFile));
  f.manifest.reviews[0].file = buildFile;
  f.diagnostic.filePath = 'build/owner.ts';
  f.writeManifest();
  assert.equal(f.evaluate().status, 'reviewed');
  f.diagnostic.filePath = '../../scripts/owner.ts';
  assert.equal(f.evaluate().status, 'requires-review');
  assert.match(f.evaluate().findings[0].reason, /outside/u);
});

function sharedFixture(t) {
  const f = fixture(t);
  const review = f.manifest.reviews[0];
  const proof = 'web/app/src/owner.test.tsx';
  fs.writeFileSync(path.join(f.repoRoot, proof), 'verifyStableElementIdentity();\n');
  const { evidence: _evidence, ...identity } = review;
  const group = {
    toolPackage: f.manifest.toolPackage,
    evidenceSets: [[{ file: proof, sha256: sourceDigest(fs.readFileSync(path.join(f.repoRoot, proof))) }]],
    reviews: [
      { ...identity, evidenceSet: 0 },
      { ...identity, rule: 'react-doctor/no-multi-comp', evidenceSet: 0 },
    ],
  };
  f.doctorReport.diagnostics.push({ ...f.diagnostic, rule: 'no-multi-comp' });
  f.doctorReport.summary.totalDiagnosticCount = 2;
  const groupFile = 'scripts/node/react-doctor-gate/reviews/owner.json';
  fs.mkdirSync(path.dirname(path.join(f.repoRoot, groupFile)), { recursive: true });
  const writeGroup = () => fs.writeFileSync(path.join(f.repoRoot, groupFile), JSON.stringify(group));
  f.manifest.schemaVersion = 2;
  delete f.manifest.reviews;
  f.manifest.reviewFiles = [groupFile];
  f.writeManifest();
  writeGroup();
  return { ...f, group, proof, writeGroup };
}

test('shared evidence preserves each review and invalidates all dependent reviews on proof changes', (t) => {
  const f = sharedFixture(t);
  assert.deepEqual(f.evaluate(), { status: 'reviewed', reviewed: 2, findings: [] });
  fs.appendFileSync(path.join(f.repoRoot, f.proof), 'changedContract();\n');
  const result = f.evaluate();
  assert.equal(result.reviewed, 0);
  assert.equal(result.findings.length, 2);
  assert.ok(result.findings.every((finding) => /evidence changed/u.test(finding.reason)));
});

test('missing, malformed, out-of-range or ambiguous shared proof references cannot authorize diagnostics', (t) => {
  const f = sharedFixture(t);
  const original = structuredClone(f.group);
  const mutations = [
    (group) => { delete group.evidenceSets; },
    (group) => { group.evidenceSets = {}; },
    (group) => { group.evidenceSets[0] = {}; },
    (group) => { group.evidenceSets[0] = []; },
    ...[-1, 1, 0.5, '0'].map((index) => (group) => { group.reviews[0].evidenceSet = index; }),
    (group) => { group.reviews[0].evidence = group.evidenceSets[0]; },
    (group) => { group.evidenceSets[0][0].sha256 = 'stale'; },
    (group) => { group.evidenceSets[0][0].file = '../outside.test.tsx'; },
  ];
  for (const mutate of mutations) {
    Object.assign(f.group, structuredClone(original));
    mutate(f.group);
    f.writeGroup();
    assert.equal(f.evaluate().status, 'requires-review');
    assert.ok(f.evaluate().reviewed < 2);
  }
});

test('direct and shared evidence coexist without changing legacy review semantics', (t) => {
  const f = sharedFixture(t);
  f.group.reviews[1].evidence = structuredClone(f.group.evidenceSets[0]);
  delete f.group.reviews[1].evidenceSet;
  f.writeGroup();
  assert.deepEqual(f.evaluate(), { status: 'reviewed', reviewed: 2, findings: [] });
});
