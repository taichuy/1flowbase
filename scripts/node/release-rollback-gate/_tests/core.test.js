const test = require('node:test');
const assert = require('node:assert/strict');
const path = require('node:path');

const {
  buildComposeContent,
  buildRollbackGatePlan,
  parseCliArgs,
  resolvePreviousImageTag,
  runHttpSmoke,
} = require('../core.js');

test('parseCliArgs keeps the gate nightly-friendly by default', () => {
  assert.deepEqual(parseCliArgs([], {
    GITHUB_REPOSITORY: 'taichuy/1flowbase',
    GITHUB_RUN_ID: '123',
    GITHUB_RUN_ATTEMPT: '2',
  }), {
    candidateImageTag: 'latest',
    help: false,
    outputRoot: path.join('tmp', 'test-governance'),
    previousImageTag: 'auto',
    projectName: 'release-rollback-123-2',
    repositoryOwner: 'taichuy',
    timeoutMs: 120_000,
    webPort: 39100,
  });
});

test('resolvePreviousImageTag selects the release before the latest when auto is requested', () => {
  const tag = resolvePreviousImageTag({
    requestedTag: 'auto',
    runCommandImpl(command, args) {
      assert.equal(command, 'gh');
      assert.deepEqual(args, ['release', 'list', '--limit', '2', '--json', 'tagName']);
      return {
        status: 0,
        stdout: JSON.stringify([{ tagName: 'v0.3.0' }, { tagName: 'v0.2.0' }]),
        stderr: '',
      };
    },
  });

  assert.equal(tag, 'v0.2.0');
});

test('buildComposeContent uses GHCR images and isolated rollback-gate volumes', () => {
  const compose = buildComposeContent({
    repositoryOwner: 'taichuy',
    webPort: 39123,
  });

  assert.match(compose, /ghcr\.io\/taichuy\/1flowbase-web:\$\{FLOWBASE_WEB_VERSION\}/u);
  assert.match(compose, /ghcr\.io\/taichuy\/1flowbase-api-server:\$\{FLOWBASE_API_SERVER_VERSION\}/u);
  assert.match(compose, /ghcr\.io\/taichuy\/1flowbase-plugin-runner:\$\{FLOWBASE_PLUGIN_RUNNER_VERSION\}/u);
  assert.match(compose, /rollback-db-data:/u);
  assert.match(compose, /"39123:80"/u);
  assert.match(compose, /BOOTSTRAP_ROOT_PASSWORD: rollback-gate-root-password/u);
  assert.match(compose, /API_PROVIDER_SECRET_MASTER_KEY: 0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef/u);
});

test('buildRollbackGatePlan verifies baseline, candidate, snapshot restore, and previous release again', () => {
  const plan = buildRollbackGatePlan({
    candidateImageTag: 'latest',
    composeFile: '/repo/tmp/release-rollback-gate/docker-compose.yml',
    previousImageTag: 'v0.2.0',
    projectName: 'release-rollback-123-1',
    repositoryOwner: 'taichuy',
    snapshotPath: '/repo/tmp/test-governance/release-rollback-db.snapshot.dump',
    webPort: 39100,
  });

  assert.deepEqual(plan.map((step) => step.id), [
    'pull-previous-images',
    'start-previous-baseline',
    'smoke-previous-baseline',
    'create-db-snapshot',
    'pull-candidate-images',
    'start-candidate',
    'smoke-candidate',
    'restore-db-snapshot',
    'restart-previous-after-restore',
    'smoke-previous-after-restore',
  ]);
  assert.equal(plan[0].imageTag, 'v0.2.0');
  assert.equal(plan[4].imageTag, 'latest');
  assert.equal(plan[5].imageTag, 'latest');
  assert.equal(plan[8].imageTag, 'v0.2.0');
});

test('runHttpSmoke signs in through public auth instance id contract', async () => {
  const calls = [];
  const result = await runHttpSmoke({
    baseUrl: 'http://127.0.0.1:7800',
    timeoutMs: 100,
    fetchJsonImpl: async (url, options = {}) => {
      calls.push({ url, options });
      if (String(url).endsWith('/health')) {
        return {
          response: { ok: true, status: 200 },
          json: { status: 'ok' },
          text: '{"status":"ok"}',
        };
      }
      if (String(url).endsWith('/api/public/auth/providers')) {
        return {
          response: { ok: true, status: 200 },
          json: { data: [] },
          text: '{"data":[]}',
        };
      }
      return {
        response: { ok: true, status: 200 },
        json: { data: { current_workspace_id: 'workspace-1' } },
        text: '{"data":{"current_workspace_id":"workspace-1"}}',
      };
    },
    sleepImpl: async () => {},
  });

  assert.equal(result.currentWorkspaceId, 'workspace-1');
  assert.equal(calls[2].url, 'http://127.0.0.1:7800/api/public/auth/sign-in');
  assert.deepEqual(JSON.parse(calls[2].options.body), {
    authenticator_id: '00000000-0000-0000-0000-000000000001',
    identifier: 'root',
    password: 'rollback-gate-root-password',
  });
});
