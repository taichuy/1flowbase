const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const test = require('node:test');
const { buildAggregateReport } = require('../core.js');
const repoRoot = path.resolve(__dirname, '../../../..');
const candidate = 'a'.repeat(40);
const artifact = (commit) => ({
  artifactName: 'test-governance-repo-tooling', scope: 'repo-tooling',
  report: { commit, status: 'passed', exitCode: 0 },
});
function report(artifacts, expected = candidate) {
  return buildAggregateReport({ repoRoot, reportType: 'ci', componentArtifacts: artifacts,
    timestamp: '2026-10-02', env: { GITHUB_SHA: expected } }).json;
}

test('aggregate accepts passing components only from the frozen candidate', () => {
  const result = report([artifact(candidate)]);
  assert.equal(result.status, 'passed');
  assert.equal(result.exitCode, 0);
  assert.equal(result.components[0].commit, candidate);
});

test('aggregate rejects mixed, missing, and unbound candidate evidence', () => {
  for (const [artifacts, expected] of [
    [[artifact(candidate), artifact('b'.repeat(40))], candidate],
    [[artifact(undefined)], candidate],
    [[artifact(candidate)], ''],
  ]) {
    const result = report(artifacts, expected);
    assert.equal(result.status, 'failed');
    assert.equal(result.exitCode, 1);
    assert.match(result.components.find((c) => c.status === 'failed').failureExcerpt, /Candidate mismatch/u);
  }
});

test('full quality jobs execute the resolver snapshot and preserve standalone scopes', () => {
  const workflow = fs.readFileSync(path.join(repoRoot, '.github/workflows/quality-gate.yml'), 'utf8');
  for (const job of ['repo-tooling-gate', 'repo-frontend-gate', 'repo-frontend-react-doctor-gate',
    'repo-backend-gate', 'backend-consistency-gate', 'coverage-frontend-gate',
    'coverage-backend-gate', 'coverage-backend-api-server-sharded',
    'coverage-backend-api-server-sharded-merge', 'container-images-gate',
    'ai-gateway-protocol-conformance', 'aggregate']) {
    const block = workflow.split(`  ${job}:\n`)[1]?.split(/\n  [a-z][a-z0-9-]*:\n/u)[0];
    assert.ok(block, `missing job: ${job}`);
    assert.match(block, /needs:(?: resolve-quality-gate-target|[\s\S]*?- resolve-quality-gate-target)/u, job);
    assert.match(block, /(?:ref|target_ref): \$\{\{ needs\.resolve-quality-gate-target\.outputs\.target_sha \}\}/u, job);
  }
  const standalone = workflow.split('  single-scope-gate:\n')[1].split('  plugin-composition-2007:')[0];
  assert.match(standalone, /ref: \$\{\{ env\.QUALITY_GATE_TARGET_BRANCH \}\}/u);
});


test('upstream macro lint policy pins only Clippy while retaining project warnings as errors', () => {
  const workflow = fs.readFileSync(path.join(repoRoot, '.github/workflows/quality-gate.yml'), 'utf8');
  assert.match(workflow, /startsWith\(matrix\.scope, 'repo-backend-clippy-'\) && '1\.98\.1' \|\| 'stable'/u);
  assert.match(workflow, /startsWith\(inputs\.scope, 'repo-backend-clippy-'\) && '1\.98\.1' \|\| 'stable'/u);
  const { buildCommands } = require('../../verify-backend.js');
  const clippy = buildCommands({ cargoJobs: 2, cargoTestThreads: 2, repoRoot, env: {}, target: 'clippy' });
  assert.ok(clippy.every((command) => command.args.slice(-2).join(' ') === '-D warnings'));
});
