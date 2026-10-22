const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const test = require('node:test');
const { runQualityGateAggregate, DEFAULT_AGGREGATE_SCOPES } = require('../core.js');

const candidate = 'a'.repeat(40);
const scope = 'coverage-backend-storage-postgres';
const componentName = `test-governance-${scope}`;

for (const scenario of [
  { name: 'old failed aggregate cannot fail a current passing component', live: 'passed', archived: 'failed', expected: 'passed', exitCode: 0 },
  { name: 'old passed aggregate cannot hide a current failing component', live: 'failed', archived: 'passed', expected: 'failed', exitCode: 7 },
  { name: 'old passed aggregate cannot satisfy a missing required component', live: null, archived: 'passed', expected: 'failed', exitCode: 1 },
  { name: 'old passed aggregate cannot replace a coverage owner from another candidate', live: 'passed', liveCommit: 'b'.repeat(40), archived: 'passed', expected: 'failed', exitCode: 1 },
]) {
  test(scenario.name, async (t) => {
    const repoRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'quality-gate-rerun-'));
    t.after(() => fs.rmSync(repoRoot, { recursive: true, force: true }));
    const artifactRoot = path.join(repoRoot, 'tmp/test-governance/parallel');
    const writeReceipt = (relativePath, receiptScope, status, commit = candidate) => {
      const directory = path.join(artifactRoot, relativePath);
      fs.mkdirSync(directory, { recursive: true });
      fs.writeFileSync(path.join(directory, 'quality-gate-report.json'), JSON.stringify({
        scope: receiptScope, commit, status,
        exitCode: status === 'passed' ? 0 : 7,
      }));
      fs.writeFileSync(path.join(directory, 'quality-gate.latest.log'), `${receiptScope}: ${status}\n`);
    };
    assert.equal(DEFAULT_AGGREGATE_SCOPES.length, 24);
    for (const requiredScope of DEFAULT_AGGREGATE_SCOPES.filter((value) => value !== scope)) {
      writeReceipt(`test-governance-${requiredScope}`, requiredScope, 'passed');
    }
    writeReceipt('test-governance-artifacts', 'ci', scenario.archived);
    writeReceipt(path.join('test-governance-artifacts', 'parallel', componentName), scope, scenario.archived);
    writeReceipt(path.join('test-governance-artifacts', 'parallel', 'test-governance-artifacts', 'parallel', componentName), scope, scenario.archived);
    if (scenario.live) {
      // Legitimate protocol artifacts may keep their receipt below the artifact root.
      writeReceipt(path.join(componentName, 'receipt'), scope, scenario.live, scenario.liveCommit);
    }

    const result = await runQualityGateAggregate({
      repoRoot, artifactRoot, env: { GITHUB_SHA: candidate },
    });
    const report = JSON.parse(fs.readFileSync(result.reportJsonPath, 'utf8'));
    assert.equal(result.status, scenario.expected);
    assert.equal(result.exitCode, scenario.exitCode);
    assert.equal(report.components.length, 24);
    const component = report.components.find((value) => value.scope === scope);
    assert.equal(component.status, scenario.expected);
    if (scenario.live) assert.equal(component.artifactName, componentName);
    else assert.match(component.failureExcerpt, /No quality gate artifact was downloaded/u);
    if (scenario.liveCommit) assert.match(component.failureExcerpt, /Candidate mismatch/u);
  });
}
