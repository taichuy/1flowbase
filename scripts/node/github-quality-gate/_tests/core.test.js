const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');

const {
  buildReport,
  buildGateCommand,
  boundIssueBody,
  buildIssueTitle,
  buildIssueLabels,
  COVERAGE_BACKEND_COMPONENT_SCOPES,
  REPO_BACKEND_COMPONENT_SCOPES,
  parseBooleanInput,
  runQualityGateAggregate,
  runQualityGate,
} = require('../core.js');

test('quality gate bounds GitHub Issue bodies while preserving the report summary', () => {
  const body = `# Quality Gate Report\n\n## Result Summary\n\n${'错误🙂\n'.repeat(30_000)}`;
  const bounded = boundIssueBody(body);
  assert.equal(Buffer.byteLength(bounded, 'utf8') <= 60 * 1024, true);
  assert.match(bounded, /^# Quality Gate Report/u);
  assert.match(bounded, /Issue report truncated to GitHub’s body limit/u);
  assert.doesNotMatch(bounded, /\uFFFD/u);
});

test('buildGateCommand maps supported scopes to repository verify scripts', () => {
  const repoRoot = '/repo';

  assert.deepEqual(buildGateCommand({ repoRoot, scope: 'ci' }), {
    command: process.execPath,
    args: [path.join(repoRoot, 'scripts', 'node', 'verify-ci.js')],
    cwd: repoRoot,
  });

  assert.deepEqual(buildGateCommand({ repoRoot, scope: 'coverage' }), {
    command: process.execPath,
    args: [path.join(repoRoot, 'scripts', 'node', 'verify-coverage.js'), 'all'],
    cwd: repoRoot,
  });

  assert.deepEqual(buildGateCommand({ repoRoot, scope: 'coverage-frontend' }), {
    command: process.execPath,
    args: [path.join(repoRoot, 'scripts', 'node', 'verify-coverage.js'), 'frontend'],
    cwd: repoRoot,
  });

  assert.deepEqual(buildGateCommand({ repoRoot, scope: 'coverage-backend' }), {
    command: process.execPath,
    args: [path.join(repoRoot, 'scripts', 'node', 'verify-coverage.js'), 'backend'],
    cwd: repoRoot,
  });

  assert.deepEqual(buildGateCommand({ repoRoot, scope: 'coverage-backend-api-server' }), {
    command: process.execPath,
    args: [path.join(repoRoot, 'scripts', 'node', 'verify-coverage.js'), 'backend', 'api-server'],
    cwd: repoRoot,
  });

  assert.deepEqual(buildGateCommand({
    repoRoot,
    scope: 'coverage-backend-api-server-shadow-3-of-4',
  }), {
    command: process.execPath,
    args: [path.join(repoRoot, 'scripts', 'node', 'coverage-shadow.js'), 'shard', 'api-server', '3', '4'],
    cwd: repoRoot,
  });

  assert.deepEqual(buildGateCommand({ repoRoot, scope: 'repo-tooling' }), {
    command: process.execPath,
    args: [path.join(repoRoot, 'scripts', 'node', 'verify-repo.js'), 'tooling'],
    cwd: repoRoot,
  });

  assert.deepEqual(buildGateCommand({ repoRoot, scope: 'repo-frontend' }), {
    command: process.execPath,
    args: [path.join(repoRoot, 'scripts', 'node', 'verify-repo.js'), 'frontend'],
    cwd: repoRoot,
  });

  assert.deepEqual(buildGateCommand({ repoRoot, scope: 'repo-frontend-pr' }), {
    command: process.execPath,
    args: [path.join(repoRoot, 'scripts', 'node', 'verify-repo.js'), 'frontend-pr'],
    cwd: repoRoot,
  });

  assert.deepEqual(buildGateCommand({ repoRoot, scope: 'repo-frontend-react-doctor' }), {
    command: process.execPath,
    args: [path.join(repoRoot, 'scripts', 'node', 'react-doctor-gate.js')],
    cwd: repoRoot,
  });

  assert.deepEqual(buildGateCommand({ repoRoot, scope: 'repo-backend' }), {
    command: process.execPath,
    args: [path.join(repoRoot, 'scripts', 'node', 'verify-repo.js'), 'backend'],
    cwd: repoRoot,
  });

  assert.deepEqual(buildGateCommand({ repoRoot, scope: 'repo-backend-static' }), {
    command: process.execPath,
    args: [path.join(repoRoot, 'scripts', 'node', 'verify-backend.js'), 'static'],
    cwd: repoRoot,
  });

  assert.deepEqual(buildGateCommand({ repoRoot, scope: 'repo-backend-fmt' }), {
    command: process.execPath,
    args: [path.join(repoRoot, 'scripts', 'node', 'verify-backend.js'), 'fmt'],
    cwd: repoRoot,
  });

  assert.deepEqual(buildGateCommand({ repoRoot, scope: 'repo-backend-image-llm-vision' }), {
    command: process.execPath,
    args: [path.join(repoRoot, 'scripts', 'node', 'verify-backend.js'), 'image-llm-vision'],
    cwd: repoRoot,
  });

  assert.deepEqual(buildGateCommand({ repoRoot, scope: 'repo-backend-official-i18n-seed' }), {
    command: process.execPath,
    args: [path.join(repoRoot, 'scripts', 'node', 'verify-backend.js'), 'official-i18n-seed'],
    cwd: repoRoot,
  });

  assert.deepEqual(buildGateCommand({ repoRoot, scope: 'repo-backend-test-api-server-1-of-4' }), {
    command: process.execPath,
    args: [path.join(repoRoot, 'scripts', 'node', 'verify-backend.js'), 'test', 'api-server-1-of-4'],
    cwd: repoRoot,
  });

  assert.deepEqual(buildGateCommand({ repoRoot, scope: 'backend-consistency' }), {
    command: process.execPath,
    args: [path.join(repoRoot, 'scripts', 'node', 'cli', 'verify-backend-consistency.js')],
    cwd: repoRoot,
  });

  assert.deepEqual(buildGateCommand({ repoRoot, scope: 'state-protocols' }), {
    command: process.execPath,
    args: [path.join(repoRoot, 'scripts', 'node', 'verify-state-protocols.js'), '--skip-live-acp'],
    cwd: repoRoot,
  });

  assert.deepEqual(buildGateCommand({ repoRoot, scope: 'container-images' }), {
    command: process.execPath,
    args: [path.join(repoRoot, 'scripts', 'node', 'cli', 'container-image-security.js')],
    cwd: repoRoot,
  });

  assert.deepEqual(buildGateCommand({ repoRoot, scope: 'release-rollback' }), {
    command: process.execPath,
    args: [path.join(repoRoot, 'scripts', 'node', 'release-rollback-gate.js')],
    cwd: repoRoot,
  });

  assert.throws(
    () => buildGateCommand({ repoRoot, scope: 'unknown' }),
    /Unknown quality gate scope: unknown/u
  );
});

test('parseBooleanInput accepts GitHub action boolean strings only', () => {
  assert.equal(parseBooleanInput('true'), true);
  assert.equal(parseBooleanInput('false'), false);
  assert.equal(parseBooleanInput(undefined), false);
  assert.throws(() => parseBooleanInput('yes'), /Expected boolean input/u);
});

test('issue title and labels describe one report run', () => {
  const title = buildIssueTitle({
    reportType: 'ci',
    timestamp: '2026-05-03 23:40',
    branch: 'main',
    shortSha: 'abc1234',
    status: 'failed',
    environment: '',
  });

  assert.equal(title, '[Quality Gate][CI] 2026-05-03 23:40 main abc1234 failed');
  assert.deepEqual(buildIssueLabels({ reportType: 'ci', status: 'failed' }), [
    'quality-gate',
    'ci-report',
    'failed',
  ]);
});

test('runQualityGate writes reports and does not create an issue when publishing is disabled', async () => {
  const repoRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'oneflowbase-quality-gate-'));
  const createdIssues = [];

  const status = await runQualityGate({
    repoRoot,
    scope: 'backend',
    reportType: 'ci',
    publishIssue: false,
    env: {
      GITHUB_ACTOR: 'taichu',
      GITHUB_REF_NAME: 'main',
      GITHUB_REPOSITORY: 'taichuy/1flowbase',
      GITHUB_RUN_ID: '123',
      GITHUB_SERVER_URL: 'https://github.com',
      GITHUB_SHA: 'abcdef1234567890',
      GITHUB_WORKFLOW: 'quality gate',
    },
    spawnSyncImpl(command, args, options) {
      assert.equal(command, process.execPath);
      assert.deepEqual(args, [path.join(repoRoot, 'scripts', 'node', 'verify-backend.js')]);
      assert.equal(options.cwd, repoRoot);
      return {
        status: 0,
        stdout: 'backend passed\n',
        stderr: '',
      };
    },
    createIssueImpl(issue) {
      createdIssues.push(issue);
      return { html_url: 'https://github.com/taichuy/1flowbase/issues/1' };
    },
    writeStdout() {},
    writeStderr() {},
  });

  assert.equal(status.exitCode, 0);
  assert.equal(status.status, 'passed');
  assert.equal(status.issueUrl, '');
  assert.equal(createdIssues.length, 0);
  assert.equal(fs.existsSync(path.join(repoRoot, 'tmp', 'test-governance', 'quality-gate-report.md')), true);
  assert.equal(fs.existsSync(path.join(repoRoot, 'tmp', 'test-governance', 'quality-gate-report.json')), true);
  assert.match(
    fs.readFileSync(path.join(repoRoot, 'tmp', 'test-governance', 'quality-gate.latest.log'), 'utf8'),
    /backend passed/u
  );
});

