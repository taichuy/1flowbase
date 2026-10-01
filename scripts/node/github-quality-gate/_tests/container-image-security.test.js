const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');

const {
  runQualityGate,
  runQualityGateAggregate,
} = require('../core.js');

const CONTAINER_IMAGE_SECURITY_REPORT = {
  status: 'warning',
  exitCode: 0,
  highCount: 9,
  criticalCount: 5,
  reportPath: 'tmp/test-governance/container-image-security.json',
  markdownPath: 'tmp/test-governance/container-image-security.md',
  components: [
    {
      component: 'api-server',
      imageRef: 'ghcr.io/taichuy/1flowbase-api-server:scan-1',
      status: 'warning',
      highCount: 9,
      criticalCount: 5,
      evidence: [
        'tmp/test-governance/trivy-api-server-high.json',
        'tmp/test-governance/trivy-api-server-critical.json',
      ],
      topVulnerabilities: [
        {
          severity: 'CRITICAL',
          id: 'CVE-2026-33845',
          packageName: 'libgnutls30',
          installedVersion: '3.7.9-2+deb12u6',
          fixedVersion: '3.7.9-2+deb12u7',
        },
      ],
    },
  ],
};

function writeContainerImageSecurityReport(repoRoot) {
  const reportPath = path.join(repoRoot, 'tmp', 'test-governance', 'container-image-security.json');
  fs.mkdirSync(path.dirname(reportPath), { recursive: true });
  fs.writeFileSync(reportPath, `${JSON.stringify(CONTAINER_IMAGE_SECURITY_REPORT, null, 2)}\n`, 'utf8');
  fs.writeFileSync(
    path.join(repoRoot, 'tmp', 'test-governance', 'container-image-security.md'),
    '# Container Image Security Report\n',
    'utf8'
  );
}

test('runQualityGate records container image security warnings without publishing CD issues', async () => {
  const repoRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'oneflowbase-quality-gate-container-images-'));
  const createdIssues = [];

  const status = await runQualityGate({
    repoRoot,
    scope: 'container-images',
    reportType: 'cd',
    environmentName: 'container-images',
    publishIssue: false,
    githubToken: '',
    env: {
      GITHUB_ACTOR: 'taichu',
      GITHUB_REF_NAME: 'latest',
      GITHUB_REPOSITORY: 'taichuy/1flowbase',
      GITHUB_RUN_ID: '794',
      GITHUB_SERVER_URL: 'https://github.com',
      GITHUB_SHA: 'abcdef1234567890',
      GITHUB_WORKFLOW: 'container images',
    },
    spawnSyncImpl(command, args) {
      assert.equal(command, process.execPath);
      assert.deepEqual(args, [path.join(repoRoot, 'scripts', 'node', 'cli', 'container-image-security.js')]);
      writeContainerImageSecurityReport(repoRoot);
      return {
        status: 0,
        stdout: 'container image security warning\n',
        stderr: '',
      };
    },
    createIssueImpl(issue) {
      createdIssues.push(issue);
      return { html_url: 'https://github.com/taichuy/1flowbase/issues/11' };
    },
    listOpenQualityGateIssuesImpl() {
      return [];
    },
    nowImpl: () => new Date('2026-05-03T23:40:00Z'),
    writeStdout() {},
    writeStderr() {},
  });

  assert.equal(status.status, 'passed');
  assert.equal(status.exitCode, 0);
  assert.equal(createdIssues.length, 0);
  const report = JSON.parse(fs.readFileSync(path.join(repoRoot, 'tmp', 'test-governance', 'quality-gate-report.json'), 'utf8'));
  assert.equal(report.status, 'passed');
  assert.equal(report.exitCode, 0);
  assert.equal(report.issueUrl, '');
  assert.equal(report.containerImageSecurity.status, 'warning');
  assert.equal(report.containerImageSecurity.criticalCount, 5);
  assert.match(
    fs.readFileSync(path.join(repoRoot, 'tmp', 'test-governance', 'quality-gate-report.md'), 'utf8'),
    /Status: warning; Components: 1; HIGH: 9; CRITICAL: 5/u
  );
});

test('runQualityGateAggregate includes container image security reports when present', async () => {
  const repoRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'oneflowbase-quality-gate-aggregate-container-images-'));
  const artifactRoot = path.join(repoRoot, 'tmp', 'test-governance', 'parallel');
  const artifactDir = path.join(artifactRoot, 'test-governance-container-images');
  const createdIssues = [];

  fs.mkdirSync(artifactDir, { recursive: true });
  fs.writeFileSync(
    path.join(artifactDir, 'quality-gate-report.json'),
    `${JSON.stringify({
      reportType: 'cd',
      commit: 'abcdef1234567890',
      status: 'passed',
      scope: 'container-images',
      exitCode: 0,
      coverageSummaries: [],
      backendConsistencyTargets: [],
      warningFiles: [],
      containerImageSecurity: CONTAINER_IMAGE_SECURITY_REPORT,
    }, null, 2)}\n`,
    'utf8'
  );
  fs.writeFileSync(path.join(artifactDir, 'quality-gate.latest.log'), 'container-images log\n', 'utf8');

  const result = await runQualityGateAggregate({
    repoRoot,
    artifactRoot: path.join('tmp', 'test-governance', 'parallel'),
    expectedScopes: ['container-images'],
    reportType: 'cd',
    publishIssue: true,
    githubToken: 'token',
    environmentName: 'container-images',
    env: {
      GITHUB_ACTOR: 'taichu',
      GITHUB_REF_NAME: 'latest',
      GITHUB_REPOSITORY: 'taichuy/1flowbase',
      GITHUB_RUN_ID: '995',
      GITHUB_SERVER_URL: 'https://github.com',
      GITHUB_SHA: 'abcdef1234567890',
      GITHUB_WORKFLOW: 'quality gate',
    },
    createIssueImpl(issue) {
      createdIssues.push(issue);
      return { html_url: 'https://github.com/taichuy/1flowbase/issues/12' };
    },
    listOpenQualityGateIssuesImpl() {
      return [];
    },
  });

  assert.equal(result.status, 'passed');
  assert.equal(result.exitCode, 0);
  assert.match(createdIssues[0].body, /## Container Image Security/u);
  assert.match(createdIssues[0].body, /container-images: warning, components 1, HIGH 9, CRITICAL 5/u);
  assert.match(createdIssues[0].body, /\| `api-server` \| warning \| 9 \| 5 \| `ghcr\.io\/taichuy\/1flowbase-api-server:scan-1` \|/u);
  assert.match(createdIssues[0].body, /CVE-2026-33845/u);
  assert.match(createdIssues[0].body, /Container image security report: tmp\/test-governance\/container-image-security\.md/u);
  const report = JSON.parse(fs.readFileSync(path.join(repoRoot, 'tmp', 'test-governance', 'quality-gate-report.json'), 'utf8'));
  assert.equal(report.containerImageSecurityReports[0].criticalCount, 5);
});
