const test = require('node:test');
const assert = require('node:assert/strict');

const {
  buildAdvisoryMessage,
  main,
  readChangedFiles,
  routeChangedFiles,
} = require('../core.js');

test('routeChangedFiles recommends related gates for frontend and backend consistency changes', () => {
  const routes = routeChangedFiles([
    'web/app/src/pages/workspaces/WorkspacePage.tsx',
    'api/crates/orchestration-runtime/src/execution_engine.rs',
  ]);

  assert.deepEqual(routes.map((route) => route.scope), [
    'repo-frontend-pr',
    'repo-backend-static',
    'repo-backend-fmt',
    'repo-backend-check-runtime-storage',
    'backend-consistency',
  ]);
});

test('buildAdvisoryMessage produces an English ASCII non-blocking prompt', () => {
  const routes = routeChangedFiles([
    'web/app/src/i18n/en-US.json',
    'api/crates/storage/durable/postgres/src/repository.rs',
  ]);
  const message = buildAdvisoryMessage({
    mode: 'staged',
    changedFiles: [
      'web/app/src/i18n/en-US.json',
      'api/crates/storage/durable/postgres/src/repository.rs',
    ],
    routes,
  });

  assert.match(message, /\[1flowbase-gate-router\] Advisory only/u);
  assert.match(message, /This hook does not block the commit/u);
  assert.match(message, /repo-tooling/u);
  assert.match(message, /repo-frontend-pr/u);
  assert.match(message, /backend-consistency/u);
  assert.doesNotMatch(message, /[^\x00-\x7F]/u);
});

test('routeChangedFiles does not route frontend state names to backend consistency', () => {
  const routes = routeChangedFiles([
    'web/app/src/features/workspace/useWorkspaceState.ts',
  ]);

  assert.deepEqual(routes.map((route) => route.scope), [
    'repo-frontend-pr',
  ]);
});

test('routeChangedFiles recommends state protocol gates for Anthropic ACP projection changes', () => {
  const routes = routeChangedFiles([
    'api/apps/api-server/src/routes/application_public_api/compat_sse/protocol_mappers/anthropic_stream.rs',
    'scripts/node/acp-claude-smoke/core.js',
  ]);

  assert(routes.some((route) => route.scope === 'state-protocols'));
  assert(
    routes.some((route) =>
      route.command === 'node scripts/node/verify-state-protocols.js'
    )
  );
  assert(routes.some((route) => route.scope === 'foundation-contract-ai-gateway-fast'));
});

test('AC-001 gate router exposes foundation-specific evidence instead of generic green', () => {
  const routes = routeChangedFiles([
    'api/apps/api-server/src/routes/mcp_protocol.rs',
    'web/packages/page-runtime/src/native-react-compiler/source-contract.ts',
  ]);

  assert(routes.some((route) => route.scope === 'foundation-contract-mcp-gateway-fast'));
  assert(routes.some((route) => route.scope === 'foundation-contract-native-react-fast'));
});

test('main reads staged changes from env override and never blocks the commit', async () => {
  let stdout = '';

  const status = await main(['--staged'], {
    env: {
      GATE_ROUTER_CHANGED_FILES: [
        'web/app/src/components/Toolbar.tsx',
        'api/apps/api-server/src/routes/applications.rs',
      ].join('\n'),
    },
    writeStdout(text) {
      stdout += text;
    },
    writeStderr() {},
  });

  assert.equal(status, 0);
  assert.match(stdout, /repo-frontend-pr/u);
  assert.match(stdout, /repo-backend-check-apps/u);
  assert.match(stdout, /This hook does not block the commit/u);
});


test('real Git deletions route staged and branch foundation changes while ignoring artifacts', (t) => {
  const fs = require('node:fs');
  const os = require('node:os');
  const path = require('node:path');
  const { execFileSync } = require('node:child_process');
  const repoRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'gate-router-deletions-'));
  t.after(() => fs.rmSync(repoRoot, { recursive: true, force: true }));
  const git = (...args) => execFileSync('git', args, { cwd: repoRoot, encoding: 'utf8' }).trim();
  git('init', '--quiet');
  git('config', 'user.name', 'Gate fixture');
  git('config', 'user.email', 'fixture@example.invalid');
  const source = 'api/apps/api-server/src/routes/mcp_protocol.rs';
  const artifact = 'tmp/deleted-fixture.log';
  for (const file of [source, artifact]) {
    fs.mkdirSync(path.dirname(path.join(repoRoot, file)), { recursive: true });
    fs.writeFileSync(path.join(repoRoot, file), 'fixture\n');
  }
  git('add', '.');
  git('commit', '--quiet', '-m', 'baseline');
  const baseRef = git('rev-parse', 'HEAD');
  git('rm', '--quiet', source, artifact);
  const staged = readChangedFiles({ repoRoot, mode: 'staged', baseRef, env: {} });
  assert.deepEqual(staged, [source]);
  git('commit', '--quiet', '-m', 'delete sources');
  const branch = readChangedFiles({ repoRoot, mode: 'branch', baseRef, env: {} });
  assert.deepEqual(branch, staged);
  for (const files of [staged, branch]) {
    const scopes = routeChangedFiles(files).map((route) => route.scope);
    assert.ok(scopes.includes('repo-backend-static'));
    assert.ok(scopes.includes('foundation-contract-mcp-gateway-fast'));
  }
  assert.deepEqual(routeChangedFiles([artifact]), []);
});


test('new contract and runtime owners route to their real CI check shard', () => {
  for (const [directory, shard] of [
    ['interface-runtime', 'core-libs'],
    ['extension-contracts', 'core-libs'],
    ['extension-package-runtime', 'core-libs'],
    ['control-plane-contracts', 'core-libs'],
    ['control-plane-test-support', 'core-libs'],
    ['postgres-test-support', 'core-libs'],
    ['runtime-extension-host', 'runtime-storage'],
    ['runtime-extension-sdk', 'runtime-storage'],
    ['control-plane-postgres-tests', 'apps'],
    ['storage/durable/postgres', 'runtime-storage'],
  ]) {
    const routes = routeChangedFiles([`api/crates/${directory}/src/lib.rs`]);
    assert.deepEqual(routes.filter(({ scope }) => scope.startsWith('repo-backend-check-'))
      .map(({ scope }) => scope), [`repo-backend-check-${shard}`], directory);
  }
});

test('unknown backend owners retain all-shard fallback', () => {
  const routes = routeChangedFiles(['api/crates/future-owner/src/lib.rs']);
  assert.deepEqual(routes.filter(({ scope }) => scope.startsWith('repo-backend-check-'))
    .map(({ scope }) => scope), [
      'repo-backend-check-core-libs', 'repo-backend-check-runtime-storage', 'repo-backend-check-apps',
    ]);
});


test('state protocol owner and wrapper changes route independently, unrelated verification does not', () => {
  for (const file of [
    'scripts/node/verify/state-protocols.js',
    'scripts/node/verify-state-protocols.js',
    'scripts/node/verify-state-protocols/_tests/executed-evidence.test.js',
  ]) {
    assert(routeChangedFiles([file]).some((route) => route.scope === 'state-protocols'), file);
  }
  assert(!routeChangedFiles(['scripts/node/verify/coverage.js']).some((route) => route.scope === 'state-protocols'));
});