test('runQualityGate creates a new issue when publishing is enabled even if the gate fails', async () => {
  const repoRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'oneflowbase-quality-gate-'));
  const createdIssues = [];

  const status = await runQualityGate({
    repoRoot,
    scope: 'repo',
    reportType: 'cd',
    environmentName: 'staging',
    publishIssue: true,
    githubToken: 'token',
    env: {
      GITHUB_ACTOR: 'taichu',
      GITHUB_REF_NAME: 'release',
      GITHUB_REPOSITORY: 'taichuy/1flowbase',
      GITHUB_RUN_ID: '456',
      GITHUB_SERVER_URL: 'https://github.com',
      GITHUB_SHA: '1234567890abcdef',
      GITHUB_WORKFLOW: 'quality gate',
    },
    spawnSyncImpl() {
      return {
        status: 1,
        stdout: 'repo failed\n',
        stderr: 'failure detail\n',
      };
    },
    createIssueImpl(issue) {
      createdIssues.push(issue);
      return { html_url: 'https://github.com/taichuy/1flowbase/issues/2' };
    },
    listOpenQualityGateIssuesImpl() {
      return [];
    },
    nowImpl: () => new Date('2026-05-03T15:40:00Z'),
    writeStdout() {},
    writeStderr() {},
  });

  assert.equal(status.exitCode, 1);
  assert.equal(status.status, 'failed');
  assert.equal(status.issueUrl, 'https://github.com/taichuy/1flowbase/issues/2');
  assert.equal(createdIssues.length, 1);
  assert.equal(createdIssues[0].title, '[Quality Gate][CD] 2026-05-03 15:40 staging 1234567 failed');
  assert.deepEqual(createdIssues[0].labels, ['quality-gate', 'cd-report', 'failed']);
  assert.match(createdIssues[0].body, /Status: failed/u);
  assert.match(createdIssues[0].body, /- Environment: staging/u);
  assert.match(createdIssues[0].body, /## Failure Excerpt/u);
  assert.match(createdIssues[0].body, /failure detail/u);
});

