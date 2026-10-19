const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');

const { main, readInputs } = require('../../cli/quality/github-quality-gate.js');
const {
  main: aggregateMain,
  parseExpectedScopes,
  readInputs: readAggregateInputs,
} = require('../../cli/quality/github-quality-gate-aggregate.js');

test('readInputs maps GitHub action environment inputs', () => {
  assert.deepEqual(readInputs({
    INPUT_SCOPE: 'repo',
    INPUT_REPORT_TYPE: 'cd',
    INPUT_PUBLISH_ISSUE: 'true',
    INPUT_GITHUB_TOKEN: 'token',
    INPUT_ENVIRONMENT: 'staging',
  }), {
    scope: 'repo',
    reportType: 'cd',
    publishIssue: true,
    githubToken: 'token',
    environmentName: 'staging',
  });
});

test('main returns the quality gate exit code from the runner', async () => {
  const repoRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'oneflowbase-quality-gate-cli-'));

  const status = await main([], {
    repoRoot,
    env: {
      INPUT_SCOPE: 'backend',
      INPUT_REPORT_TYPE: 'ci',
      INPUT_PUBLISH_ISSUE: 'false',
    },
    spawnSyncImpl() {
      return {
        status: 7,
        stdout: '',
        stderr: '',
      };
    },
    writeStdout() {},
    writeStderr() {},
  });

  assert.equal(status, 7);
});

test('main forwards quality issue maintenance dependencies to the runner', async () => {
  const repoRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'oneflowbase-quality-gate-cli-'));
  const closedIssues = [];

  const status = await main([], {
    repoRoot,
    env: {
      GITHUB_ACTOR: 'taichu',
      GITHUB_REF_NAME: 'latest',
      GITHUB_RUN_ID: '123',
      GITHUB_SERVER_URL: 'https://github.com',
      GITHUB_SHA: 'abcdef1234567890',
      GITHUB_WORKFLOW: 'verify',
      INPUT_GITHUB_TOKEN: 'token',
      INPUT_PUBLISH_ISSUE: 'true',
      INPUT_REPORT_TYPE: 'ci',
      INPUT_SCOPE: 'backend',
    },
    spawnSyncImpl() {
      return {
        status: 0,
        stdout: 'backend passed\n',
        stderr: '',
      };
    },
    createIssueImpl() {
      return {
        html_url: 'https://github.com/taichuy/1flowbase/issues/18',
        number: 18,
        title: '[Quality Gate][CI] 2026-05-06 17:16 latest abcdef1 passed',
      };
    },
    listOpenQualityGateIssuesImpl() {
      return [{
        html_url: 'https://github.com/taichuy/1flowbase/issues/17',
        number: 17,
        title: '[Quality Gate][CI] 2026-05-06 16:29 latest 1234567 failed',
      }];
    },
    closeIssueImpl(issue) {
      closedIssues.push(issue.number);
      return {};
    },
    writeStdout() {},
    writeStderr() {},
  });

  assert.equal(status, 0);
  assert.deepEqual(closedIssues, [17]);
});

test('readAggregateInputs maps aggregate environment inputs', () => {
  assert.deepEqual(readAggregateInputs({
    INPUT_ARTIFACT_ROOT: 'tmp/test-governance/parallel',
    INPUT_REPORT_TYPE: 'ci',
    INPUT_PUBLISH_ISSUE: 'true',
    INPUT_PUBLISH_PR_COMMENT: 'true',
    INPUT_PR_NUMBER: '658',
    INPUT_GITHUB_TOKEN: 'token',
    INPUT_ENVIRONMENT: 'nightly-latest',
  }), {
    artifactRoot: 'tmp/test-governance/parallel',
    reportType: 'ci',
    publishIssue: true,
    publishPrComment: true,
    prNumber: 658,
    githubToken: 'token',
    environmentName: 'nightly-latest',
  });
});

test('parseExpectedScopes maps comma-separated aggregate scope inputs', () => {
  assert.equal(parseExpectedScopes(''), null);
  assert.deepEqual(parseExpectedScopes('repo-tooling, repo-frontend,container-images'), [
    'repo-tooling',
    'repo-frontend',
    'container-images',
  ]);
});

test('readAggregateInputs can override expected aggregate scopes', () => {
  assert.deepEqual(readAggregateInputs({
    INPUT_ARTIFACT_ROOT: 'tmp/test-governance/parallel',
    INPUT_REPORT_TYPE: 'ci',
    INPUT_PUBLISH_ISSUE: 'false',
    INPUT_EXPECTED_SCOPES: 'repo-tooling,container-images',
  }), {
    artifactRoot: 'tmp/test-governance/parallel',
    reportType: 'ci',
    publishIssue: false,
    publishPrComment: false,
    prNumber: 0,
    githubToken: '',
    environmentName: '',
    expectedScopes: ['repo-tooling', 'container-images'],
  });
});

test('aggregateMain returns the aggregate quality gate exit code', async () => {
  const repoRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'oneflowbase-quality-gate-aggregate-cli-'));
  const artifactDir = path.join(repoRoot, 'tmp', 'test-governance', 'parallel', 'test-governance-repo');
  fs.mkdirSync(artifactDir, { recursive: true });
  fs.writeFileSync(
    path.join(artifactDir, 'quality-gate-report.json'),
    `${JSON.stringify({
      reportType: 'ci',
      status: 'failed',
      scope: 'repo',
      exitCode: 1,
      coverageSummaries: [],
      backendConsistencyTargets: [],
      warningFiles: [],
    })}\n`,
    'utf8'
  );

  const status = await aggregateMain([], {
    repoRoot,
    env: {
      INPUT_ARTIFACT_ROOT: 'tmp/test-governance/parallel',
      INPUT_REPORT_TYPE: 'ci',
      INPUT_PUBLISH_ISSUE: 'false',
    },
  });

  assert.equal(status, 1);
});