test('runQualityGate publishes a complete passed report with coverage details', async () => {
  const repoRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'oneflowbase-quality-gate-complete-'));
  const createdIssues = [];

  const writeJson = (relativePath, value) => {
    const filePath = path.join(repoRoot, relativePath);
    fs.mkdirSync(path.dirname(filePath), { recursive: true });
    fs.writeFileSync(filePath, `${JSON.stringify(value, null, 2)}\n`, 'utf8');
  };

  const frontendSummary = {
    total: {
      lines: { total: 100, covered: 82, pct: 82 },
      functions: { total: 40, covered: 32, pct: 80 },
      statements: { total: 100, covered: 82, pct: 82 },
      branches: { total: 50, covered: 39, pct: 78 },
    },
  };
  const backendSummary = {
    data: [{
      totals: {
        lines: { count: 1000, covered: 930, percent: 93 },
        functions: { count: 120, covered: 108, percent: 90 },
        branches: { count: 80, covered: 64, percent: 80 },
        regions: { count: 1500, covered: 1350, percent: 90 },
      },
    }],
  };

  const status = await runQualityGate({
    repoRoot,
    scope: 'ci',
    reportType: 'ci',
    publishIssue: true,
    githubToken: 'token',
    env: {
      GITHUB_ACTOR: 'taichu',
      GITHUB_REF_NAME: 'latest',
      GITHUB_REPOSITORY: 'taichuy/1flowbase',
      GITHUB_RUN_ID: '791',
      GITHUB_SERVER_URL: 'https://github.com',
      GITHUB_SHA: 'abcdef1234567890',
      GITHUB_WORKFLOW: 'verify',
    },
    spawnSyncImpl() {
      writeJson('tmp/test-governance/coverage/frontend/coverage-summary.json', frontendSummary);
      writeJson('tmp/test-governance/coverage/backend/api-server.json', backendSummary);
      return {
        status: 0,
        stdout: 'repo passed\nCoverage thresholds passed for all.\n',
        stderr: '',
      };
    },
    createIssueImpl(issue) {
      createdIssues.push(issue);
      return { html_url: 'https://github.com/taichuy/1flowbase/issues/5' };
    },
    listOpenQualityGateIssuesImpl() {
      return [];
    },
    nowImpl: () => new Date('2026-05-03T23:40:00Z'),
    writeStdout() {},
    writeStderr() {},
  });

  assert.equal(status.status, 'passed');
  assert.equal(createdIssues.length, 1);
  assert.match(createdIssues[0].body, /## Result Summary/u);
  assert.match(createdIssues[0].body, /- Status: passed/u);
  assert.match(createdIssues[0].body, /- Exit code: 0/u);
  assert.match(createdIssues[0].body, /## Warnings/u);
  assert.match(createdIssues[0].body, /No warning logs were captured/u);
  assert.match(createdIssues[0].body, /## Coverage/u);
  assert.match(createdIssues[0].body, /frontend total: lines 82\.00%, functions 80\.00%, statements 82\.00%, branches 78\.00%/u);
  assert.match(createdIssues[0].body, /api-server: lines 93\.00%, functions 90\.00%, branches 80\.00%, regions 90\.00%/u);
  assert.match(createdIssues[0].body, /## Evidence/u);
  assert.match(createdIssues[0].body, /tmp\/test-governance\/quality-gate\.latest\.log/u);
});

test('runQualityGate keeps warning-only output advisory when the gate exits cleanly', async () => {
  const repoRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'oneflowbase-quality-gate-warning-'));
  const createdIssues = [];

  const status = await runQualityGate({
    repoRoot,
    scope: 'repo',
    reportType: 'ci',
    publishIssue: true,
    githubToken: 'token',
    env: {
      GITHUB_ACTOR: 'taichu',
      GITHUB_REF_NAME: 'latest',
      GITHUB_REPOSITORY: 'taichuy/1flowbase',
      GITHUB_RUN_ID: '792',
      GITHUB_SERVER_URL: 'https://github.com',
      GITHUB_SHA: 'abcdef1234567890',
      GITHUB_WORKFLOW: 'quality gate',
    },
    spawnSyncImpl() {
      const warningPath = path.join(repoRoot, 'tmp', 'test-governance', 'repo.warnings.log');
      fs.mkdirSync(path.dirname(warningPath), { recursive: true });
      fs.writeFileSync(warningPath, 'warning detail\n', 'utf8');
      return {
        status: 0,
        stdout: 'repo passed\n',
        stderr: '',
      };
    },
    createIssueImpl(issue) {
      createdIssues.push(issue);
      return { html_url: 'https://github.com/taichuy/1flowbase/issues/9' };
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
  assert.equal(createdIssues[0].title, '[Quality Gate][CI] 2026-05-03 23:40 latest abcdef1 passed');
  assert.deepEqual(createdIssues[0].labels, ['quality-gate', 'ci-report', 'passed']);
  assert.match(createdIssues[0].body, /- Status: passed/u);
  assert.match(createdIssues[0].body, /- Warning log: tmp\/test-governance\/repo\.warnings\.log/u);
});

test('runQualityGate includes security risk findings as advisory report evidence', async () => {
  const repoRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'oneflowbase-quality-gate-security-risk-'));
  const createdIssues = [];

  const status = await runQualityGate({
    repoRoot,
    scope: 'repo-tooling',
    reportType: 'ci',
    publishIssue: true,
    githubToken: 'token',
    env: {
      GITHUB_ACTOR: 'taichu',
      GITHUB_REF_NAME: 'latest',
      GITHUB_REPOSITORY: 'taichuy/1flowbase',
      GITHUB_RUN_ID: '793',
      GITHUB_SERVER_URL: 'https://github.com',
      GITHUB_SHA: 'abcdef1234567890',
      GITHUB_WORKFLOW: 'verify',
    },
    spawnSyncImpl() {
      const reportPath = path.join(repoRoot, 'tmp', 'test-governance', 'security-risk.json');
      fs.mkdirSync(path.dirname(reportPath), { recursive: true });
      fs.writeFileSync(
        reportPath,
        `${JSON.stringify({
          status: 'review_required',
          changedFiles: ['web/pnpm-lock.yaml', 'web/app/src/api.ts'],
          findings: [
            { severity: 'medium', kind: 'sensitive-file-changed', file: 'web/pnpm-lock.yaml' },
            { severity: 'high', kind: 'insecure-url', file: 'web/app/src/api.ts' },
          ],
        }, null, 2)}\n`,
        'utf8'
      );
      return {
        status: 0,
        stdout: 'repo tooling passed\n',
        stderr: '',
      };
    },
    createIssueImpl(issue) {
      createdIssues.push(issue);
      return { html_url: 'https://github.com/taichuy/1flowbase/issues/10' };
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
  assert.match(createdIssues[0].body, /## Security Risk/u);
  assert.match(createdIssues[0].body, /Status: review_required/u);
  assert.match(createdIssues[0].body, /Findings: 2 \(high 1, medium 1\)/u);
  assert.match(createdIssues[0].body, /Changed files: 2/u);
  assert.match(createdIssues[0].body, /Security risk report: tmp\/test-governance\/security-risk\.json/u);
  const report = JSON.parse(fs.readFileSync(path.join(repoRoot, 'tmp', 'test-governance', 'quality-gate-report.json'), 'utf8'));
  assert.deepEqual(report.securityRisk, {
    status: 'review_required',
    changedFileCount: 2,
    findingCount: 2,
    highCount: 1,
    mediumCount: 1,
    reportPath: 'tmp/test-governance/security-risk.json',
  });
});

test('runQualityGate renders unavailable coverage metrics as n/a', async () => {
  const repoRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'oneflowbase-quality-gate-coverage-na-'));
  const createdIssues = [];
  const coveragePath = path.join(
    repoRoot,
    'tmp',
    'test-governance',
    'coverage',
    'backend',
    'api-server.json'
  );

  fs.mkdirSync(path.dirname(coveragePath), { recursive: true });
  fs.writeFileSync(
    coveragePath,
    `${JSON.stringify({
      data: [{
        totals: {
          lines: { count: 10, covered: 9, percent: 90 },
          branches: { count: 0, covered: 0, percent: 0 },
        },
      }],
    })}\n`,
    'utf8'
  );

  await runQualityGate({
    repoRoot,
    scope: 'ci',
    reportType: 'ci',
    publishIssue: true,
    githubToken: 'token',
    env: {
      GITHUB_ACTOR: 'taichu',
      GITHUB_REF_NAME: 'latest',
      GITHUB_REPOSITORY: 'taichuy/1flowbase',
      GITHUB_RUN_ID: '792',
      GITHUB_SERVER_URL: 'https://github.com',
      GITHUB_SHA: 'abcdef1234567890',
      GITHUB_WORKFLOW: 'verify',
    },
    spawnSyncImpl() {
      return {
        status: 0,
        stdout: 'repo passed\n',
        stderr: '',
      };
    },
    createIssueImpl(issue) {
      createdIssues.push(issue);
      return { html_url: 'https://github.com/taichuy/1flowbase/issues/6' };
    },
    listOpenQualityGateIssuesImpl() {
      return [];
    },
    writeStdout() {},
    writeStderr() {},
  });

  assert.match(createdIssues[0].body, /api-server: lines 90\.00%, functions n\/a, branches n\/a, regions n\/a/u);
});

test('buildReport includes backend consistency target results for consistency scopes', () => {
  const repoRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'oneflowbase-quality-gate-targets-'));
  const targetReportPath = path.join(
    repoRoot,
    'tmp',
    'test-governance',
    'backend-consistency-targets.json'
  );
  fs.mkdirSync(path.dirname(targetReportPath), { recursive: true });
  fs.writeFileSync(
    targetReportPath,
    `${JSON.stringify({
      targets: [
        {
          label: 'consistency-control-plane-state-transitions',
          packageName: 'control-plane',
          filter: 'state_transition_tests',
          status: 'passed',
          exitCode: 0,
          durationMs: 1250,
          passedCount: 3,
          failedCount: 0,
        },
        {
          label: 'consistency-storage-model-definition-repository',
          packageName: 'storage-postgres',
          filter: 'model_definition_repository_tests',
          status: 'failed',
          exitCode: 101,
          durationMs: 2300,
          passedCount: 2,
          failedCount: 1,
        },
      ],
    }, null, 2)}\n`,
    'utf8'
  );

  const report = buildReport({
    repoRoot,
    reportType: 'ci',
    scope: 'backend-consistency',
    status: 'passed',
    exitCode: 0,
    issueUrl: '',
    environmentName: 'nightly-latest',
    timestamp: '2026-05-07 02:32',
    env: {
      GITHUB_ACTOR: 'taichu',
      GITHUB_REF_NAME: 'latest',
      GITHUB_REPOSITORY: 'taichuy/1flowbase',
      GITHUB_RUN_ID: '25472497763',
      GITHUB_SERVER_URL: 'https://github.com',
      GITHUB_SHA: '017e740a685939bf03b88112ca5623f57127bafb',
      GITHUB_WORKFLOW: 'quality gate',
    },
  });

  assert.match(report.markdown, /## Backend Consistency Targets/u);
  assert.match(report.markdown, /\| Label \| Package \| Rust test filter \| Status \| Duration \| Passed \| Failed \|/u);
  assert.match(report.markdown, /\| `consistency-storage-model-definition-repository` \| `storage-postgres` \| `model_definition_repository_tests` \| failed \| 2\.30s \| 2 \| 1 \|/u);
  assert.equal(report.json.backendConsistencyTargets.length, 2);
  assert.deepEqual(report.json.backendConsistencyTargets[0], {
    label: 'consistency-control-plane-state-transitions',
    packageName: 'control-plane',
    filter: 'state_transition_tests',
    status: 'passed',
    exitCode: 0,
    durationMs: 1250,
    passedCount: 3,
    failedCount: 0,
  });
});

test('runQualityGate closes older open quality gate issues after publishing the latest report', async () => {
  const repoRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'oneflowbase-quality-gate-'));
  const closedIssues = [];

  const status = await runQualityGate({
    repoRoot,
    scope: 'repo',
    reportType: 'ci',
    publishIssue: true,
    githubToken: 'token',
    env: {
      GITHUB_ACTOR: 'taichu',
      GITHUB_REF_NAME: 'main',
      GITHUB_REPOSITORY: 'taichuy/1flowbase',
      GITHUB_RUN_ID: '457',
      GITHUB_SERVER_URL: 'https://github.com',
      GITHUB_SHA: '1234567890abcdef',
      GITHUB_WORKFLOW: 'quality gate',
    },
    spawnSyncImpl() {
      return {
        status: 0,
        stdout: 'repo passed\n',
        stderr: '',
      };
    },
    createIssueImpl() {
      return {
        html_url: 'https://github.com/taichuy/1flowbase/issues/12',
        number: 12,
        title: '[Quality Gate][CI] 2026-05-06 12:49 main 1234567 passed',
      };
    },
    listOpenQualityGateIssuesImpl() {
      return [
        {
          number: 10,
          title: '[Quality Gate][CI] 2026-05-06 10:24 main abc1234 passed',
          html_url: 'https://github.com/taichuy/1flowbase/issues/10',
        },
        {
          number: 11,
          title: '[Quality Gate][CI] 2026-05-06 10:24 latest def5678 passed',
          html_url: 'https://github.com/taichuy/1flowbase/issues/11',
        },
        {
          number: 12,
          title: '[Quality Gate][CI] 2026-05-06 12:49 main 1234567 passed',
          html_url: 'https://github.com/taichuy/1flowbase/issues/12',
        },
        {
          number: 14,
          title: '[Quality Gate][CD] 2026-05-06 10:24 main abc1234 passed',
          html_url: 'https://github.com/taichuy/1flowbase/issues/14',
        },
        {
          number: 15,
          title: 'Manual quality note',
          html_url: 'https://github.com/taichuy/1flowbase/issues/15',
        },
        {
          number: 13,
          title: '[Quality Gate][CI] 2026-05-06 10:24 main abc1234 failed',
          html_url: 'https://github.com/taichuy/1flowbase/pull/13',
          pull_request: {
            html_url: 'https://github.com/taichuy/1flowbase/pull/13',
          },
        },
      ];
    },
    closeIssueImpl(issue) {
      closedIssues.push(issue.number);
      return {};
    },
    writeStdout() {},
    writeStderr() {},
  });

  assert.equal(status.issueUrl, 'https://github.com/taichuy/1flowbase/issues/12');
  assert.deepEqual(closedIssues, [10]);
});

test('runQualityGate strips ANSI control sequences from published failure excerpts', async () => {
  const repoRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'oneflowbase-quality-gate-'));
  const createdIssues = [];

  await runQualityGate({
    repoRoot,
    scope: 'ci',
    reportType: 'ci',
    publishIssue: true,
    githubToken: 'token',
    env: {
      GITHUB_ACTOR: 'taichu',
      GITHUB_REF_NAME: 'main',
      GITHUB_REPOSITORY: 'taichuy/1flowbase',
      GITHUB_RUN_ID: '789',
      GITHUB_SERVER_URL: 'https://github.com',
      GITHUB_SHA: 'abcdef1234567890',
      GITHUB_WORKFLOW: 'verify',
    },
    spawnSyncImpl() {
      return {
        status: 1,
        stdout: '\u001b[2mdist/\u001b[22m\u001b[36masset.js\u001b[39m\n',
        stderr: '\u001b[31mDiff in api/apps/api-server/src/_tests/config_tests.rs:77\u001b[39m\n',
      };
    },
    createIssueImpl(issue) {
      createdIssues.push(issue);
      return { html_url: 'https://github.com/taichuy/1flowbase/issues/3' };
    },
    listOpenQualityGateIssuesImpl() {
      return [];
    },
    writeStdout() {},
    writeStderr() {},
  });

  assert.equal(createdIssues.length, 1);
  assert.doesNotMatch(createdIssues[0].body, /\u001b\[/u);
  assert.match(createdIssues[0].body, /dist\/asset\.js/u);
  assert.match(createdIssues[0].body, /Diff in api\/apps\/api-server\/src\/_tests\/config_tests\.rs:77/u);
});

test('runQualityGate publishes the rust failure block when cargo stderr hides it at the tail', async () => {
  const repoRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'oneflowbase-quality-gate-'));
  const createdIssues = [];
  const compileTail = Array.from({ length: 100 }, (_, index) => `Compiling crate-${index}`).join('\n');

  await runQualityGate({
    repoRoot,
    scope: 'ci',
    reportType: 'ci',
    publishIssue: true,
    githubToken: 'token',
    env: {
      GITHUB_ACTOR: 'taichu',
      GITHUB_REF_NAME: 'main',
      GITHUB_REPOSITORY: 'taichuy/1flowbase',
      GITHUB_RUN_ID: '790',
      GITHUB_SERVER_URL: 'https://github.com',
      GITHUB_SHA: 'abcdef1234567890',
      GITHUB_WORKFLOW: 'verify',
    },
    spawnSyncImpl() {
      return {
        status: 1,
        stdout: [
          'test _tests::workspace_routes::workspaces_route_lists_accessible_workspaces_with_current_marker ... FAILED',
          '',
          'failures:',
          '',
          "thread '_tests::workspace_routes::workspaces_route_lists_accessible_workspaces_with_current_marker' panicked at apps/api-server/src/_tests/support/auth.rs:83:10:",
          'called `Result::unwrap()` on an `Err` value: PoolTimedOut',
          '',
          'test result: FAILED. 68 passed; 111 failed; 0 ignored; 0 measured; 0 filtered out; finished in 3330.75s',
        ].join('\n'),
        stderr: `${compileTail}\nerror: test failed, to rerun pass \`-p api-server --lib\`\n`,
      };
    },
    createIssueImpl(issue) {
      createdIssues.push(issue);
      return { html_url: 'https://github.com/taichuy/1flowbase/issues/4' };
    },
    listOpenQualityGateIssuesImpl() {
      return [];
    },
    writeStdout() {},
    writeStderr() {},
  });

  assert.equal(createdIssues.length, 1);
  assert.match(createdIssues[0].body, /PoolTimedOut/u);
  assert.match(createdIssues[0].body, /test result: FAILED\. 68 passed; 111 failed/u);
  assert.doesNotMatch(createdIssues[0].body, /Compiling crate-99/u);
});

test('runQualityGateAggregate publishes one report from parallel quality gate artifacts', async () => {
  const repoRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'oneflowbase-quality-gate-aggregate-'));
  const artifactRoot = path.join(repoRoot, 'tmp', 'test-governance', 'parallel');
  const createdIssues = [];

  const writeArtifact = (artifactName, report) => {
    const artifactDir = path.join(artifactRoot, artifactName);
    fs.mkdirSync(artifactDir, { recursive: true });
    fs.writeFileSync(
      path.join(artifactDir, 'quality-gate-report.json'),
      `${JSON.stringify(report, null, 2)}\n`,
      'utf8'
    );
    fs.writeFileSync(path.join(artifactDir, 'quality-gate.latest.log'), `${report.scope} log\n`, 'utf8');
  };

  writeArtifact('test-governance-repo-tooling', {
    reportType: 'ci',
    status: 'passed',
    scope: 'repo-tooling',
    exitCode: 0,
    coverageSummaries: [],
    backendConsistencyTargets: [],
    warningFiles: [],
    securityRisk: {
      status: 'review_required',
      changedFileCount: 2,
      findingCount: 2,
      highCount: 1,
      mediumCount: 1,
      reportPath: 'tmp/test-governance/security-risk.json',
    },
  });
  writeArtifact('test-governance-repo-frontend', {
    reportType: 'ci',
    status: 'passed',
    scope: 'repo-frontend',
    exitCode: 0,
    coverageSummaries: [],
    backendConsistencyTargets: [],
    warningFiles: [],
  });
  for (const scope of REPO_BACKEND_COMPONENT_SCOPES) {
    writeArtifact(`test-governance-${scope}`, {
      reportType: 'ci',
      status: 'passed',
      scope,
      exitCode: 0,
      coverageSummaries: [],
      backendConsistencyTargets: [],
      warningFiles: [],
    });
  }
  writeArtifact('test-governance-backend-consistency', {
    reportType: 'ci',
    status: 'passed',
    scope: 'backend-consistency',
    exitCode: 0,
    coverageSummaries: [],
    backendConsistencyTargets: [{
      label: 'consistency-runtime-engine',
      packageName: 'runtime-core',
      filter: 'runtime_engine_tests',
      status: 'passed',
      exitCode: 0,
      durationMs: 250,
      passedCount: 9,
      failedCount: 0,
    }],
    warningFiles: [],
  });
  writeArtifact('test-governance-coverage-frontend', {
    reportType: 'ci',
    status: 'passed',
    scope: 'coverage-frontend',
    exitCode: 0,
    coverageSummaries: [{
      name: 'frontend total',
      kind: 'frontend',
      path: 'tmp/test-governance/coverage/frontend/coverage-summary.json',
      metrics: {
        lines: 80,
        functions: 75,
        statements: 80,
        branches: 78,
      },
    }],
    backendConsistencyTargets: [],
    warningFiles: [],
  });
  for (const scope of COVERAGE_BACKEND_COMPONENT_SCOPES) {
    writeArtifact(`test-governance-${scope}`, {
      reportType: 'ci',
      status: 'passed',
      scope,
      exitCode: 0,
      coverageSummaries: [],
      backendConsistencyTargets: [],
      warningFiles: [],
    });
  }

  const result = await runQualityGateAggregate({
    repoRoot,
    artifactRoot: path.join('tmp', 'test-governance', 'parallel'),
    reportType: 'ci',
    publishIssue: true,
    githubToken: 'token',
    env: {
      GITHUB_ACTOR: 'taichu',
      GITHUB_REF_NAME: 'latest',
      GITHUB_REPOSITORY: 'taichuy/1flowbase',
      GITHUB_RUN_ID: '999',
      GITHUB_SERVER_URL: 'https://github.com',
      GITHUB_SHA: 'abcdef1234567890',
      GITHUB_WORKFLOW: 'verify',
    },
    createIssueImpl(issue) {
      createdIssues.push(issue);
      return { html_url: 'https://github.com/taichuy/1flowbase/issues/7' };
    },
    listOpenQualityGateIssuesImpl() {
      return [];
    },
  });

  assert.equal(result.status, 'passed');
  assert.equal(result.exitCode, 0);
  assert.equal(createdIssues.length, 1);
  assert.match(createdIssues[0].body, /## Component Results/u);
  assert.match(createdIssues[0].body, /\| `repo-tooling` \| passed \| 0 \|/u);
  assert.match(createdIssues[0].body, /\| `repo-frontend` \| passed \| 0 \|/u);
  assert.match(createdIssues[0].body, /\| `repo-backend-static` \| passed \| 0 \|/u);
  assert.match(createdIssues[0].body, /\| `repo-backend-test-api-server-1-of-4` \| passed \| 0 \|/u);
  assert.match(createdIssues[0].body, /\| `coverage-frontend` \| passed \| 0 \|/u);
  assert.match(createdIssues[0].body, /\| `coverage-backend-api-server` \| passed \| 0 \|/u);
  assert.match(createdIssues[0].body, /## Security Risk/u);
  assert.match(createdIssues[0].body, /repo-tooling: review_required, findings 2 \(high 1, medium 1\), changed files 2/u);
  assert.match(createdIssues[0].body, /Security risk report: tmp\/test-governance\/security-risk\.json/u);
  assert.match(createdIssues[0].body, /frontend total: lines 80\.00%, functions 75\.00%, statements 80\.00%, branches 78\.00%/u);
  assert.match(createdIssues[0].body, /consistency-runtime-engine/u);
  assert.equal(
    fs.existsSync(path.join(repoRoot, 'tmp', 'test-governance', 'quality-gate-report.json')),
    true
  );
});

test('runQualityGateAggregate keeps component warning logs advisory when components pass', async () => {
  const repoRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'oneflowbase-quality-gate-aggregate-warning-'));
  const artifactRoot = path.join(repoRoot, 'tmp', 'test-governance', 'parallel');

  const writeArtifact = (artifactName, report) => {
    const artifactDir = path.join(artifactRoot, artifactName);
    fs.mkdirSync(artifactDir, { recursive: true });
    fs.writeFileSync(
      path.join(artifactDir, 'quality-gate-report.json'),
      `${JSON.stringify(report, null, 2)}\n`,
      'utf8'
    );
    fs.writeFileSync(path.join(artifactDir, 'quality-gate.latest.log'), `${report.scope} log\n`, 'utf8');
  };

  writeArtifact('test-governance-repo-tooling', {
    reportType: 'ci',
    status: 'passed',
    scope: 'repo-tooling',
    exitCode: 0,
    coverageSummaries: [],
    backendConsistencyTargets: [],
    warningFiles: ['tmp/test-governance/repo-tooling.warnings.log'],
  });
  writeArtifact('test-governance-repo-frontend', {
    reportType: 'ci',
    status: 'passed',
    scope: 'repo-frontend',
    exitCode: 0,
    coverageSummaries: [],
    backendConsistencyTargets: [],
    warningFiles: [],
  });
  for (const scope of REPO_BACKEND_COMPONENT_SCOPES) {
    writeArtifact(`test-governance-${scope}`, {
      reportType: 'ci',
      status: 'passed',
      scope,
      exitCode: 0,
      coverageSummaries: [],
      backendConsistencyTargets: [],
      warningFiles: [],
    });
  }
  writeArtifact('test-governance-backend-consistency', {
    reportType: 'ci',
    status: 'passed',
    scope: 'backend-consistency',
    exitCode: 0,
    coverageSummaries: [],
    backendConsistencyTargets: [],
    warningFiles: [],
  });
  writeArtifact('test-governance-coverage-frontend', {
    reportType: 'ci',
    status: 'passed',
    scope: 'coverage-frontend',
    exitCode: 0,
    coverageSummaries: [],
    backendConsistencyTargets: [],
    warningFiles: [],
  });
  for (const scope of COVERAGE_BACKEND_COMPONENT_SCOPES) {
    writeArtifact(`test-governance-${scope}`, {
      reportType: 'ci',
      status: 'passed',
      scope,
      exitCode: 0,
      coverageSummaries: [],
      backendConsistencyTargets: [],
      warningFiles: [],
    });
  }

  const result = await runQualityGateAggregate({
    repoRoot,
    artifactRoot: path.join('tmp', 'test-governance', 'parallel'),
    reportType: 'ci',
    publishIssue: false,
    githubToken: '',
    env: {
      GITHUB_ACTOR: 'taichu',
      GITHUB_REF_NAME: 'latest',
      GITHUB_REPOSITORY: 'taichuy/1flowbase',
      GITHUB_RUN_ID: '1000',
      GITHUB_SERVER_URL: 'https://github.com',
      GITHUB_SHA: 'abcdef1234567890',
      GITHUB_WORKFLOW: 'verify',
    },
  });

  assert.equal(result.status, 'passed');
  assert.equal(result.exitCode, 0);
  assert.match(
    fs.readFileSync(path.join(repoRoot, 'tmp', 'test-governance', 'quality-gate-report.md'), 'utf8'),
    /Warning log: tmp\/test-governance\/repo-tooling\.warnings\.log/u
  );
});

test('runQualityGateAggregate publishes one upserted pull request report comment when enabled', async () => {
  const repoRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'oneflowbase-quality-gate-aggregate-pr-'));
  const artifactRoot = path.join(repoRoot, 'tmp', 'test-governance', 'parallel');
  const artifactDir = path.join(artifactRoot, 'test-governance-repo-tooling');
  const comments = [];

  fs.mkdirSync(artifactDir, { recursive: true });
  fs.writeFileSync(
    path.join(artifactDir, 'quality-gate-report.json'),
    `${JSON.stringify({
      reportType: 'ci',
      status: 'passed',
      scope: 'repo-tooling',
      exitCode: 0,
      coverageSummaries: [],
      backendConsistencyTargets: [],
      warningFiles: [],
    }, null, 2)}\n`,
    'utf8'
  );
  fs.writeFileSync(path.join(artifactDir, 'quality-gate.latest.log'), 'repo-tooling log\n', 'utf8');

  const result = await runQualityGateAggregate({
    repoRoot,
    artifactRoot: path.join('tmp', 'test-governance', 'parallel'),
    expectedScopes: ['repo-tooling'],
    reportType: 'ci',
    publishPrComment: true,
    prNumber: 658,
    githubToken: 'token',
    env: {
      GITHUB_ACTOR: 'taichu',
      GITHUB_REF_NAME: 'feature',
      GITHUB_REPOSITORY: 'taichuy/1flowbase',
      GITHUB_RUN_ID: '1001',
      GITHUB_SERVER_URL: 'https://github.com',
      GITHUB_SHA: 'abcdef1234567890',
      GITHUB_WORKFLOW: 'verify',
    },
    upsertPrCommentImpl(comment) {
      comments.push(comment);
      return { html_url: 'https://github.com/taichuy/1flowbase/pull/658#issuecomment-1' };
    },
  });

  assert.equal(result.status, 'passed');
  assert.equal(result.prCommentUrl, 'https://github.com/taichuy/1flowbase/pull/658#issuecomment-1');
  assert.equal(comments.length, 1);
  assert.equal(comments[0].number, 658);
  assert.equal(comments[0].repository, 'taichuy/1flowbase');
  assert.match(comments[0].body, /^<!-- 1flowbase-quality-gate-pr-report -->/u);
  assert.match(comments[0].body, /## Component Results/u);
});
